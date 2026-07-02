use crate::commands::{run_command_output, run_git_command_allow_empty};
use crate::config::home_dir;
use crate::git::{
    git_command_dir_for_path, git_default_branch_for_path, git_is_bare_repository,
    git_is_inside_work_tree, git_worktree_label, git_worktrees_for_path,
};
use crate::model::{
    DetailTab, GitResult, GitWorktree, PreviewColors, PreviewData, PreviewSettings, PreviewTab,
    PreviewTarget,
};
use crate::search::entry_name;
use ansi_to_tui::IntoText;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
};
use std::{collections::HashSet, env, fs, path::Path, process::Command, sync::mpsc, thread};

pub(crate) fn display_path_for_user(path: &str) -> String {
    match env::var("HOME") {
        Ok(home) => display_path_with_home(path, &home),
        Err(_) => path.to_string(),
    }
}

pub(crate) fn display_path_with_home(path: &str, home: &str) -> String {
    if home.is_empty() {
        return path.to_string();
    }
    if path == home {
        return "~".to_string();
    }

    let home_with_separator = format!(
        "{}{}",
        home.trim_end_matches(std::path::MAIN_SEPARATOR),
        std::path::MAIN_SEPARATOR
    );
    if let Some(rest) = path.strip_prefix(&home_with_separator) {
        return format!("~/{}", rest);
    }

    path.to_string()
}

pub(crate) fn build_placeholder_text(
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

pub(crate) struct ApplyPreviewData<'a> {
    pub(crate) tab_index: &'a mut usize,
    pub(crate) tab_visible_index: &'a mut usize,
    pub(crate) tab_count: &'a mut usize,
    pub(crate) tab_labels: &'a mut Vec<String>,
    pub(crate) preview_text: &'a mut Text<'static>,
    pub(crate) detail_tabs: &'a mut Vec<DetailTab>,
    pub(crate) detail_tab_index: &'a mut usize,
    pub(crate) worktree_filter: &'a str,
}

pub(crate) fn apply_preview_data(data: &PreviewData, view: ApplyPreviewData<'_>) {
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

pub(crate) fn preview_tab_visible_indexes(data: &PreviewData, worktree_filter: &str) -> Vec<usize> {
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

pub(crate) fn detail_tabs_for_preview(tab: &PreviewTab) -> Vec<DetailTab> {
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

pub(crate) fn apply_git_result(
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

pub(crate) fn apply_github_readme_result(
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

pub(crate) fn ensure_git_for_preview(
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

pub(crate) fn build_preview_data(
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

pub(crate) fn build_remote_branch_preview_data(
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

    if let Some(log_output) = run_git_dir_command_allow_empty(
        Path::new(bare_path),
        &["log", "-3", "--pretty=format:%s (%cr)", remote_branch],
    ) {
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

fn preview_targets_for_path(path: &str, shorten_worktree_tab_labels: bool) -> Vec<PreviewTarget> {
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

pub(crate) fn sort_worktrees_default_first(
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

fn build_git_text_for_preview(
    path: &str,
    selected_repo_is_bare: bool,
    accent: Color,
    _muted: Color,
    text: Color,
) -> Option<Text<'static>> {
    let heading = Style::default().fg(accent).add_modifier(Modifier::BOLD);
    let value = Style::default().fg(text);
    let max_lines = 200usize;

    let path_buf = Path::new(path);
    let repo_dir = git_command_dir_for_path(path_buf)?;
    let inside_work_tree = git_is_inside_work_tree(&repo_dir);
    let bare_repository = git_is_bare_repository(&repo_dir);
    if !inside_work_tree && !bare_repository {
        return None;
    }

    let mut lines = Vec::new();
    if selected_repo_is_bare {
        lines.push(Line::from(Span::styled("Bare repository", heading)));
    }

    if inside_work_tree {
        if let Some(status_output) = run_git_command_allow_empty(&repo_dir, &["status", "-sb"]) {
            if let Some(first_line) = status_output.lines().next() {
                let branch = first_line.trim_start_matches("## ");
                if !branch.trim().is_empty() {
                    lines.push(Line::from(Span::styled(
                        format!("Branch: {}", branch),
                        heading,
                    )));
                }
            }
        }
    }

    if bare_repository {
        let worktrees = git_worktrees_for_path(path_buf);
        let non_bare_worktrees = worktrees
            .iter()
            .filter(|worktree| !worktree.bare)
            .collect::<Vec<&GitWorktree>>();
        if !lines.is_empty() {
            lines.push(Line::from(""));
        }
        lines.push(Line::from(Span::styled("Worktrees", heading)));
        if non_bare_worktrees.is_empty() {
            lines.push(Line::from(Span::styled("No worktrees", value)));
        } else {
            for worktree in non_bare_worktrees {
                lines.push(Line::from(Span::styled(
                    format!(
                        "{}  {}",
                        git_worktree_label(worktree, false),
                        display_path_for_user(&worktree.path)
                    ),
                    value,
                )));
            }
        }
    }

    if let Some(log_output) =
        run_git_command_allow_empty(&repo_dir, &["log", "-3", "--pretty=format:%s (%cr)"])
    {
        if !log_output.trim().is_empty() {
            if !lines.is_empty() {
                lines.push(Line::from(""));
            }
            lines.push(Line::from(Span::styled("Recent commits", heading)));
            lines.extend(lines_from_output(&log_output, value, max_lines));
        }
    } else if inside_work_tree {
        return None;
    }

    if !inside_work_tree {
        if lines.is_empty() {
            return None;
        }
        return Some(Text::from(lines));
    }

    if let Some(staged_output) =
        run_git_command_allow_empty(&repo_dir, &["diff", "--stat", "--cached"])
    {
        if !staged_output.trim().is_empty() {
            if !lines.is_empty() {
                lines.push(Line::from(""));
            }
            lines.push(Line::from(Span::styled("Staged changes", heading)));
            lines.extend(lines_from_output(&staged_output, value, max_lines));
        }
    }

    if let Some(unstaged_output) = run_git_command_allow_empty(&repo_dir, &["diff", "--stat"]) {
        if !unstaged_output.trim().is_empty() {
            if !lines.is_empty() {
                lines.push(Line::from(""));
            }
            lines.push(Line::from(Span::styled("Unstaged changes", heading)));
            lines.extend(lines_from_output(&unstaged_output, value, max_lines));
        }
    }

    if let Some(untracked_output) =
        run_git_command_allow_empty(&repo_dir, &["ls-files", "--others", "--exclude-standard"])
    {
        if !untracked_output.trim().is_empty() {
            if !lines.is_empty() {
                lines.push(Line::from(""));
            }
            lines.push(Line::from(Span::styled("Untracked", heading)));
            lines.extend(lines_from_output(&untracked_output, value, max_lines));
        }
    }

    if lines.is_empty() {
        return None;
    }
    Some(Text::from(lines))
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

fn erd_output(path: &Path) -> Option<String> {
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

fn lines_from_output(output: &str, style: Style, max_lines: usize) -> Vec<Line<'static>> {
    output
        .lines()
        .take(max_lines)
        .map(|line| Line::from(Span::styled(line.to_string(), style)))
        .collect()
}

fn lines_from_ansi_output(output: &str, style: Style, max_lines: usize) -> Vec<Line<'static>> {
    let text_result = output.as_bytes().to_vec().into_text();
    let Ok(text) = text_result else {
        return lines_from_output(output, style, max_lines);
    };
    text.lines
        .into_iter()
        .take(max_lines)
        .map(|line| line.style(style))
        .collect()
}
