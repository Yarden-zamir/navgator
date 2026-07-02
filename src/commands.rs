use std::{path::Path, process::Command};

pub(crate) fn run_command_output(
    program: &str,
    args: &[String],
    current_dir: Option<&Path>,
) -> Option<String> {
    let mut cmd = Command::new(program);
    cmd.args(args);
    if let Some(dir) = current_dir {
        cmd.current_dir(dir);
    }
    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .to_string();
    if stdout.is_empty() {
        None
    } else {
        Some(stdout)
    }
}

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
