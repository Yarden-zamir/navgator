//! Configured actions: which ones apply to a path, and what they resolve to.

use crate::github::github_url_for_path;
use crate::model::{ActionDefinition, ActionKind};
use std::path::{Path, PathBuf};

/// What one configured action does for one target path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolvedAction {
    /// Hand the path back to the caller, for example the shell that `cd`s into it.
    Navigate(String),
    Run(RunnableAction),
}

/// An action that runs a program or opens a URL.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunnableAction {
    Command {
        command: String,
        args: Vec<String>,
        current_dir: Option<PathBuf>,
    },
    OpenUrl(String),
}

/// Expand `{path}` and `{github_url}` in every part of `action`.
/// Returns `None` when a placeholder has no value for this path.
pub fn resolve_action(action: &ActionDefinition, path: Option<&str>) -> Option<ResolvedAction> {
    match &action.kind {
        ActionKind::Navigate => path.map(|value| ResolvedAction::Navigate(value.to_string())),
        ActionKind::Command {
            command,
            args,
            current_dir,
        } => {
            let github_url = github_url_for_action(path);
            let command = expand_action_value(command, path, github_url.as_deref())?;
            let args = args
                .iter()
                .map(|arg| expand_action_value(arg, path, github_url.as_deref()))
                .collect::<Option<Vec<String>>>()?;
            let current_dir_value = current_dir
                .as_ref()
                .map(|dir| expand_action_value(dir, path, github_url.as_deref()))
                .unwrap_or(Some(String::new()))?;
            let current_dir = if current_dir_value.is_empty() {
                None
            } else {
                Some(PathBuf::from(current_dir_value))
            };
            Some(ResolvedAction::Run(RunnableAction::Command {
                command,
                args,
                current_dir,
            }))
        }
        ActionKind::OpenUrl { url } => {
            let github_url = github_url_for_action(path);
            let url = expand_action_value(url, path, github_url.as_deref())?;
            Some(ResolvedAction::Run(RunnableAction::OpenUrl(url)))
        }
    }
}

pub fn github_url_for_action(path: Option<&str>) -> Option<String> {
    github_url_for_path(Path::new(path?))
}

pub fn expand_action_value(
    value: &str,
    path: Option<&str>,
    github_url: Option<&str>,
) -> Option<String> {
    if value.contains("{path}") && path.is_none() {
        return None;
    }
    if value.contains("{github_url}") && github_url.is_none() {
        return None;
    }

    let mut expanded = value.to_string();
    if let Some(path) = path {
        expanded = expanded.replace("{path}", path);
    }
    if let Some(github_url) = github_url {
        expanded = expanded.replace("{github_url}", github_url);
    }
    Some(expanded)
}

/// Indexes into `actions` for the picker: the `picker` allowlist order (or
/// all actions), minus unmet file conditions, filtered by `query`.
pub fn filtered_action_indexes(
    actions: &[ActionDefinition],
    picker: Option<&[String]>,
    query: &str,
    path: Option<&str>,
) -> Vec<usize> {
    let query = query.trim().to_lowercase();
    let indexes = picker.map_or_else(
        || (0..actions.len()).collect::<Vec<_>>(),
        |ids| {
            ids.iter()
                .filter_map(|id| {
                    actions
                        .iter()
                        .position(|action| action.id.as_deref() == Some(id.as_str()))
                })
                .collect()
        },
    );
    indexes
        .into_iter()
        .filter(|index| action_file_condition_matches(&actions[*index], path))
        .filter(|index| {
            let action = &actions[*index];
            query.is_empty()
                || action.label.to_lowercase().contains(&query)
                || action
                    .icon
                    .as_deref()
                    .is_some_and(|icon| icon.to_lowercase().contains(&query))
        })
        .collect()
}

pub fn action_file_condition_matches(action: &ActionDefinition, path: Option<&str>) -> bool {
    let Some(condition) = action.file_condition.as_deref() else {
        return true;
    };
    let Some(path) = path else {
        return false;
    };
    let condition_path = Path::new(condition);
    let target = if condition_path.is_absolute() {
        condition_path.to_path_buf()
    } else {
        Path::new(path).join(condition_path)
    };
    target.exists()
}

#[cfg(test)]
mod tests {
    use super::expand_action_value;

    #[test]
    fn expands_action_path_placeholders() {
        assert_eq!(
            expand_action_value("cd {path}", Some("/repos/project"), None),
            Some("cd /repos/project".to_string())
        );
        assert_eq!(expand_action_value("cd {path}", None, None), None);
    }
}
