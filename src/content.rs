use crate::commands::{run_command_output, run_git_command_allow_empty};
use crate::git::{
    git_command_dir_for_path, git_default_branch_for_path, git_is_bare_repository,
    git_is_inside_work_tree, git_worktree_label, git_worktrees_for_path,
};
use crate::model::{
    DetailTab, GitResult, GitWorktree, PreviewColors, PreviewData, PreviewSettings, PreviewTab,
    PreviewTarget,
};
use crate::search::entry_name;
use gator::config::home_dir;
use gator::text::{lines_from_ansi_output, lines_from_output};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
};
use std::{collections::HashSet, fs, path::Path, process::Command, sync::mpsc, thread};

pub fn display_path_for_user(path: &str) -> String {
    gator::text::collapse_home_env(path)
}

pub fn build_placeholder_text(
    path: Option<&str>,
    accent: Color,
    muted: Color,
    text: Color,
    message: &str,
) -> Text<'static> {
    let value = Style::default().fg(text);
    let message_style = if message.starts_with("Loading") {
        Style::default().fg(accent).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(muted)
    };

    let mut lines = Vec::new();
    if let Some(path) = path {
        lines.extend(build_path_lines(path, value));
        lines.push(Line::from(""));
    }
    lines.push(Line::from(Span::styled(message.to_string(), message_style)));
    Text::from(lines)
}

pub struct ApplyPreviewData<'a> {
    pub tab_index: &'a mut usize,
    pub tab_visible_index: &'a mut usize,
    pub tab_count: &'a mut usize,
    pub tab_labels: &'a mut Vec<String>,
    pub preview_text: &'a mut Text<'static>,
    pub detail_tabs: &'a mut Vec<DetailTab>,
    pub detail_tab_index: &'a mut usize,
    pub worktree_filter: &'a str,
}

pub fn apply_preview_data(data: &PreviewData, view: ApplyPreviewData<'_>) {
    if data.previews.is_empty() {
        return;
    }

    let visible_indexes = preview_tab_visible_indexes(data, view.worktree_filter);
    *view.tab_count = visible_indexes.len();
    *view.tab_labels = visible_indexes
        .iter()
        .filter_map(|index| data.previews.get(*index))
        .map(|tab| tab.label.clone())
        .collect();
    if let Some(visible_index) = visible_indexes
        .iter()
        .position(|index| *index == *view.tab_index)
    {
        *view.tab_visible_index = visible_index;
    } else if let Some(first_index) = visible_indexes.first() {
        *view.tab_index = *first_index;
        *view.tab_visible_index = 0;
    } else {
        *view.tab_index = (*view.tab_index).min(data.previews.len().saturating_sub(1));
        *view.tab_visible_index = 0;
    }
    let tab = &data.previews[*view.tab_index];
    *view.preview_text = tab.text.clone();
    *view.detail_tabs = detail_tabs_for_preview(tab);
    *view.detail_tab_index = (*view.detail_tab_index).min(view.detail_tabs.len().saturating_sub(1));
}

pub fn preview_tab_visible_indexes(data: &PreviewData, worktree_filter: &str) -> Vec<usize> {
    let filter = worktree_filter.trim().to_lowercase();
    if filter.is_empty() {
        return (0..data.previews.len()).collect();
    }

    let matches = data
        .previews
        .iter()
        .enumerate()
        .filter_map(|(index, tab)| {
            let label = tab.label.to_lowercase();
            let path = tab.path.to_lowercase();
            if label.contains(&filter) || path.contains(&filter) {
                Some(index)
            } else {
                None
            }
        })
        .collect::<Vec<usize>>();

    if matches.is_empty() {
        (0..data.previews.len()).collect()
    } else {
        matches
    }
}

pub fn detail_tabs_for_preview(tab: &PreviewTab) -> Vec<DetailTab> {
    let mut tabs = Vec::new();
    if let Some(readme) = tab.github_readme.as_ref() {
        tabs.push(DetailTab {
            label: "GitHub".to_string(),
            text: readme.clone(),
        });
    }
    if let Some(git) = tab.git.as_ref() {
        tabs.push(DetailTab {
            label: "Git".to_string(),
            text: git.clone(),
        });
    }
    tabs
}

pub fn apply_git_result(
    data: &mut PreviewData,
    tab_index: usize,
    git: Option<Text<'static>>,
    done: bool,
) {
    if let Some(tab) = data.previews.get_mut(tab_index) {
        tab.git = git;
    }
    if done {
        data.git_loaded = true;
    }
}

pub fn apply_github_readme_result(
    data: &mut PreviewData,
    tab_index: usize,
    readme: Option<Text<'static>>,
    done: bool,
) {
    if let Some(tab) = data.previews.get_mut(tab_index) {
        tab.github_readme = readme;
    }
    if done {
        data.github_readme_loaded = true;
    }
}

pub fn ensure_git_for_preview(
    path: &str,
    data: &PreviewData,
    git_in_flight: &mut HashSet<String>,
    git_tx: &mpsc::Sender<GitResult>,
    preferred_tab_index: usize,
    colors: PreviewColors,
) {
    if data.git_loaded || data.previews.is_empty() || git_in_flight.contains(path) {
        return;
    }

    git_in_flight.insert(path.to_string());
    let tx = git_tx.clone();
    let path_owned = path.to_string();
    let selected_repo_is_bare = data.selected_repo_is_bare;
    let tab_paths = data
        .previews
        .iter()
        .map(|tab| tab.path.clone())
        .collect::<Vec<String>>();

    thread::spawn(move || {
        let mut order = Vec::with_capacity(tab_paths.len());
        let preferred = preferred_tab_index.min(tab_paths.len().saturating_sub(1));
        order.push(preferred);
        order.extend((0..tab_paths.len()).filter(|index| *index != preferred));

        for (order_index, tab_index) in order.iter().enumerate() {
            let git = build_git_text_for_preview(
                &tab_paths[*tab_index],
                selected_repo_is_bare,
                colors.accent,
                colors.muted,
                colors.text,
            );
            let _ = tx.send(GitResult {
                path: path_owned.clone(),
                tab_index: *tab_index,
                git,
                done: order_index + 1 == order.len(),
            });
        }
    });
}

pub fn build_preview_data(
    path: &str,
    accent: Color,
    muted: Color,
    text: Color,
    preview_settings: PreviewSettings,
) -> PreviewData {
    let selected_repo_is_bare = git_command_dir_for_path(Path::new(path))
        .map(|dir| git_is_bare_repository(&dir))
        .unwrap_or(false);
    let targets = preview_targets_for_path(path, preview_settings.shorten_worktree_tab_labels);
    let previews = targets
        .into_iter()
        .map(|target| PreviewTab {
            path: target.path.clone(),
            label: target.label,
            text: build_preview_text(&target.path, accent, muted, text),
            git: None,
            github_readme: None,
        })
        .collect();
    PreviewData {
        previews,
        selected_repo_is_bare,
        git_loaded: false,
        github_readme_loaded: false,
    }
}

pub fn build_remote_branch_preview_data(
    bare_path: &str,
    remote_branch: &str,
    target_path: &str,
    accent: Color,
    muted: Color,
    text: Color,
) -> PreviewData {
    let heading = Style::default().fg(accent).add_modifier(Modifier::BOLD);
    let value = Style::default().fg(text);
    let subtle = Style::default().fg(muted);
    let mut lines = vec![
        Line::from(Span::styled("Remote branch", heading)),
        Line::from(Span::styled(remote_branch.to_string(), value)),
        Line::from(""),
        Line::from(Span::styled("Target worktree", heading)),
        Line::from(Span::styled(display_path_for_user(target_path), value)),
    ];

    if let Some(log_output) = remote_branch_recent_commits(bare_path, remote_branch) {
        if !log_output.trim().is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled("Recent commits", heading)));
            lines.extend(lines_from_output(&log_output, value, 200));
        }
    } else {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled("No git summary available", subtle)));
    }

    PreviewData {
        previews: vec![PreviewTab {
            path: target_path.to_string(),
            label: remote_branch.to_string(),
            text: Text::from(lines),
            git: None,
            github_readme: None,
        }],
        selected_repo_is_bare: true,
        git_loaded: true,
        github_readme_loaded: true,
    }
}

pub fn preview_targets_for_path(
    path: &str,
    shorten_worktree_tab_labels: bool,
) -> Vec<PreviewTarget> {
    let fallback = PreviewTarget {
        path: path.to_string(),
        label: entry_name(path),
    };
    let path_buf = Path::new(path);
    let worktrees = git_worktrees_for_path(path_buf);
    if worktrees.is_empty() {
        return vec![fallback];
    }

    let mut non_bare: Vec<&GitWorktree> =
        worktrees.iter().filter(|worktree| !worktree.bare).collect();
    let default_branch = git_default_branch_for_path(path_buf);
    sort_worktrees_default_first(&mut non_bare, default_branch.as_deref());
    let selected_is_bare = git_command_dir_for_path(path_buf)
        .map(|dir| git_is_bare_repository(&dir))
        .unwrap_or(false);
    if selected_is_bare && !non_bare.is_empty() {
        return preview_targets_from_worktrees(&non_bare, shorten_worktree_tab_labels);
    }
    if non_bare.len() > 1 {
        return preview_targets_from_worktrees(&non_bare, shorten_worktree_tab_labels);
    }

    vec![fallback]
}

pub fn sort_worktrees_default_first(
    worktrees: &mut Vec<&GitWorktree>,
    default_branch: Option<&str>,
) {
    let Some(default_branch) = default_branch else {
        return;
    };
    worktrees.sort_by_key(|worktree| {
        if worktree.branch.as_deref() == Some(default_branch) {
            0
        } else {
            1
        }
    });
}

fn preview_targets_from_worktrees(
    worktrees: &[&GitWorktree],
    shorten_worktree_tab_labels: bool,
) -> Vec<PreviewTarget> {
    worktrees
        .iter()
        .map(|worktree| PreviewTarget {
            path: worktree.path.clone(),
            label: git_worktree_label(worktree, shorten_worktree_tab_labels),
        })
        .collect()
}

fn build_preview_text(path: &str, accent: Color, muted: Color, text: Color) -> Text<'static> {
    let value = Style::default().fg(text);
    let heading = Style::default().fg(accent).add_modifier(Modifier::BOLD);
    let subtle = Style::default().fg(muted);
    let max_lines = 200usize;

    let path_buf = Path::new(path);
    let mut lines = build_path_lines(path, value);
    lines.push(Line::from(""));

    if path_buf.is_dir() {
        lines.push(Line::from(Span::styled("Contents", heading)));
        if let Some(output) = erd_output(path_buf) {
            lines.extend(lines_from_ansi_output(&output, value, max_lines));
        } else {
            lines.push(Line::from(Span::styled("erd output not available", subtle)));
        }
    } else {
        lines.push(Line::from(Span::styled("Not a directory", subtle)));
    }

    Text::from(lines)
}

/// Git facts for one preview tab. Both UIs render it.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct GitSummary {
    /// The selected project is a bare repository (container layout).
    pub bare_repository: bool,
    /// First line of `git status -sb`, without `## `.
    pub branch: Option<String>,
    /// Linked worktrees, only for a bare repository.
    pub worktrees: Option<Vec<GitSummaryWorktree>>,
    pub recent_commits: Vec<String>,
    pub staged: Vec<String>,
    pub unstaged: Vec<String>,
    pub untracked: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct GitSummaryWorktree {
    pub label: String,
    pub path: String,
}

const GIT_SUMMARY_MAX_LINES: usize = 200;

fn output_lines(output: &str) -> Vec<String> {
    output
        .lines()
        .take(GIT_SUMMARY_MAX_LINES)
        .map(str::to_string)
        .collect()
}

/// Collect the git panel facts for `path`. Returns `None` when `path` is not
/// in a repository or when git has nothing to show.
pub fn git_summary_for_path(path: &str, selected_repo_is_bare: bool) -> Option<GitSummary> {
    let path_buf = Path::new(path);
    let repo_dir = git_command_dir_for_path(path_buf)?;
    let inside_work_tree = git_is_inside_work_tree(&repo_dir);
    let bare_repository = git_is_bare_repository(&repo_dir);
    if !inside_work_tree && !bare_repository {
        return None;
    }

    let mut summary = GitSummary {
        bare_repository: selected_repo_is_bare,
        ..GitSummary::default()
    };

    if inside_work_tree {
        summary.branch = run_git_command_allow_empty(&repo_dir, &["status", "-sb"])
            .and_then(|output| output.lines().next().map(str::to_string))
            .map(|line| line.trim_start_matches("## ").to_string())
            .filter(|branch| !branch.trim().is_empty());
    }

    if bare_repository {
        summary.worktrees = Some(
            git_worktrees_for_path(path_buf)
                .iter()
                .filter(|worktree| !worktree.bare)
                .map(|worktree| GitSummaryWorktree {
                    label: git_worktree_label(worktree, false),
                    path: worktree.path.clone(),
                })
                .collect(),
        );
    }

    match run_git_command_allow_empty(&repo_dir, &["log", "-3", "--pretty=format:%s (%cr)"]) {
        Some(output) => summary.recent_commits = output_lines(output.trim_end()),
        None if inside_work_tree => return None,
        None => {}
    }
    summary.recent_commits.retain(|line| !line.is_empty());

    if inside_work_tree {
        summary.staged = run_git_command_allow_empty(&repo_dir, &["diff", "--stat", "--cached"])
            .map(|output| output_lines(&output))
            .unwrap_or_default();
        summary.unstaged = run_git_command_allow_empty(&repo_dir, &["diff", "--stat"])
            .map(|output| output_lines(&output))
            .unwrap_or_default();
        summary.untracked =
            run_git_command_allow_empty(&repo_dir, &["ls-files", "--others", "--exclude-standard"])
                .map(|output| output_lines(&output))
                .unwrap_or_default();
    }

    (summary != GitSummary::default()).then_some(summary)
}

fn build_git_text_for_preview(
    path: &str,
    selected_repo_is_bare: bool,
    accent: Color,
    _muted: Color,
    text: Color,
) -> Option<Text<'static>> {
    let summary = git_summary_for_path(path, selected_repo_is_bare)?;
    let heading = Style::default().fg(accent).add_modifier(Modifier::BOLD);
    let value = Style::default().fg(text);
    let mut lines: Vec<Line<'static>> = Vec::new();
    let section = |lines: &mut Vec<Line<'static>>, title: &str, body: &[String]| {
        if body.is_empty() {
            return;
        }
        if !lines.is_empty() {
            lines.push(Line::from(""));
        }
        lines.push(Line::from(Span::styled(title.to_string(), heading)));
        lines.extend(
            body.iter()
                .map(|line| Line::from(Span::styled(line.clone(), value))),
        );
    };

    if summary.bare_repository {
        lines.push(Line::from(Span::styled("Bare repository", heading)));
    }
    if let Some(branch) = summary.branch.as_deref() {
        lines.push(Line::from(Span::styled(
            format!("Branch: {branch}"),
            heading,
        )));
    }
    if let Some(worktrees) = summary.worktrees.as_ref() {
        let body = if worktrees.is_empty() {
            vec!["No worktrees".to_string()]
        } else {
            worktrees
                .iter()
                .map(|worktree| {
                    format!(
                        "{}  {}",
                        worktree.label,
                        display_path_for_user(&worktree.path)
                    )
                })
                .collect()
        };
        section(&mut lines, "Worktrees", &body);
    }
    section(&mut lines, "Recent commits", &summary.recent_commits);
    section(&mut lines, "Staged changes", &summary.staged);
    section(&mut lines, "Unstaged changes", &summary.unstaged);
    section(&mut lines, "Untracked", &summary.untracked);

    (!lines.is_empty()).then(|| Text::from(lines))
}

/// `git log -3` of a remote branch, read from the bare repository.
pub fn remote_branch_recent_commits(bare_path: &str, remote_branch: &str) -> Option<String> {
    run_git_dir_command_allow_empty(
        Path::new(bare_path),
        &["log", "-3", "--pretty=format:%s (%cr)", remote_branch],
    )
}

fn run_git_dir_command_allow_empty(bare_path: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("--git-dir")
        .arg(bare_path)
        .arg("-c")
        .arg("color.ui=never")
        .args(args)
        .env("NO_COLOR", "1")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&output.stdout)
            .trim_end()
            .to_string(),
    )
}

fn build_path_lines(path: &str, value: Style) -> Vec<Line<'static>> {
    vec![Line::from(Span::styled(display_path_for_user(path), value))]
}

pub fn erd_output(path: &Path) -> Option<String> {
    let path_str = path.to_string_lossy().to_string();
    let (mut args, used_default) = erd_args();
    args.push(path_str.clone());
    if let Some(output) = run_command_output("erd", &args, None) {
        return Some(output);
    }

    if !used_default {
        let mut fallback = erd_default_args();
        fallback.push(path_str);
        return run_command_output("erd", &fallback, None);
    }
    None
}

fn erd_args() -> (Vec<String>, bool) {
    let mut args = Vec::new();
    let mut used_default = true;
    if let Ok(home) = home_dir() {
        let config_path = home.join(".erdtreerc");
        if let Ok(contents) = fs::read_to_string(config_path) {
            args = parse_erd_config(&contents);
            if !args.is_empty() {
                used_default = false;
            }
        }
    }

    if args.is_empty() {
        args = erd_default_args();
    }
    (args, used_default)
}

fn parse_erd_config(contents: &str) -> Vec<String> {
    let mut args = Vec::new();
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let line = trimmed.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        for token in line.split_whitespace() {
            args.push(token.to_string());
        }
    }
    args
}

fn erd_default_args() -> Vec<String> {
    vec![
        "--dir-order=first".to_string(),
        "--icons".to_string(),
        "--sort=name".to_string(),
        "--level=4".to_string(),
        "--color".to_string(),
        "force".to_string(),
        "--layout=inverted".to_string(),
        "--human".to_string(),
        "--suppress-size".to_string(),
    ]
}
