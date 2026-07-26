use std::{path::Path, process::Command};

// Generic process helpers live in gator; navgator keeps only the git-specific
// command runners below.
pub(crate) use gator::process::{
    open_url, run_command_output, run_interactive_command, run_shell_recipe,
};

pub(crate) fn run_git_command_allow_empty(repo_dir: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_dir)
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

pub(crate) fn git_command_succeeds(repo_dir: &Path, args: &[&str]) -> bool {
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
