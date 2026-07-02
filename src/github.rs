use crate::commands::{run_command_output, run_git_command_allow_empty};
use crate::git::git_command_dir_for_path;
use crate::model::{GithubReadmeResult, PreviewColors, PreviewData};
use ratatui::{
    style::{Color, Style},
    text::{Line, Span, Text},
};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
};

#[derive(Debug, PartialEq, Eq)]
struct GithubRepoSummary {
    description: Option<String>,
    language: Option<String>,
    default_branch: Option<String>,
    stars: Option<u64>,
    forks: Option<u64>,
    open_issues: Option<u64>,
}

#[derive(Default)]
struct GithubContentParts {
    summary: Option<GithubRepoSummary>,
    readme: Option<String>,
}

enum GithubRepoPart {
    Summary(Option<GithubRepoSummary>),
    Readme(Option<String>),
}

pub(crate) fn ensure_github_readme_for_preview(
    path: &str,
    data: &PreviewData,
    github_in_flight: &mut HashSet<String>,
    github_tx: &mpsc::Sender<GithubReadmeResult>,
    preferred_tab_index: usize,
    colors: PreviewColors,
) {
    if data.github_readme_loaded || data.previews.is_empty() || github_in_flight.contains(path) {
        return;
    }

    github_in_flight.insert(path.to_string());
    let tx = github_tx.clone();
    let path_owned = path.to_string();
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

        let total_results = tab_paths.len();
        let (result_tx, result_rx) = mpsc::channel::<(usize, Option<Text<'static>>, bool)>();
        let mut repo_jobs: HashMap<String, (PathBuf, Vec<usize>)> = HashMap::new();

        for tab_index in order {
            let Some((repo, repo_dir)) = github_repo_for_path(&tab_paths[tab_index]) else {
                let _ = result_tx.send((tab_index, None, true));
                continue;
            };
            repo_jobs
                .entry(repo)
                .and_modify(|(_, tab_indexes)| tab_indexes.push(tab_index))
                .or_insert_with(|| (repo_dir, vec![tab_index]));
        }

        for (repo, (repo_dir, tab_indexes)) in repo_jobs {
            let result_tx = result_tx.clone();
            thread::spawn(move || {
                stream_github_text_for_repo(
                    &repo,
                    &repo_dir,
                    &tab_indexes,
                    colors.accent,
                    colors.muted,
                    colors.text,
                    &result_tx,
                );
            });
        }
        drop(result_tx);

        let mut completed = 0usize;
        for (tab_index, readme, tab_done) in result_rx {
            if tab_done {
                completed += 1;
            }
            let _ = tx.send(GithubReadmeResult {
                path: path_owned.clone(),
                tab_index,
                readme,
                done: completed == total_results,
            });
        }
    });
}

fn github_repo_for_path(path: &str) -> Option<(String, PathBuf)> {
    let repo_dir = git_command_dir_for_path(Path::new(path))?;
    let remote = run_git_command_allow_empty(&repo_dir, &["remote", "get-url", "origin"])?;
    let repo = github_repo_from_remote(&remote)?;
    Some((repo, repo_dir))
}

fn stream_github_text_for_repo(
    repo: &str,
    repo_dir: &Path,
    tab_indexes: &[usize],
    _accent: Color,
    muted: Color,
    text: Color,
    result_tx: &mpsc::Sender<(usize, Option<Text<'static>>, bool)>,
) {
    let (part_tx, part_rx) = mpsc::channel::<GithubRepoPart>();
    let summary_tx = part_tx.clone();
    let summary_repo = repo.to_string();
    let summary_dir = repo_dir.to_path_buf();
    thread::spawn(move || {
        let summary = github_summary_for_repo(&summary_repo, Some(&summary_dir));
        let _ = summary_tx.send(GithubRepoPart::Summary(summary));
    });

    let readme_tx = part_tx.clone();
    let readme_repo = repo.to_string();
    let readme_dir = repo_dir.to_path_buf();
    thread::spawn(move || {
        let readme = github_readme_for_repo(&readme_repo, Some(&readme_dir));
        let _ = readme_tx.send(GithubRepoPart::Readme(readme));
    });
    drop(part_tx);

    let mut parts = GithubContentParts::default();
    for part in part_rx {
        match part {
            GithubRepoPart::Summary(summary) => parts.summary = summary,
            GithubRepoPart::Readme(readme) => parts.readme = readme,
        }
        if let Some(content) = github_text_from_parts(&parts, muted, text) {
            for tab_index in tab_indexes {
                let _ = result_tx.send((*tab_index, Some(content.clone()), false));
            }
        }
    }

    let content = github_text_from_parts(&parts, muted, text);
    for tab_index in tab_indexes {
        let _ = result_tx.send((*tab_index, content.clone(), true));
    }
}

fn github_text_from_parts(
    parts: &GithubContentParts,
    muted: Color,
    text: Color,
) -> Option<Text<'static>> {
    if parts.summary.is_none()
        && parts
            .readme
            .as_ref()
            .is_none_or(|value| value.trim().is_empty())
    {
        return None;
    }

    let value = Style::default().fg(text);
    let subtle = Style::default().fg(muted);
    let mut lines = github_summary_lines(parts.summary.as_ref(), value, subtle);
    if let Some(readme) = parts
        .readme
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        if !lines.is_empty() {
            lines.push(Line::from(""));
        }
        lines.extend(
            readme
                .lines()
                .take(300)
                .map(|line| Line::from(Span::styled(line.to_string(), value))),
        );
    }
    Some(Text::from(lines))
}

fn github_summary_lines(
    summary: Option<&GithubRepoSummary>,
    value: Style,
    subtle: Style,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let Some(summary) = summary else {
        return lines;
    };

    if let Some(description) = summary
        .description
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        lines.push(Line::from(Span::styled(description.to_string(), value)));
    }

    let mut metadata = Vec::new();
    if let Some(language) = summary.language.as_deref() {
        metadata.push(format!("Language: {language}"));
    }
    if let Some(default_branch) = summary.default_branch.as_deref() {
        metadata.push(format!("Default branch: {default_branch}"));
    }
    if let Some(stars) = summary.stars {
        metadata.push(format!("Stars: {stars}"));
    }
    if let Some(forks) = summary.forks {
        metadata.push(format!("Forks: {forks}"));
    }
    if let Some(open_issues) = summary.open_issues {
        metadata.push(format!("Open issues: {open_issues}"));
    }
    if !metadata.is_empty() {
        lines.push(Line::from(Span::styled(metadata.join(" | "), subtle)));
    }

    lines
}

fn github_summary_for_repo(repo: &str, current_dir: Option<&Path>) -> Option<GithubRepoSummary> {
    let args = vec!["api".to_string(), format!("repos/{repo}")];
    let output = run_command_output("gh", &args, current_dir)?;
    github_summary_from_json(&output)
}

fn github_summary_from_json(output: &str) -> Option<GithubRepoSummary> {
    let value = serde_json::from_str::<Value>(output).ok()?;
    Some(GithubRepoSummary {
        description: string_field(&value, "description"),
        language: string_field(&value, "language"),
        default_branch: string_field(&value, "default_branch"),
        stars: u64_field(&value, "stargazers_count"),
        forks: u64_field(&value, "forks_count"),
        open_issues: u64_field(&value, "open_issues_count"),
    })
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
}

fn u64_field(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(Value::as_u64)
}

fn github_readme_for_repo(repo: &str, current_dir: Option<&Path>) -> Option<String> {
    let args = vec![
        "api".to_string(),
        "-H".to_string(),
        "Accept: application/vnd.github.raw".to_string(),
        format!("repos/{repo}/readme"),
    ];
    run_command_output("gh", &args, current_dir)
}

pub(crate) fn github_repo_from_remote(remote: &str) -> Option<String> {
    let trimmed = remote.trim();
    if trimmed.is_empty() {
        return None;
    }

    let path = if let Some(value) = trimmed.strip_prefix("git@github.com:") {
        value
    } else {
        let (_, value) = trimmed.split_once("github.com/")?;
        value
    };
    github_repo_from_path(path)
}

fn github_repo_from_path(path: &str) -> Option<String> {
    let without_query = path.split('?').next().unwrap_or(path);
    let without_fragment = without_query.split('#').next().unwrap_or(without_query);
    let normalized = without_fragment
        .trim_matches('/')
        .strip_suffix(".git")
        .unwrap_or_else(|| without_fragment.trim_matches('/'));
    let mut parts = normalized.split('/');
    let owner = parts.next()?.trim();
    let repo = parts.next()?.trim();
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some(format!("{owner}/{repo}"))
}

#[cfg(test)]
mod tests {
    use super::{github_repo_from_remote, github_summary_from_json, GithubRepoSummary};

    #[test]
    fn parses_https_github_remote() {
        assert_eq!(
            github_repo_from_remote("https://github.com/Yarden-zamir/navgator.git"),
            Some("Yarden-zamir/navgator".to_string())
        );
    }

    #[test]
    fn parses_ssh_github_remote() {
        assert_eq!(
            github_repo_from_remote("git@github.com:Yarden-zamir/navgator.git"),
            Some("Yarden-zamir/navgator".to_string())
        );
    }

    #[test]
    fn parses_ssh_url_github_remote() {
        assert_eq!(
            github_repo_from_remote("ssh://git@github.com/Yarden-zamir/navgator.git"),
            Some("Yarden-zamir/navgator".to_string())
        );
    }

    #[test]
    fn ignores_non_github_remote() {
        assert_eq!(
            github_repo_from_remote("git@example.com:owner/repo.git"),
            None
        );
    }

    #[test]
    fn parses_repo_summary_json() {
        let summary = github_summary_from_json(
            r#"{
                "description": "Fast project navigation",
                "language": "Rust",
                "default_branch": "main",
                "stargazers_count": 7,
                "forks_count": 1,
                "open_issues_count": 2
            }"#,
        );

        assert_eq!(
            summary,
            Some(GithubRepoSummary {
                description: Some("Fast project navigation".to_string()),
                language: Some("Rust".to_string()),
                default_branch: Some("main".to_string()),
                stars: Some(7),
                forks: Some(1),
                open_issues: Some(2),
            })
        );
    }

    #[test]
    fn ignores_empty_summary_fields() {
        let summary = github_summary_from_json(
            r#"{
                "description": "",
                "language": null,
                "default_branch": "main"
            }"#,
        );

        assert_eq!(
            summary,
            Some(GithubRepoSummary {
                description: None,
                language: None,
                default_branch: Some("main".to_string()),
                stars: None,
                forks: None,
                open_issues: None,
            })
        );
    }
}
