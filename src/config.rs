use crate::model::keybindings::{
    default_keymap, is_valid_action_id, target_is_compatible, Binding, BindingContext,
    BindingTarget, CoreAction, KeyChord, Keymap,
};
use crate::model::{
    default_action_definitions, default_action_picker_bindings, default_create_definitions,
    default_create_picker_bindings, default_preview_settings, ActionBinding, ActionBindingKey,
    ActionDefinition, ActionKind, ActionSettings, AppResult, BranchSelectBehavior, BranchSettings,
    CreateDefinition, CreatePrompt, CreatePromptKind, CreateSettings, LoadedConfig,
    PreviewSettings, RemoteSettings, SortMode, SortSettings, ThemeColors, CONFIG_SCHEMA_URL,
};
use figment::providers::{Format, Toml};
use figment::Figment;
use schemars::{schema_for, JsonSchema};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, HashSet},
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
    #[schemars(
        title = "Branches",
        description = "Behavior when selecting remote branch results."
    )]
    branches: Option<ConfigBranches>,
    #[serde(default)]
    #[schemars(title = "Actions", description = "Action picker settings.")]
    actions: Option<ConfigActions>,
    #[serde(default)]
    #[schemars(
        title = "Create",
        description = "Create picker settings for prompted shell recipes. Use {path} for the selected path and {prompt_name} for prompt values."
    )]
    create: Option<ConfigCreate>,
    #[serde(default)]
    #[schemars(
        title = "Keybindings",
        description = "Context-specific mappings from key chords to core or configured action identifiers."
    )]
    keybindings: Option<ConfigKeybindings>,
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
    branch_settings: BranchSettings,
    action_settings: ActionSettings,
    create_settings: CreateSettings,
    theme_colors: ThemeColors,
    legacy_action_bindings: Option<Vec<ActionBinding>>,
    legacy_create_bindings: Option<Vec<ActionBinding>>,
    keybinding_layer: Keymap,
    cli_keybinding_layer: Keymap,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ConfigLayerSource {
    File,
    Cli,
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
            branch_settings: BranchSettings::default(),
            action_settings: ActionSettings::default(),
            create_settings: CreateSettings::default(),
            theme_colors: defaults.theme_colors,
            legacy_action_bindings: None,
            legacy_create_bindings: None,
            keybinding_layer: Keymap::default(),
            cli_keybinding_layer: Keymap::default(),
        }
    }

    fn into_loaded_config(self) -> AppResult<LoadedConfig> {
        validate_action_ids(&self.action_settings.items)?;
        validate_picker_action_ids(&self.action_settings)?;
        let keymap = self.finalize_keymap()?;
        Ok(LoadedConfig {
            index_folders: self.index_folders,
            static_items: self.static_items,
            preview_settings: self.preview_settings,
            sort_settings: self.sort_settings,
            remote_settings: self.remote_settings,
            branch_settings: self.branch_settings,
            action_settings: self.action_settings,
            create_settings: self.create_settings,
            theme_colors: self.theme_colors,
            keymap,
        })
    }

    fn finalize_keymap(&self) -> AppResult<Keymap> {
        let mut keymap = default_keymap();
        if let Some(bindings) = &self.legacy_action_bindings {
            apply_legacy_action_bindings(&mut keymap, bindings);
        }
        if let Some(bindings) = &self.legacy_create_bindings {
            apply_legacy_create_bindings(&mut keymap, bindings);
        }
        keymap.apply_layer(&self.keybinding_layer);
        keymap.apply_layer(&self.cli_keybinding_layer);
        validate_keymap(&keymap, &self.action_settings.items)?;
        Ok(keymap)
    }
}

#[derive(Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ConfigKeybindings {
    #[serde(default)]
    global: Option<BTreeMap<String, String>>,
    #[serde(default)]
    navigator: Option<BTreeMap<String, String>>,
    #[serde(default)]
    preview: Option<BTreeMap<String, String>>,
    #[serde(default)]
    detail: Option<BTreeMap<String, String>>,
    #[serde(default, rename = "tag-editor")]
    tag_editor: Option<BTreeMap<String, String>>,
    #[serde(default, rename = "action-picker")]
    action_picker: Option<BTreeMap<String, String>>,
    #[serde(default, rename = "create-picker")]
    create_picker: Option<BTreeMap<String, String>>,
    #[serde(default, rename = "create-form")]
    create_form: Option<BTreeMap<String, String>>,
    #[serde(default, rename = "create-completions")]
    create_completions: Option<BTreeMap<String, String>>,
    #[serde(default, rename = "progress-overlay")]
    progress_overlay: Option<BTreeMap<String, String>>,
    #[serde(default, rename = "error-overlay")]
    error_overlay: Option<BTreeMap<String, String>>,
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
    title = "Branch Settings",
    description = "Settings for what navgator does when a remote branch result is selected."
)]
struct ConfigBranches {
    #[serde(default)]
    #[schemars(
        title = "On Select",
        description = "Remote branch selection behavior. worktree creates/reuses a worktree. checkout checks the branch out in an existing worktree. Defaults to worktree."
    )]
    on_select: Option<ConfigBranchSelectBehavior>,
}

#[derive(Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum ConfigBranchSelectBehavior {
    Worktree,
    Checkout,
}

impl ConfigBranchSelectBehavior {
    fn to_branch_select_behavior(self) -> BranchSelectBehavior {
        match self {
            ConfigBranchSelectBehavior::Worktree => BranchSelectBehavior::Worktree,
            ConfigBranchSelectBehavior::Checkout => BranchSelectBehavior::Checkout,
        }
    }
}

#[derive(Default, Deserialize, JsonSchema)]
#[schemars(
    title = "Action Settings",
    description = "Actions shown by the action picker. When items are listed, they replace built-ins unless include_defaults is true. Use {path} for the selected path and {github_url} for the selected repo's GitHub URL."
)]
struct ConfigActions {
    #[serde(default)]
    #[schemars(
        title = "Include Default Actions",
        description = "When true, built-in actions are included before listed items. Defaults to false when items are present."
    )]
    include_defaults: Option<bool>,
    #[serde(default)]
    #[schemars(
        title = "Picker Actions",
        description = "Optional ordered allowlist of action IDs shown in the action picker. Omit to show every effective action. Hidden actions remain available as direct keybinding targets."
    )]
    picker: Option<Vec<String>>,
    #[serde(default)]
    #[schemars(
        title = "Legacy Picker Bindings",
        description = "Deprecated compatibility bindings that open the action picker and run+close inside it. Supported values: ctrl-enter, ctrl-space. Use keybindings instead."
    )]
    bindings: Vec<String>,
    #[serde(default)]
    #[schemars(
        title = "Items",
        description = "Actions shown in the picker. When present, these replace built-ins unless include_defaults is true."
    )]
    items: Vec<ConfigAction>,
}

#[derive(Default, Deserialize, JsonSchema)]
#[schemars(
    title = "Create Settings",
    description = "Create recipes shown by the create picker. Recipes collect prompt values, run shell, and navigate to success_path when the shell succeeds."
)]
struct ConfigCreate {
    #[serde(default)]
    #[schemars(
        title = "Include Default Create Recipes",
        description = "When true, built-in create recipes are included before listed items. Defaults to false when items are present."
    )]
    include_defaults: Option<bool>,
    #[serde(default)]
    #[schemars(
        title = "Legacy Create Picker Bindings",
        description = "Deprecated compatibility bindings that open the create picker. Supported value: ctrl-n. Use keybindings instead."
    )]
    bindings: Vec<String>,
    #[serde(default)]
    #[schemars(
        title = "Create Items",
        description = "Create recipes shown in the picker. When present, these replace built-ins unless include_defaults is true."
    )]
    items: Vec<ConfigCreateItem>,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(
    title = "Create Item",
    description = "One prompted shell create recipe."
)]
struct ConfigCreateItem {
    #[schemars(title = "Label", description = "Text shown in the create picker.")]
    label: String,
    #[serde(default)]
    #[schemars(
        title = "Icon",
        description = "Optional icon text shown before the label. Nerd Font glyphs and emoji are supported."
    )]
    icon: Option<String>,
    #[serde(default)]
    #[schemars(
        title = "Prompts",
        description = "Input prompts collected before running the shell recipe."
    )]
    prompts: Vec<ConfigCreatePrompt>,
    #[schemars(
        title = "Shell",
        description = "Shell script executed after prompts are collected. Prompt values are exposed as NAVGATOR_CREATE_* environment variables."
    )]
    shell: String,
    #[serde(default)]
    #[schemars(
        title = "Current Directory",
        description = "Optional command working directory. Supports {path} and prompt placeholders."
    )]
    current_dir: Option<String>,
    #[schemars(
        title = "Success Path",
        description = "Path to navigate to after shell success. Supports {path} and prompt placeholders."
    )]
    success_path: String,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(
    title = "Create Prompt",
    description = "One prompt in a create recipe."
)]
struct ConfigCreatePrompt {
    #[schemars(
        title = "Name",
        description = "Prompt placeholder name. Use ASCII letters, numbers, underscore, or dash."
    )]
    name: String,
    #[schemars(title = "Label", description = "Text shown next to the input.")]
    label: String,
    #[serde(default, rename = "type")]
    #[schemars(title = "Type", description = "Prompt input type. Defaults to text.")]
    kind: Option<ConfigCreatePromptKind>,
    #[serde(default)]
    #[schemars(
        title = "Default",
        description = "Optional default value. Supports earlier prompt placeholders and {path}."
    )]
    default: Option<String>,
    #[serde(default)]
    #[schemars(
        title = "Required",
        description = "When true, empty values are rejected. Defaults to false."
    )]
    required: Option<bool>,
}

#[derive(Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum ConfigCreatePromptKind {
    Text,
    Path,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(title = "Action", description = "One action picker item.")]
struct ConfigAction {
    #[serde(default)]
    #[schemars(
        title = "Action ID",
        description = "Optional stable identifier used as a keybinding target.",
        regex(pattern = "^[a-z0-9]+(-[a-z0-9]+)*$")
    )]
    id: Option<String>,
    #[schemars(title = "Label", description = "Text shown in the action picker.")]
    label: String,
    #[serde(default)]
    #[schemars(
        title = "Icon",
        description = "Optional icon text shown before the label. Nerd Font glyphs and emoji are supported."
    )]
    icon: Option<String>,
    #[serde(default)]
    #[schemars(
        title = "File Condition",
        description = "Optional file or directory path that must exist under the selected target path for the action to be shown."
    )]
    file_condition: Option<String>,
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
            id: self.id,
            label: label.to_string(),
            icon: non_empty_trimmed(self.icon),
            file_condition: non_empty_trimmed(self.file_condition),
            kind,
        })
    }
}

impl ConfigCreateItem {
    fn into_create_definition(self) -> Option<CreateDefinition> {
        let label = self.label.trim();
        let shell = self.shell.trim();
        let success_path = self.success_path.trim();
        if label.is_empty() || shell.is_empty() || success_path.is_empty() {
            return None;
        }

        let mut seen = HashSet::new();
        let prompts = self
            .prompts
            .into_iter()
            .filter_map(ConfigCreatePrompt::into_create_prompt)
            .filter(|prompt| seen.insert(prompt.name.clone()))
            .collect::<Vec<CreatePrompt>>();

        Some(CreateDefinition {
            label: label.to_string(),
            icon: non_empty_trimmed(self.icon),
            prompts,
            shell: shell.to_string(),
            current_dir: non_empty_trimmed(self.current_dir),
            success_path: success_path.to_string(),
        })
    }
}

impl ConfigCreatePrompt {
    fn into_create_prompt(self) -> Option<CreatePrompt> {
        let name = self.name.trim().replace('-', "_");
        let label = self.label.trim();
        if !valid_create_prompt_name(&name) || label.is_empty() {
            return None;
        }
        Some(CreatePrompt {
            name,
            label: label.to_string(),
            kind: self
                .kind
                .map(ConfigCreatePromptKind::to_prompt_kind)
                .unwrap_or(CreatePromptKind::Text),
            default: non_empty_trimmed(self.default),
            required: self.required.unwrap_or(false),
        })
    }
}

impl ConfigCreatePromptKind {
    fn to_prompt_kind(self) -> CreatePromptKind {
        match self {
            ConfigCreatePromptKind::Text => CreatePromptKind::Text,
            ConfigCreatePromptKind::Path => CreatePromptKind::Path,
        }
    }
}

fn valid_create_prompt_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn non_empty_trimmed(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
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

pub(crate) fn load_config(config_entries: &[String]) -> AppResult<LoadedConfig> {
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
        state.apply_config_file(config, base_dir, &home, ConfigLayerSource::File)?;
    }

    if !found_config {
        let path = create_default_config(&home)?;
        return load_config_from_created_file(path, config_entries);
    }

    let base_dir = env::current_dir().unwrap_or_else(|_| home.clone());
    for (index, entry) in config_entries.iter().enumerate() {
        let config: ConfigFile = Figment::from(Toml::string(entry))
            .extract()
            .map_err(|err| {
                format!(
                    "Failed to parse config entry {} ({entry:?}): {err}",
                    index + 1
                )
            })?;
        state
            .apply_config_file(config, &base_dir, &home, ConfigLayerSource::Cli)
            .map_err(|err| {
                format!(
                    "Failed to apply config entry {} ({entry:?}): {err}",
                    index + 1
                )
            })?;
    }

    state.into_loaded_config()
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
    fn apply_config_file(
        &mut self,
        config: ConfigFile,
        base_dir: &Path,
        home: &Path,
        source: ConfigLayerSource,
    ) -> AppResult<()> {
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
        if let Some(branches) = config.branches {
            if let Some(value) = branches.on_select {
                self.branch_settings.on_select = value.to_branch_select_behavior();
            }
        }
        if let Some(actions) = config.actions {
            let picker_only_cli_override = source == ConfigLayerSource::Cli
                && actions.picker.is_some()
                && actions.include_defaults.is_none()
                && actions.bindings.is_empty()
                && actions.items.is_empty();
            if picker_only_cli_override {
                self.action_settings.picker = actions.picker;
            } else {
                self.action_settings = action_settings_from_config(actions);
                self.legacy_action_bindings = Some(self.action_settings.picker_bindings.clone());
            }
        }
        if let Some(create) = config.create {
            self.create_settings = create_settings_from_config(create);
            self.legacy_create_bindings = Some(self.create_settings.picker_bindings.clone());
        }
        if let Some(keybindings) = config.keybindings {
            let layer = keybindings.into_keymap()?;
            match source {
                ConfigLayerSource::File => self.keybinding_layer.apply_layer(&layer),
                ConfigLayerSource::Cli => self.cli_keybinding_layer.apply_layer(&layer),
            }
        }
        if let Some(ui) = config.ui {
            if let Some(theme) = ui.theme {
                self.theme_colors = theme.colors();
            }
        }
        Ok(())
    }
}

impl ConfigKeybindings {
    fn into_keymap(self) -> AppResult<Keymap> {
        let mut keymap = Keymap::default();
        for (context, table) in [
            (BindingContext::Global, self.global),
            (BindingContext::Navigator, self.navigator),
            (BindingContext::Preview, self.preview),
            (BindingContext::Detail, self.detail),
            (BindingContext::TagEditor, self.tag_editor),
            (BindingContext::ActionPicker, self.action_picker),
            (BindingContext::CreatePicker, self.create_picker),
            (BindingContext::CreateForm, self.create_form),
            (BindingContext::CreateCompletions, self.create_completions),
            (BindingContext::ProgressOverlay, self.progress_overlay),
            (BindingContext::ErrorOverlay, self.error_overlay),
        ] {
            let Some(table) = table else {
                continue;
            };
            let mut seen = HashSet::new();
            for (raw_chord, raw_target) in table {
                let chord = KeyChord::parse(&raw_chord).map_err(|error| {
                    format!(
                        "Invalid key chord {raw_chord:?} in [keybindings.{}]: {error}",
                        context.as_str()
                    )
                })?;
                if !seen.insert(chord) {
                    return Err(format!(
                        "Duplicate canonical key chord {chord} in [keybindings.{}]",
                        context.as_str()
                    )
                    .into());
                }
                let target = BindingTarget::parse(&raw_target).map_err(|error| {
                    format!(
                        "Invalid keybinding target {raw_target:?} in [keybindings.{}]: {error}",
                        context.as_str()
                    )
                })?;
                keymap.set(context, Binding::new(chord, target));
            }
        }
        Ok(keymap)
    }
}

fn action_settings_from_config(config: ConfigActions) -> ActionSettings {
    let include_defaults = config.include_defaults.unwrap_or(false);
    let picker = config.picker;
    let mut items = Vec::new();
    if include_defaults {
        items.extend(default_action_definitions());
    }
    items.extend(
        config
            .items
            .into_iter()
            .filter_map(ConfigAction::into_action_definition),
    );
    if items.is_empty() {
        items = default_action_definitions();
    }
    let picker_bindings = action_bindings_from_config(config.bindings);
    ActionSettings {
        items,
        picker,
        picker_bindings,
    }
}

fn create_settings_from_config(config: ConfigCreate) -> CreateSettings {
    let include_defaults = config.include_defaults.unwrap_or(false);
    let mut items = Vec::new();
    if include_defaults {
        items.extend(default_create_definitions());
    }
    items.extend(
        config
            .items
            .into_iter()
            .filter_map(ConfigCreateItem::into_create_definition),
    );
    if items.is_empty() {
        items = default_create_definitions();
    }
    let picker_bindings = create_bindings_from_config(config.bindings);
    CreateSettings {
        items,
        picker_bindings,
    }
}

fn create_bindings_from_config(values: Vec<String>) -> Vec<ActionBinding> {
    let mut bindings = values
        .into_iter()
        .filter_map(|value| create_binding_from_string(&value))
        .collect::<Vec<ActionBinding>>();
    if bindings.is_empty() {
        bindings = default_create_picker_bindings();
    }
    bindings
}

fn action_bindings_from_config(values: Vec<String>) -> Vec<ActionBinding> {
    let mut bindings = values
        .into_iter()
        .filter_map(|value| action_binding_from_string(&value))
        .collect::<Vec<ActionBinding>>();
    if bindings.is_empty() {
        bindings = default_action_picker_bindings();
    }
    bindings
}

fn action_binding_from_string(value: &str) -> Option<ActionBinding> {
    let normalized = value.trim().to_lowercase().replace('_', "-");
    match normalized.as_str() {
        "ctrl-enter" | "control-enter" | "c-enter" => Some(ActionBinding {
            label: "Ctrl+Enter".to_string(),
            key: ActionBindingKey::CtrlEnter,
        }),
        "ctrl-space" | "control-space" | "ctrl-spacebar" | "control-spacebar" | "c-space" => {
            Some(ActionBinding {
                label: "Ctrl+Space".to_string(),
                key: ActionBindingKey::CtrlSpace,
            })
        }
        _ => None,
    }
}

fn create_binding_from_string(value: &str) -> Option<ActionBinding> {
    let normalized = value.trim().to_lowercase().replace('_', "-");
    match normalized.as_str() {
        "ctrl-n" | "control-n" | "c-n" => Some(ActionBinding {
            label: "Ctrl+N".to_string(),
            key: ActionBindingKey::CtrlN,
        }),
        _ => None,
    }
}

fn validate_action_ids(items: &[ActionDefinition]) -> AppResult<()> {
    let mut seen = HashSet::new();
    for item in items {
        let Some(id) = item.id.as_deref() else {
            continue;
        };
        if !is_valid_action_id(id) {
            return Err(format!("Invalid action ID {id:?} for action {:?}", item.label).into());
        }
        if id == "none" || CoreAction::parse(id).is_some() {
            return Err(format!("Action ID {id:?} is reserved").into());
        }
        if !seen.insert(id) {
            return Err(format!("Duplicate action ID {id:?}").into());
        }
    }
    Ok(())
}

fn validate_picker_action_ids(settings: &ActionSettings) -> AppResult<()> {
    let Some(picker) = settings.picker.as_deref() else {
        return Ok(());
    };
    let action_ids = settings
        .items
        .iter()
        .filter_map(|item| item.id.as_deref())
        .collect::<HashSet<_>>();
    let mut seen = HashSet::new();
    for id in picker {
        if !seen.insert(id.as_str()) {
            return Err(format!("Duplicate action picker ID {id:?}").into());
        }
        if !action_ids.contains(id.as_str()) {
            return Err(format!("Unknown action picker ID {id:?}").into());
        }
    }
    Ok(())
}

fn validate_keymap(keymap: &Keymap, items: &[ActionDefinition]) -> AppResult<()> {
    let action_ids = items
        .iter()
        .filter_map(|item| item.id.as_deref())
        .collect::<HashSet<_>>();
    keymap
        .validate_targets(|context, target| {
            if !target_is_compatible(context, target) {
                return Err(format!(
                    "target {:?} is not valid in context {:?}",
                    target.as_str(),
                    context.as_str()
                ));
            }
            if let BindingTarget::Configured(id) = target {
                if !action_ids.contains(id.as_str()) {
                    return Err(format!("unknown action ID {id:?}"));
                }
            }
            Ok(())
        })
        .map_err(Into::into)
}

fn apply_legacy_action_bindings(keymap: &mut Keymap, bindings: &[ActionBinding]) {
    let actions = BindingTarget::Core(CoreAction::Actions);
    let run_and_close = BindingTarget::Core(CoreAction::RunAndClose);
    keymap.remove_target(BindingContext::Navigator, &actions);
    keymap.remove_target(BindingContext::ActionPicker, &run_and_close);
    for binding in bindings {
        let Some(chord) = legacy_binding_chord(&binding.key) else {
            continue;
        };
        keymap.set(
            BindingContext::Navigator,
            Binding::new(chord, actions.clone()),
        );
        keymap.set(
            BindingContext::ActionPicker,
            Binding::new(chord, run_and_close.clone()),
        );
    }
}

fn apply_legacy_create_bindings(keymap: &mut Keymap, bindings: &[ActionBinding]) {
    let create = BindingTarget::Core(CoreAction::Create);
    keymap.remove_target(BindingContext::Navigator, &create);
    for binding in bindings {
        let Some(chord) = legacy_binding_chord(&binding.key) else {
            continue;
        };
        keymap.set(
            BindingContext::Navigator,
            Binding::new(chord, create.clone()),
        );
    }
}

fn legacy_binding_chord(key: &ActionBindingKey) -> Option<KeyChord> {
    let value = match key {
        ActionBindingKey::CtrlEnter => "ctrl+enter",
        ActionBindingKey::CtrlSpace => "ctrl+space",
        ActionBindingKey::CtrlN => "ctrl+n",
    };
    KeyChord::parse(value).ok()
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

fn load_config_from_created_file(
    _path: PathBuf,
    config_entries: &[String],
) -> AppResult<LoadedConfig> {
    load_config(config_entries)
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
    let create = default_create_config_contents();
    let keybindings = default_keybindings_config_contents();
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

[branches]
on_select = "worktree"

[actions]
{actions}

[create]
{create}

[ui]
theme = "auto"

[preview]
shorten_worktree_tab_labels = true
worktree_tab_min_chars = 6
selected_worktree_tab_min_chars = 10

{keybindings}
"#
    )
}

fn default_actions_config_contents() -> String {
    let mut output = String::new();
    for action in default_action_definitions() {
        output.push_str("\n[[actions.items]]\n");
        if let Some(id) = &action.id {
            output.push_str(&format!("id = {}\n", toml_string(id)));
        }
        output.push_str(&format!("label = {}\n", toml_string(&action.label)));
        if let Some(icon) = &action.icon {
            output.push_str(&format!("icon = {}\n", toml_string(icon)));
        }
        if let Some(file_condition) = &action.file_condition {
            output.push_str(&format!(
                "file_condition = {}\n",
                toml_string(file_condition)
            ));
        }
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

fn default_keybindings_config_contents() -> String {
    let keymap = default_keymap();
    let mut output = String::new();
    for &context in BindingContext::ordered() {
        output.push_str(&format!("[keybindings.{}]\n", context.as_str()));
        for binding in keymap.bindings_for_context(context) {
            output.push_str(&format!(
                "{} = {}\n",
                toml_string(&binding.chord.as_str()),
                toml_string(binding.target.as_str())
            ));
        }
        output.push('\n');
    }
    output
}

fn default_create_config_contents() -> String {
    let mut output = String::new();
    for item in default_create_definitions() {
        output.push_str("\n[[create.items]]\n");
        output.push_str(&format!("label = {}\n", toml_string(&item.label)));
        if let Some(icon) = &item.icon {
            output.push_str(&format!("icon = {}\n", toml_string(icon)));
        }
        output.push_str(&format!("shell = {}\n", toml_string(&item.shell)));
        if let Some(current_dir) = &item.current_dir {
            output.push_str(&format!("current_dir = {}\n", toml_string(current_dir)));
        }
        output.push_str(&format!(
            "success_path = {}\n",
            toml_string(&item.success_path)
        ));
        for prompt in item.prompts {
            output.push_str("\n[[create.items.prompts]]\n");
            output.push_str(&format!("name = {}\n", toml_string(&prompt.name)));
            output.push_str(&format!("label = {}\n", toml_string(&prompt.label)));
            output.push_str(&format!(
                "type = {}\n",
                toml_string(create_prompt_kind_name(prompt.kind))
            ));
            if let Some(default) = &prompt.default {
                output.push_str(&format!("default = {}\n", toml_string(default)));
            }
            output.push_str(&format!("required = {}\n", prompt.required));
        }
    }
    output
}

fn create_prompt_kind_name(kind: CreatePromptKind) -> &'static str {
    match kind {
        CreatePromptKind::Text => "text",
        CreatePromptKind::Path => "path",
    }
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
        action_settings_from_config, config_paths, config_schema_json, create_settings_from_config,
        default_config_contents, ensure_schema_link_in_config_file, load_config,
        validate_picker_action_ids, ConfigAction, ConfigActionKind, ConfigActions, ConfigCreate,
        ConfigCreateItem, ConfigCreatePrompt, ConfigCreatePromptKind, ConfigFile,
        ConfigLayerSource, ConfigLoadState, CONFIG_SCHEMA_URL,
    };
    use crate::model::keybindings::{BindingContext, BindingTarget, CoreAction, KeyChord};
    use crate::model::{
        ActionBindingKey, ActionKind, ActionSettings, BranchSelectBehavior, CreatePromptKind,
        SortMode,
    };
    use figment::providers::{Format, Toml};
    use figment::Figment;
    use std::{env, fs, path::PathBuf, sync::Mutex};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn checked_in_schema_matches_generated_schema() {
        let generated = config_schema_json().expect("generated schema");
        assert_eq!(
            generated.trim(),
            include_str!("../config-schema.json").trim()
        );
    }

    #[test]
    fn written_default_config_expands_default_actions() {
        let config = default_config_contents();

        assert!(!config.contains("bindings = ["));
        assert!(config.contains("id = \"navigate-to\""));
        assert!(config.contains("id = \"open-vs-code\""));
        assert!(config.contains("label = \"Navigate to\""));
        assert!(config.contains("icon = \"󰁔\""));
        assert!(config.contains("label = \"Open IntelliJ\""));
        assert!(config.contains("command = \"idea\""));
        assert!(config.contains("args = [\".\"]"));
        assert!(config.contains("current_dir = \"{path}\""));
        assert!(config.contains("label = \"Open repo online\""));
        assert!(config.contains("file_condition = \".git\""));
        assert!(config.contains("url = \"{github_url}\""));
        assert!(config.contains("[branches]"));
        assert!(config.contains("on_select = \"worktree\""));
        assert!(config.contains("[create]"));
        assert!(config.contains("label = \"New project\""));
        assert!(config.contains("success_path = \"{parent}/{name}\""));
        assert!(config.contains("label = \"Parent folder\""));
        assert!(config.contains("type = \"path\""));
        assert!(config.contains("[keybindings.navigator]"));
        assert!(config.contains("\"ctrl+enter\" = \"actions\""));
        assert!(config.contains("[keybindings.error-overlay]"));
    }

    #[test]
    fn written_default_config_parses() {
        let config = toml_config(&default_config_contents());

        let action_ids = config
            .actions
            .expect("actions")
            .items
            .into_iter()
            .map(|action| action.id.expect("built-in action ID"))
            .collect::<Vec<_>>();
        assert_eq!(
            action_ids,
            vec![
                "navigate-to",
                "open-github-desktop",
                "open-vs-code",
                "open-intellij",
                "open-repo-online",
                "open-claude-session",
                "open-opencode-session",
            ]
        );
        assert!(config.create.is_some());
        assert!(config.branches.is_some());
        let keybindings = config.keybindings.expect("keybindings");
        assert!(keybindings.global.is_some());
        assert!(keybindings.navigator.is_some());
        assert!(keybindings.preview.is_some());
        assert!(keybindings.detail.is_some());
        assert!(keybindings.tag_editor.is_some());
        assert!(keybindings.action_picker.is_some());
        assert!(keybindings.create_picker.is_some());
        assert!(keybindings.create_form.is_some());
        assert!(keybindings.create_completions.is_some());
        assert!(keybindings.progress_overlay.is_some());
        assert!(keybindings.error_overlay.is_some());
    }

    #[test]
    fn branch_config_parses_checkout_behavior() {
        let mut state = ConfigLoadState::new();
        state
            .apply_config_file(
                toml_config(
                    r#"
[branches]
on_select = "checkout"
"#,
                ),
                &PathBuf::from("/tmp"),
                &PathBuf::from("/home/example"),
                ConfigLayerSource::File,
            )
            .expect("apply config");

        assert_eq!(
            state.branch_settings.on_select,
            BranchSelectBehavior::Checkout
        );
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
            include_defaults: None,
            picker: None,
            bindings: Vec::new(),
            items: vec![ConfigAction {
                id: None,
                label: "".to_string(),
                icon: None,
                file_condition: None,
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
    fn action_config_uses_listed_items_without_defaults_by_default() {
        let settings = action_settings_from_config(ConfigActions {
            include_defaults: None,
            picker: None,
            bindings: vec!["ctrl-space".to_string()],
            items: vec![ConfigAction {
                id: Some("only-custom".to_string()),
                label: "Only custom".to_string(),
                icon: None,
                file_condition: None,
                kind: ConfigActionKind::Navigate,
            }],
        });

        assert_eq!(settings.items.len(), 1);
        assert_eq!(settings.items[0].label, "Only custom");
        assert_eq!(settings.picker_bindings[0].label, "Ctrl+Space");
    }

    #[test]
    fn picker_action_ids_must_be_known_and_unique() {
        let mut settings = ActionSettings {
            picker: Some(vec!["open-vs-code".to_string()]),
            ..ActionSettings::default()
        };
        assert!(validate_picker_action_ids(&settings).is_ok());

        settings.picker = Some(vec!["missing".to_string()]);
        assert!(validate_picker_action_ids(&settings).is_err());

        settings.picker = Some(vec!["open-vs-code".to_string(), "open-vs-code".to_string()]);
        assert!(validate_picker_action_ids(&settings).is_err());
    }

    #[test]
    fn create_config_uses_custom_recipe_and_prompt_types() {
        let settings = create_settings_from_config(ConfigCreate {
            include_defaults: None,
            bindings: vec!["ctrl-n".to_string()],
            items: vec![ConfigCreateItem {
                label: "Clone".to_string(),
                icon: None,
                prompts: vec![ConfigCreatePrompt {
                    name: "target-path".to_string(),
                    label: "Target".to_string(),
                    kind: Some(ConfigCreatePromptKind::Path),
                    default: Some("~/Github/example".to_string()),
                    required: Some(true),
                }],
                shell: "mkdir -p \"$NAVGATOR_CREATE_TARGET_PATH\"".to_string(),
                current_dir: None,
                success_path: "{target_path}".to_string(),
            }],
        });

        assert_eq!(settings.items.len(), 1);
        assert_eq!(settings.items[0].label, "Clone");
        assert_eq!(settings.items[0].prompts[0].name, "target_path");
        assert_eq!(settings.items[0].prompts[0].kind, CreatePromptKind::Path);
        assert!(matches!(
            settings.picker_bindings[0].key,
            ActionBindingKey::CtrlN
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

        state
            .apply_config_file(first, &base, &home, ConfigLayerSource::File)
            .expect("apply first config");
        state
            .apply_config_file(second, &base, &home, ConfigLayerSource::File)
            .expect("apply second config");

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

    #[test]
    fn keybindings_merge_canonical_chords_and_none_consumes_defaults() {
        let mut state = ConfigLoadState::new();
        apply_toml(
            &mut state,
            r#"
[keybindings.navigator]
"control+enter" = "create"
enter = "none"
"#,
            ConfigLayerSource::File,
        );
        apply_toml(
            &mut state,
            r#"
[keybindings.navigator]
"ctrl+enter" = "actions"
"#,
            ConfigLayerSource::File,
        );

        let loaded = state.into_loaded_config().expect("final config");
        assert_eq!(
            target(&loaded.keymap, BindingContext::Navigator, "ctrl+enter"),
            Some(&BindingTarget::Core(CoreAction::Actions))
        );
        assert_eq!(
            target(&loaded.keymap, BindingContext::Navigator, "enter"),
            Some(&BindingTarget::Disabled)
        );
    }

    #[test]
    fn canonical_duplicates_in_one_keybinding_table_fail() {
        let config = toml_config(
            r#"
[keybindings.navigator]
"ctrl+enter" = "actions"
"control+enter" = "create"
"#,
        );
        let mut state = ConfigLoadState::new();

        let error = state
            .apply_config_file(
                config,
                &PathBuf::from("/tmp"),
                &PathBuf::from("/home/example"),
                ConfigLayerSource::File,
            )
            .expect_err("canonical duplicate should fail")
            .to_string();

        assert!(error.contains("Duplicate canonical key chord ctrl+enter"));
    }

    #[test]
    fn unknown_keybinding_context_fails_deserialization() {
        let error = Figment::from(Toml::string(
            r#"
[keybindings.typo]
enter = "navigate"
"#,
        ))
        .extract::<ConfigFile>()
        .err()
        .expect("unknown key should fail")
        .to_string();

        assert!(!error.is_empty());
    }

    #[test]
    fn action_ids_reject_invalid_duplicate_and_reserved_values() {
        for (contents, expected) in [
            (
                r#"
[[actions.items]]
id = "Invalid"
label = "Invalid"
type = "navigate"
"#,
                "Invalid action ID",
            ),
            (
                r#"
[[actions.items]]
id = "same"
label = "One"
type = "navigate"

[[actions.items]]
id = "same"
label = "Two"
type = "navigate"
"#,
                "Duplicate action ID",
            ),
            (
                r#"
[[actions.items]]
id = "navigate"
label = "Reserved"
type = "navigate"
"#,
                "reserved",
            ),
            (
                r#"
[[actions.items]]
id = "none"
label = "Reserved"
type = "navigate"
"#,
                "reserved",
            ),
        ] {
            let mut state = ConfigLoadState::new();
            apply_toml(&mut state, contents, ConfigLayerSource::File);
            let error = state
                .into_loaded_config()
                .err()
                .expect("invalid ID should fail")
                .to_string();
            assert!(error.contains(expected), "{error}");
        }
    }

    #[test]
    fn invalid_action_id_in_replaced_items_does_not_fail() {
        let mut state = ConfigLoadState::new();
        apply_toml(
            &mut state,
            r#"
[[actions.items]]
id = "Invalid"
label = "Replaced"
type = "navigate"
"#,
            ConfigLayerSource::File,
        );
        apply_toml(
            &mut state,
            r#"
[[actions.items]]
id = "valid"
label = "Effective"
type = "navigate"
"#,
            ConfigLayerSource::File,
        );

        let loaded = state.into_loaded_config().expect("effective ID is valid");
        assert_eq!(loaded.action_settings.items[0].id.as_deref(), Some("valid"));
    }

    #[test]
    fn picker_only_action_may_omit_id_but_known_targets_resolve() {
        let mut state = ConfigLoadState::new();
        apply_toml(
            &mut state,
            r#"
[[actions.items]]
label = "Picker only"
type = "navigate"

[[actions.items]]
id = "direct-action"
label = "Direct"
type = "navigate"

[keybindings.navigator]
x = "direct-action"
"#,
            ConfigLayerSource::File,
        );

        let loaded = state.into_loaded_config().expect("valid action target");
        assert_eq!(loaded.action_settings.items[0].id, None);
        assert_eq!(
            target(&loaded.keymap, BindingContext::Navigator, "x"),
            Some(&BindingTarget::Configured("direct-action".to_string()))
        );
    }

    #[test]
    fn unknown_and_context_incompatible_targets_fail_finalization() {
        for (contents, expected) in [
            (
                r#"
[keybindings.navigator]
x = "missing-action"
"#,
                "unknown action ID",
            ),
            (
                r#"
[keybindings.action-picker]
x = "actions"
"#,
                "not valid in context",
            ),
            (
                r#"
[keybindings.navigator]
x = "run"
"#,
                "not valid in context",
            ),
        ] {
            let mut state = ConfigLoadState::new();
            apply_toml(&mut state, contents, ConfigLayerSource::File);
            let error = state
                .into_loaded_config()
                .err()
                .expect("target should fail")
                .to_string();
            assert!(error.contains(expected), "{error}");
        }
    }

    #[test]
    fn legacy_replacements_run_before_new_keybindings() {
        let mut state = ConfigLoadState::new();
        apply_toml(
            &mut state,
            r#"
[actions]
bindings = ["ctrl-space"]

[create]
bindings = ["invalid"]

[keybindings.navigator]
"ctrl+space" = "none"
"#,
            ConfigLayerSource::File,
        );

        let loaded = state.into_loaded_config().expect("legacy config");
        assert_eq!(
            target(&loaded.keymap, BindingContext::Navigator, "ctrl+enter"),
            None
        );
        assert_eq!(
            target(&loaded.keymap, BindingContext::ActionPicker, "ctrl+enter"),
            None
        );
        assert_eq!(
            target(&loaded.keymap, BindingContext::Navigator, "ctrl+space"),
            Some(&BindingTarget::Disabled)
        );
        assert_eq!(
            target(&loaded.keymap, BindingContext::Navigator, "ctrl+n"),
            Some(&BindingTarget::Core(CoreAction::Create))
        );
    }

    #[test]
    fn repeated_config_entries_apply_in_order_and_report_invalid_toml() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let path = env::temp_dir().join(format!(
            "navgator-config-entry-test-{}.toml",
            std::process::id()
        ));
        fs::write(&path, "[paths]\nstatic_items = []\n").expect("write config");
        let old_navgator = env::var("NAVGATOR_CONFIG").ok();
        env::set_var("NAVGATOR_CONFIG", &path);

        let entries = vec![
            "keybindings.navigator.enter=\"actions\"".to_string(),
            "keybindings.navigator.enter=\"none\"".to_string(),
        ];
        let loaded = load_config(&entries).expect("config entries");
        assert_eq!(
            target(&loaded.keymap, BindingContext::Navigator, "enter"),
            Some(&BindingTarget::Disabled)
        );

        let error = load_config(&["keybindings.navigator.enter=[".to_string()])
            .err()
            .expect("invalid TOML should fail")
            .to_string();

        restore_env("NAVGATOR_CONFIG", old_navgator);
        let _ = fs::remove_file(path);
        assert!(error.contains("Failed to parse config entry 1"), "{error}");
        assert!(error.contains("keybindings.navigator.enter=["), "{error}");
    }

    #[test]
    fn cli_picker_override_preserves_configured_actions() {
        let mut state = ConfigLoadState::new();
        apply_toml(
            &mut state,
            r#"
[actions]
include_defaults = true

[[actions.items]]
id = "custom-action"
label = "Custom action"
type = "navigate"
"#,
            ConfigLayerSource::File,
        );
        apply_toml(
            &mut state,
            r#"actions.picker = ["custom-action"]"#,
            ConfigLayerSource::Cli,
        );

        let loaded = state.into_loaded_config().expect("picker override");
        assert!(loaded
            .action_settings
            .items
            .iter()
            .any(|action| action.id.as_deref() == Some("custom-action")));
        assert_eq!(
            loaded.action_settings.picker,
            Some(vec!["custom-action".to_string()])
        );
    }

    fn apply_toml(state: &mut ConfigLoadState, contents: &str, source: ConfigLayerSource) {
        state
            .apply_config_file(
                toml_config(contents),
                &PathBuf::from("/tmp"),
                &PathBuf::from("/home/example"),
                source,
            )
            .expect("apply config");
    }

    fn target<'a>(
        keymap: &'a crate::model::keybindings::Keymap,
        context: BindingContext,
        chord: &str,
    ) -> Option<&'a BindingTarget> {
        let chord = KeyChord::parse(chord).expect("test chord");
        keymap
            .bindings_for_context(context)
            .iter()
            .find(|binding| binding.chord == chord)
            .map(|binding| &binding.target)
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
