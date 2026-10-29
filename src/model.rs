#![allow(dead_code)]

use ratatui::{layout::Rect, style::Color, text::Text};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, error::Error, path::PathBuf};

#[path = "keybindings.rs"]
pub mod keybindings;

use keybindings::Keymap;

pub type AppResult<T> = Result<T, Box<dyn Error>>;
pub use gator::search::MatchScore;

pub const DATE_WIDTH: usize = 16;
pub const DATE_PLACEHOLDER: &str = "---- -- -- --:--";
pub const TAB_DIVIDER_WIDTH: usize = 3;
pub const DEFAULT_WORKTREE_TAB_MIN_CHARS: usize = 6;
pub const DEFAULT_SELECTED_WORKTREE_TAB_MIN_CHARS: usize = 10;
pub const MIN_PARTIAL_TAB_WIDTH: usize = 4;
pub const CONFIG_SCHEMA_URL: &str =
    "https://raw.githubusercontent.com/Yarden-zamir/navgator/main/config-schema.json";

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProviderId(pub String);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResultId(pub String);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentId(pub String);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MetadataKey(pub String);

#[derive(Clone, Debug)]
pub enum SelectionValue {
    Path(PathBuf),
    Url(String),
    Text(String),
    ProviderSpecific {
        provider_id: ProviderId,
        value: String,
    },
}

#[derive(Clone, Debug)]
pub enum MetadataValue {
    Text(String),
    Number(i64),
    Decimal(f64),
    Bool(bool),
    DateTime(i64),
    Tags(Vec<String>),
    List(Vec<MetadataValue>),
}

#[derive(Clone, Debug)]
pub struct MetadataEntry {
    pub key: MetadataKey,
    pub value: MetadataValue,
    pub display: Option<String>,
    pub sort_value: Option<MetadataValue>,
}

pub type MetadataMap = BTreeMap<MetadataKey, MetadataEntry>;

#[derive(Clone, Debug)]
pub struct ResultEntry {
    pub id: ResultId,
    pub provider_id: ProviderId,
    pub display: String,
    pub metadata: MetadataMap,
}

#[derive(Clone, Debug)]
pub struct ContentTarget {
    pub id: ContentId,
    pub source_result_id: ResultId,
    pub provider_id: ProviderId,
    pub display: String,
    pub metadata: MetadataMap,
    pub selection_value: SelectionValue,
}

#[derive(Clone, Debug)]
pub enum ContentBlock {
    Text { lines: Vec<String> },
    List { items: Vec<String> },
    Tree { lines: Vec<String> },
    Empty { message: String },
    Loading { message: String },
    Error { message: String },
}

#[derive(Clone)]
pub struct PreviewTab {
    pub path: String,
    pub label: String,
    pub text: Text<'static>,
    pub git: Option<Text<'static>>,
    pub github_readme: Option<Text<'static>>,
}

#[derive(Clone)]
pub struct PreviewData {
    pub previews: Vec<PreviewTab>,
    pub selected_repo_is_bare: bool,
    pub git_loaded: bool,
    pub github_readme_loaded: bool,
}

pub struct PreviewTarget {
    pub path: String,
    pub label: String,
}

pub struct PreviewTargets {
    pub tabs: Vec<PreviewTarget>,
    pub selected_repo_is_bare: bool,
}

pub struct GitWorktree {
    pub path: String,
    pub branch: Option<String>,
    pub detached: bool,
    pub bare: bool,
}

#[derive(Clone, Copy)]
pub struct PreviewSettings {
    pub shorten_worktree_tab_labels: bool,
    pub worktree_tab_min_chars: usize,
    pub selected_worktree_tab_min_chars: usize,
}

pub fn default_preview_settings() -> PreviewSettings {
    PreviewSettings {
        shorten_worktree_tab_labels: true,
        worktree_tab_min_chars: DEFAULT_WORKTREE_TAB_MIN_CHARS,
        selected_worktree_tab_min_chars: DEFAULT_SELECTED_WORKTREE_TAB_MIN_CHARS,
    }
}

#[derive(Clone, Copy)]
pub struct PreviewColors {
    pub accent: Color,
    pub muted: Color,
    pub text: Color,
}

#[derive(Clone, Copy, Default)]
pub struct SortMeta {
    pub modified_epoch: Option<i64>,
    pub created_epoch: Option<i64>,
    pub accessed_epoch: Option<i64>,
}

impl SortMeta {
    pub fn recent_epoch(self) -> Option<i64> {
        match (self.modified_epoch, self.accessed_epoch) {
            (Some(modified), Some(accessed)) => Some(modified.max(accessed)),
            (Some(modified), None) => Some(modified),
            (None, Some(accessed)) => Some(accessed),
            (None, None) => None,
        }
    }
}

pub struct MetaResult {
    pub path: String,
    pub display: Option<String>,
    pub modified_epoch: Option<i64>,
    pub created_epoch: Option<i64>,
}

pub struct TagResult {
    pub path: String,
    pub tags: Vec<String>,
}

pub struct PreviewResult {
    pub path: String,
    pub data: PreviewData,
}

pub struct GitResult {
    pub path: String,
    pub tab_index: usize,
    pub git: Option<Text<'static>>,
    pub done: bool,
}

pub struct GithubReadmeResult {
    pub path: String,
    pub tab_index: usize,
    pub readme: Option<Text<'static>>,
    pub done: bool,
}

#[derive(Clone)]
pub struct DetailTab {
    pub label: String,
    pub text: Text<'static>,
}

pub struct BuildItemsResult {
    pub entries: Vec<NavigateEntry>,
    pub preview_settings: PreviewSettings,
    pub sort_settings: SortSettings,
    pub remote_settings: RemoteSettings,
    pub branch_settings: BranchSettings,
    pub action_settings: ActionSettings,
    pub create_settings: CreateSettings,
    pub theme_colors: ThemeColors,
    pub keymap: Keymap,
}

pub enum ResultUpdate {
    Entries {
        entries: Vec<NavigateEntry>,
    },
    ReplaceProviderEntries {
        provider_prefix: String,
        entries: Vec<NavigateEntry>,
    },
    Status {
        provider_id: String,
        message: String,
    },
}

pub struct LoadedConfig {
    pub index_folders: Vec<PathBuf>,
    pub static_items: Vec<PathBuf>,
    pub preview_settings: PreviewSettings,
    pub sort_settings: SortSettings,
    pub remote_settings: RemoteSettings,
    pub branch_settings: BranchSettings,
    pub action_settings: ActionSettings,
    pub create_settings: CreateSettings,
    pub theme_colors: ThemeColors,
    pub keymap: Keymap,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActionSettings {
    pub items: Vec<ActionDefinition>,
    pub picker: Option<Vec<String>>,
    pub picker_bindings: Vec<ActionBinding>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActionBinding {
    pub label: String,
    pub key: ActionBindingKey,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActionBindingKey {
    CtrlEnter,
    CtrlSpace,
    CtrlN,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateSettings {
    pub items: Vec<CreateDefinition>,
    pub picker_bindings: Vec<ActionBinding>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateDefinition {
    pub label: String,
    pub icon: Option<String>,
    pub prompts: Vec<CreatePrompt>,
    pub shell: String,
    pub current_dir: Option<String>,
    pub success_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreatePrompt {
    pub name: String,
    pub label: String,
    pub kind: CreatePromptKind,
    pub default: Option<String>,
    pub required: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreatePromptKind {
    Text,
    Path,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActionDefinition {
    pub id: Option<String>,
    pub label: String,
    pub icon: Option<String>,
    pub file_condition: Option<String>,
    pub kind: ActionKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActionKind {
    Navigate,
    Command {
        command: String,
        args: Vec<String>,
        current_dir: Option<String>,
    },
    OpenUrl {
        url: String,
    },
}

impl Default for ActionSettings {
    fn default() -> Self {
        Self {
            items: default_action_definitions(),
            picker: None,
            picker_bindings: default_action_picker_bindings(),
        }
    }
}

impl Default for CreateSettings {
    fn default() -> Self {
        Self {
            items: default_create_definitions(),
            picker_bindings: default_create_picker_bindings(),
        }
    }
}

pub fn default_action_picker_bindings() -> Vec<ActionBinding> {
    vec![
        ActionBinding {
            label: "Ctrl+Enter".to_string(),
            key: ActionBindingKey::CtrlEnter,
        },
        ActionBinding {
            label: "Ctrl+Space".to_string(),
            key: ActionBindingKey::CtrlSpace,
        },
    ]
}

pub fn default_create_picker_bindings() -> Vec<ActionBinding> {
    vec![ActionBinding {
        label: "Ctrl+N".to_string(),
        key: ActionBindingKey::CtrlN,
    }]
}

pub fn default_action_definitions() -> Vec<ActionDefinition> {
    vec![
        ActionDefinition {
            id: Some("navigate-to".to_string()),
            label: "Navigate to".to_string(),
            icon: Some("󰁔".to_string()),
            file_condition: None,
            kind: ActionKind::Navigate,
        },
        ActionDefinition {
            id: Some("open-github-desktop".to_string()),
            label: "Open GitHub Desktop".to_string(),
            icon: Some("󰊢".to_string()),
            file_condition: Some(".git".to_string()),
            kind: ActionKind::Command {
                command: "open".to_string(),
                args: vec![
                    "-a".to_string(),
                    "GitHub Desktop".to_string(),
                    "{path}".to_string(),
                ],
                current_dir: None,
            },
        },
        ActionDefinition {
            id: Some("open-vs-code".to_string()),
            label: "Open VS Code".to_string(),
            icon: Some("󰨞".to_string()),
            file_condition: None,
            kind: ActionKind::Command {
                command: "open".to_string(),
                args: vec![
                    "-a".to_string(),
                    "Visual Studio Code".to_string(),
                    "{path}".to_string(),
                ],
                current_dir: None,
            },
        },
        ActionDefinition {
            id: Some("open-intellij".to_string()),
            label: "Open IntelliJ".to_string(),
            icon: Some("".to_string()),
            file_condition: None,
            kind: ActionKind::Command {
                command: "idea".to_string(),
                args: vec![".".to_string()],
                current_dir: Some("{path}".to_string()),
            },
        },
        ActionDefinition {
            id: Some("open-repo-online".to_string()),
            label: "Open repo online".to_string(),
            icon: Some("󰖟".to_string()),
            file_condition: Some(".git".to_string()),
            kind: ActionKind::OpenUrl {
                url: "{github_url}".to_string(),
            },
        },
        ActionDefinition {
            id: Some("open-claude-session".to_string()),
            label: "Open Claude session".to_string(),
            icon: Some("󰚩".to_string()),
            file_condition: None,
            kind: ActionKind::Command {
                command: "claude".to_string(),
                args: Vec::new(),
                current_dir: Some("{path}".to_string()),
            },
        },
    ]
}

pub fn default_create_definitions() -> Vec<CreateDefinition> {
    vec![
        CreateDefinition {
            label: "New project".to_string(),
            icon: Some("󰉋".to_string()),
            prompts: vec![
                CreatePrompt {
                    name: "name".to_string(),
                    label: "Project name".to_string(),
                    kind: CreatePromptKind::Text,
                    default: None,
                    required: true,
                },
                CreatePrompt {
                    name: "parent".to_string(),
                    label: "Parent folder".to_string(),
                    kind: CreatePromptKind::Path,
                    default: Some("~/Github".to_string()),
                    required: true,
                },
            ],
            shell: "mkdir -p \"$NAVGATOR_CREATE_PARENT/$NAVGATOR_CREATE_NAME\"".to_string(),
            current_dir: None,
            success_path: "{parent}/{name}".to_string(),
        },
        CreateDefinition {
            label: "New branch + worktree".to_string(),
            icon: Some("".to_string()),
            prompts: vec![
                CreatePrompt {
                    name: "branch".to_string(),
                    label: "Branch name".to_string(),
                    kind: CreatePromptKind::Text,
                    default: None,
                    required: true,
                },
                CreatePrompt {
                    name: "base".to_string(),
                    label: "Base branch".to_string(),
                    kind: CreatePromptKind::Text,
                    default: Some("main".to_string()),
                    required: true,
                },
                CreatePrompt {
                    name: "target".to_string(),
                    label: "Worktree path".to_string(),
                    kind: CreatePromptKind::Path,
                    default: Some("../{branch}".to_string()),
                    required: true,
                },
            ],
            shell: "git worktree add -b \"$NAVGATOR_CREATE_BRANCH\" \"$NAVGATOR_CREATE_TARGET\" \"$NAVGATOR_CREATE_BASE\"".to_string(),
            current_dir: Some("{path}".to_string()),
            success_path: "{target}".to_string(),
        },
    ]
}

#[derive(Clone, Copy)]
pub struct ThemeColors {
    pub accent: Color,
    pub warm: Color,
    pub key_color: Color,
    pub text: Color,
    pub muted: Color,
}

impl ThemeColors {
    pub fn light() -> Self {
        Self::from(gator::theme::Palette::light())
    }

    pub fn dark() -> Self {
        Self::from(gator::theme::Palette::dark())
    }
}

impl From<gator::theme::Palette> for ThemeColors {
    fn from(palette: gator::theme::Palette) -> Self {
        Self {
            accent: palette.accent,
            warm: palette.warm,
            key_color: palette.key,
            text: palette.text,
            muted: palette.muted,
        }
    }
}

#[derive(Clone)]
pub struct SortSettings {
    pub default_mode: SortMode,
    pub pin_current_project: bool,
    configured_ignored_paths:
        std::collections::BTreeMap<SortMode, std::collections::HashSet<String>>,
    ignored_paths: std::collections::BTreeMap<SortMode, std::collections::HashSet<String>>,
}

impl Default for SortSettings {
    fn default() -> Self {
        Self {
            default_mode: SortMode::Recents,
            pin_current_project: true,
            configured_ignored_paths: std::collections::BTreeMap::new(),
            ignored_paths: std::collections::BTreeMap::new(),
        }
    }
}

impl SortSettings {
    pub fn ignored_paths(&self, mode: SortMode) -> Option<&std::collections::HashSet<String>> {
        self.ignored_paths.get(&mode)
    }

    pub fn set_ignored_paths(&mut self, mode: SortMode, paths: std::collections::HashSet<String>) {
        self.configured_ignored_paths.insert(mode, paths.clone());
        self.ignored_paths.insert(mode, paths);
    }

    pub fn resolve_ignored_paths<'a>(
        &mut self,
        paths: impl Iterator<Item = &'a str>,
    ) -> std::io::Result<()> {
        let paths = paths
            .map(|path| Ok((path.to_string(), crate::path_identity::path_key(path)?)))
            .collect::<std::io::Result<Vec<_>>>()?;
        self.ignored_paths = self
            .configured_ignored_paths
            .iter()
            .map(|(mode, configured_paths)| {
                let ignored_keys = configured_paths
                    .iter()
                    .map(|path| crate::path_identity::path_key(path))
                    .collect::<std::io::Result<std::collections::HashSet<_>>>()?;
                let ignored_paths = paths
                    .iter()
                    .filter(|(_, key)| ignored_keys.contains(key))
                    .map(|(path, _)| path.clone())
                    .collect();
                Ok((*mode, ignored_paths))
            })
            .collect::<std::io::Result<_>>()?;
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub struct RemoteSettings {
    pub enabled_by_default: bool,
    pub refresh_on_toggle: bool,
    pub use_cache: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BranchSettings {
    pub on_select: BranchSelectBehavior,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BranchSelectBehavior {
    Worktree,
    Checkout,
}

impl Default for RemoteSettings {
    fn default() -> Self {
        Self {
            enabled_by_default: false,
            refresh_on_toggle: true,
            use_cache: true,
        }
    }
}

impl Default for BranchSettings {
    fn default() -> Self {
        Self {
            on_select: BranchSelectBehavior::Worktree,
        }
    }
}

/// Serialized names match the `sort.default` config values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum SortMode {
    Match,
    AlphaAsc,
    AlphaDesc,
    CreatedAsc,
    CreatedDesc,
    ModifiedAsc,
    ModifiedDesc,
    AccessedDesc,
    Recents,
}

impl SortMode {
    pub const ALL: [SortMode; 9] = [
        SortMode::Match,
        SortMode::AlphaAsc,
        SortMode::AlphaDesc,
        SortMode::CreatedAsc,
        SortMode::CreatedDesc,
        SortMode::ModifiedAsc,
        SortMode::ModifiedDesc,
        SortMode::AccessedDesc,
        SortMode::Recents,
    ];

    pub fn next(self) -> Self {
        match self {
            SortMode::Match => SortMode::AlphaAsc,
            SortMode::AlphaAsc => SortMode::AlphaDesc,
            SortMode::AlphaDesc => SortMode::CreatedAsc,
            SortMode::CreatedAsc => SortMode::CreatedDesc,
            SortMode::CreatedDesc => SortMode::ModifiedAsc,
            SortMode::ModifiedAsc => SortMode::ModifiedDesc,
            SortMode::ModifiedDesc => SortMode::AccessedDesc,
            SortMode::AccessedDesc => SortMode::Recents,
            SortMode::Recents => SortMode::Match,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SortMode::Match => "Match",
            SortMode::AlphaAsc => "A->Z",
            SortMode::AlphaDesc => "Z->A",
            SortMode::CreatedAsc => "Created ^",
            SortMode::CreatedDesc => "Created v",
            SortMode::ModifiedAsc => "Modified ^",
            SortMode::ModifiedDesc => "Modified v",
            SortMode::AccessedDesc => "Accessed v",
            SortMode::Recents => "Recents",
        }
    }

    pub fn uses_filesystem_time(self) -> bool {
        matches!(
            self,
            SortMode::CreatedAsc
                | SortMode::CreatedDesc
                | SortMode::ModifiedAsc
                | SortMode::ModifiedDesc
                | SortMode::Recents
        )
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Search,
    Preview,
    Detail,
    TagEdit,
}

#[derive(Clone)]
pub struct HelpContext<'a> {
    pub keymap: &'a Keymap,
    pub focus: Focus,
    pub sort_mode: SortMode,
    pub remote_state: RemoteToggleState,
    pub can_delete_worktree: bool,
    pub show_detail: bool,
    pub cursor_at_end: bool,
    pub has_tag_input: bool,
    pub preview_tab_index: usize,
    pub preview_tab_count: usize,
    pub preview_scroll: usize,
    pub preview_max_scroll: usize,
    pub detail_tab_index: usize,
    pub detail_tab_count: usize,
    pub detail_scroll: usize,
}

#[derive(Clone, Copy)]
pub struct HelpColors {
    pub text: Color,
    pub accent: Color,
    pub key_color: Color,
    pub remote_color: Color,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RemoteToggleState {
    Off,
    Fetching,
    Active,
    Error,
}

pub struct VisibleListArgs<'a> {
    pub entries: &'a [NavigateEntry],
    pub filtered: &'a [usize],
    pub selected: usize,
    pub offset: usize,
    pub height: usize,
    pub text: Color,
    pub accent: Color,
    pub muted: Color,
    pub dates: &'a std::collections::HashMap<String, String>,
    pub sort_meta: &'a std::collections::HashMap<String, SortMeta>,
    pub sort_mode: SortMode,
    pub tags: &'a std::collections::HashMap<String, Vec<String>>,
    pub inner_width: usize,
    pub tokens: &'a crate::search::QueryTokens,
    pub elapsed_ms: u64,
}

pub struct SidePanelRender<'a> {
    pub area: Rect,
    pub preview: &'a Text<'static>,
    pub detail_tabs: &'a [DetailTab],
    pub detail_tab_index: usize,
    pub preview_title: &'a str,
    pub preview_tab_labels: &'a [String],
    pub preview_tab_index: usize,
    pub preview_settings: PreviewSettings,
    pub focus: Focus,
    pub accent: Color,
    pub text: Color,
    pub preview_scroll: u16,
    pub detail_scroll: u16,
}

#[derive(Clone, Copy)]
pub struct UiLayout {
    pub list_area: Rect,
    pub detail_area: Rect,
    pub search_area: Rect,
    pub results_area: Rect,
    pub preview_area: Rect,
    pub detail_panel_area: Option<Rect>,
    pub help_area: Rect,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum NavigateEntryKind {
    Project,
    Worktree {
        repo_label: String,
        branch: String,
    },
    RemoteBranch {
        repo_label: String,
        branch: String,
        remote_branch: String,
        bare_path: String,
        container_path: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct NavigateEntry {
    pub id: String,
    pub display: String,
    pub context: Option<String>,
    pub preview_root_path: String,
    pub preferred_preview_path: Option<String>,
    pub selection_path: String,
    pub metadata_path: String,
    pub search_text: Vec<String>,
    pub kind: NavigateEntryKind,
}
