use crate::config::load_config;
use crate::git::{
    dot_bare_for_path, git_worktree_label, git_worktrees_for_path, local_branch_for_remote,
    local_remote_branches_for_bare, ls_remote_heads, remote_branch_summaries_for_bare,
    remote_branch_target_paths, RemoteBranchSummary,
};
use crate::model::{
    AppResult, BuildItemsResult, GitWorktree, NavigateEntry, NavigateEntryKind, RemoteSettings,
    ResultUpdate,
};
use crate::provider_runtime::{
    load_json_cache, save_json_cache, spawn_batched_jobs, unix_timestamp,
};
use crate::search::entry_name;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
    time::Duration,
};

const WORKTREE_PROVIDER_PREFIX: &str = "worktree:";
pub(crate) const REMOTE_BRANCH_PROVIDER_PREFIX: &str = "remote-branch:";
const WORKTREE_CACHE_FILE: &str = "worktrees.json";
const WORKTREE_CACHE_VERSION: u32 = 2;
const REMOTE_BRANCH_CACHE_FILE: &str = "remote-branches.json";
const REMOTE_BRANCH_CACHE_VERSION: u32 = 1;
const BRANCH_ICON: &str = "";

pub(crate) trait ResultProvider {
    fn initial_entries(&self) -> Vec<NavigateEntry>;
    fn spawn_updates(&self, _tx: mpsc::Sender<ResultUpdate>) {}
}

pub(crate) struct ProjectResultProvider {
    paths: Vec<PathBuf>,
}

pub(crate) struct WorktreeResultProvider {
    project_entries: Vec<NavigateEntry>,
}

#[derive(Deserialize, Serialize)]
struct WorktreeCache {
    version: u32,
    generated_at: u64,
    entries: Vec<NavigateEntry>,
}

#[derive(Deserialize, Serialize)]
struct RemoteBranchCache {
    version: u32,
    generated_at: u64,
    repos: Vec<RemoteBranchCacheRepo>,
}

#[derive(Deserialize, Serialize)]
struct RemoteBranchCacheRepo {
    bare_path: String,
    branches: Vec<String>,
}

pub(crate) fn build_items() -> AppResult<BuildItemsResult> {
    let config = load_config()?;
    let project_provider =
        ProjectResultProvider::from_config_paths(config.static_items, config.index_folders);
    let mut entries = project_provider.initial_entries();
    let worktree_provider = WorktreeResultProvider {
        project_entries: entries.clone(),
    };
    entries.extend(worktree_provider.initial_entries());
    Ok(BuildItemsResult {
        entries,
        preview_settings: config.preview_settings,
        sort_settings: config.sort_settings,
        remote_settings: config.remote_settings,
        branch_settings: config.branch_settings,
        action_settings: config.action_settings,
        create_settings: config.create_settings,
        theme_colors: config.theme_colors,
    })
}

pub(crate) fn spawn_worktree_result_provider(
    project_entries: &[NavigateEntry],
    tx: mpsc::Sender<ResultUpdate>,
) {
    let provider = WorktreeResultProvider {
        project_entries: project_entries.to_vec(),
    };
    provider.spawn_updates(tx);
}

pub(crate) fn spawn_remote_branch_result_provider(
    entry: NavigateEntry,
    tx: mpsc::Sender<ResultUpdate>,
    settings: RemoteSettings,
) {
    thread::spawn(move || {
        let Some((bare, container)) = dot_bare_for_path(Path::new(&entry.selection_path)) else {
            let _ = tx.send(ResultUpdate::Status {
                provider_id: REMOTE_BRANCH_PROVIDER_PREFIX.to_string(),
                message: "This repo must be migrated to a bare repo first.".to_string(),
            });
            let _ = tx.send(ResultUpdate::ReplaceProviderEntries {
                provider_prefix: REMOTE_BRANCH_PROVIDER_PREFIX.to_string(),
                entries: Vec::new(),
            });
            return;
        };

        let repo_label = entry_name(&container.to_string_lossy());
        let summaries = remote_branch_summaries_for_bare(&bare);
        if settings.use_cache {
            send_remote_entries_if_any(
                &tx,
                remote_branch_entries_for_branches(
                    &repo_label,
                    &bare,
                    &container,
                    load_remote_branch_cache_for_bare(&bare),
                    &summaries,
                ),
            );
        }

        send_remote_entries_if_any(
            &tx,
            remote_branch_entries_for_branches(
                &repo_label,
                &bare,
                &container,
                local_remote_branches_for_bare(&bare),
                &summaries,
            ),
        );
        if !settings.refresh_on_toggle {
            send_remote_status(&tx, "remote branches loaded".to_string());
            return;
        }
        send_remote_status(&tx, format!("refreshing {repo_label}..."));
        let remote_branches = match ls_remote_heads(&bare) {
            Ok(branches) => branches,
            Err(message) => {
                send_remote_status(&tx, message);
                return;
            }
        };

        let _ = save_remote_branch_cache_for_bare(&bare, &remote_branches);
        replace_remote_entries(
            &tx,
            remote_branch_entries_for_branches(
                &repo_label,
                &bare,
                &container,
                remote_branches,
                &summaries,
            ),
        );
        send_remote_status(&tx, "remote branches loaded".to_string());
    });
}

fn send_remote_entries_if_any(tx: &mpsc::Sender<ResultUpdate>, entries: Vec<NavigateEntry>) {
    if !entries.is_empty() {
        replace_remote_entries(tx, entries);
    }
}

fn replace_remote_entries(tx: &mpsc::Sender<ResultUpdate>, entries: Vec<NavigateEntry>) {
    let _ = tx.send(ResultUpdate::ReplaceProviderEntries {
        provider_prefix: REMOTE_BRANCH_PROVIDER_PREFIX.to_string(),
        entries,
    });
}

fn send_remote_status(tx: &mpsc::Sender<ResultUpdate>, message: String) {
    let _ = tx.send(ResultUpdate::Status {
        provider_id: REMOTE_BRANCH_PROVIDER_PREFIX.to_string(),
        message,
    });
}

fn remote_branch_entries_for_branches(
    repo_label: &str,
    bare: &Path,
    container: &Path,
    remote_branches: Vec<String>,
    summaries: &HashMap<String, RemoteBranchSummary>,
) -> Vec<NavigateEntry> {
    let existing_branches = git_worktrees_for_path(bare)
        .into_iter()
        .filter_map(|worktree| worktree.branch)
        .collect::<HashSet<String>>();
    let mut entries = Vec::new();
    let mut seen_entries = HashSet::new();
    let target_paths = remote_branch_target_paths(bare, container, &remote_branches);
    let bare_path = bare.to_string_lossy();
    let container_path = container.to_string_lossy();
    for remote_branch in remote_branches {
        let branch = local_branch_for_remote(&remote_branch);
        if existing_branches.contains(&branch) {
            continue;
        }
        let Some(target_path) = target_paths.get(&remote_branch) else {
            continue;
        };
        let remote_entry = remote_branch_entry(
            repo_label,
            bare_path.as_ref(),
            container_path.as_ref(),
            &remote_branch,
            target_path.to_string_lossy().to_string(),
            summaries.get(&remote_branch),
        );
        if seen_entries.insert(remote_entry.id.clone()) {
            entries.push(remote_entry);
        }
    }
    entries
}

fn load_remote_branch_cache_for_bare(bare: &Path) -> Vec<String> {
    let bare_path = bare.to_string_lossy();
    load_json_cache::<RemoteBranchCache>(REMOTE_BRANCH_CACHE_FILE)
        .filter(|cache| cache.version == REMOTE_BRANCH_CACHE_VERSION)
        .and_then(|cache| {
            cache
                .repos
                .into_iter()
                .find(|repo| repo.bare_path == bare_path)
                .map(|repo| repo.branches)
        })
        .unwrap_or_default()
}

fn save_remote_branch_cache_for_bare(bare: &Path, branches: &[String]) -> std::io::Result<()> {
    let bare_path = bare.to_string_lossy().to_string();
    let mut cache = load_json_cache::<RemoteBranchCache>(REMOTE_BRANCH_CACHE_FILE)
        .filter(|cache| cache.version == REMOTE_BRANCH_CACHE_VERSION)
        .unwrap_or_else(|| RemoteBranchCache {
            version: REMOTE_BRANCH_CACHE_VERSION,
            generated_at: unix_timestamp(),
            repos: Vec::new(),
        });
    cache.generated_at = unix_timestamp();
    cache.repos.retain(|repo| repo.bare_path != bare_path);
    cache.repos.push(RemoteBranchCacheRepo {
        bare_path,
        branches: branches.to_vec(),
    });
    save_json_cache(REMOTE_BRANCH_CACHE_FILE, &cache)
}

impl ProjectResultProvider {
    fn from_config_paths(static_items: Vec<PathBuf>, index_folders: Vec<PathBuf>) -> Self {
        let mut paths: Vec<PathBuf> = static_items;

        for folder in index_folders {
            paths.push(folder.clone());
            let mut children: Vec<PathBuf> = Vec::new();
            if let Ok(read_dir) = fs::read_dir(&folder) {
                for entry in read_dir.flatten() {
                    let path = entry.path();
                    if is_dir(&path) {
                        children.push(path);
                    }
                }
            }
            children.sort();
            paths.extend(children);
        }

        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for path in paths {
            let key = path.to_string_lossy().to_string();
            if seen.insert(key.clone()) {
                out.push(PathBuf::from(key));
            }
        }
        Self { paths: out }
    }
}

impl ResultProvider for ProjectResultProvider {
    fn initial_entries(&self) -> Vec<NavigateEntry> {
        self.paths
            .iter()
            .map(|path| project_entry(&path.to_string_lossy()))
            .collect()
    }
}

impl ResultProvider for WorktreeResultProvider {
    fn initial_entries(&self) -> Vec<NavigateEntry> {
        let project_roots = self
            .project_entries
            .iter()
            .map(|entry| entry.preview_root_path.as_str())
            .collect::<HashSet<&str>>();
        load_worktree_cache()
            .into_iter()
            .filter(|entry| project_roots.contains(entry.preview_root_path.as_str()))
            .collect()
    }

    fn spawn_updates(&self, tx: mpsc::Sender<ResultUpdate>) {
        let project_entries = self.project_entries.clone();
        thread::spawn(move || {
            let jobs = dedupe_worktree_scan_jobs(project_entries);
            if jobs.is_empty() {
                return;
            }

            let (batch_tx, batch_rx) = mpsc::channel::<Vec<NavigateEntry>>();
            spawn_batched_jobs(
                jobs,
                32,
                Duration::from_millis(100),
                batch_tx,
                scan_worktree_job,
            );

            let mut refreshed = Vec::new();
            let mut seen = HashSet::new();
            for batch in batch_rx {
                let mut unique_batch = Vec::new();
                for entry in batch {
                    if seen.insert(entry.selection_path.clone()) {
                        unique_batch.push(entry.clone());
                        refreshed.push(entry);
                    }
                }
                if !unique_batch.is_empty() {
                    let _ = tx.send(ResultUpdate::Entries {
                        entries: unique_batch,
                    });
                }
            }

            let _ = save_worktree_cache(&refreshed);
            let _ = tx.send(ResultUpdate::ReplaceProviderEntries {
                provider_prefix: WORKTREE_PROVIDER_PREFIX.to_string(),
                entries: refreshed,
            });
        });
    }
}

fn load_worktree_cache() -> Vec<NavigateEntry> {
    load_json_cache::<WorktreeCache>(WORKTREE_CACHE_FILE)
        .filter(|cache| cache.version == WORKTREE_CACHE_VERSION)
        .map(|cache| cache.entries)
        .unwrap_or_default()
}

fn save_worktree_cache(entries: &[NavigateEntry]) -> std::io::Result<()> {
    save_json_cache(
        WORKTREE_CACHE_FILE,
        &WorktreeCache {
            version: WORKTREE_CACHE_VERSION,
            generated_at: unix_timestamp(),
            entries: entries.to_vec(),
        },
    )
}

fn dedupe_worktree_scan_jobs(project_entries: Vec<NavigateEntry>) -> Vec<NavigateEntry> {
    let mut seen = HashSet::new();
    let mut jobs = Vec::new();
    for entry in project_entries {
        if !matches!(entry.kind, NavigateEntryKind::Project) {
            continue;
        }
        if seen.insert(entry.preview_root_path.clone()) {
            jobs.push(entry);
        }
    }
    jobs
}

fn scan_worktree_job(entry: NavigateEntry) -> Vec<NavigateEntry> {
    let repo_path = entry.preview_root_path.clone();
    let worktrees = git_worktrees_for_path(Path::new(&repo_path));
    if worktrees.is_empty() {
        return Vec::new();
    }
    worktrees
        .into_iter()
        .filter(|worktree| !worktree.bare && worktree.path != repo_path)
        .map(|worktree| worktree_entry(&entry, &worktree))
        .collect()
}

fn project_entry(path: &str) -> NavigateEntry {
    let display = entry_name(path);
    NavigateEntry {
        id: format!("project:{path}"),
        display: display.clone(),
        context: None,
        preview_root_path: path.to_string(),
        preferred_preview_path: None,
        selection_path: path.to_string(),
        metadata_path: path.to_string(),
        search_text: vec![display],
        kind: NavigateEntryKind::Project,
    }
}

fn worktree_entry(repo_entry: &NavigateEntry, worktree: &GitWorktree) -> NavigateEntry {
    let branch = git_worktree_label(worktree, true);
    let repo_label = repo_entry.display.clone();
    let display = format!("{BRANCH_ICON} {repo_label} {branch}");
    NavigateEntry {
        id: format!(
            "{WORKTREE_PROVIDER_PREFIX}{}:{}",
            repo_entry.preview_root_path, worktree.path
        ),
        display: display.clone(),
        context: None,
        preview_root_path: repo_entry.preview_root_path.clone(),
        preferred_preview_path: Some(worktree.path.clone()),
        selection_path: worktree.path.clone(),
        metadata_path: worktree.path.clone(),
        search_text: vec![display],
        kind: NavigateEntryKind::Worktree { repo_label, branch },
    }
}

fn remote_branch_entry(
    repo_label: &str,
    bare_path: &str,
    container_path: &str,
    remote_branch: &str,
    target_path: String,
    summary: Option<&RemoteBranchSummary>,
) -> NavigateEntry {
    let branch = local_branch_for_remote(remote_branch);
    let display = format!("{BRANCH_ICON} {repo_label} {remote_branch}");
    let context = remote_branch_context(summary);
    NavigateEntry {
        id: format!("{REMOTE_BRANCH_PROVIDER_PREFIX}{bare_path}:{remote_branch}"),
        display: display.clone(),
        context: Some(context.clone()),
        preview_root_path: format!("remote:{bare_path}:{remote_branch}"),
        preferred_preview_path: None,
        selection_path: target_path.clone(),
        metadata_path: target_path,
        search_text: vec![display, context],
        kind: NavigateEntryKind::RemoteBranch {
            repo_label: repo_label.to_string(),
            branch,
            remote_branch: remote_branch.to_string(),
            bare_path: bare_path.to_string(),
            container_path: container_path.to_string(),
        },
    }
}

fn remote_branch_context(summary: Option<&RemoteBranchSummary>) -> String {
    let Some(summary) = summary else {
        return "remote".to_string();
    };
    let age = short_relative_age(&summary.age);
    match (summary.age.is_empty(), summary.author.is_empty()) {
        (false, false) => format!("{age} - {}", summary.author),
        (false, true) => age,
        (true, false) => summary.author.clone(),
        (true, true) => "remote".to_string(),
    }
}

fn short_relative_age(age: &str) -> String {
    let trimmed = age.trim();
    if trimmed.eq_ignore_ascii_case("now") || trimmed.eq_ignore_ascii_case("just now") {
        return "now".to_string();
    }

    let mut parts = trimmed.split_whitespace();
    let Some(value) = parts.next() else {
        return trimmed.to_string();
    };
    let Some(unit) = parts.next() else {
        return trimmed.to_string();
    };
    let suffix = match unit.trim_end_matches('s') {
        "second" => "s",
        "minute" => "m",
        "hour" => "h",
        "day" => "d",
        "week" => "w",
        "month" => "mo",
        "year" => "y",
        _ => return trimmed.to_string(),
    };
    format!("{value}{suffix}")
}

fn is_dir(path: &Path) -> bool {
    fs::metadata(path)
        .map(|meta| meta.is_dir())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::{
        project_entry, short_relative_age, worktree_entry, WorktreeCache, WORKTREE_CACHE_VERSION,
    };
    use crate::model::GitWorktree;

    #[test]
    fn worktree_entry_selects_worktree_but_previews_repo() {
        let repo = super::project_entry("/repos/app");
        let worktree = GitWorktree {
            path: "/repos/app-QCDI-8206".to_string(),
            branch: Some("QCDI-8206".to_string()),
            detached: false,
            bare: false,
        };

        let entry = worktree_entry(&repo, &worktree);

        assert_eq!(entry.display, " app QCDI-8206");
        assert!(entry.context.is_none());
        assert_eq!(entry.preview_root_path, "/repos/app");
        assert_eq!(
            entry.preferred_preview_path.as_deref(),
            Some("/repos/app-QCDI-8206")
        );
        assert_eq!(entry.selection_path, "/repos/app-QCDI-8206");
        assert!(matches!(
            entry.kind,
            crate::model::NavigateEntryKind::Worktree { .. }
        ));
    }

    #[test]
    fn worktree_cache_round_trips_entries() {
        let repo = project_entry("/repos/app");
        let worktree = GitWorktree {
            path: "/repos/app-QCDI-8206".to_string(),
            branch: Some("QCDI-8206".to_string()),
            detached: false,
            bare: false,
        };
        let cache = WorktreeCache {
            version: WORKTREE_CACHE_VERSION,
            generated_at: 1,
            entries: vec![worktree_entry(&repo, &worktree)],
        };

        let json = serde_json::to_string(&cache).expect("cache should serialize");
        let restored: WorktreeCache = serde_json::from_str(&json).expect("cache should parse");

        assert_eq!(restored.version, WORKTREE_CACHE_VERSION);
        assert_eq!(restored.entries[0].display, " app QCDI-8206");
        assert_eq!(restored.entries[0].selection_path, "/repos/app-QCDI-8206");
    }

    #[test]
    fn shortens_relative_ages() {
        assert_eq!(short_relative_age("12 minutes ago"), "12m");
        assert_eq!(short_relative_age("3 hours ago"), "3h");
        assert_eq!(short_relative_age("1 month ago"), "1mo");
        assert_eq!(short_relative_age("just now"), "now");
    }
}
