use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};
use std::{
    collections::{HashMap, HashSet},
    env, fs,
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
use tui_input::backend::crossterm::EventHandler;
use tui_input::{Input, InputRequest};

mod commands;
mod compositor;
mod config;
mod content;
mod git;
mod github;
mod metadata;
mod model;
mod provider_runtime;
mod results;
mod search;
mod tags;
mod ui;

use compositor::{CurrentCompositor, FocusChange, NavigationCompositor};
use config::config_schema_json;
use content::{
    apply_git_result, apply_github_readme_result, build_placeholder_text, build_preview_data,
    build_remote_branch_preview_data, ensure_git_for_preview,
};
use gator::{copy_to_clipboard, ensure_tty_stdin, input_at_end, setup_terminal, write_selection};
use github::ensure_github_readme_for_preview;
use metadata::{ensure_dates_for_paths, spawn_bulk_metadata_fetch};
use model::{
    ActionBinding, ActionBindingKey, ActionDefinition, ActionKind, ActionSettings, AppResult,
    BranchSelectBehavior, BranchSettings, CreateDefinition, CreatePromptKind, CreateSettings,
    Focus, GitResult, GithubReadmeResult, HelpColors, HelpContext, MetaResult, NavigateEntry,
    NavigateEntryKind, PreviewColors, PreviewData, PreviewResult, PreviewSettings, RemoteSettings,
    RemoteToggleState, ResultUpdate, SidePanelRender, SortMeta, SortMode, SortSettings, TagResult,
    ThemeColors, VisibleListArgs, DATE_PLACEHOLDER,
};
use results::{
    build_items, spawn_remote_branch_result_provider, spawn_worktree_result_provider,
    REMOTE_BRANCH_PROVIDER_PREFIX,
};
use search::{filter_and_sort, index_for_entry_id, parse_query_tokens};
use tags::{
    collect_tag_suggestions, commit_tag_input, ensure_tags_for_paths, read_tags_for_path,
    save_tags_for_path, spawn_bulk_tag_fetch,
};
use ui::{
    build_help_line, build_visible_list_items, compose_preview_text,
    compose_preview_text_with_input, compute_ui_layout, preview_content_area, rect_contains,
    render_side_panels, text_line_count,
};

fn main() -> AppResult<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() || args[0] == "navigate" {
        ensure_tty_stdin()?;
        return run_navigate(InitialLaunch::Navigate);
    }
    if args[0] == "actions" || args[0] == "action-picker" {
        ensure_tty_stdin()?;
        let launch = if let Some(path) = args.get(1) {
            InitialLaunch::ActionsPath(PathBuf::from(path))
        } else {
            InitialLaunch::ActionsFirstResult
        };
        return run_navigate(launch);
    }
    if args[0] == "create" || args[0] == "new" {
        ensure_tty_stdin()?;
        return run_navigate(create_launch_from_args(&args[1..], &env::current_dir()?));
    }
    if args[0] == "config-schema" || args[0] == "schema" {
        return print_config_schema();
    }
    if args[0] == "--help" || args[0] == "-h" {
        print_usage();
        return Ok(());
    }

    eprintln!("Unknown command.");
    print_usage();
    std::process::exit(2);
}

fn print_usage() {
    eprintln!("Usage:\n  navgator [navigate|actions [path]|create [recipe] [path]|config-schema]");
}

fn print_config_schema() -> AppResult<()> {
    let json = config_schema_json()?;
    println!("{json}");
    Ok(())
}

enum WorktreeProgress {
    Message(String),
    Select(String),
    Deleted(String),
    Error(String),
}

enum CreateProgress {
    Message(String),
    Select(String),
    Error(String),
}

struct WorktreeOverlay {
    title: String,
    messages: Vec<String>,
    error: Option<String>,
}

impl WorktreeOverlay {
    fn new(title: String, first_message: &str) -> Self {
        Self {
            title,
            messages: vec![first_message.to_string()],
            error: None,
        }
    }

    fn is_error(&self) -> bool {
        self.error.is_some()
    }

    fn push_message(&mut self, message: String) {
        self.messages.push(message);
    }

    fn fail(&mut self, message: String) {
        self.error = Some(message);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum InitialLaunch {
    Navigate,
    ActionsFirstResult,
    ActionsPath(PathBuf),
    CreatePicker { path: PathBuf },
    CreateRecipe { selector: String, path: PathBuf },
}

fn create_launch_from_args(args: &[String], cwd: &Path) -> InitialLaunch {
    match args {
        [] => InitialLaunch::CreatePicker {
            path: cwd.to_path_buf(),
        },
        [path] if Path::new(path).exists() => InitialLaunch::CreatePicker {
            path: PathBuf::from(path),
        },
        [selector] => InitialLaunch::CreateRecipe {
            selector: selector.clone(),
            path: cwd.to_path_buf(),
        },
        [selector, path, ..] => InitialLaunch::CreateRecipe {
            selector: selector.clone(),
            path: PathBuf::from(path),
        },
    }
}

fn run_navigate(initial_launch: InitialLaunch) -> AppResult<()> {
    let result = build_items()?;
    match select_from_list(SelectListArgs {
        entries: result.entries,
        preview_settings: result.preview_settings,
        sort_settings: result.sort_settings,
        remote_settings: result.remote_settings,
        branch_settings: result.branch_settings,
        action_settings: result.action_settings,
        create_settings: result.create_settings,
        theme_colors: result.theme_colors,
        initial_launch,
    })? {
        Some(NavigateOutcome::Navigate {
            path,
            close_session,
        }) => write_navigation_outcome(&path, close_session),
        Some(NavigateOutcome::RunAction {
            action,
            close_session,
        }) => {
            run_action(action)?;
            if close_session {
                write_close_session_outcome(None)?;
            }
            Ok(())
        }
        None => std::process::exit(1),
    }
}

const CLOSE_SESSION_MARKER: &str = "__NAVGATOR_CLOSE_SESSION__";

enum NavigateOutcome {
    Navigate {
        path: String,
        close_session: bool,
    },
    RunAction {
        action: ResolvedAction,
        close_session: bool,
    },
}

struct SelectListArgs {
    entries: Vec<NavigateEntry>,
    preview_settings: PreviewSettings,
    sort_settings: SortSettings,
    remote_settings: RemoteSettings,
    branch_settings: BranchSettings,
    action_settings: ActionSettings,
    create_settings: CreateSettings,
    theme_colors: ThemeColors,
    initial_launch: InitialLaunch,
}

enum ResolvedAction {
    Command {
        command: String,
        args: Vec<String>,
        current_dir: Option<PathBuf>,
    },
    OpenUrl(String),
}

struct ActionPicker {
    selected: usize,
    query: String,
    fixed_path: Option<String>,
}

#[derive(Clone, Copy)]
struct ActionPickerColors {
    accent: Color,
    warm: Color,
    text: Color,
    muted: Color,
}

struct ActionPickerRender<'a> {
    actions: &'a [ActionDefinition],
    visible_actions: &'a [usize],
    selected: usize,
    query: &'a str,
    action_binding_label: &'a str,
    colors: ActionPickerColors,
}

struct CreatePicker {
    selected: usize,
    query: String,
    fixed_path: Option<String>,
}

struct CreateForm {
    item_index: usize,
    active_prompt: usize,
    focus: CreateFormFocus,
    fixed_path: Option<String>,
    inputs: Vec<Input>,
    path_suggestions: Vec<PathSuggestion>,
    selected_suggestion: usize,
    error: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CreateFormFocus {
    Fields,
    Completions,
}

#[derive(Clone)]
struct PathSuggestion {
    display: String,
    value: String,
    is_dir: bool,
}

enum CreateModal {
    Picker(CreatePicker),
    Form(CreateForm),
}

struct CreatePickerRender<'a> {
    items: &'a [CreateDefinition],
    visible_items: &'a [usize],
    selected: usize,
    query: &'a str,
    create_binding_label: &'a str,
    colors: ActionPickerColors,
}

struct CreateFormRender<'a> {
    item: &'a CreateDefinition,
    form: &'a CreateForm,
    colors: ActionPickerColors,
}

fn run_action(action: ResolvedAction) -> AppResult<()> {
    let result = match action {
        ResolvedAction::Command {
            command,
            args,
            current_dir,
        } => commands::run_interactive_command(&command, &args, current_dir.as_deref()),
        ResolvedAction::OpenUrl(url) => commands::open_url(&url),
    };
    result.map_err(Into::into)
}

fn write_navigation_outcome(path: &str, close_session: bool) -> AppResult<()> {
    if close_session {
        write_close_session_outcome(Some(path))
    } else {
        write_selection(path)
    }
}

fn write_close_session_outcome(path: Option<&str>) -> AppResult<()> {
    if env::var("NAVGATOR_OUTPUT_PROTOCOL").ok().as_deref() != Some("2") {
        if let Some(path) = path {
            write_selection(path)?;
        }
        return Ok(());
    }

    let Some(output_path) = env::var_os("GATOR_OUTPUT") else {
        if let Some(path) = path {
            println!("{path}");
        }
        return Ok(());
    };

    let contents = match path {
        Some(path) => format!("{CLOSE_SESSION_MARKER}\n{path}\n"),
        None => format!("{CLOSE_SESSION_MARKER}\n"),
    };
    fs::write(&output_path, contents).map_err(|err| {
        format!(
            "Failed to write navgator close-session output {}: {err}",
            PathBuf::from(output_path).display()
        )
        .into()
    })
}

fn resolve_picker_action(
    action: Option<&ActionDefinition>,
    path: Option<&str>,
    close_session: bool,
) -> Option<NavigateOutcome> {
    let action = action?;
    match &action.kind {
        ActionKind::Navigate => path.map(|value| NavigateOutcome::Navigate {
            path: value.to_string(),
            close_session,
        }),
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
            Some(NavigateOutcome::RunAction {
                action: ResolvedAction::Command {
                    command,
                    args,
                    current_dir,
                },
                close_session,
            })
        }
        ActionKind::OpenUrl { url } => {
            let github_url = github_url_for_action(path);
            let url = expand_action_value(url, path, github_url.as_deref())?;
            Some(NavigateOutcome::RunAction {
                action: ResolvedAction::OpenUrl(url),
                close_session,
            })
        }
    }
}

fn github_url_for_action(path: Option<&str>) -> Option<String> {
    github::github_url_for_path(Path::new(path?))
}

fn expand_action_value(
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

fn next_picker_index(selected: usize, len: usize) -> usize {
    if len == 0 {
        0
    } else {
        (selected + 1) % len
    }
}

fn previous_picker_index(selected: usize, len: usize) -> usize {
    if len == 0 {
        0
    } else if selected == 0 {
        len - 1
    } else {
        selected - 1
    }
}

fn first_action_binding_label(bindings: &[ActionBinding]) -> String {
    bindings
        .first()
        .map(|binding| binding.label.clone())
        .unwrap_or_else(|| "Ctrl+Enter".to_string())
}

fn first_create_binding_label(bindings: &[ActionBinding]) -> String {
    bindings
        .first()
        .map(|binding| binding.label.clone())
        .unwrap_or_else(|| "Ctrl+N".to_string())
}

fn matches_action_binding(key: &KeyEvent, bindings: &[ActionBinding]) -> bool {
    bindings
        .iter()
        .any(|binding| matches_action_binding_key(key, &binding.key))
}

fn matches_action_binding_key(key: &KeyEvent, binding: &ActionBindingKey) -> bool {
    match binding {
        ActionBindingKey::CtrlEnter => {
            key.code == KeyCode::Enter && key.modifiers.contains(KeyModifiers::CONTROL)
        }
        ActionBindingKey::CtrlSpace => {
            key.code == KeyCode::Char(' ') && key.modifiers.contains(KeyModifiers::CONTROL)
        }
        ActionBindingKey::CtrlN => {
            key.code == KeyCode::Char('n') && key.modifiers.contains(KeyModifiers::CONTROL)
        }
    }
}

fn matches_create_binding(key: &KeyEvent, bindings: &[ActionBinding]) -> bool {
    matches_action_binding(key, bindings)
}

fn filtered_action_indexes(
    actions: &[ActionDefinition],
    query: &str,
    path: Option<&str>,
) -> Vec<usize> {
    let query = query.trim().to_lowercase();
    actions
        .iter()
        .enumerate()
        .filter(|(_, action)| action_file_condition_matches(action, path))
        .filter(|(_, action)| {
            query.is_empty()
                || action.label.to_lowercase().contains(&query)
                || action
                    .icon
                    .as_deref()
                    .is_some_and(|icon| icon.to_lowercase().contains(&query))
        })
        .map(|(index, _)| index)
        .collect()
}

fn filtered_create_indexes(items: &[CreateDefinition], query: &str) -> Vec<usize> {
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

fn action_file_condition_matches(action: &ActionDefinition, path: Option<&str>) -> bool {
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

fn clamp_picker_selection(picker: &mut ActionPicker, visible_len: usize) {
    if visible_len == 0 {
        picker.selected = 0;
    } else if picker.selected >= visible_len {
        picker.selected = visible_len - 1;
    }
}

fn clamp_create_picker_selection(picker: &mut CreatePicker, visible_len: usize) {
    if visible_len == 0 {
        picker.selected = 0;
    } else if picker.selected >= visible_len {
        picker.selected = visible_len - 1;
    }
}

fn initial_create_modal(
    launch: &InitialLaunch,
    items: &[CreateDefinition],
    fallback_path: Option<&str>,
) -> Option<CreateModal> {
    match launch {
        InitialLaunch::CreatePicker { path } => Some(CreateModal::Picker(CreatePicker {
            selected: 0,
            query: String::new(),
            fixed_path: Some(path.to_string_lossy().to_string()),
        })),
        InitialLaunch::CreateRecipe { selector, path } => {
            let fixed_path = Some(path.to_string_lossy().to_string());
            if let Some(item_index) = create_recipe_index(items, selector) {
                items.get(item_index).map(|item| {
                    let selected_path = fixed_path.clone();
                    CreateModal::Form(create_form_for_item(
                        item_index,
                        item,
                        selected_path.as_deref().or(fallback_path),
                        fixed_path,
                    ))
                })
            } else {
                Some(CreateModal::Picker(CreatePicker {
                    selected: 0,
                    query: selector.clone(),
                    fixed_path,
                }))
            }
        }
        InitialLaunch::Navigate
        | InitialLaunch::ActionsFirstResult
        | InitialLaunch::ActionsPath(_) => None,
    }
}

fn create_recipe_index(items: &[CreateDefinition], selector: &str) -> Option<usize> {
    let normalized_selector = create_recipe_selector(selector);
    items.iter().position(|item| {
        item.label.eq_ignore_ascii_case(selector)
            || create_recipe_selector(&item.label) == normalized_selector
    })
}

fn create_recipe_selector(value: &str) -> String {
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

fn create_modal_fixed_path(modal: Option<&CreateModal>) -> Option<String> {
    match modal {
        Some(CreateModal::Picker(picker)) => picker.fixed_path.clone(),
        Some(CreateModal::Form(form)) => form.fixed_path.clone(),
        None => None,
    }
}

fn create_form_for_item(
    item_index: usize,
    item: &CreateDefinition,
    selected_path: Option<&str>,
    fixed_path: Option<String>,
) -> CreateForm {
    let mut values = Vec::new();
    let inputs = item
        .prompts
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
            Input::new(value)
        })
        .collect::<Vec<Input>>();
    let path_suggestions = item
        .prompts
        .first()
        .filter(|prompt| prompt.kind == CreatePromptKind::Path)
        .and_then(|_| inputs.first())
        .map(|input| path_suggestions_for_input(input.value()))
        .unwrap_or_default();
    CreateForm {
        item_index,
        active_prompt: 0,
        focus: CreateFormFocus::Fields,
        fixed_path,
        inputs,
        path_suggestions,
        selected_suggestion: 0,
        error: None,
    }
}

fn create_form_current_prompt<'a>(
    form: &CreateForm,
    item: &'a CreateDefinition,
) -> Option<&'a model::CreatePrompt> {
    item.prompts.get(form.active_prompt)
}

fn update_create_path_suggestions(form: &mut CreateForm, item: &CreateDefinition) {
    if create_form_current_prompt(form, item)
        .is_some_and(|prompt| prompt.kind == CreatePromptKind::Path)
    {
        let value = form
            .inputs
            .get(form.active_prompt)
            .map(Input::value)
            .unwrap_or_default();
        form.path_suggestions = path_suggestions_for_input(value);
        form.selected_suggestion = form
            .selected_suggestion
            .min(form.path_suggestions.len().saturating_sub(1));
        if form.path_suggestions.is_empty() {
            form.focus = CreateFormFocus::Fields;
        }
    } else {
        form.path_suggestions.clear();
        form.selected_suggestion = 0;
        form.focus = CreateFormFocus::Fields;
    }
}

fn accept_create_path_suggestion(form: &mut CreateForm) {
    let Some(suggestion) = form.path_suggestions.get(form.selected_suggestion) else {
        return;
    };
    if let Some(input) = form.inputs.get_mut(form.active_prompt) {
        *input = Input::new(suggestion.value.clone());
    }
}

fn move_create_form_prompt(form: &mut CreateForm, item: &CreateDefinition, next: bool) {
    if item.prompts.is_empty() {
        form.active_prompt = 0;
    } else if next {
        form.active_prompt = next_picker_index(form.active_prompt, item.prompts.len());
    } else {
        form.active_prompt = previous_picker_index(form.active_prompt, item.prompts.len());
    }
    form.error = None;
    form.focus = CreateFormFocus::Fields;
    update_create_path_suggestions(form, item);
}

fn update_dependent_create_defaults(
    form: &mut CreateForm,
    item: &CreateDefinition,
    selected_path: Option<&str>,
) {
    let mut values = Vec::new();
    for (index, prompt) in item.prompts.iter().enumerate() {
        let current = form
            .inputs
            .get(index)
            .map(Input::value)
            .unwrap_or_default()
            .to_string();
        if index > form.active_prompt && current.trim().is_empty() {
            if let Some(default) = prompt.default.as_deref() {
                let value = expand_create_value(default, selected_path, &values);
                if let Some(input) = form.inputs.get_mut(index) {
                    *input = Input::new(value.clone());
                }
                values.push((
                    prompt.name.clone(),
                    normalized_prompt_value(prompt.kind, &value),
                ));
                continue;
            }
        }
        values.push((
            prompt.name.clone(),
            normalized_prompt_value(prompt.kind, &current),
        ));
    }
    update_create_path_suggestions(form, item);
}

fn collect_create_form_values(
    form: &mut CreateForm,
    item: &CreateDefinition,
) -> Option<Vec<(String, String)>> {
    let mut values = Vec::new();
    for (index, prompt) in item.prompts.iter().enumerate() {
        let raw_value = form
            .inputs
            .get(index)
            .map(Input::value)
            .unwrap_or_default()
            .trim()
            .to_string();
        if prompt.required && raw_value.is_empty() {
            form.active_prompt = index;
            form.error = Some(format!("{} is required", prompt.label));
            update_create_path_suggestions(form, item);
            return None;
        }
        values.push((
            prompt.name.clone(),
            normalized_prompt_value(prompt.kind, &raw_value),
        ));
    }
    form.error = None;
    Some(values)
}

fn normalized_prompt_value(kind: CreatePromptKind, value: &str) -> String {
    if kind == CreatePromptKind::Path {
        expand_home_prefix(value.trim())
    } else {
        value.trim().to_string()
    }
}

fn create_form_input_row(active_prompt: usize) -> usize {
    2 + active_prompt.saturating_mul(2)
}

fn expand_create_value(
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

fn create_env_values(
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

fn run_create_recipe(
    item: CreateDefinition,
    selected_path: Option<String>,
    values: Vec<(String, String)>,
    tx: mpsc::Sender<CreateProgress>,
) {
    thread::spawn(move || {
        let selected_path = selected_path.as_deref();
        let success_path = expand_create_value(&item.success_path, selected_path, &values);
        if success_path.trim().is_empty() {
            let _ = tx.send(CreateProgress::Error(
                "success_path expanded to an empty value".to_string(),
            ));
            return;
        }
        let current_dir = item
            .current_dir
            .as_deref()
            .map(|value| PathBuf::from(expand_create_value(value, selected_path, &values)));
        let envs = create_env_values(selected_path, &values);
        let _ = tx.send(CreateProgress::Message(format!("Running {}", item.label)));
        match commands::run_shell_recipe(&item.shell, current_dir.as_deref(), &envs) {
            Ok(result) => {
                if !result.stdout.is_empty() {
                    let _ = tx.send(CreateProgress::Message(result.stdout));
                }
                let success = PathBuf::from(&success_path);
                if !success.exists() {
                    let _ = tx.send(CreateProgress::Error(format!(
                        "success_path does not exist: {}",
                        success.display()
                    )));
                    return;
                }
                let _ = tx.send(CreateProgress::Select(
                    success.to_string_lossy().to_string(),
                ));
            }
            Err(message) => {
                let _ = tx.send(CreateProgress::Error(message));
            }
        }
    });
}

fn path_suggestions_for_input(input: &str) -> Vec<PathSuggestion> {
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

fn expand_home_prefix(value: &str) -> String {
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

fn display_path_for_input(value: &str) -> String {
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

fn action_path_entry(path: &Path) -> NavigateEntry {
    let path_string = path.to_string_lossy().to_string();
    let display = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or(&path_string)
        .to_string();
    NavigateEntry {
        id: format!("action-path:{path_string}"),
        display,
        context: None,
        preview_root_path: path_string.clone(),
        preferred_preview_path: None,
        selection_path: path_string.clone(),
        metadata_path: path_string.clone(),
        search_text: vec![path_string],
        kind: NavigateEntryKind::Project,
    }
}

fn select_from_list(args: SelectListArgs) -> AppResult<Option<NavigateOutcome>> {
    let SelectListArgs {
        mut entries,
        preview_settings,
        sort_settings,
        remote_settings,
        branch_settings,
        action_settings,
        create_settings,
        theme_colors,
        initial_launch,
    } = args;
    let initial_launch = match initial_launch {
        InitialLaunch::ActionsPath(path) if entries.is_empty() => {
            entries.push(action_path_entry(&path));
            InitialLaunch::ActionsPath(path)
        }
        InitialLaunch::CreatePicker { path } if entries.is_empty() => {
            entries.push(action_path_entry(&path));
            InitialLaunch::CreatePicker { path }
        }
        InitialLaunch::CreateRecipe { selector, path } if entries.is_empty() => {
            entries.push(action_path_entry(&path));
            InitialLaunch::CreateRecipe { selector, path }
        }
        other => other,
    };
    if entries.is_empty() {
        return Ok(None);
    }

    let (mut terminal, _guard) = setup_terminal()?;
    let mut input = Input::default();
    let mut selected = 0usize;
    let mut stick_to_first_result = true;
    let mut sort_mode = sort_settings.default_mode;
    let mut focus = Focus::Search;
    let mut meta_cache: HashMap<String, SortMeta> = HashMap::new();
    let mut list_offset = 0usize;
    let accent = theme_colors.accent;
    let warm = theme_colors.warm;
    let key_color = theme_colors.key_color;
    let text = theme_colors.text;
    let muted = theme_colors.muted;
    let action_binding_label = first_action_binding_label(&action_settings.picker_bindings);
    let create_binding_label = first_create_binding_label(&create_settings.picker_bindings);
    let (preview_tx, preview_rx) = mpsc::channel::<PreviewResult>();
    let (git_tx, git_rx) = mpsc::channel::<GitResult>();
    let (github_tx, github_rx) = mpsc::channel::<GithubReadmeResult>();
    let (date_tx, date_rx) = mpsc::channel::<MetaResult>();
    let (tag_tx, tag_rx) = mpsc::channel::<TagResult>();
    let (result_tx, result_rx) = mpsc::channel::<ResultUpdate>();
    let (worktree_tx, worktree_rx) = mpsc::channel::<WorktreeProgress>();
    let (create_tx, create_rx) = mpsc::channel::<CreateProgress>();
    let mut preview_cache: HashMap<String, PreviewData> = HashMap::new();
    let mut git_in_flight: HashSet<String> = HashSet::new();
    let mut github_in_flight: HashSet<String> = HashSet::new();
    let mut date_cache: HashMap<String, String> = HashMap::new();
    let mut date_in_flight: HashSet<String> = HashSet::new();
    let mut tag_cache: HashMap<String, Vec<String>> = HashMap::new();
    let mut tag_in_flight: HashSet<String> = HashSet::new();
    let mut tag_scan_started = false;
    let mut show_remote_branches = remote_settings.enabled_by_default;
    let mut remote_fetching =
        remote_settings.enabled_by_default && remote_settings.refresh_on_toggle;
    let mut remote_error = false;
    let mut remote_status: Option<String> = None;
    let current_project_path = env::current_dir()
        .ok()
        .and_then(|path| git::git_worktree_root_for_path(&path));
    let mut filtered = filter_and_sort_visible(
        &entries,
        input.value(),
        sort_mode,
        &meta_cache,
        &tag_cache,
        show_remote_branches,
        pinned_path(
            sort_settings.pin_current_project,
            current_project_path.as_deref(),
        ),
    );
    let mut preview_path: Option<String> = None;
    let mut preview_entry_id: Option<String> = None;
    let mut in_flight: Option<String> = None;
    let mut compositor = CurrentCompositor::new(build_placeholder_text(
        None,
        accent,
        muted,
        text,
        "No selection",
    ));
    let start_time = Instant::now();
    let mut tag_edit_path: Option<String> = None;
    let mut tag_edit_tags: Vec<String> = Vec::new();
    let mut tag_input = Input::default();
    let mut tag_suggestions: Vec<String> = Vec::new();
    let mut worktree_overlay: Option<WorktreeOverlay> = None;
    let mut create_overlay: Option<WorktreeOverlay> = None;
    let mut create_modal: Option<CreateModal> = initial_create_modal(
        &initial_launch,
        &create_settings.items,
        current_project_path.as_deref(),
    );
    let mut action_picker: Option<ActionPicker> = match &initial_launch {
        InitialLaunch::ActionsFirstResult => Some(ActionPicker {
            selected: 0,
            query: String::new(),
            fixed_path: None,
        }),
        InitialLaunch::ActionsPath(path) => Some(ActionPicker {
            selected: 0,
            query: String::new(),
            fixed_path: Some(path.to_string_lossy().to_string()),
        }),
        InitialLaunch::Navigate
        | InitialLaunch::CreatePicker { .. }
        | InitialLaunch::CreateRecipe { .. } => None,
    };
    spawn_worktree_result_provider(&entries, result_tx.clone());
    if show_remote_branches {
        if let Some(entry) = filtered
            .first()
            .and_then(|index| entries.get(*index))
            .cloned()
        {
            remote_status = Some("loading local remotes...".to_string());
            spawn_remote_branch_result_provider(entry, result_tx.clone(), remote_settings);
        }
    }
    if sort_mode.uses_time() {
        let paths = all_metadata_paths(&entries);
        spawn_bulk_metadata_fetch(&paths, &date_cache, &mut date_in_flight, &date_tx);
    }

    loop {
        let current_entry = current_selection_entry(&entries, &filtered, selected).cloned();
        let current = current_entry
            .as_ref()
            .map(|entry| entry.preview_root_path.clone());
        let query_value = input.value();
        let tokens = parse_query_tokens(query_value);

        while let Ok(progress) = worktree_rx.try_recv() {
            match progress {
                WorktreeProgress::Message(message) => {
                    if let Some(overlay) = worktree_overlay.as_mut() {
                        overlay.push_message(message);
                    }
                }
                WorktreeProgress::Select(path) => {
                    terminal.show_cursor()?;
                    return Ok(Some(NavigateOutcome::Navigate {
                        path,
                        close_session: false,
                    }));
                }
                WorktreeProgress::Deleted(path) => {
                    entries.retain(|entry| entry.selection_path != path);
                    filtered = filter_and_sort_visible(
                        &entries,
                        input.value(),
                        sort_mode,
                        &meta_cache,
                        &tag_cache,
                        show_remote_branches,
                        pinned_path(
                            sort_settings.pin_current_project,
                            current_project_path.as_deref(),
                        ),
                    );
                    selected = adjust_selected_index(selected, filtered.len());
                    preview_cache.remove(&path);
                    worktree_overlay = None;
                }
                WorktreeProgress::Error(message) => {
                    if let Some(overlay) = worktree_overlay.as_mut() {
                        overlay.fail(message);
                    }
                }
            }
        }

        while let Ok(progress) = create_rx.try_recv() {
            match progress {
                CreateProgress::Message(message) => {
                    if let Some(overlay) = create_overlay.as_mut() {
                        overlay.push_message(message);
                    }
                }
                CreateProgress::Select(path) => {
                    terminal.show_cursor()?;
                    return Ok(Some(NavigateOutcome::Navigate {
                        path,
                        close_session: false,
                    }));
                }
                CreateProgress::Error(message) => {
                    if let Some(overlay) = create_overlay.as_mut() {
                        overlay.fail(message);
                    }
                }
            }
        }

        let mut entries_changed = false;
        while let Ok(update) = result_rx.try_recv() {
            match update {
                ResultUpdate::Entries { entries: incoming } => {
                    for entry in incoming {
                        if entries.iter().any(|candidate| candidate.id == entry.id) {
                            continue;
                        }
                        entries.push(entry);
                        entries_changed = true;
                    }
                }
                ResultUpdate::ReplaceProviderEntries {
                    provider_prefix,
                    entries: replacement,
                } => {
                    entries.retain(|entry| !entry.id.starts_with(&provider_prefix));
                    entries.extend(replacement);
                    entries_changed = true;
                }
                ResultUpdate::Status {
                    provider_id,
                    message,
                } => {
                    if provider_id == REMOTE_BRANCH_PROVIDER_PREFIX {
                        remote_fetching = message.starts_with("refreshing ");
                        remote_error = !remote_fetching && message != "remote branches loaded";
                        remote_status = Some(message);
                    }
                }
            }
        }

        if entries_changed {
            let selected_id = current_entry.as_ref().map(|entry| entry.id.clone());
            filtered = filter_and_sort_visible(
                &entries,
                input.value(),
                sort_mode,
                &meta_cache,
                &tag_cache,
                show_remote_branches,
                pinned_path(
                    sort_settings.pin_current_project,
                    current_project_path.as_deref(),
                ),
            );
            selected = selected_after_refilter(
                &entries,
                &filtered,
                selected,
                selected_id.as_deref(),
                stick_to_first_result,
            );
        }

        while let Ok(result) = preview_rx.try_recv() {
            preview_cache.insert(result.path.clone(), result.data.clone());
            if let Some(data) = preview_cache.get(&result.path) {
                ensure_git_for_preview(
                    &result.path,
                    data,
                    &mut git_in_flight,
                    &git_tx,
                    compositor.active_content_index(),
                    PreviewColors {
                        accent,
                        muted,
                        text,
                    },
                );
                ensure_github_readme_for_preview(
                    &result.path,
                    data,
                    &mut github_in_flight,
                    &github_tx,
                    compositor.active_content_index(),
                    PreviewColors {
                        accent,
                        muted,
                        text,
                    },
                );
            }
            if current.as_deref() == Some(result.path.as_str()) {
                apply_preview_for_entry(&mut compositor, &result.data, current_entry.as_ref());
                preview_path = Some(result.path.clone());
                preview_entry_id = current_entry.as_ref().map(|entry| entry.id.clone());
            }
            if in_flight.as_deref() == Some(result.path.as_str()) {
                in_flight = None;
            }
        }

        while let Ok(result) = git_rx.try_recv() {
            if result.done {
                git_in_flight.remove(&result.path);
            }
            let mut updated = false;
            if let Some(data) = preview_cache.get_mut(&result.path) {
                apply_git_result(data, result.tab_index, result.git, result.done);
                updated = true;
            }
            if updated && current.as_deref() == Some(result.path.as_str()) {
                if let Some(data) = preview_cache.get(&result.path) {
                    apply_preview_for_entry(&mut compositor, data, current_entry.as_ref());
                }
            }
        }

        while let Ok(result) = github_rx.try_recv() {
            if result.done {
                github_in_flight.remove(&result.path);
            }
            let mut updated = false;
            if let Some(data) = preview_cache.get_mut(&result.path) {
                apply_github_readme_result(data, result.tab_index, result.readme, result.done);
                updated = true;
            }
            if updated && current.as_deref() == Some(result.path.as_str()) {
                if let Some(data) = preview_cache.get(&result.path) {
                    apply_preview_for_entry(&mut compositor, data, current_entry.as_ref());
                }
            }
        }

        let mut resort_needed = false;
        while let Ok(result) = date_rx.try_recv() {
            let display = result
                .display
                .unwrap_or_else(|| DATE_PLACEHOLDER.to_string());
            date_cache.insert(result.path.clone(), display);
            meta_cache.insert(
                result.path.clone(),
                SortMeta {
                    modified_epoch: result.modified_epoch,
                    created_epoch: result.created_epoch,
                },
            );
            date_in_flight.remove(&result.path);
            if sort_mode.uses_time() {
                resort_needed = true;
            }
        }

        let mut tags_changed = false;
        while let Ok(result) = tag_rx.try_recv() {
            tag_cache.insert(result.path.clone(), result.tags);
            tag_in_flight.remove(&result.path);
            tags_changed = true;
        }

        let query_uses_tags = tokens.needs_tags();
        if query_uses_tags && !tag_scan_started {
            let paths = all_metadata_paths(&entries);
            spawn_bulk_tag_fetch(&paths, &tag_cache, &mut tag_in_flight, &tag_tx);
            tag_scan_started = true;
        }

        if resort_needed {
            let selected_id = current_selection_entry(&entries, &filtered, selected)
                .map(|entry| entry.id.clone());
            filtered = filter_and_sort_visible(
                &entries,
                input.value(),
                sort_mode,
                &meta_cache,
                &tag_cache,
                show_remote_branches,
                pinned_path(
                    sort_settings.pin_current_project,
                    current_project_path.as_deref(),
                ),
            );
            selected = selected_after_refilter(
                &entries,
                &filtered,
                selected,
                selected_id.as_deref(),
                stick_to_first_result,
            );
        }

        if tags_changed && query_uses_tags {
            let selected_id = current_selection_entry(&entries, &filtered, selected)
                .map(|entry| entry.id.clone());
            filtered = filter_and_sort_visible(
                &entries,
                input.value(),
                sort_mode,
                &meta_cache,
                &tag_cache,
                show_remote_branches,
                pinned_path(
                    sort_settings.pin_current_project,
                    current_project_path.as_deref(),
                ),
            );
            selected = selected_after_refilter(
                &entries,
                &filtered,
                selected,
                selected_id.as_deref(),
                stick_to_first_result,
            );
        }

        match current.as_deref() {
            None => {
                if preview_path.is_some() || in_flight.is_some() {
                    let placeholder =
                        build_placeholder_text(None, accent, muted, text, "No selection");
                    compositor.reset_for_no_selection(placeholder);
                    preview_path = None;
                    preview_entry_id = None;
                    in_flight = None;
                }
            }
            Some(path) => {
                let current_entry_id = current_entry.as_ref().map(|entry| entry.id.as_str());
                if preview_path.as_deref() != Some(path)
                    || preview_entry_id.as_deref() != current_entry_id
                {
                    compositor.reset_for_new_selection();
                    if let Some(data) = preview_cache.get(path) {
                        apply_preview_for_entry(&mut compositor, data, current_entry.as_ref());
                        preview_path = Some(path.to_string());
                        preview_entry_id = current_entry.as_ref().map(|entry| entry.id.clone());
                        ensure_git_for_preview(
                            path,
                            data,
                            &mut git_in_flight,
                            &git_tx,
                            compositor.active_content_index(),
                            PreviewColors {
                                accent,
                                muted,
                                text,
                            },
                        );
                        ensure_github_readme_for_preview(
                            path,
                            data,
                            &mut github_in_flight,
                            &github_tx,
                            compositor.active_content_index(),
                            PreviewColors {
                                accent,
                                muted,
                                text,
                            },
                        );
                    } else if in_flight.as_deref() != Some(path) {
                        let placeholder = build_placeholder_text(
                            preview_placeholder_path(current_entry.as_ref()),
                            accent,
                            muted,
                            text,
                            "Loading preview...",
                        );
                        compositor.set_loading(placeholder);
                        preview_path = Some(path.to_string());
                        preview_entry_id = current_entry.as_ref().map(|entry| entry.id.clone());
                        in_flight = Some(path.to_string());
                        let tx = preview_tx.clone();
                        let path_owned = path.to_string();
                        let entry_owned = current_entry.clone();
                        thread::spawn(move || {
                            let data = build_preview_data_for_entry(
                                &path_owned,
                                entry_owned.as_ref(),
                                accent,
                                muted,
                                text,
                                preview_settings,
                            );
                            let _ = tx.send(PreviewResult {
                                path: path_owned,
                                data,
                            });
                        });
                    }
                }
            }
        }

        focus = compositor.ensure_valid_focus(focus);
        if focus == Focus::TagEdit && tag_edit_path.is_none() {
            focus = Focus::Preview;
        }

        let show_detail = compositor.show_detail();
        let size = terminal.size()?;
        let ui = compute_ui_layout(size.into(), show_detail);

        terminal.draw(|frame| {
            let list_area = ui.list_area;
            let detail_area = ui.detail_area;

            let remote_state =
                remote_toggle_state(show_remote_branches, remote_fetching, remote_error);
            let can_delete_worktree = current_entry
                .as_ref()
                .is_some_and(|entry| matches!(entry.kind, NavigateEntryKind::Worktree { .. }));
            let list_title = if show_remote_branches {
                match remote_status.as_deref() {
                    Some(status) => format!(
                        "Results {}/{} + remote {status}",
                        filtered.len(),
                        visible_entry_count(&entries, true)
                    ),
                    None => format!(
                        "Results {}/{} + remote",
                        filtered.len(),
                        visible_entry_count(&entries, true)
                    ),
                }
            } else {
                format!(
                    "Results {}/{}",
                    filtered.len(),
                    visible_entry_count(&entries, false)
                )
            };
            let left_title = if focus == Focus::Search {
                format!("* {}", list_title)
            } else {
                list_title
            };
            let left_border_style = if focus == Focus::Search {
                Style::default().fg(accent)
            } else {
                Style::default().fg(muted)
            };
            let left_block = Block::default()
                .borders(Borders::ALL)
                .title(left_title)
                .border_style(left_border_style)
                .border_type(BorderType::Rounded);
            frame.render_widget(left_block, list_area);

            let search_area = ui.search_area;
            let results_area = ui.results_area;

            let search_width = search_area.width.saturating_sub(1) as usize;
            let scroll = if search_width > 0 {
                input.visual_scroll(search_width)
            } else {
                0
            };
            let search = Paragraph::new(input.value())
                .scroll((0, scroll as u16))
                .alignment(Alignment::Left)
                .wrap(Wrap { trim: false });
            frame.render_widget(search, search_area);
            if focus == Focus::Search && search_area.width > 0 && search_area.height > 0 {
                let cursor_x = input.visual_cursor().max(scroll).saturating_sub(scroll);
                frame.set_cursor_position((search_area.x + cursor_x as u16, search_area.y));
            }

            let list_inner_height = results_area.height as usize;
            let total = filtered.len();
            list_offset =
                compute_list_window_offset(selected, list_offset, list_inner_height, total);

            let scrollbar_space = if total > 0 { 1 } else { 0 };
            let list_inner_width = results_area.width.saturating_sub(scrollbar_space) as usize;
            let visible_paths =
                visible_paths_for_window(&entries, &filtered, list_offset, list_inner_height);
            ensure_dates_for_paths(&visible_paths, &date_cache, &mut date_in_flight, &date_tx);
            ensure_tags_for_paths(&visible_paths, &tag_cache, &mut tag_in_flight, &tag_tx);

            let (list_items, list_selected) = build_visible_list_items(VisibleListArgs {
                entries: &entries,
                filtered: &filtered,
                selected,
                offset: list_offset,
                height: list_inner_height,
                text,
                accent,
                muted,
                dates: &date_cache,
                tags: &tag_cache,
                inner_width: list_inner_width,
                tokens: &tokens,
                elapsed_ms: start_time.elapsed().as_millis() as u64,
            });

            let list = List::new(list_items).highlight_style(
                Style::default()
                    .fg(Color::Black)
                    .bg(warm)
                    .add_modifier(Modifier::BOLD),
            );

            let mut state = ListState::default();
            state.select(list_selected);
            frame.render_stateful_widget(list, results_area, &mut state);

            let preview_body_area =
                preview_content_area(ui.preview_area, compositor.preview_tab_count);
            let preview_height = preview_body_area.height as usize;
            let detail_height = ui
                .detail_panel_area
                .map(|rect| {
                    let tab_row = if compositor.detail_tabs.len() > 1 {
                        1
                    } else {
                        0
                    };
                    rect.height.saturating_sub(2).saturating_sub(tab_row) as usize
                })
                .unwrap_or(0);
            compositor.preview_page_step = preview_height.max(1);
            compositor.detail_page_step = detail_height.max(1);
            let preview_title = compositor.preview_title(current.as_deref());
            let preview_tags = if focus == Focus::TagEdit {
                tag_edit_tags.clone()
            } else {
                current
                    .as_deref()
                    .and_then(|path| tag_cache.get(path))
                    .cloned()
                    .unwrap_or_default()
            };
            let preview_width = preview_body_area.width as usize;
            let (preview_combined, tag_cursor) = if focus == Focus::TagEdit {
                compose_preview_text_with_input(
                    &compositor.preview_text,
                    &preview_tags,
                    &tag_input,
                    preview_width,
                    text,
                )
            } else {
                (
                    compose_preview_text(
                        &compositor.preview_text,
                        &preview_tags,
                        preview_width,
                        text,
                    ),
                    None,
                )
            };
            compositor.preview_max_scroll =
                text_line_count(&preview_combined).saturating_sub(preview_height);
            let active_detail = compositor.detail_tabs.get(compositor.detail_tab_index);
            compositor.detail_max_scroll = active_detail
                .map(|tab| text_line_count(&tab.text).saturating_sub(detail_height))
                .unwrap_or(0);
            if focus == Focus::TagEdit {
                if let Some((row, _)) = tag_cursor {
                    if row < compositor.preview_scroll {
                        compositor.preview_scroll = row;
                    } else if row >= compositor.preview_scroll + preview_height {
                        compositor.preview_scroll =
                            row.saturating_sub(preview_height.saturating_sub(1));
                    }
                }
            }
            compositor.preview_scroll =
                compositor.preview_scroll.min(compositor.preview_max_scroll);
            compositor.detail_scroll = compositor.detail_scroll.min(compositor.detail_max_scroll);
            render_side_panels(
                frame,
                SidePanelRender {
                    area: detail_area,
                    preview: &preview_combined,
                    detail_tabs: &compositor.detail_tabs,
                    detail_tab_index: compositor.detail_tab_index,
                    preview_title: &preview_title,
                    preview_tab_labels: &compositor.preview_tab_labels,
                    preview_tab_index: compositor.preview_tab_visible_index,
                    preview_settings,
                    focus,
                    accent,
                    text,
                    preview_scroll: compositor.preview_scroll as u16,
                    detail_scroll: compositor.detail_scroll as u16,
                },
            );
            if focus == Focus::TagEdit {
                if let Some((row, col)) = tag_cursor {
                    let visible_row = row.saturating_sub(compositor.preview_scroll);
                    if visible_row < preview_height {
                        let x = preview_body_area.x + col as u16;
                        let y = preview_body_area.y + visible_row as u16;
                        frame.set_cursor_position((x, y));
                    }
                }
            }

            let help_line = build_help_line(
                HelpContext {
                    focus,
                    sort_mode,
                    remote_state,
                    can_delete_worktree,
                    show_detail,
                    cursor_at_end: input_at_end(&input),
                    has_tag_input: !tag_input.value().trim().is_empty(),
                    preview_tab_index: compositor.preview_tab_visible_index,
                    preview_tab_count: compositor.preview_tab_count,
                    preview_scroll: compositor.preview_scroll,
                    preview_max_scroll: compositor.preview_max_scroll,
                    detail_tab_index: compositor.detail_tab_index,
                    detail_tab_count: compositor.detail_tabs.len(),
                    detail_scroll: compositor.detail_scroll,
                    action_binding_label: action_binding_label.clone(),
                    create_binding_label: create_binding_label.clone(),
                },
                HelpColors {
                    text,
                    accent,
                    key_color,
                    remote_color: warm,
                },
            );
            let help = Paragraph::new(Text::from(help_line))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Keys")
                        .border_style(Style::default().fg(muted))
                        .border_type(BorderType::Rounded),
                )
                .alignment(Alignment::Left)
                .wrap(Wrap { trim: true });
            frame.render_widget(help, ui.help_area);

            if let Some(overlay) = worktree_overlay.as_ref() {
                render_worktree_overlay(frame, size.into(), overlay, accent, warm, text, muted);
            }
            if let Some(overlay) = create_overlay.as_ref() {
                render_worktree_overlay(frame, size.into(), overlay, accent, warm, text, muted);
            }
            if let Some(picker) = action_picker.as_ref() {
                let path = picker.fixed_path.clone().or_else(|| {
                    selection_path_for_action(
                        focus,
                        current.as_deref(),
                        compositor.active_content_index(),
                        &preview_cache,
                        current_selection_entry(&entries, &filtered, selected),
                    )
                });
                let visible_actions =
                    filtered_action_indexes(&action_settings.items, &picker.query, path.as_deref());
                render_action_picker(
                    frame,
                    size.into(),
                    ActionPickerRender {
                        actions: &action_settings.items,
                        visible_actions: &visible_actions,
                        selected: picker.selected,
                        query: &picker.query,
                        action_binding_label: &action_binding_label,
                        colors: ActionPickerColors {
                            accent,
                            warm,
                            text,
                            muted,
                        },
                    },
                );
            }
            if let Some(modal) = create_modal.as_ref() {
                match modal {
                    CreateModal::Picker(picker) => {
                        let visible_items =
                            filtered_create_indexes(&create_settings.items, &picker.query);
                        render_create_picker(
                            frame,
                            size.into(),
                            CreatePickerRender {
                                items: &create_settings.items,
                                visible_items: &visible_items,
                                selected: picker.selected,
                                query: &picker.query,
                                create_binding_label: &create_binding_label,
                                colors: ActionPickerColors {
                                    accent,
                                    warm,
                                    text,
                                    muted,
                                },
                            },
                        );
                    }
                    CreateModal::Form(form) => {
                        if let Some(item) = create_settings.items.get(form.item_index) {
                            render_create_form(
                                frame,
                                size.into(),
                                CreateFormRender {
                                    item,
                                    form,
                                    colors: ActionPickerColors {
                                        accent,
                                        warm,
                                        text,
                                        muted,
                                    },
                                },
                            );
                        }
                    }
                }
            }
        })?;

        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => {
                    if create_modal.is_some() {
                        let selected_path =
                            create_modal_fixed_path(create_modal.as_ref()).or_else(|| {
                                selection_path_for_action(
                                    focus,
                                    current.as_deref(),
                                    compositor.active_content_index(),
                                    &preview_cache,
                                    current_selection_entry(&entries, &filtered, selected),
                                )
                            });
                        let mut close_modal = false;
                        let mut start_create: Option<(CreateDefinition, Vec<(String, String)>)> =
                            None;
                        if let Some(modal) = create_modal.as_mut() {
                            match modal {
                                CreateModal::Picker(picker) => {
                                    if key.code == KeyCode::Esc
                                        || (key.code == KeyCode::Char('c')
                                            && key.modifiers.contains(KeyModifiers::CONTROL))
                                    {
                                        close_modal = true;
                                    } else {
                                        let visible_items = filtered_create_indexes(
                                            &create_settings.items,
                                            &picker.query,
                                        );
                                        match key.code {
                                            KeyCode::Enter => {
                                                if let Some(item_index) =
                                                    visible_items.get(picker.selected).copied()
                                                {
                                                    if let Some(item) =
                                                        create_settings.items.get(item_index)
                                                    {
                                                        if item.prompts.is_empty() {
                                                            start_create =
                                                                Some((item.clone(), Vec::new()));
                                                        } else {
                                                            *modal = CreateModal::Form(
                                                                create_form_for_item(
                                                                    item_index,
                                                                    item,
                                                                    selected_path.as_deref(),
                                                                    picker.fixed_path.clone(),
                                                                ),
                                                            );
                                                        }
                                                    }
                                                }
                                            }
                                            KeyCode::Up | KeyCode::Char('k') => {
                                                picker.selected = previous_picker_index(
                                                    picker.selected,
                                                    visible_items.len(),
                                                );
                                            }
                                            KeyCode::Down | KeyCode::Char('j') => {
                                                picker.selected = next_picker_index(
                                                    picker.selected,
                                                    visible_items.len(),
                                                );
                                            }
                                            KeyCode::Backspace => {
                                                picker.query.pop();
                                                let visible_items = filtered_create_indexes(
                                                    &create_settings.items,
                                                    &picker.query,
                                                );
                                                clamp_create_picker_selection(
                                                    picker,
                                                    visible_items.len(),
                                                );
                                            }
                                            KeyCode::Char(value)
                                                if key.modifiers.is_empty()
                                                    || key.modifiers == KeyModifiers::SHIFT =>
                                            {
                                                picker.query.push(value);
                                                let visible_items = filtered_create_indexes(
                                                    &create_settings.items,
                                                    &picker.query,
                                                );
                                                clamp_create_picker_selection(
                                                    picker,
                                                    visible_items.len(),
                                                );
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                                CreateModal::Form(form) => {
                                    if let Some(item) = create_settings.items.get(form.item_index) {
                                        if key.code == KeyCode::Esc {
                                            *modal = CreateModal::Picker(CreatePicker {
                                                selected: form.item_index,
                                                query: String::new(),
                                                fixed_path: form.fixed_path.clone(),
                                            });
                                        } else if key.code == KeyCode::Char('c')
                                            && key.modifiers.contains(KeyModifiers::CONTROL)
                                        {
                                            close_modal = true;
                                        } else {
                                            match key.code {
                                                KeyCode::Enter => {
                                                    if let Some(values) =
                                                        collect_create_form_values(form, item)
                                                    {
                                                        start_create = Some((item.clone(), values));
                                                    }
                                                }
                                                KeyCode::Tab => {
                                                    accept_create_path_suggestion(form);
                                                    update_create_path_suggestions(form, item);
                                                    form.focus = CreateFormFocus::Fields;
                                                }
                                                KeyCode::Right => {
                                                    if !form.path_suggestions.is_empty() {
                                                        form.focus = CreateFormFocus::Completions;
                                                    }
                                                }
                                                KeyCode::Left => {
                                                    form.focus = CreateFormFocus::Fields;
                                                }
                                                KeyCode::Up => {
                                                    if form.focus == CreateFormFocus::Completions {
                                                        form.selected_suggestion =
                                                            previous_picker_index(
                                                                form.selected_suggestion,
                                                                form.path_suggestions.len(),
                                                            );
                                                    } else {
                                                        move_create_form_prompt(form, item, false);
                                                    }
                                                }
                                                KeyCode::Down => {
                                                    if form.focus == CreateFormFocus::Completions {
                                                        form.selected_suggestion =
                                                            next_picker_index(
                                                                form.selected_suggestion,
                                                                form.path_suggestions.len(),
                                                            );
                                                    } else {
                                                        move_create_form_prompt(form, item, true);
                                                    }
                                                }
                                                KeyCode::Char('k')
                                                    if form.focus
                                                        == CreateFormFocus::Completions =>
                                                {
                                                    form.selected_suggestion =
                                                        previous_picker_index(
                                                            form.selected_suggestion,
                                                            form.path_suggestions.len(),
                                                        );
                                                }
                                                KeyCode::Char('j')
                                                    if form.focus
                                                        == CreateFormFocus::Completions =>
                                                {
                                                    form.selected_suggestion = next_picker_index(
                                                        form.selected_suggestion,
                                                        form.path_suggestions.len(),
                                                    );
                                                }
                                                KeyCode::Char('u')
                                                    if key
                                                        .modifiers
                                                        .contains(KeyModifiers::CONTROL) =>
                                                {
                                                    if let Some(input) =
                                                        form.inputs.get_mut(form.active_prompt)
                                                    {
                                                        input.reset();
                                                    }
                                                    form.error = None;
                                                    update_dependent_create_defaults(
                                                        form,
                                                        item,
                                                        selected_path.as_deref(),
                                                    );
                                                    update_create_path_suggestions(form, item);
                                                }
                                                _ => {
                                                    let Some(input) =
                                                        form.inputs.get_mut(form.active_prompt)
                                                    else {
                                                        continue;
                                                    };
                                                    let before = input.value().to_string();
                                                    let _ = input.handle_event(&Event::Key(key));
                                                    if input.value() != before {
                                                        form.error = None;
                                                        update_dependent_create_defaults(
                                                            form,
                                                            item,
                                                            selected_path.as_deref(),
                                                        );
                                                        update_create_path_suggestions(form, item);
                                                    }
                                                }
                                            }
                                        }
                                    } else {
                                        close_modal = true;
                                    }
                                }
                            }
                        }
                        if close_modal {
                            create_modal = None;
                        }
                        if let Some((item, values)) = start_create {
                            create_overlay = Some(WorktreeOverlay::new(
                                format!("Creating {}", item.label),
                                "Starting create recipe",
                            ));
                            create_modal = None;
                            run_create_recipe(item, selected_path, values, create_tx.clone());
                        }
                        continue;
                    }
                    if action_picker.is_some() {
                        if key.code == KeyCode::Esc
                            || (key.code == KeyCode::Char('c')
                                && key.modifiers.contains(KeyModifiers::CONTROL))
                        {
                            action_picker = None;
                            continue;
                        }
                        let picker_path = action_picker.as_ref().and_then(|picker| {
                            picker.fixed_path.clone().or_else(|| {
                                selection_path_for_action(
                                    focus,
                                    current.as_deref(),
                                    compositor.active_content_index(),
                                    &preview_cache,
                                    current_selection_entry(&entries, &filtered, selected),
                                )
                            })
                        });
                        let visible_actions = action_picker
                            .as_ref()
                            .map(|picker| {
                                filtered_action_indexes(
                                    &action_settings.items,
                                    &picker.query,
                                    picker_path.as_deref(),
                                )
                            })
                            .unwrap_or_default();
                        if key.code == KeyCode::Enter
                            || matches_action_binding(&key, &action_settings.picker_bindings)
                        {
                            let Some(picker) = action_picker.as_ref() else {
                                continue;
                            };
                            let close_session =
                                matches_action_binding(&key, &action_settings.picker_bindings);
                            let action = visible_actions
                                .get(picker.selected)
                                .and_then(|index| action_settings.items.get(*index));
                            let path = picker_path;
                            if let Some(outcome) =
                                resolve_picker_action(action, path.as_deref(), close_session)
                            {
                                terminal.show_cursor()?;
                                return Ok(Some(outcome));
                            }
                            continue;
                        }
                        match key.code {
                            KeyCode::Up | KeyCode::Char('k') => {
                                if let Some(picker) = action_picker.as_mut() {
                                    picker.selected = previous_picker_index(
                                        picker.selected,
                                        visible_actions.len(),
                                    );
                                }
                                continue;
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                if let Some(picker) = action_picker.as_mut() {
                                    picker.selected =
                                        next_picker_index(picker.selected, visible_actions.len());
                                }
                                continue;
                            }
                            KeyCode::Backspace => {
                                if let Some(picker) = action_picker.as_mut() {
                                    picker.query.pop();
                                    let visible_actions = filtered_action_indexes(
                                        &action_settings.items,
                                        &picker.query,
                                        picker_path.as_deref(),
                                    );
                                    clamp_picker_selection(picker, visible_actions.len());
                                }
                                continue;
                            }
                            KeyCode::Char(value)
                                if key.modifiers.is_empty()
                                    || key.modifiers == KeyModifiers::SHIFT =>
                            {
                                if let Some(picker) = action_picker.as_mut() {
                                    picker.query.push(value);
                                    let visible_actions = filtered_action_indexes(
                                        &action_settings.items,
                                        &picker.query,
                                        picker_path.as_deref(),
                                    );
                                    clamp_picker_selection(picker, visible_actions.len());
                                }
                                continue;
                            }
                            _ => continue,
                        }
                    }
                    if key.code == KeyCode::Esc {
                        if worktree_overlay.is_some() {
                            if worktree_overlay
                                .as_ref()
                                .is_some_and(WorktreeOverlay::is_error)
                            {
                                worktree_overlay = None;
                            }
                            continue;
                        }
                        if create_overlay.is_some() {
                            if create_overlay
                                .as_ref()
                                .is_some_and(WorktreeOverlay::is_error)
                            {
                                create_overlay = None;
                            }
                            continue;
                        }
                        terminal.show_cursor()?;
                        return Ok(None);
                    }
                    if key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        terminal.show_cursor()?;
                        return Ok(None);
                    }
                    if worktree_overlay.is_some() {
                        if key.code == KeyCode::Enter
                            && worktree_overlay
                                .as_ref()
                                .is_some_and(WorktreeOverlay::is_error)
                        {
                            worktree_overlay = None;
                        }
                        continue;
                    }
                    if create_overlay.is_some() {
                        if key.code == KeyCode::Enter
                            && create_overlay
                                .as_ref()
                                .is_some_and(WorktreeOverlay::is_error)
                        {
                            create_overlay = None;
                        }
                        continue;
                    }
                    if matches_create_binding(&key, &create_settings.picker_bindings)
                        && focus != Focus::TagEdit
                    {
                        if !create_settings.items.is_empty() {
                            stick_to_first_result = false;
                            create_modal = Some(CreateModal::Picker(CreatePicker {
                                selected: 0,
                                query: String::new(),
                                fixed_path: None,
                            }));
                        }
                        continue;
                    }
                    if matches_action_binding(&key, &action_settings.picker_bindings)
                        && focus != Focus::TagEdit
                    {
                        if !action_settings.items.is_empty() {
                            stick_to_first_result = false;
                            action_picker = Some(ActionPicker {
                                selected: 0,
                                query: String::new(),
                                fixed_path: None,
                            });
                        }
                        continue;
                    }
                    if key.code == KeyCode::Char('y')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        stick_to_first_result = false;
                        if let Some(value) = selection_path_for_action(
                            focus,
                            current.as_deref(),
                            compositor.active_content_index(),
                            &preview_cache,
                            current_selection_entry(&entries, &filtered, selected),
                        ) {
                            let _ = copy_to_clipboard(&value);
                        }
                        continue;
                    }
                    if key.code == KeyCode::Char('d')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                        && focus != Focus::TagEdit
                    {
                        let Some(entry) = current_selection_entry(&entries, &filtered, selected)
                            .filter(|entry| {
                                matches!(entry.kind, NavigateEntryKind::Worktree { .. })
                            })
                            .cloned()
                        else {
                            continue;
                        };
                        stick_to_first_result = false;
                        worktree_overlay = Some(WorktreeOverlay::new(
                            delete_worktree_overlay_title(&entry),
                            "Starting safety checks",
                        ));
                        start_worktree_deletion(entry, worktree_tx.clone());
                        continue;
                    }
                    if key.code == KeyCode::Char('o')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                        && focus != Focus::TagEdit
                    {
                        stick_to_first_result = false;
                        let selected_id = current_selection_entry(&entries, &filtered, selected)
                            .map(|entry| entry.id.clone());
                        show_remote_branches = !show_remote_branches;
                        remote_error = false;
                        if show_remote_branches {
                            if let Some(entry) =
                                current_selection_entry(&entries, &filtered, selected).cloned()
                            {
                                remote_fetching = remote_settings.refresh_on_toggle;
                                remote_status = Some("loading local remotes...".to_string());
                                entries.retain(|entry| {
                                    !entry.id.starts_with(REMOTE_BRANCH_PROVIDER_PREFIX)
                                });
                                spawn_remote_branch_result_provider(
                                    entry,
                                    result_tx.clone(),
                                    remote_settings,
                                );
                            } else {
                                remote_fetching = false;
                                remote_error = true;
                                remote_status = Some("No repo selected".to_string());
                            }
                        } else {
                            remote_fetching = false;
                            remote_status = None;
                        }
                        filtered = filter_and_sort_visible(
                            &entries,
                            input.value(),
                            sort_mode,
                            &meta_cache,
                            &tag_cache,
                            show_remote_branches,
                            pinned_path(
                                sort_settings.pin_current_project,
                                current_project_path.as_deref(),
                            ),
                        );
                        selected = selected_id
                            .as_deref()
                            .and_then(|id| index_for_entry_id(&entries, &filtered, id))
                            .unwrap_or_else(|| adjust_selected_index(selected, filtered.len()));
                        list_offset = 0;
                        continue;
                    }
                    if key.code == KeyCode::Char('t')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                        && focus != Focus::TagEdit
                    {
                        stick_to_first_result = false;
                        if let Some(path) = current_selection_entry(&entries, &filtered, selected)
                            .map(|entry| entry.metadata_path.clone())
                        {
                            tag_edit_path = Some(path.clone());
                            tag_edit_tags = read_tags_for_path(&path);
                            tag_input.reset();
                            tag_suggestions = collect_tag_suggestions(&tag_cache);
                            focus = Focus::TagEdit;
                            compositor.preview_scroll = 0;
                        }
                        continue;
                    }
                    if key.code == KeyCode::Enter && focus != Focus::TagEdit {
                        stick_to_first_result = false;
                        if let Some(entry) = current_selection_entry(&entries, &filtered, selected)
                            .filter(|entry| is_remote_branch_entry(entry))
                            .cloned()
                        {
                            worktree_overlay = Some(WorktreeOverlay::new(
                                branch_overlay_title(&entry, branch_settings.on_select),
                                branch_overlay_first_message(branch_settings.on_select),
                            ));
                            start_remote_branch_selection(
                                entry,
                                branch_settings.on_select,
                                worktree_tx.clone(),
                            );
                            continue;
                        }
                        let value = selection_path_for_action(
                            focus,
                            current.as_deref(),
                            compositor.active_content_index(),
                            &preview_cache,
                            current_selection_entry(&entries, &filtered, selected),
                        );
                        if let Some(value) = value {
                            terminal.show_cursor()?;
                            return Ok(Some(NavigateOutcome::Navigate {
                                path: value,
                                close_session: false,
                            }));
                        }
                    }
                    if key.code == KeyCode::Char('s')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        sort_mode = sort_mode.next();
                        filtered = filter_and_sort_visible(
                            &entries,
                            input.value(),
                            sort_mode,
                            &meta_cache,
                            &tag_cache,
                            show_remote_branches,
                            pinned_path(
                                sort_settings.pin_current_project,
                                current_project_path.as_deref(),
                            ),
                        );
                        selected = 0;
                        stick_to_first_result = true;
                        list_offset = 0;
                        if sort_mode.uses_time() {
                            let paths = all_metadata_paths(&entries);
                            spawn_bulk_metadata_fetch(
                                &paths,
                                &date_cache,
                                &mut date_in_flight,
                                &date_tx,
                            );
                        }
                        if parse_query_tokens(input.value()).needs_tags() && !tag_scan_started {
                            let paths = all_metadata_paths(&entries);
                            spawn_bulk_tag_fetch(&paths, &tag_cache, &mut tag_in_flight, &tag_tx);
                            tag_scan_started = true;
                        }
                        continue;
                    }

                    match focus {
                        Focus::Search => match key.code {
                            KeyCode::Up => {
                                let next = selected.saturating_sub(1);
                                if next != selected {
                                    stick_to_first_result = false;
                                }
                                selected = next;
                            }
                            KeyCode::Down => {
                                if selected + 1 < filtered.len() {
                                    selected += 1;
                                    stick_to_first_result = false;
                                }
                            }
                            KeyCode::Right
                                if !key.modifiers.intersects(
                                    KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER,
                                ) && input_at_end(&input) =>
                            {
                                stick_to_first_result = false;
                                focus = Focus::Preview;
                            }
                            _ => {
                                let before = input.value().to_string();
                                if key.modifiers.contains(KeyModifiers::SUPER) {
                                    if key.code == KeyCode::Left {
                                        input.handle(InputRequest::GoToStart);
                                    } else if key.code == KeyCode::Right {
                                        input.handle(InputRequest::GoToEnd);
                                    }
                                } else if key.code == KeyCode::Char('u')
                                    && key.modifiers.contains(KeyModifiers::CONTROL)
                                {
                                    input.handle(InputRequest::DeleteLine);
                                } else {
                                    let _ = input.handle_event(&Event::Key(key));
                                }
                                if input.value() != before {
                                    filtered = filter_and_sort_visible(
                                        &entries,
                                        input.value(),
                                        sort_mode,
                                        &meta_cache,
                                        &tag_cache,
                                        show_remote_branches,
                                        pinned_path(
                                            sort_settings.pin_current_project,
                                            current_project_path.as_deref(),
                                        ),
                                    );
                                    selected = 0;
                                    stick_to_first_result = true;
                                    list_offset = 0;
                                }
                            }
                        },
                        Focus::TagEdit => match key.code {
                            KeyCode::Enter => {
                                commit_tag_input(
                                    &mut tag_input,
                                    &mut tag_edit_tags,
                                    &tag_suggestions,
                                );
                                if let Some(path) = tag_edit_path.clone() {
                                    save_tags_for_path(&path, &tag_edit_tags)?;
                                    tag_cache.insert(path.clone(), tag_edit_tags.clone());
                                }
                                focus = Focus::Preview;
                                tag_edit_path = None;
                                tag_edit_tags.clear();
                                tag_input.reset();
                                let selected_id =
                                    current_selection_entry(&entries, &filtered, selected)
                                        .map(|entry| entry.id.clone());
                                filtered = filter_and_sort_visible(
                                    &entries,
                                    input.value(),
                                    sort_mode,
                                    &meta_cache,
                                    &tag_cache,
                                    show_remote_branches,
                                    pinned_path(
                                        sort_settings.pin_current_project,
                                        current_project_path.as_deref(),
                                    ),
                                );
                                selected = match selected_id {
                                    Some(value) => {
                                        index_for_entry_id(&entries, &filtered, &value).unwrap_or(0)
                                    }
                                    None => adjust_selected_index(selected, filtered.len()),
                                };
                            }
                            KeyCode::Tab => {
                                commit_tag_input(
                                    &mut tag_input,
                                    &mut tag_edit_tags,
                                    &tag_suggestions,
                                );
                            }
                            KeyCode::Backspace if tag_input.value().is_empty() => {
                                tag_edit_tags.pop();
                            }
                            KeyCode::Backspace => {
                                let _ = tag_input.handle_event(&Event::Key(key));
                            }
                            _ => {
                                let _ = tag_input.handle_event(&Event::Key(key));
                            }
                        },
                        Focus::Preview => {
                            let data = current.as_deref().and_then(|path| preview_cache.get(path));
                            if let Some(change) = compositor.handle_preview_key(key, data) {
                                focus = focus_from_change(change);
                            }
                        }
                        Focus::Detail => {
                            let data = current.as_deref().and_then(|path| preview_cache.get(path));
                            if let Some(change) = compositor.handle_detail_key(key, data) {
                                focus = focus_from_change(change);
                            }
                        }
                    }
                }
                Event::Paste(value) => {
                    if let Some(CreateModal::Form(form)) = create_modal.as_mut() {
                        if let Some(item) = create_settings.items.get(form.item_index) {
                            if let Some(input) = form.inputs.get_mut(form.active_prompt) {
                                insert_paste(input, &value);
                            }
                            form.error = None;
                            let selected_path = form.fixed_path.clone().or_else(|| {
                                selection_path_for_action(
                                    focus,
                                    current.as_deref(),
                                    compositor.active_content_index(),
                                    &preview_cache,
                                    current_selection_entry(&entries, &filtered, selected),
                                )
                            });
                            update_dependent_create_defaults(form, item, selected_path.as_deref());
                            update_create_path_suggestions(form, item);
                        }
                        continue;
                    }
                    match focus {
                        Focus::Search => {
                            insert_paste(&mut input, &value);
                            filtered = filter_and_sort_visible(
                                &entries,
                                input.value(),
                                sort_mode,
                                &meta_cache,
                                &tag_cache,
                                show_remote_branches,
                                pinned_path(
                                    sort_settings.pin_current_project,
                                    current_project_path.as_deref(),
                                ),
                            );
                            selected = 0;
                            stick_to_first_result = true;
                            list_offset = 0;
                        }
                        Focus::TagEdit => {
                            insert_paste(&mut tag_input, &value);
                        }
                        Focus::Preview | Focus::Detail => {}
                    }
                }
                Event::Mouse(mouse) => {
                    let col = mouse.column;
                    let row = mouse.row;
                    match mouse.kind {
                        MouseEventKind::Down(MouseButton::Left) => {
                            if rect_contains(ui.list_area, col, row) {
                                focus = Focus::Search;
                            } else if let Some(detail_panel_area) = ui.detail_panel_area {
                                if rect_contains(detail_panel_area, col, row) {
                                    stick_to_first_result = false;
                                    focus = Focus::Detail;
                                } else if rect_contains(ui.preview_area, col, row) {
                                    stick_to_first_result = false;
                                    focus = Focus::Preview;
                                }
                            } else if rect_contains(ui.preview_area, col, row) {
                                stick_to_first_result = false;
                                focus = Focus::Preview;
                            }
                        }
                        MouseEventKind::ScrollUp => {
                            if rect_contains(ui.preview_area, col, row) {
                                stick_to_first_result = false;
                                compositor.scroll_preview_up();
                            } else if let Some(detail_panel_area) = ui.detail_panel_area {
                                if rect_contains(detail_panel_area, col, row) {
                                    stick_to_first_result = false;
                                    compositor.scroll_detail_up();
                                }
                            } else if rect_contains(ui.results_area, col, row) {
                                let next = selected.saturating_sub(1);
                                if next != selected {
                                    stick_to_first_result = false;
                                }
                                selected = next;
                            }
                        }
                        MouseEventKind::ScrollDown => {
                            if rect_contains(ui.preview_area, col, row) {
                                stick_to_first_result = false;
                                compositor.scroll_preview_down();
                            } else if let Some(detail_panel_area) = ui.detail_panel_area {
                                if rect_contains(detail_panel_area, col, row) {
                                    stick_to_first_result = false;
                                    compositor.scroll_detail_down();
                                }
                            } else if rect_contains(ui.results_area, col, row) {
                                let next = selected
                                    .saturating_add(1)
                                    .min(filtered.len().saturating_sub(1));
                                if next != selected {
                                    stick_to_first_result = false;
                                }
                                selected = next;
                            }
                        }
                        _ => {}
                    }
                }
                Event::Resize(_, _) => {}
                _ => {}
            }
        }
    }
}

fn adjust_selected_index(current: usize, len: usize) -> usize {
    if len == 0 {
        0
    } else if current >= len {
        len - 1
    } else {
        current
    }
}

fn selected_after_refilter(
    entries: &[NavigateEntry],
    filtered: &[usize],
    current: usize,
    selected_id: Option<&str>,
    stick_to_first_result: bool,
) -> usize {
    if stick_to_first_result {
        return adjust_selected_index(0, filtered.len());
    }

    selected_id
        .and_then(|id| index_for_entry_id(entries, filtered, id))
        .unwrap_or_else(|| adjust_selected_index(current, filtered.len()))
}

fn insert_paste(input: &mut Input, value: &str) {
    for ch in value.chars().filter(|ch| *ch != '\r') {
        input.handle(InputRequest::InsertChar(ch));
    }
}

fn filter_and_sort_visible(
    entries: &[NavigateEntry],
    query: &str,
    sort_mode: SortMode,
    meta_cache: &HashMap<String, SortMeta>,
    tag_cache: &HashMap<String, Vec<String>>,
    show_remote_branches: bool,
    pinned_path: Option<&str>,
) -> Vec<usize> {
    let mut indices: Vec<usize> = filter_and_sort(entries, query, sort_mode, meta_cache, tag_cache)
        .into_iter()
        .filter(|index| show_remote_branches || !is_remote_branch_entry(&entries[*index]))
        .collect();
    if query.trim().is_empty() {
        pin_current_project(&mut indices, entries, pinned_path);
    }
    indices
}

fn pin_current_project(
    indices: &mut Vec<usize>,
    entries: &[NavigateEntry],
    pinned_path: Option<&str>,
) {
    let Some(pinned_path) = pinned_path else {
        return;
    };
    let Some(position) = indices
        .iter()
        .position(|index| entry_matches_path(&entries[*index], pinned_path))
    else {
        return;
    };
    let index = indices.remove(position);
    indices.insert(0, index);
}

fn pinned_path(pin_current_project: bool, current_project_path: Option<&str>) -> Option<&str> {
    if pin_current_project {
        current_project_path
    } else {
        None
    }
}

fn entry_matches_path(entry: &NavigateEntry, path: &str) -> bool {
    entry.selection_path == path
        || entry.metadata_path == path
        || entry.preview_root_path == path
        || entry.preferred_preview_path.as_deref() == Some(path)
}

fn visible_entry_count(entries: &[NavigateEntry], show_remote_branches: bool) -> usize {
    entries
        .iter()
        .filter(|entry| show_remote_branches || !is_remote_branch_entry(entry))
        .count()
}

fn is_remote_branch_entry(entry: &NavigateEntry) -> bool {
    matches!(entry.kind, NavigateEntryKind::RemoteBranch { .. })
}

fn remote_toggle_state(
    show_remote_branches: bool,
    remote_fetching: bool,
    remote_error: bool,
) -> RemoteToggleState {
    if remote_fetching {
        RemoteToggleState::Fetching
    } else if remote_error {
        RemoteToggleState::Error
    } else if show_remote_branches {
        RemoteToggleState::Active
    } else {
        RemoteToggleState::Off
    }
}

fn start_remote_branch_selection(
    entry: NavigateEntry,
    behavior: BranchSelectBehavior,
    tx: mpsc::Sender<WorktreeProgress>,
) {
    thread::spawn(move || {
        let NavigateEntryKind::RemoteBranch {
            remote_branch,
            bare_path,
            container_path,
            ..
        } = entry.kind
        else {
            let _ = tx.send(WorktreeProgress::Error("not a remote branch".to_string()));
            return;
        };

        let bare = Path::new(&bare_path);
        let result = match behavior {
            BranchSelectBehavior::Worktree => git::add_worktree_for_remote_with_progress(
                bare,
                Path::new(&container_path),
                &remote_branch,
                progress_sender(&tx),
            ),
            BranchSelectBehavior::Checkout => git::checkout_remote_branch_with_progress(
                bare,
                &remote_branch,
                progress_sender(&tx),
            ),
        };
        match result {
            Ok(path) => {
                let _ = tx.send(WorktreeProgress::Select(path));
            }
            Err(message) => {
                let _ = tx.send(WorktreeProgress::Error(message));
            }
        }
    });
}

fn start_worktree_deletion(entry: NavigateEntry, tx: mpsc::Sender<WorktreeProgress>) {
    thread::spawn(move || {
        let path = entry.selection_path.clone();
        let result =
            git::remove_worktree_safely_with_progress(Path::new(&path), progress_sender(&tx));
        match result {
            Ok(path) => {
                let _ = tx.send(WorktreeProgress::Deleted(path));
            }
            Err(message) => {
                let _ = tx.send(WorktreeProgress::Error(message));
            }
        }
    });
}

fn progress_sender(tx: &mpsc::Sender<WorktreeProgress>) -> impl FnMut(String) + '_ {
    move |message| {
        let _ = tx.send(WorktreeProgress::Message(message));
    }
}

fn branch_overlay_title(entry: &NavigateEntry, behavior: BranchSelectBehavior) -> String {
    if let NavigateEntryKind::RemoteBranch { remote_branch, .. } = &entry.kind {
        match behavior {
            BranchSelectBehavior::Worktree => format!("Creating {remote_branch}"),
            BranchSelectBehavior::Checkout => format!("Checking out {remote_branch}"),
        }
    } else {
        match behavior {
            BranchSelectBehavior::Worktree => "Creating worktree".to_string(),
            BranchSelectBehavior::Checkout => "Checking out branch".to_string(),
        }
    }
}

fn branch_overlay_first_message(behavior: BranchSelectBehavior) -> &'static str {
    match behavior {
        BranchSelectBehavior::Worktree => "Starting worktree creation",
        BranchSelectBehavior::Checkout => "Starting branch checkout",
    }
}

fn delete_worktree_overlay_title(entry: &NavigateEntry) -> String {
    if let NavigateEntryKind::Worktree { branch, .. } = &entry.kind {
        format!("Deleting {branch}")
    } else {
        "Deleting worktree".to_string()
    }
}

fn render_worktree_overlay(
    frame: &mut ratatui::Frame,
    area: Rect,
    overlay: &WorktreeOverlay,
    accent: Color,
    warm: Color,
    text: Color,
    muted: Color,
) {
    let width = area.width.saturating_mul(2).saturating_div(3).clamp(48, 96);
    let height = area
        .height
        .saturating_mul(2)
        .saturating_div(5)
        .clamp(11, 18);
    let popup = centered_rect(area, width.min(area.width), height.min(area.height));
    frame.render_widget(Clear, popup);

    let border = if overlay.error.is_some() {
        warm
    } else {
        accent
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(overlay.title.clone())
        .border_style(Style::default().fg(border))
        .border_type(BorderType::Rounded);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let progress_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: 4.min(inner.height),
    };
    let progress = if overlay.error.is_some() {
        1.0
    } else {
        (overlay.messages.len() as f64 / 5.0).clamp(0.08, 0.95)
    };
    let status = if overlay.error.is_some() {
        "Failed"
    } else {
        "Working"
    };
    let current_step = overlay
        .messages
        .last()
        .map(String::as_str)
        .unwrap_or("Starting");
    let progress_text = Text::from(vec![
        Line::from(vec![
            Span::styled(
                status,
                Style::default().fg(border).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(current_step.to_string(), Style::default().fg(text)),
        ]),
        render_progress_bar(progress, progress_area.width as usize, border, muted),
    ]);
    frame.render_widget(
        Paragraph::new(progress_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" {:>3}% ", (progress * 100.0).round() as u8))
                    .border_style(Style::default().fg(border)),
            )
            .wrap(Wrap { trim: false }),
        progress_area,
    );

    let messages_area = Rect {
        x: inner.x,
        y: inner.y.saturating_add(progress_area.height),
        width: inner.width,
        height: inner.height.saturating_sub(progress_area.height),
    };
    let max_messages = messages_area.height.saturating_sub(2) as usize;
    let start = overlay.messages.len().saturating_sub(max_messages);
    let mut lines = overlay.messages[start..].join("\n");
    if let Some(error) = &overlay.error {
        if !lines.is_empty() {
            lines.push('\n');
        }
        lines.push_str(error);
        lines.push_str("\n\nPress Enter or Esc to dismiss");
    }
    let messages = Paragraph::new(lines)
        .style(Style::default().fg(if overlay.error.is_some() { warm } else { text }))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Messages")
                .border_style(Style::default().fg(muted)),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(messages, messages_area);
}

fn render_action_picker(frame: &mut ratatui::Frame, area: Rect, render: ActionPickerRender<'_>) {
    let ActionPickerRender {
        actions,
        visible_actions,
        selected,
        query,
        action_binding_label,
        colors,
    } = render;
    let content_width = actions
        .iter()
        .map(|action| action.label.chars().count() as u16)
        .max()
        .unwrap_or(24)
        .saturating_add(18);
    let width = content_width.clamp(36, 72).min(area.width);
    let height = (visible_actions.len() as u16)
        .saturating_add(5)
        .clamp(8, 18)
        .min(area.height);
    let popup = centered_rect(area, width, height);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .title("Actions")
        .border_style(Style::default().fg(colors.accent))
        .border_type(BorderType::Rounded);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let search_height = 1.min(inner.height);
    let footer_height = 1.min(inner.height.saturating_sub(search_height));
    let list_height = inner.height.saturating_sub(search_height + footer_height);
    let search_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: search_height,
    };
    let list_area = Rect {
        x: inner.x,
        y: inner.y.saturating_add(search_height),
        width: inner.width,
        height: list_height,
    };
    let footer_area = Rect {
        x: inner.x,
        y: inner.y.saturating_add(search_height + list_height),
        width: inner.width,
        height: footer_height,
    };

    let search = if query.is_empty() {
        "Search actions...".to_string()
    } else {
        format!("Search: {query}")
    };
    frame.render_widget(
        Paragraph::new(search).style(Style::default().fg(if query.is_empty() {
            colors.muted
        } else {
            colors.text
        })),
        search_area,
    );

    let selected = selected.min(visible_actions.len().saturating_sub(1));
    let offset = if selected >= list_height as usize {
        selected + 1 - list_height as usize
    } else {
        0
    };
    let items = visible_actions
        .iter()
        .skip(offset)
        .take(list_height as usize)
        .enumerate()
        .filter_map(|(visible_index, action_index)| {
            let action = actions.get(*action_index)?;
            let action_index = offset + visible_index;
            let prefix = if action_index == selected {
                "› "
            } else {
                "  "
            };
            let icon = action.icon.as_deref().unwrap_or(" ");
            Some(ListItem::new(Line::from(vec![
                Span::styled(prefix, Style::default().fg(colors.warm)),
                Span::styled(icon.to_string(), Style::default().fg(colors.warm)),
                Span::raw(" "),
                Span::styled(action.label.clone(), Style::default().fg(colors.text)),
            ])))
        })
        .collect::<Vec<ListItem>>();
    let items = if items.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "  No matching actions",
            Style::default().fg(colors.muted),
        )))]
    } else {
        items
    };
    let list = List::new(items).highlight_style(
        Style::default()
            .fg(Color::Black)
            .bg(colors.warm)
            .add_modifier(Modifier::BOLD),
    );
    let mut state = ListState::default();
    state.select(selected.checked_sub(offset));
    frame.render_stateful_widget(list, list_area, &mut state);

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(
            "Enter",
            Style::default()
                .fg(colors.warm)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" run  ", Style::default().fg(colors.muted)),
        Span::styled(
            action_binding_label.to_string(),
            Style::default()
                .fg(colors.warm)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" run+close  ", Style::default().fg(colors.muted)),
        Span::styled(
            "Esc",
            Style::default()
                .fg(colors.warm)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" close", Style::default().fg(colors.muted)),
    ]));
    frame.render_widget(footer, footer_area);
}

fn render_create_picker(frame: &mut ratatui::Frame, area: Rect, render: CreatePickerRender<'_>) {
    let CreatePickerRender {
        items,
        visible_items,
        selected,
        query,
        create_binding_label,
        colors,
    } = render;
    let content_width = items
        .iter()
        .map(|item| item.label.chars().count() as u16)
        .max()
        .unwrap_or(24)
        .saturating_add(18);
    let width = content_width.clamp(36, 72).min(area.width);
    let height = (visible_items.len() as u16)
        .saturating_add(5)
        .clamp(8, 18)
        .min(area.height);
    let popup = centered_rect(area, width, height);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .title("Create")
        .border_style(Style::default().fg(colors.accent))
        .border_type(BorderType::Rounded);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let search_height = 1.min(inner.height);
    let footer_height = 1.min(inner.height.saturating_sub(search_height));
    let list_height = inner.height.saturating_sub(search_height + footer_height);
    let search_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: search_height,
    };
    let list_area = Rect {
        x: inner.x,
        y: inner.y.saturating_add(search_height),
        width: inner.width,
        height: list_height,
    };
    let footer_area = Rect {
        x: inner.x,
        y: inner.y.saturating_add(search_height + list_height),
        width: inner.width,
        height: footer_height,
    };

    let search = if query.is_empty() {
        "Search create recipes...".to_string()
    } else {
        format!("Search: {query}")
    };
    frame.render_widget(
        Paragraph::new(search).style(Style::default().fg(if query.is_empty() {
            colors.muted
        } else {
            colors.text
        })),
        search_area,
    );

    let selected = selected.min(visible_items.len().saturating_sub(1));
    let offset = if selected >= list_height as usize {
        selected + 1 - list_height as usize
    } else {
        0
    };
    let list_items = visible_items
        .iter()
        .skip(offset)
        .take(list_height as usize)
        .enumerate()
        .filter_map(|(visible_index, item_index)| {
            let item = items.get(*item_index)?;
            let item_index = offset + visible_index;
            let prefix = if item_index == selected { "› " } else { "  " };
            let icon = item.icon.as_deref().unwrap_or(" ");
            Some(ListItem::new(Line::from(vec![
                Span::styled(prefix, Style::default().fg(colors.warm)),
                Span::styled(icon.to_string(), Style::default().fg(colors.warm)),
                Span::raw(" "),
                Span::styled(item.label.clone(), Style::default().fg(colors.text)),
            ])))
        })
        .collect::<Vec<ListItem>>();
    let list_items = if list_items.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "  No matching create recipes",
            Style::default().fg(colors.muted),
        )))]
    } else {
        list_items
    };
    let list = List::new(list_items).highlight_style(
        Style::default()
            .fg(Color::Black)
            .bg(colors.warm)
            .add_modifier(Modifier::BOLD),
    );
    let mut state = ListState::default();
    state.select(selected.checked_sub(offset));
    frame.render_stateful_widget(list, list_area, &mut state);

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(
            "Enter",
            Style::default()
                .fg(colors.warm)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" choose  ", Style::default().fg(colors.muted)),
        Span::styled(
            create_binding_label.to_string(),
            Style::default()
                .fg(colors.warm)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" open  ", Style::default().fg(colors.muted)),
        Span::styled(
            "Esc",
            Style::default()
                .fg(colors.warm)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" close", Style::default().fg(colors.muted)),
    ]));
    frame.render_widget(footer, footer_area);
}

fn render_create_form(frame: &mut ratatui::Frame, area: Rect, render: CreateFormRender<'_>) {
    let CreateFormRender { item, form, colors } = render;
    let active_prompt = item.prompts.get(form.active_prompt);
    let show_suggestions = active_prompt
        .is_some_and(|prompt| prompt.kind == CreatePromptKind::Path)
        && !form.path_suggestions.is_empty();
    let side_by_side = show_suggestions && area.width >= 92;
    let width = if side_by_side { 92 } else { 78 }.min(area.width).max(42);
    let suggestion_rows = if side_by_side {
        0
    } else {
        form.path_suggestions.len().min(6) as u16
    };
    let prompt_rows = item.prompts.len().saturating_mul(2) as u16;
    let height = prompt_rows
        .saturating_add(suggestion_rows)
        .saturating_add(5)
        .min(area.height)
        .max(10);
    let popup = centered_rect(area, width, height);
    frame.render_widget(Clear, popup);

    let title = format!("Create: {}", item.label);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(colors.accent))
        .border_type(BorderType::Rounded);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let suggestion_width = if side_by_side {
        inner
            .width
            .saturating_mul(36)
            .saturating_div(100)
            .clamp(24, 34)
    } else {
        0
    };
    let form_width = inner.width.saturating_sub(if side_by_side {
        suggestion_width + 1
    } else {
        0
    });
    let form_area = Rect {
        x: inner.x,
        y: inner.y,
        width: form_width,
        height: inner.height,
    };
    let suggestion_area = if side_by_side {
        Some(Rect {
            x: inner.x.saturating_add(form_width).saturating_add(1),
            y: inner.y,
            width: suggestion_width,
            height: inner.height,
        })
    } else {
        None
    };

    let mut lines = Vec::new();
    let mut cursor_row: Option<usize> = None;
    if let Some(error) = &form.error {
        lines.push(Line::from(Span::styled(
            error.clone(),
            Style::default().fg(colors.warm),
        )));
    } else {
        lines.push(Line::from(Span::raw("")));
    }
    for (index, prompt) in item.prompts.iter().enumerate() {
        let active = index == form.active_prompt;
        let kind = match prompt.kind {
            CreatePromptKind::Text => "text",
            CreatePromptKind::Path => "path",
        };
        let label_style = if active {
            Style::default()
                .fg(colors.warm)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(colors.text)
        };
        let value_style = if active {
            Style::default()
                .fg(colors.text)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(colors.text)
        };
        let marker = if active { ">" } else { " " };
        lines.push(Line::from(vec![
            Span::styled(format!("{marker} "), Style::default().fg(colors.warm)),
            Span::styled(prompt.label.clone(), label_style),
            Span::styled(format!(" ({kind})"), Style::default().fg(colors.muted)),
        ]));
        let value = form.inputs.get(index).map(Input::value).unwrap_or_default();
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled("- ", Style::default().fg(colors.muted)),
            Span::styled(value.to_string(), value_style),
        ]));
        if active {
            cursor_row = Some(create_form_input_row(index));
        }
        if active && show_suggestions && !side_by_side {
            render_create_suggestion_lines(&mut lines, form, colors, 6);
        }
    }
    lines.push(Line::from(Span::raw("")));
    lines.push(Line::from(vec![
        Span::styled(
            "Enter",
            Style::default()
                .fg(colors.warm)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" run  ", Style::default().fg(colors.muted)),
        Span::styled(
            "Up/Down",
            Style::default()
                .fg(colors.warm)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" field  ", Style::default().fg(colors.muted)),
        Span::styled(
            "Right/Left",
            Style::default()
                .fg(colors.warm)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" completions  ", Style::default().fg(colors.muted)),
        Span::styled(
            "Tab",
            Style::default()
                .fg(colors.warm)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" accept  ", Style::default().fg(colors.muted)),
        Span::styled(
            "j/k",
            Style::default()
                .fg(colors.warm)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" pick  ", Style::default().fg(colors.muted)),
        Span::styled(
            "Esc",
            Style::default()
                .fg(colors.warm)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" back", Style::default().fg(colors.muted)),
    ]));

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), form_area);
    if let Some(area) = suggestion_area {
        render_create_suggestion_panel(frame, area, form, colors);
    }
    let input_row = cursor_row
        .map(|row| form_area.y.saturating_add(row as u16))
        .unwrap_or(form_area.y);
    let cursor_x = form
        .inputs
        .get(form.active_prompt)
        .map(Input::visual_cursor)
        .unwrap_or(0) as u16;
    if input_row < form_area.y.saturating_add(form_area.height) {
        frame.set_cursor_position((
            form_area.x.saturating_add(4).saturating_add(cursor_x),
            input_row,
        ));
    }
}

fn render_create_suggestion_lines(
    lines: &mut Vec<Line<'static>>,
    form: &CreateForm,
    colors: ActionPickerColors,
    limit: usize,
) {
    if form.path_suggestions.is_empty() {
        return;
    }
    lines.push(Line::from(Span::styled(
        "  Path suggestions",
        Style::default().fg(colors.muted),
    )));
    for (index, suggestion) in form.path_suggestions.iter().take(limit).enumerate() {
        let selected = index == form.selected_suggestion;
        let focused = form.focus == CreateFormFocus::Completions;
        let style = if selected {
            if focused {
                Style::default()
                    .fg(Color::Black)
                    .bg(colors.warm)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(colors.warm)
            }
        } else if suggestion.is_dir {
            Style::default().fg(colors.accent)
        } else {
            Style::default().fg(colors.text)
        };
        let prefix = if selected { "  > " } else { "    " };
        lines.push(Line::from(Span::styled(
            format!("{prefix}{}", suggestion.display),
            style,
        )));
    }
}

fn render_create_suggestion_panel(
    frame: &mut ratatui::Frame,
    area: Rect,
    form: &CreateForm,
    colors: ActionPickerColors,
) {
    let mut lines = vec![Line::from(Span::styled(
        "Path suggestions",
        Style::default().fg(colors.muted),
    ))];
    let limit = area.height.saturating_sub(1) as usize;
    for (index, suggestion) in form.path_suggestions.iter().take(limit).enumerate() {
        let selected = index == form.selected_suggestion;
        let focused = form.focus == CreateFormFocus::Completions;
        let style = if selected {
            if focused {
                Style::default()
                    .fg(Color::Black)
                    .bg(colors.warm)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(colors.warm)
            }
        } else if suggestion.is_dir {
            Style::default().fg(colors.accent)
        } else {
            Style::default().fg(colors.text)
        };
        let prefix = if selected { "> " } else { "  " };
        lines.push(Line::from(Span::styled(
            format!("{prefix}{}", suggestion.display),
            style,
        )));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), area);
}

fn render_progress_bar(progress: f64, width: usize, fill: Color, empty: Color) -> Line<'static> {
    let bar_width = width.saturating_sub(6).clamp(8, 72);
    let filled = ((bar_width as f64) * progress).round() as usize;
    let empty_count = bar_width.saturating_sub(filled);
    Line::from(vec![
        Span::styled("[", Style::default().fg(empty)),
        Span::styled("█".repeat(filled), Style::default().fg(fill)),
        Span::styled("░".repeat(empty_count), Style::default().fg(empty)),
        Span::styled("]", Style::default().fg(empty)),
    ])
}

fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

fn apply_preview_for_entry(
    compositor: &mut CurrentCompositor,
    data: &PreviewData,
    entry: Option<&NavigateEntry>,
) {
    compositor.apply_preview(data);
    if let Some(preferred_path) = entry.and_then(|entry| entry.preferred_preview_path.as_deref()) {
        compositor.select_preview_path(data, preferred_path);
    }
}

fn build_preview_data_for_entry(
    path: &str,
    entry: Option<&NavigateEntry>,
    accent: Color,
    muted: Color,
    text: Color,
    preview_settings: PreviewSettings,
) -> PreviewData {
    if let Some(NavigateEntry {
        kind:
            NavigateEntryKind::RemoteBranch {
                remote_branch,
                bare_path,
                ..
            },
        selection_path,
        ..
    }) = entry
    {
        return build_remote_branch_preview_data(
            bare_path,
            remote_branch,
            selection_path,
            accent,
            muted,
            text,
        );
    }

    build_preview_data(path, accent, muted, text, preview_settings)
}

fn preview_placeholder_path(entry: Option<&NavigateEntry>) -> Option<&str> {
    let entry = entry?;
    if is_remote_branch_entry(entry) {
        Some(entry.selection_path.as_str())
    } else {
        Some(entry.preview_root_path.as_str())
    }
}

fn focus_from_change(change: FocusChange) -> Focus {
    match change {
        FocusChange::Search => Focus::Search,
        FocusChange::Preview => Focus::Preview,
        FocusChange::Detail => Focus::Detail,
    }
}

fn compute_list_window_offset(
    selected: usize,
    current_offset: usize,
    height: usize,
    total: usize,
) -> usize {
    if total == 0 || height == 0 {
        return 0;
    }

    let mut offset = current_offset.min(total.saturating_sub(1));
    if selected < offset {
        offset = selected;
    } else if selected >= offset + height {
        offset = selected + 1 - height;
    }

    let max_offset = total.saturating_sub(height);
    if offset > max_offset {
        offset = max_offset;
    }
    offset
}

fn current_selection_entry<'a>(
    entries: &'a [NavigateEntry],
    filtered: &[usize],
    selected: usize,
) -> Option<&'a NavigateEntry> {
    filtered.get(selected).and_then(|index| entries.get(*index))
}

fn enter_selection_path(
    focus: Focus,
    current_path: Option<&str>,
    preview_tab_index: usize,
    preview_cache: &HashMap<String, PreviewData>,
) -> Option<String> {
    if !matches!(focus, Focus::Preview | Focus::Detail) {
        return None;
    }
    let current_path = current_path?;
    preview_cache
        .get(current_path)
        .and_then(|data| data.previews.get(preview_tab_index))
        .map(|tab| tab.path.clone())
}

fn selection_path_for_action(
    focus: Focus,
    current_path: Option<&str>,
    preview_tab_index: usize,
    preview_cache: &HashMap<String, PreviewData>,
    selected_entry: Option<&NavigateEntry>,
) -> Option<String> {
    enter_selection_path(focus, current_path, preview_tab_index, preview_cache).or_else(|| {
        selected_entry.map(|entry| match &entry.kind {
            NavigateEntryKind::Project => selectable_project_path(entry),
            _ => entry.selection_path.clone(),
        })
    })
}

fn selectable_project_path(entry: &NavigateEntry) -> String {
    git::selectable_worktree_path_for_project(Path::new(&entry.selection_path))
        .unwrap_or_else(|| entry.selection_path.clone())
}

fn visible_paths_for_window(
    entries: &[NavigateEntry],
    filtered: &[usize],
    offset: usize,
    height: usize,
) -> Vec<String> {
    if filtered.is_empty() || height == 0 {
        return Vec::new();
    }
    let end = (offset + height).min(filtered.len());
    filtered[offset..end]
        .iter()
        .filter_map(|index| entries.get(*index))
        .map(|entry| entry.metadata_path.clone())
        .collect()
}

fn all_metadata_paths(entries: &[NavigateEntry]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut paths = Vec::new();
    for entry in entries {
        if seen.insert(entry.metadata_path.clone()) {
            paths.push(entry.metadata_path.clone());
        }
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::default_preview_settings;
    use ratatui::text::Line;

    #[test]
    fn parses_git_worktree_porcelain_with_bare_and_branch_entries() {
        let output = "worktree /repos/example.git\nbare\n\nworktree /repos/example\nHEAD 123456\nbranch refs/heads/main\n\nworktree /repos/example-feature\nHEAD abcdef\nbranch refs/heads/feature/worktree\n";

        let worktrees = git::parse_git_worktree_list(output);

        assert_eq!(worktrees.len(), 3);
        assert_eq!(worktrees[0].path, "/repos/example.git");
        assert!(worktrees[0].bare);
        assert_eq!(worktrees[1].path, "/repos/example");
        assert_eq!(worktrees[1].branch.as_deref(), Some("main"));
        assert_eq!(worktrees[2].branch.as_deref(), Some("feature/worktree"));
    }

    #[test]
    fn labels_detached_worktree_when_no_branch_is_reported() {
        let output = "worktree /repos/example-detached\nHEAD abcdef\ndetached\n";

        let worktrees = git::parse_git_worktree_list(output);

        assert_eq!(worktrees.len(), 1);
        assert!(worktrees[0].detached);
        assert_eq!(git::git_worktree_label(&worktrees[0], true), "detached");
    }

    #[test]
    fn shortens_worktree_tab_label_after_last_slash() {
        assert_eq!(
            git::worktree_tab_label("feat/yarden/potato", true),
            "potato"
        );
        assert_eq!(
            git::worktree_tab_label("feat/yarden/potato", false),
            "feat/yarden/potato"
        );
    }

    #[test]
    fn pseudo_scrolls_tabs_from_previous_label() {
        let labels = vec![
            "main".to_string(),
            "feature-1".to_string(),
            "feature-2".to_string(),
            "feature-3".to_string(),
            "feature-4".to_string(),
        ];

        let settings = default_preview_settings();
        let (first_visible, first_selected) = ui::visible_tab_window(&labels, 0, 35, settings);
        let (middle_visible, middle_selected) = ui::visible_tab_window(&labels, 2, 40, settings);

        assert_eq!(first_selected, 0);
        assert_eq!(
            first_visible,
            vec!["main", "feature-1", "feature-2", "f..."]
        );
        assert_eq!(middle_selected, 1);
        assert_eq!(
            middle_visible,
            vec!["feature-1", "feature-2", "feature-3", "f..."]
        );
    }

    #[test]
    fn truncates_long_tab_labels_with_ellipsis() {
        assert_eq!(
            ui::truncate_tab_label("very-long-feature", 10),
            "very-lo..."
        );
        assert_eq!(ui::truncate_tab_label("abc", 10), "abc");
        assert_eq!(ui::truncate_tab_label("abcdef", 3), "...");
    }

    #[test]
    fn keeps_more_selected_tab_chars_before_ellipsis() {
        let labels = vec![
            "previous-worktree".to_string(),
            "selected-worktree".to_string(),
            "next-worktree".to_string(),
        ];
        let settings = PreviewSettings {
            shorten_worktree_tab_labels: true,
            worktree_tab_min_chars: 6,
            selected_worktree_tab_min_chars: 10,
        };

        let (visible, selected) = ui::visible_tab_window(&labels, 1, 25, settings);

        assert_eq!(selected, 1);
        assert_eq!(visible, vec!["previo...", "selected-w..."]);
    }

    #[test]
    fn enter_from_preview_returns_active_worktree_path() {
        let mut cache = HashMap::new();
        cache.insert(
            "/repos/project.git".to_string(),
            PreviewData {
                previews: vec![
                    model::PreviewTab {
                        path: "/repos/project".to_string(),
                        label: "main".to_string(),
                        text: Text::default(),
                        git: None,
                        github_readme: None,
                    },
                    model::PreviewTab {
                        path: "/repos/project-feature".to_string(),
                        label: "feature".to_string(),
                        text: Text::default(),
                        git: None,
                        github_readme: None,
                    },
                ],
                selected_repo_is_bare: false,
                git_loaded: false,
                github_readme_loaded: false,
            },
        );

        let value = enter_selection_path(Focus::Preview, Some("/repos/project.git"), 1, &cache);

        assert_eq!(value.as_deref(), Some("/repos/project-feature"));
        assert!(
            enter_selection_path(Focus::Search, Some("/repos/project.git"), 1, &cache).is_none()
        );
        let selected_entry = test_entry("project", "project.git", "/repos/project.git");
        assert_eq!(
            selection_path_for_action(
                Focus::Search,
                Some("/repos/project.git"),
                1,
                &cache,
                Some(&selected_entry),
            )
            .as_deref(),
            Some("/repos/project.git")
        );
    }

    #[test]
    fn default_actions_include_requested_tools() {
        let actions = model::default_action_definitions();
        let labels = actions
            .iter()
            .map(|action| action.label.as_str())
            .collect::<Vec<&str>>();

        assert_eq!(labels[0], "Navigate to");
        assert!(labels.contains(&"Open GitHub Desktop"));
        assert!(labels.contains(&"Open VS Code"));
        assert!(labels.contains(&"Open IntelliJ"));
        assert!(labels.contains(&"Open repo online"));
        assert!(labels.contains(&"Open Claude session"));
        assert!(labels.contains(&"Open OpenCode session"));
        assert!(actions.iter().all(|action| action.icon.is_some()));
    }

    #[test]
    fn default_intellij_action_runs_idea_dot_in_selected_path() {
        let actions = model::default_action_definitions();
        let action = actions
            .iter()
            .find(|action| action.label == "Open IntelliJ")
            .expect("default IntelliJ action");

        assert!(matches!(
            &action.kind,
            ActionKind::Command {
                command,
                args,
                current_dir: Some(current_dir),
            } if command == "idea" && args == &["."] && current_dir == "{path}"
        ));
    }

    #[test]
    fn expands_action_path_placeholders() {
        assert_eq!(
            expand_action_value("cd {path}", Some("/repos/project"), None),
            Some("cd /repos/project".to_string())
        );
        assert_eq!(expand_action_value("cd {path}", None, None), None);
    }

    #[test]
    fn picker_index_wraps() {
        assert_eq!(next_picker_index(2, 3), 0);
        assert_eq!(previous_picker_index(0, 3), 2);
        assert_eq!(next_picker_index(0, 0), 0);
        assert_eq!(previous_picker_index(0, 0), 0);
    }

    #[test]
    fn action_picker_filters_by_search_query() {
        let actions = model::default_action_definitions();
        let indexes = filtered_action_indexes(&actions, "code", Some("/tmp"));
        let labels = indexes
            .iter()
            .map(|index| actions[*index].label.as_str())
            .collect::<Vec<&str>>();

        assert_eq!(labels, vec!["Open VS Code", "Open OpenCode session"]);
    }

    #[test]
    fn action_bindings_match_ctrl_enter_and_ctrl_space() {
        let bindings = model::default_action_picker_bindings();
        assert!(matches_action_binding(
            &KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL),
            &bindings
        ));
        assert!(matches_action_binding(
            &KeyEvent::new(KeyCode::Char(' '), KeyModifiers::CONTROL),
            &bindings
        ));
        assert!(!matches_action_binding(
            &KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &bindings
        ));
    }

    #[test]
    fn create_binding_matches_ctrl_n() {
        let bindings = model::default_create_picker_bindings();

        assert!(matches_create_binding(
            &KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL),
            &bindings
        ));
        assert!(!matches_create_binding(
            &KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE),
            &bindings
        ));
    }

    #[test]
    fn create_cli_launch_defaults_to_current_dir() {
        let cwd = PathBuf::from("/tmp/navgator-cwd");

        assert_eq!(
            create_launch_from_args(&[], &cwd),
            InitialLaunch::CreatePicker { path: cwd.clone() }
        );
        assert_eq!(
            create_launch_from_args(&["new-project".to_string()], &cwd),
            InitialLaunch::CreateRecipe {
                selector: "new-project".to_string(),
                path: cwd,
            }
        );
    }

    #[test]
    fn create_cli_launch_accepts_recipe_path_argument() {
        let cwd = PathBuf::from("/tmp/navgator-cwd");
        let path = PathBuf::from("/tmp/navgator-target");

        assert_eq!(
            create_launch_from_args(
                &[
                    "new-branch-worktree".to_string(),
                    path.to_string_lossy().to_string(),
                ],
                &cwd,
            ),
            InitialLaunch::CreateRecipe {
                selector: "new-branch-worktree".to_string(),
                path,
            }
        );
    }

    #[test]
    fn create_recipe_selector_matches_labels() {
        let items = model::default_create_definitions();

        assert_eq!(create_recipe_selector("New project"), "new-project");
        assert_eq!(create_recipe_index(&items, "new-project"), Some(0));
        assert_eq!(
            create_recipe_index(&items, "New branch + worktree"),
            Some(1)
        );
    }

    #[test]
    fn expands_create_placeholders_and_home_prefix() {
        let old_home = env::var("HOME").ok();
        env::set_var("HOME", "/Users/example");

        let expanded = expand_create_value(
            "~/Github/{name}",
            Some("selected"),
            &[("name".to_string(), "repo".to_string())],
        );

        restore_env_var("HOME", old_home);
        assert_eq!(expanded, "/Users/example/Github/repo");
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
    fn create_form_collects_all_prompt_values() {
        let item = CreateDefinition {
            label: "New".to_string(),
            icon: None,
            prompts: vec![
                model::CreatePrompt {
                    name: "name".to_string(),
                    label: "Name".to_string(),
                    kind: CreatePromptKind::Text,
                    default: Some("repo".to_string()),
                    required: true,
                },
                model::CreatePrompt {
                    name: "target".to_string(),
                    label: "Target".to_string(),
                    kind: CreatePromptKind::Path,
                    default: Some("/tmp/{name}".to_string()),
                    required: true,
                },
            ],
            shell: "true".to_string(),
            current_dir: None,
            success_path: "{target}".to_string(),
        };
        let mut form = create_form_for_item(0, &item, None, None);

        assert_eq!(form.inputs[0].value(), "repo");
        assert_eq!(form.inputs[1].value(), "/tmp/repo");
        form.active_prompt = 0;
        form.inputs[0] = Input::new("custom".to_string());
        form.inputs[1] = Input::new(String::new());
        update_dependent_create_defaults(&mut form, &item, None);

        let values = collect_create_form_values(&mut form, &item).expect("valid values");
        assert_eq!(
            values,
            vec![
                ("name".to_string(), "custom".to_string()),
                ("target".to_string(), "/tmp/custom".to_string()),
            ]
        );
    }

    #[test]
    fn create_form_cursor_row_tracks_active_value_row() {
        assert_eq!(create_form_input_row(0), 2);
        assert_eq!(create_form_input_row(1), 4);
        assert_eq!(create_form_input_row(2), 6);
    }

    #[test]
    fn create_form_drops_completion_focus_when_leaving_path_prompt() {
        let item = model::default_create_definitions()
            .into_iter()
            .find(|item| item.label == "New project")
            .expect("new project recipe");
        let mut form = create_form_for_item(0, &item, None, None);
        form.active_prompt = 1;
        update_create_path_suggestions(&mut form, &item);
        form.focus = CreateFormFocus::Completions;

        move_create_form_prompt(&mut form, &item, false);

        assert_eq!(form.active_prompt, 0);
        assert!(matches!(form.focus, CreateFormFocus::Fields));
        assert!(form.path_suggestions.is_empty());
    }

    #[test]
    fn path_suggestions_complete_directory_prefixes() {
        let root = env::temp_dir().join(format!(
            "navgator-create-suggestions-{}",
            std::process::id()
        ));
        let nested = root.join("alpha");
        fs::create_dir_all(&nested).expect("create nested dir");
        fs::write(root.join("alpine.txt"), "test").expect("create file");

        let input = root.join("al").to_string_lossy().to_string();
        let suggestions = path_suggestions_for_input(&input);

        let _ = fs::remove_dir_all(&root);
        assert!(suggestions
            .iter()
            .any(|suggestion| suggestion.display == "alpha/" && suggestion.is_dir));
        assert!(suggestions
            .iter()
            .any(|suggestion| suggestion.display == "alpine.txt" && !suggestion.is_dir));
    }

    #[test]
    fn action_picker_hides_unmet_file_conditions() {
        let temp =
            env::temp_dir().join(format!("navgator-action-condition-{}", std::process::id()));
        fs::create_dir_all(&temp).expect("create temp dir");
        let mut action = model::default_action_definitions()
            .into_iter()
            .find(|action| action.label == "Open GitHub Desktop")
            .expect("github desktop action");

        assert!(!action_file_condition_matches(
            &action,
            Some(&temp.to_string_lossy())
        ));
        fs::create_dir_all(temp.join(".git")).expect("create git marker");
        assert!(action_file_condition_matches(
            &action,
            Some(&temp.to_string_lossy())
        ));
        action.file_condition = Some("/definitely/missing/navgator-condition".to_string());
        assert!(!action_file_condition_matches(
            &action,
            Some(&temp.to_string_lossy())
        ));
        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn picker_ctrl_enter_marks_action_for_session_close() {
        let action = ActionDefinition {
            label: "Navigate".to_string(),
            icon: None,
            file_condition: None,
            kind: ActionKind::Navigate,
        };

        let outcome = resolve_picker_action(Some(&action), Some("/repos/project"), true)
            .expect("resolved action");

        assert!(matches!(
            outcome,
            NavigateOutcome::Navigate {
                path,
                close_session: true,
            } if path == "/repos/project"
        ));
    }

    #[test]
    fn close_session_output_protocol_writes_marker_and_path() {
        let old_protocol = env::var("NAVGATOR_OUTPUT_PROTOCOL").ok();
        let old_output = env::var_os("GATOR_OUTPUT");
        let path =
            env::temp_dir().join(format!("navgator-close-session-{}.txt", std::process::id()));
        env::set_var("NAVGATOR_OUTPUT_PROTOCOL", "2");
        env::set_var("GATOR_OUTPUT", &path);

        write_close_session_outcome(Some("/repos/project")).expect("write outcome");

        restore_env_var("NAVGATOR_OUTPUT_PROTOCOL", old_protocol);
        restore_env_os("GATOR_OUTPUT", old_output);
        let contents = fs::read_to_string(&path).expect("read outcome");
        let _ = fs::remove_file(&path);
        assert_eq!(contents, "__NAVGATOR_CLOSE_SESSION__\n/repos/project\n");
    }

    #[test]
    fn applies_git_result_to_one_preview_tab() {
        let mut data = PreviewData {
            previews: vec![
                model::PreviewTab {
                    path: "/repos/project".to_string(),
                    label: "main".to_string(),
                    text: Text::default(),
                    git: None,
                    github_readme: None,
                },
                model::PreviewTab {
                    path: "/repos/project-feature".to_string(),
                    label: "feature".to_string(),
                    text: Text::default(),
                    git: None,
                    github_readme: None,
                },
            ],
            selected_repo_is_bare: true,
            git_loaded: false,
            github_readme_loaded: false,
        };

        apply_git_result(
            &mut data,
            1,
            Some(Text::from(Line::from("Branch: feature"))),
            false,
        );

        assert!(data.previews[0].git.is_none());
        assert!(data.previews[1].git.is_some());
        assert!(!data.git_loaded);
    }

    #[test]
    fn orders_github_detail_before_git_detail() {
        let tab = model::PreviewTab {
            path: "/repos/project".to_string(),
            label: "main".to_string(),
            text: Text::default(),
            git: Some(Text::from(Line::from("Branch: main"))),
            github_readme: Some(Text::from(Line::from("# Project"))),
        };

        let detail_tabs = content::detail_tabs_for_preview(&tab);

        assert_eq!(detail_tabs.len(), 2);
        assert_eq!(detail_tabs[0].label, "GitHub");
        assert_eq!(detail_tabs[1].label, "Git");
    }

    #[test]
    fn filters_preview_worktree_tabs_by_label_or_path() {
        let data = PreviewData {
            previews: vec![
                model::PreviewTab {
                    path: "/repos/project".to_string(),
                    label: "main".to_string(),
                    text: Text::default(),
                    git: None,
                    github_readme: None,
                },
                model::PreviewTab {
                    path: "/repos/project-feature".to_string(),
                    label: "feature".to_string(),
                    text: Text::default(),
                    git: None,
                    github_readme: None,
                },
                model::PreviewTab {
                    path: "/repos/project-hotfix".to_string(),
                    label: "hotfix".to_string(),
                    text: Text::default(),
                    git: None,
                    github_readme: None,
                },
            ],
            selected_repo_is_bare: false,
            git_loaded: false,
            github_readme_loaded: false,
        };

        assert_eq!(content::preview_tab_visible_indexes(&data, "feat"), vec![1]);
        assert_eq!(content::preview_tab_visible_indexes(&data, "hot"), vec![2]);
        assert_eq!(
            content::preview_tab_visible_indexes(&data, "missing"),
            vec![0, 1, 2]
        );
    }

    #[test]
    fn visible_name_match_does_not_include_deep_parent_path_match() {
        let entries = vec![
            test_entry(
                "child",
                "Funnel Not Working",
                "/repos/Trading Platform Location/Funnel Not Working",
            ),
            test_entry(
                "parent",
                "Trading Platform Location",
                "/repos/Trading Platform Location",
            ),
        ];
        let filtered = search::filter_and_sort(
            &entries,
            "trading platform location",
            SortMode::Match,
            &HashMap::new(),
            &HashMap::new(),
        );

        assert_eq!(filtered, vec![1]);

        let tokens = search::parse_query_tokens("trading platform location");
        assert_eq!(
            search::entry_match_context(&entries[0], &[], &tokens).as_deref(),
            None
        );
    }

    #[test]
    fn entry_match_context_explains_hidden_search_field_match() {
        let entry = model::NavigateEntry {
            id: "hidden".to_string(),
            display: "dev-on-tuesday-github-org-portal-ui".to_string(),
            context: None,
            preview_root_path: "/repos/ideda/dev-on-tuesday-github-org-portal-ui".to_string(),
            preferred_preview_path: None,
            selection_path: "/repos/ideda/dev-on-tuesday-github-org-portal-ui".to_string(),
            metadata_path: "/repos/ideda/dev-on-tuesday-github-org-portal-ui".to_string(),
            search_text: vec![
                "dev-on-tuesday-github-org-portal-ui".to_string(),
                "/repos/ideda/dev-on-tuesday-github-org-portal-ui".to_string(),
            ],
            kind: model::NavigateEntryKind::Project,
        };
        let tokens = search::parse_query_tokens("ideda");

        assert_eq!(
            search::entry_match_context(&entry, &[], &tokens).as_deref(),
            None
        );
    }

    #[test]
    fn compositor_selects_preferred_worktree_preview_tab() {
        let data = PreviewData {
            previews: vec![
                model::PreviewTab {
                    path: "/repos/project".to_string(),
                    label: "main".to_string(),
                    text: Text::from(Line::from("main")),
                    git: None,
                    github_readme: None,
                },
                model::PreviewTab {
                    path: "/repos/project-QCDI-8206".to_string(),
                    label: "QCDI-8206".to_string(),
                    text: Text::from(Line::from("worktree")),
                    git: None,
                    github_readme: None,
                },
            ],
            selected_repo_is_bare: false,
            git_loaded: false,
            github_readme_loaded: false,
        };
        let mut compositor = CurrentCompositor::new(Text::default());

        compositor.apply_preview(&data);
        compositor.select_preview_path(&data, "/repos/project-QCDI-8206");

        assert_eq!(compositor.active_content_index(), 1);
        assert_eq!(compositor.preview_tab_visible_index, 1);
    }

    #[test]
    fn unclaimed_selection_follows_first_result_after_refilter() {
        let entries = vec![
            test_entry("recycle", "$RECYCLE.BIN", "/repos/$RECYCLE.BIN"),
            test_entry("project", "project", "/repos/project"),
        ];
        let filtered = vec![1, 0];

        let selected = selected_after_refilter(&entries, &filtered, 0, Some("recycle"), true);

        assert_eq!(selected, 0);
        assert_eq!(entries[filtered[selected]].id, "project");
    }

    #[test]
    fn user_controlled_selection_is_preserved_after_refilter() {
        let entries = vec![
            test_entry("recycle", "$RECYCLE.BIN", "/repos/$RECYCLE.BIN"),
            test_entry("project", "project", "/repos/project"),
        ];
        let filtered = vec![1, 0];

        let selected = selected_after_refilter(&entries, &filtered, 0, Some("recycle"), false);

        assert_eq!(selected, 1);
        assert_eq!(entries[filtered[selected]].id, "recycle");
    }

    fn test_entry(id: &str, display: &str, path: &str) -> model::NavigateEntry {
        model::NavigateEntry {
            id: id.to_string(),
            display: display.to_string(),
            context: None,
            preview_root_path: path.to_string(),
            preferred_preview_path: None,
            selection_path: path.to_string(),
            metadata_path: path.to_string(),
            search_text: vec![display.to_string()],
            kind: model::NavigateEntryKind::Project,
        }
    }

    fn restore_env_var(name: &str, value: Option<String>) {
        if let Some(value) = value {
            env::set_var(name, value);
        } else {
            env::remove_var(name);
        }
    }

    fn restore_env_os(name: &str, value: Option<std::ffi::OsString>) {
        if let Some(value) = value {
            env::set_var(name, value);
        } else {
            env::remove_var(name);
        }
    }

    #[test]
    fn sorts_default_branch_worktree_first() {
        let trunk = model::GitWorktree {
            path: "/repos/project-trunk".to_string(),
            branch: Some("trunk".to_string()),
            detached: false,
            bare: false,
        };
        let feature = model::GitWorktree {
            path: "/repos/project-feature".to_string(),
            branch: Some("feature".to_string()),
            detached: false,
            bare: false,
        };
        let mut worktrees = vec![&feature, &trunk];

        content::sort_worktrees_default_first(&mut worktrees, Some("trunk"));

        assert_eq!(worktrees[0].branch.as_deref(), Some("trunk"));
        assert_eq!(worktrees[1].branch.as_deref(), Some("feature"));
    }

    #[test]
    fn displays_home_paths_with_tilde() {
        assert_eq!(
            content::display_path_with_home("/Users/kcw", "/Users/kcw"),
            "~"
        );
        assert_eq!(
            content::display_path_with_home("/Users/kcw/Github/navgator", "/Users/kcw"),
            "~/Github/navgator"
        );
        assert_eq!(
            content::display_path_with_home("/Users/kcw-other/Github", "/Users/kcw"),
            "/Users/kcw-other/Github"
        );
    }

    #[test]
    fn resolves_dot_bare_worktree_container() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        let root = env::temp_dir().join(format!("navgator-dot-bare-test-{unique}"));
        let dot_bare = root.join(".bare");
        std::fs::create_dir_all(&root).expect("test root should be created");

        let status = std::process::Command::new("git")
            .arg("init")
            .arg("--bare")
            .arg(&dot_bare)
            .status()
            .expect("git should be available");
        assert!(status.success());

        let resolved = git::git_command_dir_for_path(&root);
        let _ = std::fs::remove_dir_all(&root);

        assert_eq!(resolved.as_deref(), Some(dot_bare.as_path()));
    }
}
