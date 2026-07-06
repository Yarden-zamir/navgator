use crate::model::{
    default_action_definitions, default_preview_settings, ActionDefinition, ActionKind,
    ActionSettings, AppResult, LoadedConfig, PreviewSettings, RemoteSettings, SortMode,
    SortSettings, ThemeColors, CONFIG_SCHEMA_URL,
};
use figment::providers::{Format, Toml};
use figment::Figment;
use schemars::{schema_for, JsonSchema};
use serde::Deserialize;
use std::{
    collections::HashSet,
    env, fs,
    path::{Path, PathBuf},
};

#[derive(Default, Deserialize, JsonSchema)]
#[schemars(
    title = "Navgator Config",
    description = "Configuration file for navgator path indexing and static items."
)]
struct ConfigFile {
    #[serde(default, rename = "$schema")]
    #[schemars(
        title = "Schema URL",
        description = "Optional JSON Schema URL for editor autocompletion and validation."
    )]
    _schema_url: Option<String>,
    #[serde(default)]
    #[schemars(
        title = "Paths",
        description = "Path collection settings used to build the navigation list."
    )]
    paths: Option<ConfigPaths>,
    #[serde(default)]
    #[schemars(title = "Preview", description = "Preview panel settings.")]
    preview: Option<ConfigPreview>,
    #[serde(default)]
    #[schemars(title = "Sort", description = "Result sorting settings.")]
    sort: Option<ConfigSort>,
    #[serde(default)]
    #[schemars(title = "Remote", description = "Remote branch discovery settings.")]
    remote: Option<ConfigRemote>,
    #[serde(default)]
    #[schemars(title = "Actions", description = "Ctrl+Enter action picker settings.")]
    actions: Option<ConfigActions>,
    #[serde(default)]
    #[schemars(
        title = "UI",
        description = "User interface color and display settings."
    )]
    ui: Option<ConfigUi>,
}

#[derive(Clone, Copy)]
struct ConfigRuntimeDefaults {
    preview_settings: PreviewSettings,
    sort_settings: SortSettings,
    remote_settings: RemoteSettings,
    theme_colors: ThemeColors,
}

struct ConfigLoadState {
    index_folders: Vec<PathBuf>,
    static_items: Vec<PathBuf>,
    seen_index: HashSet<String>,
    seen_static: HashSet<String>,
    preview_settings: PreviewSettings,
    sort_settings: SortSettings,
    remote_settings: RemoteSettings,
    action_settings: ActionSettings,
    theme_colors: ThemeColors,
}

impl ConfigLoadState {
    fn new() -> Self {
        let defaults = config_runtime_defaults();
        Self {
            index_folders: Vec::new(),
            static_items: Vec::new(),
            seen_index: HashSet::new(),
            seen_static: HashSet::new(),
            preview_settings: defaults.preview_settings,
            sort_settings: defaults.sort_settings,
            remote_settings: defaults.remote_settings,
            action_settings: ActionSettings::default(),
            theme_colors: defaults.theme_colors,
        }
    }

    fn into_loaded_config(self) -> LoadedConfig {
        LoadedConfig {
            index_folders: self.index_folders,
            static_items: self.static_items,
            preview_settings: self.preview_settings,
            sort_settings: self.sort_settings,
            remote_settings: self.remote_settings,
            action_settings: self.action_settings,
            theme_colors: self.theme_colors,
        }
    }
}

#[derive(Default, Deserialize, JsonSchema)]
#[schemars(
    title = "Path Settings",
    description = "Groups of folders that navgator indexes or always includes."
)]
struct ConfigPaths {
    #[serde(default)]
    #[schemars(
        title = "Index Folders",
        description = "Directories to index; each directory and its direct child directories are included."
    )]
    index_folders: Vec<String>,
    #[serde(default)]
    #[schemars(
        title = "Static Items",
        description = "Directories or files to include as-is without indexing children."
    )]
    static_items: Vec<String>,
}

#[derive(Default, Deserialize, JsonSchema)]
#[schemars(
    title = "Preview Settings",
    description = "Settings for preview and worktree preview tabs."
)]
struct ConfigPreview {
    #[serde(default)]
    #[schemars(
        title = "Shorten Worktree Tab Labels",
        description = "When true, worktree tab labels use only the segment after the last slash, for example feat/yarden/potato becomes potato. Defaults to true."
    )]
    shorten_worktree_tab_labels: Option<bool>,
    #[serde(default)]
    #[schemars(
        title = "Worktree Tab Minimum Characters",
        description = "Minimum label characters to keep before the ellipsis for non-selected worktree preview tabs. Defaults to 6."
    )]
    worktree_tab_min_chars: Option<usize>,
    #[serde(default)]
    #[schemars(
        title = "Selected Worktree Tab Minimum Characters",
        description = "Minimum label characters to keep before the ellipsis for the selected worktree preview tab. Defaults to 10."
    )]
    selected_worktree_tab_min_chars: Option<usize>,
}

#[derive(Default, Deserialize, JsonSchema)]
#[schemars(title = "Sort Settings", description = "Settings for result ordering.")]
struct ConfigSort {
    #[serde(default)]
    #[schemars(
        title = "Default Sort",
        description = "Initial result sort mode. Defaults to modified-desc."
    )]
    default: Option<ConfigSortMode>,
    #[serde(default)]
    #[schemars(
        title = "Pin Current Project",
        description = "When true, the current Git worktree/project is pinned to the first row for empty searches. Defaults to true."
    )]
    pin_current_project: Option<bool>,
}

#[derive(Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum ConfigSortMode {
    Match,
    AlphaAsc,
    AlphaDesc,
    CreatedAsc,
    CreatedDesc,
    ModifiedAsc,
    ModifiedDesc,
}

impl ConfigSortMode {
    fn to_sort_mode(self) -> SortMode {
        match self {
            ConfigSortMode::Match => SortMode::Match,
            ConfigSortMode::AlphaAsc => SortMode::AlphaAsc,
            ConfigSortMode::AlphaDesc => SortMode::AlphaDesc,
            ConfigSortMode::CreatedAsc => SortMode::CreatedAsc,
            ConfigSortMode::CreatedDesc => SortMode::CreatedDesc,
            ConfigSortMode::ModifiedAsc => SortMode::ModifiedAsc,
            ConfigSortMode::ModifiedDesc => SortMode::ModifiedDesc,
        }
    }
}

#[derive(Default, Deserialize, JsonSchema)]
#[schemars(
    title = "Remote Settings",
    description = "Settings for remote branch discovery and caching."
)]
struct ConfigRemote {
    #[serde(default)]
    #[schemars(
        title = "Enabled By Default",
        description = "When true, remote branch mode starts enabled for the initially selected project. Defaults to false."
    )]
    enabled_by_default: Option<bool>,
    #[serde(default)]
    #[schemars(
        title = "Refresh On Toggle",
        description = "When true, enabling remote branch mode runs a background ls-remote refresh. Defaults to true."
    )]
    refresh_on_toggle: Option<bool>,
    #[serde(default)]
    #[schemars(
        title = "Use Cache",
        description = "When true, cached remote branches are shown before local refs and background refresh. Defaults to true."
    )]
    use_cache: Option<bool>,
}

#[derive(Default, Deserialize, JsonSchema)]
#[schemars(
    title = "Action Settings",
    description = "Actions shown by the Ctrl+Enter picker. Set defaults = false to replace the built-in actions. Use {path} for the selected path and {github_url} for the selected repo's GitHub URL."
)]
struct ConfigActions {
    #[serde(default)]
    #[schemars(
        title = "Include Default Actions",
        description = "When true, built-in actions are included before custom items. Defaults to true."
    )]
    defaults: Option<bool>,
    #[serde(default)]
    #[schemars(
        title = "Items",
        description = "Custom actions appended to the picker."
    )]
    items: Vec<ConfigAction>,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(title = "Action", description = "One action picker item.")]
struct ConfigAction {
    #[schemars(title = "Label", description = "Text shown in the action picker.")]
    label: String,
    #[serde(flatten)]
    kind: ConfigActionKind,
}

#[derive(Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum ConfigActionKind {
    #[schemars(description = "Return the selected path to the shell wrapper.")]
    Navigate,
    #[schemars(description = "Run a command and close navgator.")]
    Command {
        #[schemars(title = "Command", description = "Executable name or absolute path.")]
        command: String,
        #[serde(default)]
        #[schemars(title = "Arguments", description = "Command arguments.")]
        args: Vec<String>,
        #[serde(default)]
        #[schemars(
            title = "Current Directory",
            description = "Optional command working directory."
        )]
        current_dir: Option<String>,
    },
    #[schemars(description = "Open a URL and close navgator.")]
    OpenUrl {
        #[schemars(title = "URL", description = "URL to open.")]
        url: String,
    },
}

impl ConfigAction {
    fn into_action_definition(self) -> Option<ActionDefinition> {
        let label = self.label.trim();
        if label.is_empty() {
            return None;
        }
        let kind = match self.kind {
            ConfigActionKind::Navigate => ActionKind::Navigate,
            ConfigActionKind::Command {
                command,
                args,
                current_dir,
            } => {
                let command = command.trim();
                if command.is_empty() {
                    return None;
                }
                ActionKind::Command {
                    command: command.to_string(),
                    args,
                    current_dir,
                }
            }
            ConfigActionKind::OpenUrl { url } => {
                let url = url.trim();
                if url.is_empty() {
                    return None;
                }
                ActionKind::OpenUrl {
                    url: url.to_string(),
                }
            }
        };
        Some(ActionDefinition {
            label: label.to_string(),
            kind,
        })
    }
}

#[derive(Default, Deserialize, JsonSchema)]
#[schemars(title = "UI Settings", description = "User interface color settings.")]
struct ConfigUi {
    #[serde(default)]
    #[schemars(title = "Theme", description = "Color theme to use. Defaults to auto.")]
    theme: Option<ConfigTheme>,
}

#[derive(Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum ConfigTheme {
    Auto,
    Light,
    Dark,
}

impl ConfigTheme {
    fn colors(self) -> ThemeColors {
        match self {
            ConfigTheme::Auto => auto_theme_colors(),
            ConfigTheme::Light => ThemeColors::light(),
            ConfigTheme::Dark => ThemeColors::dark(),
        }
    }
}

pub(crate) fn config_schema_json() -> AppResult<String> {
    let schema = schema_for!(ConfigFile);
    serde_json::to_string_pretty(&schema)
        .map_err(|err| format!("Failed to serialize config schema: {err}").into())
}

pub(crate) fn load_config() -> AppResult<LoadedConfig> {
    let home = home_dir()?;
    let mut state = ConfigLoadState::new();
    let mut found_config = false;

    for path in config_paths(&home) {
        if !path.is_file() {
            continue;
        }
        found_config = true;
        let base_dir = path.parent().unwrap_or(&home);
        let config: ConfigFile = Figment::from(Toml::file(&path)).extract().map_err(|err| {
            let display_path = display_path_for_user(&path.to_string_lossy());
            format!("Failed to parse config {}: {}", display_path, err)
        })?;
        ensure_schema_link_in_config_file(&path, &config);
        state.apply_config_file(config, base_dir, &home);
    }

    if !found_config {
        let path = create_default_config(&home)?;
        return load_config_from_created_file(path);
    }

    Ok(state.into_loaded_config())
}

fn config_runtime_defaults() -> ConfigRuntimeDefaults {
    ConfigRuntimeDefaults {
        preview_settings: default_preview_settings(),
        sort_settings: SortSettings::default(),
        remote_settings: RemoteSettings::default(),
        theme_colors: auto_theme_colors(),
    }
}

impl ConfigLoadState {
    fn apply_config_file(&mut self, config: ConfigFile, base_dir: &Path, home: &Path) {
        if let Some(paths) = config.paths {
            merge_paths(
                &paths.index_folders,
                base_dir,
                home,
                &mut self.index_folders,
                &mut self.seen_index,
            );
            merge_paths(
                &paths.static_items,
                base_dir,
                home,
                &mut self.static_items,
                &mut self.seen_static,
            );
        }
        if let Some(preview) = config.preview {
            if let Some(value) = preview.shorten_worktree_tab_labels {
                self.preview_settings.shorten_worktree_tab_labels = value;
            }
            if let Some(value) = preview.worktree_tab_min_chars {
                self.preview_settings.worktree_tab_min_chars = value.max(1);
            }
            if let Some(value) = preview.selected_worktree_tab_min_chars {
                self.preview_settings.selected_worktree_tab_min_chars = value.max(1);
            }
        }
        if let Some(sort) = config.sort {
            if let Some(value) = sort.default {
                self.sort_settings.default_mode = value.to_sort_mode();
            }
            if let Some(value) = sort.pin_current_project {
                self.sort_settings.pin_current_project = value;
            }
        }
        if let Some(remote) = config.remote {
            if let Some(value) = remote.enabled_by_default {
                self.remote_settings.enabled_by_default = value;
            }
            if let Some(value) = remote.refresh_on_toggle {
                self.remote_settings.refresh_on_toggle = value;
            }
            if let Some(value) = remote.use_cache {
                self.remote_settings.use_cache = value;
            }
        }
        if let Some(actions) = config.actions {
            self.action_settings = action_settings_from_config(actions);
        }
        if let Some(ui) = config.ui {
            if let Some(theme) = ui.theme {
                self.theme_colors = theme.colors();
            }
        }
    }
}

fn action_settings_from_config(config: ConfigActions) -> ActionSettings {
    let mut items = if config.defaults.unwrap_or(true) {
        default_action_definitions()
    } else {
        Vec::new()
    };
    items.extend(
        config
            .items
            .into_iter()
            .filter_map(ConfigAction::into_action_definition),
    );
    if items.is_empty() {
        items = default_action_definitions();
    }
    ActionSettings { items }
}

pub(crate) fn home_dir() -> AppResult<PathBuf> {
    let value = env::var("HOME").map_err(|_| "HOME is not set")?;
    Ok(PathBuf::from(value))
}

fn ensure_schema_link_in_config_file(path: &Path, config: &ConfigFile) {
    if config._schema_url.is_some() {
        return;
    }

    let Ok(contents) = fs::read_to_string(path) else {
        return;
    };

    let schema_line = format!("\"$schema\" = \"{CONFIG_SCHEMA_URL}\"");
    let updated = if contents.trim().is_empty() {
        format!("{schema_line}\n")
    } else if contents.starts_with('\n') {
        format!("{schema_line}\n{contents}")
    } else {
        format!("{schema_line}\n\n{contents}")
    };

    if updated != contents {
        let _ = fs::write(path, updated);
    }
}

fn create_default_config(home: &Path) -> AppResult<PathBuf> {
    let path = default_config_path(home);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "Failed to create config directory {}: {err}",
                display_path_for_user(&parent.to_string_lossy())
            )
        })?;
    }
    fs::write(&path, default_config_contents()).map_err(|err| {
        format!(
            "Failed to create default config {}: {err}",
            display_path_for_user(&path.to_string_lossy())
        )
    })?;
    Ok(path)
}

fn load_config_from_created_file(_path: PathBuf) -> AppResult<LoadedConfig> {
    load_config()
}

fn default_config_path(home: &Path) -> PathBuf {
    if let Some(path) = env_path("NAVGATOR_CONFIG") {
        return path;
    }
    let xdg = config_home(home);
    xdg.join("navgator/config.toml")
}

fn default_config_contents() -> String {
    let actions = default_actions_config_contents();
    format!(
        r#""$schema" = "{CONFIG_SCHEMA_URL}"

[paths]
index_folders = ["~/Github", "~/Projects"]
static_items = []

[sort]
default = "modified-desc"
pin_current_project = true

[remote]
enabled_by_default = false
refresh_on_toggle = true
use_cache = true

[actions]
defaults = false
{actions}

[ui]
theme = "auto"

[preview]
shorten_worktree_tab_labels = true
worktree_tab_min_chars = 6
selected_worktree_tab_min_chars = 10
"#
    )
}

fn default_actions_config_contents() -> String {
    let mut output = String::new();
    for action in default_action_definitions() {
        output.push_str("\n[[actions.items]]\n");
        output.push_str(&format!("label = {}\n", toml_string(&action.label)));
        match action.kind {
            ActionKind::Navigate => {
                output.push_str("type = \"navigate\"\n");
            }
            ActionKind::Command {
                command,
                args,
                current_dir,
            } => {
                output.push_str("type = \"command\"\n");
                output.push_str(&format!("command = {}\n", toml_string(&command)));
                if !args.is_empty() {
                    let args = args
                        .iter()
                        .map(|arg| toml_string(arg))
                        .collect::<Vec<String>>()
                        .join(", ");
                    output.push_str(&format!("args = [{args}]\n"));
                }
                if let Some(current_dir) = current_dir {
                    output.push_str(&format!("current_dir = {}\n", toml_string(&current_dir)));
                }
            }
            ActionKind::OpenUrl { url } => {
                output.push_str("type = \"open-url\"\n");
                output.push_str(&format!("url = {}\n", toml_string(&url)));
            }
        }
    }
    output
}

fn toml_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn auto_theme_colors() -> ThemeColors {
    if os_prefers_dark_theme() {
        ThemeColors::dark()
    } else {
        ThemeColors::light()
    }
}

fn os_prefers_dark_theme() -> bool {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("defaults")
            .arg("read")
            .arg("-g")
            .arg("AppleInterfaceStyle")
            .output()
            .map(|output| {
                output.status.success()
                    && String::from_utf8_lossy(&output.stdout)
                        .trim()
                        .eq_ignore_ascii_case("Dark")
            })
            .unwrap_or(false)
    }

    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

fn config_paths(home: &Path) -> Vec<PathBuf> {
    if let Some(path) = env_path("NAVGATOR_CONFIG") {
        return vec![path];
    }

    let mut paths = Vec::new();
    paths.push(PathBuf::from("/etc/navgator/config.toml"));
    let xdg = config_home(home);
    paths.push(xdg.join("navgator/config.toml"));
    paths.push(home.join(".config/navgator/config.toml"));
    paths.push(home.join(".navgator.toml"));
    if let Ok(cwd) = env::current_dir() {
        paths.push(cwd.join(".navgator.toml"));
        paths.push(cwd.join(".navgator/config.toml"));
    }

    let mut seen = HashSet::new();
    let mut unique = Vec::new();
    for path in paths {
        let key = path.to_string_lossy().to_string();
        if seen.insert(key) {
            unique.push(path);
        }
    }
    unique
}

fn env_path(name: &str) -> Option<PathBuf> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn config_home(home: &Path) -> PathBuf {
    env_path("XDG_CONFIG_HOME").unwrap_or_else(|| home.join(".config"))
}

fn merge_paths(
    raw_paths: &[String],
    base_dir: &Path,
    home: &Path,
    target: &mut Vec<PathBuf>,
    seen: &mut HashSet<String>,
) {
    for raw in raw_paths {
        if let Some(path) = normalize_path(raw, base_dir, home) {
            let key = path.to_string_lossy().to_string();
            if seen.insert(key) {
                target.push(path);
            }
        }
    }
}

fn normalize_path(raw: &str, base_dir: &Path, home: &Path) -> Option<PathBuf> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut value = trimmed.to_string();
    if value.starts_with("~/") {
        value = value.replacen("~", &home.to_string_lossy(), 1);
    }
    if value.contains("$HOME") {
        value = value.replace("$HOME", &home.to_string_lossy());
    }
    let mut path = PathBuf::from(value);
    if path.is_relative() {
        path = base_dir.join(path);
    }
    if path.exists() {
        Some(path)
    } else {
        None
    }
}

fn display_path_for_user(path: &str) -> String {
    match env::var("HOME") {
        Ok(home) => display_path_with_home(path, &home),
        Err(_) => path.to_string(),
    }
}

fn display_path_with_home(path: &str, home: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::{
        action_settings_from_config, config_paths, default_config_contents,
        ensure_schema_link_in_config_file, ConfigAction, ConfigActionKind, ConfigActions,
        ConfigFile, ConfigLoadState, CONFIG_SCHEMA_URL,
    };
    use crate::model::{ActionKind, SortMode};
    use figment::providers::{Format, Toml};
    use figment::Figment;
    use std::{env, fs, path::PathBuf, sync::Mutex};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn written_default_config_expands_default_actions() {
        let config = default_config_contents();

        assert!(config.contains("[actions]\ndefaults = false"));
        assert!(config.contains("label = \"Navigate to\"\ntype = \"navigate\""));
        assert!(config.contains(
            "label = \"Open IntelliJ\"\ntype = \"command\"\ncommand = \"idea\"\nargs = [\".\"]\ncurrent_dir = \"{path}\""
        ));
        assert!(config
            .contains("label = \"Open repo online\"\ntype = \"open-url\"\nurl = \"{github_url}\""));
    }

    #[test]
    fn navgator_config_overrides_other_config_paths() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let old_navgator = env::var("NAVGATOR_CONFIG").ok();
        let old_xdg = env::var("XDG_CONFIG_HOME").ok();
        env::set_var("NAVGATOR_CONFIG", "/tmp/navgator-only.toml");
        env::set_var("XDG_CONFIG_HOME", "/tmp/xdg-config");

        let paths = config_paths(&PathBuf::from("/home/example"));

        restore_env("NAVGATOR_CONFIG", old_navgator);
        restore_env("XDG_CONFIG_HOME", old_xdg);
        assert_eq!(paths, vec![PathBuf::from("/tmp/navgator-only.toml")]);
    }

    #[test]
    fn empty_xdg_config_home_uses_home_config_dir() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let old_navgator = env::var("NAVGATOR_CONFIG").ok();
        let old_xdg = env::var("XDG_CONFIG_HOME").ok();
        env::remove_var("NAVGATOR_CONFIG");
        env::set_var("XDG_CONFIG_HOME", "");

        let paths = config_paths(&PathBuf::from("/home/example"));

        restore_env("NAVGATOR_CONFIG", old_navgator);
        restore_env("XDG_CONFIG_HOME", old_xdg);
        assert!(paths.contains(&PathBuf::from("/home/example/.config/navgator/config.toml")));
    }

    #[test]
    fn schema_link_is_inserted_for_config_without_paths() {
        let path =
            env::temp_dir().join(format!("navgator-schema-test-{}.toml", std::process::id()));
        fs::write(&path, "[ui]\ntheme = \"dark\"\n").expect("write temp config");
        let config = ConfigFile {
            ui: None,
            ..ConfigFile::default()
        };

        ensure_schema_link_in_config_file(&path, &config);

        let contents = fs::read_to_string(&path).expect("read temp config");
        let _ = fs::remove_file(&path);
        assert!(contents.starts_with(&format!("\"$schema\" = \"{CONFIG_SCHEMA_URL}\"")));
        assert!(contents.contains("[ui]\ntheme = \"dark\""));
    }

    #[test]
    fn action_config_falls_back_when_no_valid_items_remain() {
        let settings = action_settings_from_config(ConfigActions {
            defaults: Some(false),
            items: vec![ConfigAction {
                label: "".to_string(),
                kind: ConfigActionKind::Command {
                    command: "".to_string(),
                    args: Vec::new(),
                    current_dir: None,
                },
            }],
        });

        assert!(matches!(
            settings.items.first().map(|action| &action.kind),
            Some(ActionKind::Navigate)
        ));
    }

    #[test]
    fn later_config_overrides_scalar_settings_and_merges_paths() {
        let home = PathBuf::from("/Users/example");
        let base = env::temp_dir().join(format!("navgator-config-{}", std::process::id()));
        fs::create_dir_all(base.join("one")).expect("create one");
        fs::create_dir_all(base.join("two")).expect("create two");
        let mut state = ConfigLoadState::new();
        let first = toml_config(
            r#"
            [paths]
            index_folders = ["one"]

            [sort]
            default = "alpha-asc"
            "#,
        );
        let second = toml_config(
            r#"
            [paths]
            index_folders = ["two"]

            [sort]
            default = "alpha-desc"
            "#,
        );

        state.apply_config_file(first, &base, &home);
        state.apply_config_file(second, &base, &home);

        let _ = fs::remove_dir_all(&base);
        assert_eq!(
            state.index_folders,
            vec![base.join("one"), base.join("two")]
        );
        assert!(matches!(
            state.sort_settings.default_mode,
            SortMode::AlphaDesc
        ));
    }

    fn toml_config(contents: &str) -> ConfigFile {
        Figment::from(Toml::string(contents))
            .extract()
            .expect("valid config")
    }

    fn restore_env(name: &str, value: Option<String>) {
        if let Some(value) = value {
            env::set_var(name, value);
        } else {
            env::remove_var(name);
        }
    }
}
