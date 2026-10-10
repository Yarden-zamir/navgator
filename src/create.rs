//! Create recipes: prompt defaults, validation, path completion and the
//! shell run. Both UIs keep their own form state and call these functions.

use crate::commands::run_shell_recipe;
use crate::model::{CreateDefinition, CreatePromptKind};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

/// One `(prompt name, value)` pair after normalization.
pub type CreateValues = Vec<(String, String)>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathSuggestion {
    pub display: String,
    pub value: String,
    pub is_dir: bool,
}

/// A required prompt has no value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissingCreateValue {
    pub prompt_index: usize,
    pub message: String,
}

pub fn create_recipe_index(items: &[CreateDefinition], selector: &str) -> Option<usize> {
    let normalized_selector = create_recipe_selector(selector);
    items.iter().position(|item| {
        item.label.eq_ignore_ascii_case(selector)
            || create_recipe_selector(&item.label) == normalized_selector
    })
}

pub fn create_recipe_selector(value: &str) -> String {
    let mut selector = String::new();
    let mut pending_dash = false;
    for ch in value.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            if pending_dash && !selector.is_empty() {
                selector.push('-');
            }
            selector.push(ch);
            pending_dash = false;
        } else {
            pending_dash = true;
        }
    }
    selector
}

pub fn filtered_create_indexes(items: &[CreateDefinition], query: &str) -> Vec<usize> {
    let query = query.trim().to_lowercase();
    items
        .iter()
        .enumerate()
        .filter(|(_, item)| {
            query.is_empty()
                || item.label.to_lowercase().contains(&query)
                || item
                    .icon
                    .as_deref()
                    .is_some_and(|icon| icon.to_lowercase().contains(&query))
        })
        .map(|(index, _)| index)
        .collect()
}

/// The first value of every prompt: its default, expanded with the earlier prompts.
pub fn initial_prompt_values(item: &CreateDefinition, selected_path: Option<&str>) -> Vec<String> {
    let mut values = Vec::new();
    item.prompts
        .iter()
        .map(|prompt| {
            let value = prompt
                .default
                .as_deref()
                .map(|default| expand_create_value(default, selected_path, &values))
                .unwrap_or_default();
            values.push((
                prompt.name.clone(),
                normalized_prompt_value(prompt.kind, &value),
            ));
            value
        })
        .collect()
}

/// Fill empty prompts after `active_prompt` from their defaults, so a default
/// such as `../{branch}` follows the value typed above it.
pub fn refill_dependent_defaults(
    item: &CreateDefinition,
    selected_path: Option<&str>,
    current: &[String],
    active_prompt: usize,
) -> Vec<String> {
    let mut values = Vec::new();
    item.prompts
        .iter()
        .enumerate()
        .map(|(index, prompt)| {
            let current = current.get(index).cloned().unwrap_or_default();
            let refilled = prompt
                .default
                .as_deref()
                .filter(|_| index > active_prompt && current.trim().is_empty())
                .map(|default| expand_create_value(default, selected_path, &values));
            let value = refilled.unwrap_or(current);
            values.push((
                prompt.name.clone(),
                normalized_prompt_value(prompt.kind, &value),
            ));
            value
        })
        .collect()
}

pub fn validate_create_values(
    item: &CreateDefinition,
    raw: &[String],
) -> Result<CreateValues, MissingCreateValue> {
    let mut values = Vec::new();
    for (index, prompt) in item.prompts.iter().enumerate() {
        let raw_value = raw.get(index).map(|value| value.trim()).unwrap_or_default();
        if prompt.required && raw_value.is_empty() {
            return Err(MissingCreateValue {
                prompt_index: index,
                message: format!("{} is required", prompt.label),
            });
        }
        values.push((
            prompt.name.clone(),
            normalized_prompt_value(prompt.kind, raw_value),
        ));
    }
    Ok(values)
}

pub fn normalized_prompt_value(kind: CreatePromptKind, value: &str) -> String {
    if kind == CreatePromptKind::Path {
        expand_home_prefix(value.trim())
    } else {
        value.trim().to_string()
    }
}

pub fn expand_create_value(
    value: &str,
    selected_path: Option<&str>,
    values: &[(String, String)],
) -> String {
    let mut expanded = value.to_string();
    if let Some(path) = selected_path {
        expanded = expanded.replace("{path}", path);
    }
    for (name, prompt_value) in values {
        expanded = expanded.replace(&format!("{{{name}}}"), prompt_value);
    }
    expand_home_prefix(&expanded)
}

pub fn create_env_values(
    selected_path: Option<&str>,
    values: &[(String, String)],
) -> Vec<(String, String)> {
    let mut envs = Vec::new();
    if let Some(path) = selected_path {
        envs.push(("NAVGATOR_SELECTED_PATH".to_string(), path.to_string()));
    }
    for (name, value) in values {
        envs.push((
            format!("NAVGATOR_CREATE_{}", name.to_ascii_uppercase()),
            value.clone(),
        ));
    }
    envs
}

/// Run the recipe shell and return its `success_path`. Blocks until the shell exits.
pub fn run_create_recipe(
    item: &CreateDefinition,
    selected_path: Option<&str>,
    values: &[(String, String)],
    mut progress: impl FnMut(String),
) -> Result<String, String> {
    let success_path = expand_create_value(&item.success_path, selected_path, values);
    if success_path.trim().is_empty() {
        return Err("success_path expanded to an empty value".to_string());
    }
    let current_dir = item
        .current_dir
        .as_deref()
        .map(|value| PathBuf::from(expand_create_value(value, selected_path, values)));
    let envs = create_env_values(selected_path, values);
    progress(format!("Running {}", item.label));
    let result = run_shell_recipe(&item.shell, current_dir.as_deref(), &envs)?;
    if !result.stdout.is_empty() {
        progress(result.stdout);
    }
    let success = PathBuf::from(&success_path);
    if !success.exists() {
        return Err(format!(
            "success_path does not exist: {}",
            success.display()
        ));
    }
    Ok(success.to_string_lossy().to_string())
}

pub fn path_suggestions_for_input(input: &str) -> Vec<PathSuggestion> {
    let expanded = expand_home_prefix(input);
    let expanded_path = Path::new(&expanded);
    let input_ends_with_separator =
        input.ends_with('/') || input.ends_with(std::path::MAIN_SEPARATOR);
    let (parent, partial) = if input_ends_with_separator {
        (expanded_path, "")
    } else {
        (
            expanded_path.parent().unwrap_or_else(|| Path::new(".")),
            expanded_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(""),
        )
    };
    let Ok(entries) = fs::read_dir(parent) else {
        return Vec::new();
    };
    let mut suggestions = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            if !partial.is_empty() && !name.starts_with(partial) {
                return None;
            }
            let path = entry.path();
            let is_dir = path.is_dir();
            let mut value = path.to_string_lossy().to_string();
            if is_dir && !value.ends_with(std::path::MAIN_SEPARATOR) {
                value.push(std::path::MAIN_SEPARATOR);
            }
            Some(PathSuggestion {
                display: if is_dir { format!("{name}/") } else { name },
                value: display_path_for_input(&value),
                is_dir,
            })
        })
        .collect::<Vec<PathSuggestion>>();
    suggestions.sort_by(|left, right| {
        right
            .is_dir
            .cmp(&left.is_dir)
            .then(left.display.cmp(&right.display))
    });
    suggestions.truncate(8);
    suggestions
}

pub fn expand_home_prefix(value: &str) -> String {
    if value == "~" {
        return env::var("HOME").unwrap_or_else(|_| value.to_string());
    }
    if let Some(rest) = value.strip_prefix("~/") {
        if let Ok(home) = env::var("HOME") {
            return Path::new(&home).join(rest).to_string_lossy().to_string();
        }
    }
    value.to_string()
}

pub fn display_path_for_input(value: &str) -> String {
    let Ok(home) = env::var("HOME") else {
        return value.to_string();
    };
    if value == home {
        return "~".to_string();
    }
    let home_prefix = format!(
        "{}{}",
        home.trim_end_matches(std::path::MAIN_SEPARATOR),
        std::path::MAIN_SEPARATOR
    );
    if let Some(rest) = value.strip_prefix(&home_prefix) {
        return format!("~/{rest}");
    }
    value.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::default_create_definitions;

    #[test]
    fn create_recipe_selector_matches_labels() {
        let items = default_create_definitions();

        assert_eq!(create_recipe_selector("New project"), "new-project");
        assert_eq!(create_recipe_index(&items, "new-project"), Some(0));
        assert_eq!(
            create_recipe_index(&items, "New branch + worktree"),
            Some(1)
        );
    }

    #[test]
    fn expands_create_placeholders() {
        assert_eq!(
            expand_create_value(
                "/tmp/{name}/{path}",
                Some("selected"),
                &[("name".to_string(), "repo".to_string())],
            ),
            "/tmp/repo/selected"
        );
    }

    #[test]
    fn dependent_default_follows_the_prompt_above() {
        let worktree = &default_create_definitions()[1];
        let current = vec!["feat-x".to_string(), "main".to_string(), String::new()];

        let refilled = refill_dependent_defaults(worktree, Some("/repos/app"), &current, 0);

        assert_eq!(refilled[2], "../feat-x");
    }

    #[test]
    fn dependent_default_keeps_typed_values() {
        let worktree = &default_create_definitions()[1];
        let current = vec![
            "feat-x".to_string(),
            "main".to_string(),
            "/tmp/x".to_string(),
        ];

        let refilled = refill_dependent_defaults(worktree, Some("/repos/app"), &current, 0);

        assert_eq!(refilled[2], "/tmp/x");
    }

    #[test]
    fn missing_required_value_names_the_prompt() {
        let project = &default_create_definitions()[0];

        let error = validate_create_values(project, &[String::new(), "~/Github".to_string()])
            .expect_err("name is required");

        assert_eq!(error.prompt_index, 0);
        assert_eq!(error.message, "Project name is required");
    }
}
