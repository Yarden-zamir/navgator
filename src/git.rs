use crate::commands::{git_command_succeeds, run_git_command_allow_empty};
use crate::model::GitWorktree;
use crate::search::entry_name;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Clone)]
pub(crate) struct RemoteBranchSummary {
    pub(crate) age: String,
    pub(crate) author: String,
}

pub(crate) fn git_command_dir_for_path(path: &Path) -> Option<PathBuf> {
    let dir = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent().map(Path::to_path_buf)?
    };

    if git_command_succeeds(&dir, &["rev-parse", "--git-dir"]) {
        return Some(dir);
    }

    let dot_bare = dir.join(".bare");
    if dot_bare.is_dir() && git_is_bare_repository(&dot_bare) {
        return Some(dot_bare);
    }

    Some(dir)
}

pub(crate) fn git_is_inside_work_tree(repo_dir: &Path) -> bool {
    run_git_command_allow_empty(repo_dir, &["rev-parse", "--is-inside-work-tree"])
        .map(|value| value.trim() == "true")
        .unwrap_or(false)
}

pub(crate) fn git_is_bare_repository(repo_dir: &Path) -> bool {
    run_git_command_allow_empty(repo_dir, &["rev-parse", "--is-bare-repository"])
        .map(|value| value.trim() == "true")
        .unwrap_or(false)
}

pub(crate) fn git_worktrees_for_path(path: &Path) -> Vec<GitWorktree> {
    let Some(repo_dir) = git_command_dir_for_path(path) else {
        return Vec::new();
    };
    let Some(output) = run_git_command_allow_empty(&repo_dir, &["worktree", "list", "--porcelain"])
    else {
        return Vec::new();
    };
    parse_git_worktree_list(&output)
}

pub(crate) fn git_worktree_root_for_path(path: &Path) -> Option<String> {
    run_git_command_allow_empty(path, &["rev-parse", "--show-toplevel"])
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(crate) fn git_default_branch_for_path(path: &Path) -> Option<String> {
    let repo_dir = git_command_dir_for_path(path)?;
    let output = run_git_command_allow_empty(
        &repo_dir,
        &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
    )?;
    output
        .trim()
        .strip_prefix("origin/")
        .filter(|branch| !branch.trim().is_empty())
        .map(ToString::to_string)
}

pub(crate) fn selectable_worktree_path_for_project(path: &Path) -> Option<String> {
    let worktrees = git_worktrees_for_path(path)
        .into_iter()
        .filter(|worktree| !worktree.bare)
        .collect::<Vec<GitWorktree>>();
    if worktrees.is_empty() {
        return None;
    }

    if let Some(default_branch) = git_default_branch_for_path(path) {
        if let Some(worktree) = worktrees
            .iter()
            .find(|worktree| worktree.branch.as_deref() == Some(default_branch.as_str()))
        {
            return Some(worktree.path.clone());
        }
    }

    worktrees
        .iter()
        .find(|worktree| {
            matches!(
                worktree.branch.as_deref(),
                Some("main" | "master" | "trunk")
            )
        })
        .or_else(|| worktrees.first())
        .map(|worktree| worktree.path.clone())
}

pub(crate) fn dot_bare_for_path(path: &Path) -> Option<(PathBuf, PathBuf)> {
    if path.is_dir()
        && path.file_name().and_then(|name| name.to_str()) == Some(".bare")
        && git_is_bare_repository(path)
    {
        return Some((path.to_path_buf(), path.parent()?.to_path_buf()));
    }

    let dot_bare = path.join(".bare");
    if dot_bare.is_dir() && git_is_bare_repository(&dot_bare) {
        return Some((dot_bare, path.to_path_buf()));
    }

    let repo_root = git_stdout(&["-C", path.to_str()?, "rev-parse", "--show-toplevel"])?;
    let repo_root = PathBuf::from(repo_root.trim());
    let container = repo_root.parent()?.to_path_buf();
    let bare = container.join(".bare");
    if bare.is_dir() && git_is_bare_repository(&bare) {
        Some((bare, container))
    } else {
        None
    }
}

pub(crate) fn fetch_remote_branch(bare: &Path, remote_branch: &str) -> Result<(), String> {
    let branch = local_branch_for_remote(remote_branch);
    let output = Command::new("git")
        .arg("--git-dir")
        .arg(bare)
        .arg("fetch")
        .arg("origin")
        .arg(format!("refs/heads/{branch}:refs/remotes/origin/{branch}"))
        .env("NO_COLOR", "1")
        .output()
        .map_err(|err| format!("failed to run git fetch: {err}"))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.is_empty() {
            Err("failed to fetch origin".to_string())
        } else {
            Err(stderr)
        }
    }
}

pub(crate) fn ls_remote_heads(bare: &Path) -> Result<Vec<String>, String> {
    let output = Command::new("git")
        .arg("--git-dir")
        .arg(bare)
        .arg("ls-remote")
        .arg("--heads")
        .arg("origin")
        .env("NO_COLOR", "1")
        .output()
        .map_err(|err| format!("failed to run git ls-remote: {err}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return if stderr.is_empty() {
            Err("failed to query remote branches".to_string())
        } else {
            Err(stderr)
        };
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.split_once("refs/heads/"))
        .map(|(_, branch)| format!("origin/{branch}"))
        .filter(|branch| !branch.trim().is_empty())
        .collect())
}

pub(crate) fn local_remote_branches_for_bare(bare: &Path) -> Vec<String> {
    let Some(output) = git_stdout_path(
        bare,
        &["for-each-ref", "--format=%(refname:short)", "refs/remotes"],
    ) else {
        return Vec::new();
    };
    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.ends_with("/HEAD"))
        .map(ToString::to_string)
        .collect()
}

pub(crate) fn remote_branch_summaries_for_bare(
    bare: &Path,
) -> HashMap<String, RemoteBranchSummary> {
    let Some(output) = git_stdout_path(
        bare,
        &[
            "for-each-ref",
            "--format=%(refname:short)%00%(committerdate:relative)%00%(authorname)",
            "refs/remotes",
        ],
    ) else {
        return HashMap::new();
    };

    let mut summaries = HashMap::new();
    for line in output.lines() {
        let mut parts = line.split('\0');
        let Some(branch) = parts
            .next()
            .map(str::trim)
            .filter(|branch| !branch.is_empty())
        else {
            continue;
        };
        if branch.ends_with("/HEAD") {
            continue;
        }
        let age = parts.next().map(str::trim).unwrap_or_default();
        let author = parts.next().map(str::trim).unwrap_or_default();
        if !age.is_empty() || !author.is_empty() {
            summaries.insert(
                branch.to_string(),
                RemoteBranchSummary {
                    age: age.to_string(),
                    author: author.to_string(),
                },
            );
        }
    }
    summaries
}

pub(crate) fn add_worktree_for_remote_with_progress(
    bare: &Path,
    container: &Path,
    remote_branch: &str,
    mut progress: impl FnMut(String),
) -> Result<String, String> {
    let branch = local_branch_for_remote(remote_branch);
    progress(format!("Checking worktrees for {branch}"));
    if let Some(existing) = worktree_for_branch(bare, &branch) {
        progress(format!("Using existing worktree at {existing}"));
        return Ok(existing);
    }

    if !remote_ref_exists(bare, remote_branch) {
        progress(format!("Fetching {remote_branch}"));
        fetch_remote_branch(bare, remote_branch)?;
    } else {
        progress(format!("Remote ref {remote_branch} is available locally"));
    }

    let target = remote_branch_target_path(bare, container, remote_branch);
    progress(format!("Preparing {}", target.display()));
    if target.exists() || target.symlink_metadata().is_ok() {
        return Err(format!("target already exists: {}", target.display()));
    }

    progress(format!("Creating worktree for {branch}"));
    let mut command = Command::new("git");
    command
        .arg("--git-dir")
        .arg(bare)
        .arg("worktree")
        .arg("add")
        .env("NO_COLOR", "1");
    if branch_exists(bare, &branch) {
        command.arg(&target).arg(&branch);
    } else {
        command
            .arg("-b")
            .arg(&branch)
            .arg(&target)
            .arg(remote_branch);
    }
    let output = command
        .output()
        .map_err(|err| format!("failed to run git worktree add: {err}"))?;
    if !output.status.success() {
        return Err(format!("failed to create worktree for {branch}"));
    }
    progress(format!("Created {}", target.display()));
    Ok(target.to_string_lossy().to_string())
}

pub(crate) fn remove_worktree_safely_with_progress(
    path: &Path,
    mut progress: impl FnMut(String),
) -> Result<String, String> {
    progress(format!("Checking {}", path.display()));
    if current_dir_is_inside(path) {
        return Err("cannot delete the current working directory".to_string());
    }
    if !git_is_inside_work_tree(path) {
        return Err("selected path is not a git worktree".to_string());
    }

    progress("Checking for local, untracked, and ignored files".to_string());
    let status = run_git_command_result(
        path,
        &[
            "status",
            "--porcelain=v1",
            "--untracked-files=all",
            "--ignored=matching",
        ],
    )?;
    if !status.trim().is_empty() {
        return Err("worktree has local, untracked, or ignored files".to_string());
    }

    progress("Checking branch and upstream".to_string());
    let branch = run_git_command_result(path, &["symbolic-ref", "--short", "HEAD"])?;
    if branch.trim().is_empty() {
        return Err("worktree is detached".to_string());
    }
    if !branch_has_upstream(path) {
        return Err("branch has no upstream".to_string());
    }

    progress("Checking for unpushed commits".to_string());
    let counts = run_git_command_result(
        path,
        &["rev-list", "--left-right", "--count", "HEAD...@{u}"],
    )?;
    let mut parts = counts.split_whitespace();
    let ahead = parts
        .next()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(1);
    if ahead > 0 {
        return Err("branch has unpushed commits".to_string());
    }

    progress("Removing worktree".to_string());
    let git_dir = worktree_common_git_dir(path)?;
    let output = Command::new("git")
        .arg("--git-dir")
        .arg(git_dir)
        .arg("worktree")
        .arg("remove")
        .arg(path)
        .env("NO_COLOR", "1")
        .output()
        .map_err(|err| format!("failed to run git worktree remove: {err}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return if stderr.is_empty() {
            Err("failed to remove worktree".to_string())
        } else {
            Err(stderr)
        };
    }
    progress(format!("Removed {}", path.display()));
    Ok(path.to_string_lossy().to_string())
}

pub(crate) fn remote_branch_target_path(
    bare: &Path,
    container: &Path,
    remote_branch: &str,
) -> PathBuf {
    let branch = local_branch_for_remote(remote_branch);
    container.join(dir_name_for_branch(bare, container, &branch))
}

pub(crate) fn remote_branch_target_paths(
    bare: &Path,
    container: &Path,
    remote_branches: &[String],
) -> HashMap<String, PathBuf> {
    let mut candidate_branches = local_ref_branch_names(bare);
    candidate_branches.extend(
        remote_branches
            .iter()
            .map(|branch| local_branch_for_remote(branch)),
    );
    let basename_branches = basename_branch_sets(candidate_branches);
    remote_branches
        .iter()
        .map(|remote_branch| {
            let branch = local_branch_for_remote(remote_branch);
            let dir_name =
                dir_name_for_branch_with_candidates(container, &branch, &basename_branches);
            (remote_branch.clone(), container.join(dir_name))
        })
        .collect()
}

pub(crate) fn local_branch_for_remote(remote_branch: &str) -> String {
    remote_branch
        .split_once('/')
        .map(|(_, branch)| branch.to_string())
        .unwrap_or_else(|| remote_branch.to_string())
}

pub(crate) fn parse_git_worktree_list(output: &str) -> Vec<GitWorktree> {
    let mut worktrees = Vec::new();
    let mut current: Option<GitWorktree> = None;

    for raw_line in output.lines() {
        let line = raw_line.trim_end();
        if line.is_empty() {
            if let Some(worktree) = current.take() {
                worktrees.push(worktree);
            }
            continue;
        }

        if let Some(path) = line.strip_prefix("worktree ") {
            if let Some(worktree) = current.take() {
                worktrees.push(worktree);
            }
            current = Some(GitWorktree {
                path: path.to_string(),
                branch: None,
                detached: false,
                bare: false,
            });
        } else if let Some(worktree) = current.as_mut() {
            if let Some(branch) = line.strip_prefix("branch ") {
                worktree.branch = Some(git_branch_label(branch));
            } else if line == "detached" {
                worktree.detached = true;
            } else if line == "bare" {
                worktree.bare = true;
            }
        }
    }

    if let Some(worktree) = current {
        worktrees.push(worktree);
    }

    worktrees
}

fn dir_name_for_branch(bare: &Path, container: &Path, branch: &str) -> String {
    let basename_branches = basename_branch_sets(local_ref_branch_names(bare));
    dir_name_for_branch_with_candidates(container, branch, &basename_branches)
}

fn dir_name_for_branch_with_candidates(
    container: &Path,
    branch: &str,
    basename_branches: &HashMap<String, HashSet<String>>,
) -> String {
    let basename = branch_basename(branch);
    let conflicts = basename_branches
        .get(&basename)
        .map(|branches| branches.iter().any(|candidate| candidate != branch))
        .unwrap_or(false);
    let basename_path = container.join(&basename);
    if conflicts
        || (basename_path.exists()
            && !basename_path.join(".git").is_file()
            && !basename_path.join(".git").is_dir())
    {
        branch.replace('/', "-")
    } else {
        basename
    }
}

fn local_ref_branch_names(bare: &Path) -> Vec<String> {
    let Some(output) = git_stdout_path(
        bare,
        &[
            "for-each-ref",
            "--format=%(refname:short)",
            "refs/heads",
            "refs/remotes",
        ],
    ) else {
        return Vec::new();
    };
    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.ends_with("/HEAD"))
        .map(|candidate| {
            if candidate.contains('/') {
                local_branch_for_remote(candidate)
            } else {
                candidate.to_string()
            }
        })
        .collect()
}

fn basename_branch_sets(branches: Vec<String>) -> HashMap<String, HashSet<String>> {
    let mut by_basename: HashMap<String, HashSet<String>> = HashMap::new();
    for branch in branches {
        by_basename
            .entry(branch_basename(&branch))
            .or_default()
            .insert(branch);
    }
    by_basename
}

fn worktree_for_branch(bare: &Path, branch: &str) -> Option<String> {
    git_worktrees_for_path(bare)
        .into_iter()
        .find(|worktree| worktree.branch.as_deref() == Some(branch))
        .map(|worktree| worktree.path)
}

fn branch_exists(bare: &Path, branch: &str) -> bool {
    Command::new("git")
        .arg("--git-dir")
        .arg(bare)
        .arg("show-ref")
        .arg("--verify")
        .arg("--quiet")
        .arg(format!("refs/heads/{branch}"))
        .env("NO_COLOR", "1")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn remote_ref_exists(bare: &Path, remote_branch: &str) -> bool {
    Command::new("git")
        .arg("--git-dir")
        .arg(bare)
        .arg("show-ref")
        .arg("--verify")
        .arg("--quiet")
        .arg(format!("refs/remotes/{remote_branch}"))
        .env("NO_COLOR", "1")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn branch_basename(branch: &str) -> String {
    branch.rsplit('/').next().unwrap_or(branch).to_string()
}

fn current_dir_is_inside(path: &Path) -> bool {
    let Ok(current_dir) = std::env::current_dir().and_then(|path| path.canonicalize()) else {
        return false;
    };
    let Ok(path) = path.canonicalize() else {
        return false;
    };
    current_dir.starts_with(path)
}

fn branch_has_upstream(path: &Path) -> bool {
    run_git_command_success(path, &["rev-parse", "--verify", "--quiet", "@{u}"])
}

fn worktree_common_git_dir(path: &Path) -> Result<PathBuf, String> {
    if let Some((bare, _)) = dot_bare_for_path(path) {
        return Ok(bare);
    }
    let common_dir = run_git_command_result(path, &["rev-parse", "--git-common-dir"])?;
    let common_path = PathBuf::from(common_dir.trim());
    if common_path.is_absolute() {
        Ok(common_path)
    } else {
        Ok(path.join(common_path))
    }
}

fn run_git_command_result(repo_dir: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_dir)
        .arg("-c")
        .arg("color.ui=never")
        .args(args)
        .env("NO_COLOR", "1")
        .output()
        .map_err(|err| format!("failed to run git {}: {err}", args.join(" ")))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout)
            .trim_end()
            .to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.is_empty() {
            Err(format!("git {} failed", args.join(" ")))
        } else {
            Err(stderr)
        }
    }
}

fn run_git_command_success(repo_dir: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(repo_dir)
        .arg("-c")
        .arg("color.ui=never")
        .args(args)
        .env("NO_COLOR", "1")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn git_stdout(args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .env("NO_COLOR", "1")
        .output()
        .ok()?;
    if output.status.success() {
        Some(
            String::from_utf8_lossy(&output.stdout)
                .trim_end()
                .to_string(),
        )
    } else {
        None
    }
}

fn git_stdout_path(bare: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("--git-dir")
        .arg(bare)
        .args(args)
        .env("NO_COLOR", "1")
        .output()
        .ok()?;
    if output.status.success() {
        Some(
            String::from_utf8_lossy(&output.stdout)
                .trim_end()
                .to_string(),
        )
    } else {
        None
    }
}

pub(crate) fn git_worktree_label(worktree: &GitWorktree, shorten_after_slash: bool) -> String {
    if let Some(branch) = worktree.branch.as_ref() {
        if !branch.trim().is_empty() {
            return worktree_tab_label(branch, shorten_after_slash);
        }
    }
    if worktree.detached {
        return "detached".to_string();
    }
    let name = entry_name(&worktree.path);
    if name.trim().is_empty() {
        "worktree".to_string()
    } else {
        worktree_tab_label(&name, shorten_after_slash)
    }
}

pub(crate) fn worktree_tab_label(label: &str, shorten_after_slash: bool) -> String {
    if !shorten_after_slash {
        return label.to_string();
    }
    label
        .rsplit('/')
        .find(|segment| !segment.trim().is_empty())
        .unwrap_or(label)
        .to_string()
}

fn git_branch_label(branch: &str) -> String {
    if let Some(value) = branch.strip_prefix("refs/heads/") {
        return value.to_string();
    }
    if let Some(value) = branch.strip_prefix("refs/remotes/") {
        return value.to_string();
    }
    branch.to_string()
}
