#![allow(dead_code)]

use ratatui::{layout::Rect, style::Color, text::Text};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, error::Error, path::PathBuf};

#[path = "keybindings.rs"]
pub(crate) mod keybindings;

use keybindings::Keymap;

pub(crate) type AppResult<T> = Result<T, Box<dyn Error>>;
pub(crate) use gator::search::MatchScore;

pub(crate) const DATE_WIDTH: usize = 16;
pub(crate) const DATE_PLACEHOLDER: &str = "---- -- -- --:--";
pub(crate) const TAB_DIVIDER_WIDTH: usize = 3;
pub(crate) const DEFAULT_WORKTREE_TAB_MIN_CHARS: usize = 6;
pub(crate) const DEFAULT_SELECTED_WORKTREE_TAB_MIN_CHARS: usize = 10;
pub(crate) const MIN_PARTIAL_TAB_WIDTH: usize = 4;
pub(crate) const CONFIG_SCHEMA_URL: &str =
    "https://raw.githubusercontent.com/Yarden-zamir/navgator/main/config-schema.json";

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ProviderId(pub(crate) String);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ResultId(pub(crate) String);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ContentId(pub(crate) String);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct MetadataKey(pub(crate) String);

#[derive(Clone, Debug)]
pub(crate) enum SelectionValue {
    Path(PathBuf),
    Url(String),
    Text(String),
    ProviderSpecific {
        provider_id: ProviderId,
        value: String,
    },
}

#[derive(Clone, Debug)]
pub(crate) enum MetadataValue {
    Text(String),
    Number(i64),
    Decimal(f64),
    Bool(bool),
    DateTime(i64),
    Tags(Vec<String>),
    List(Vec<MetadataValue>),
}

#[derive(Clone, Debug)]
pub(crate) struct MetadataEntry {
    pub(crate) key: MetadataKey,
    pub(crate) value: MetadataValue,
    pub(crate) display: Option<String>,
    pub(crate) sort_value: Option<MetadataValue>,
}

pub(crate) type MetadataMap = BTreeMap<MetadataKey, MetadataEntry>;

#[derive(Clone, Debug)]
pub(crate) struct ResultEntry {
    pub(crate) id: ResultId,
    pub(crate) provider_id: ProviderId,
    pub(crate) display: String,
    pub(crate) metadata: MetadataMap,
}

#[derive(Clone, Debug)]
pub(crate) struct ContentTarget {
    pub(crate) id: ContentId,
    pub(crate) source_result_id: ResultId,
    pub(crate) provider_id: ProviderId,
    pub(crate) display: String,
    pub(crate) metadata: MetadataMap,
    pub(crate) selection_value: SelectionValue,
}

#[derive(Clone, Debug)]
pub(crate) enum ContentBlock {
    Text { lines: Vec<String> },
    List { items: Vec<String> },
    Tree { lines: Vec<String> },
    Empty { message: String },
    Loading { message: String },
    Error { message: String },
}

#[derive(Clone)]
pub(crate) struct PreviewTab {
    pub(crate) path: String,
    pub(crate) label: String,
    pub(crate) text: Text<'static>,
    pub(crate) git: Option<Text<'static>>,
    pub(crate) github_readme: Option<Text<'static>>,
}

#[derive(Clone)]
pub(crate) struct PreviewData {
    pub(crate) previews: Vec<PreviewTab>,
    pub(crate) selected_repo_is_bare: bool,
    pub(crate) git_loaded: bool,
    pub(crate) github_readme_loaded: bool,
}

pub(crate) struct PreviewTarget {
    pub(crate) path: String,
    pub(crate) label: String,
}

pub(crate) struct GitWorktree {
    pub(crate) path: String,
    pub(crate) branch: Option<String>,
    pub(crate) detached: bool,
    pub(crate) bare: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct PreviewSettings {
    pub(crate) shorten_worktree_tab_labels: bool,
    pub(crate) worktree_tab_min_chars: usize,
    pub(crate) selected_worktree_tab_min_chars: usize,
}

pub(crate) fn default_preview_settings() -> PreviewSettings {
    PreviewSettings {
        shorten_worktree_tab_labels: true,
        worktree_tab_min_chars: DEFAULT_WORKTREE_TAB_MIN_CHARS,
        selected_worktree_tab_min_chars: DEFAULT_SELECTED_WORKTREE_TAB_MIN_CHARS,
    }
}

#[derive(Clone, Copy)]
pub(crate) struct PreviewColors {
    pub(crate) accent: Color,
    pub(crate) muted: Color,
    pub(crate) text: Color,
}

#[derive(Clone, Copy, Default)]
pub(crate) struct SortMeta {
    pub(crate) modified_epoch: Option<i64>,
    pub(crate) created_epoch: Option<i64>,
    pub(crate) accessed_epoch: Option<i64>,
}

impl SortMeta {
    pub(crate) fn recent_epoch(self) -> Option<i64> {
        match (self.modified_epoch, self.accessed_epoch) {
            (Some(modified), Some(accessed)) => Some(modified.max(accessed)),
            (Some(modified), None) => Some(modified),
            (None, Some(accessed)) => Some(accessed),
            (None, None) => None,
        }
    }
}

pub(crate) struct MetaResult {
    pub(crate) path: String,
    pub(crate) display: Option<String>,
    pub(crate) modified_epoch: Option<i64>,
    pub(crate) created_epoch: Option<i64>,
}

pub(crate) struct TagResult {
    pub(crate) path: String,
    pub(crate) tags: Vec<String>,
}

pub(crate) struct PreviewResult {
    pub(crate) path: String,
    pub(crate) data: PreviewData,
}

pub(crate) struct GitResult {
    pub(crate) path: String,
    pub(crate) tab_index: usize,
    pub(crate) git: Option<Text<'static>>,
    pub(crate) done: bool,
}

pub(crate) struct GithubReadmeResult {
    pub(crate) path: String,
    pub(crate) tab_index: usize,
    pub(crate) readme: Option<Text<'static>>,
    pub(crate) done: bool,
}

#[derive(Clone)]
pub(crate) struct DetailTab {
    pub(crate) label: String,
    pub(crate) text: Text<'static>,
}

pub(crate) struct BuildItemsResult {
    pub(crate) entries: Vec<NavigateEntry>,
    pub(crate) preview_settings: PreviewSettings,
    pub(crate) sort_settings: SortSettings,
    pub(crate) remote_settings: RemoteSettings,
    pub(crate) branch_settings: BranchSettings,
    pub(crate) action_settings: ActionSettings,
    pub(crate) create_settings: CreateSettings,
    pub(crate) theme_colors: ThemeColors,
    pub(crate) keymap: Keymap,
}

pub(crate) enum ResultUpdate {
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

pub(crate) struct LoadedConfig {
    pub(crate) index_folders: Vec<PathBuf>,
    pub(crate) static_items: Vec<PathBuf>,
    pub(crate) preview_settings: PreviewSettings,
    pub(crate) sort_settings: SortSettings,
    pub(crate) remote_settings: RemoteSettings,
    pub(crate) branch_settings: BranchSettings,
    pub(crate) action_settings: ActionSettings,
    pub(crate) create_settings: CreateSettings,
    pub(crate) theme_colors: ThemeColors,
    pub(crate) keymap: Keymap,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ActionSettings {
    pub(crate) items: Vec<ActionDefinition>,
    pub(crate) picker: Option<Vec<String>>,
    pub(crate) picker_bindings: Vec<ActionBinding>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ActionBinding {
    pub(crate) label: String,
    pub(crate) key: ActionBindingKey,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ActionBindingKey {
    CtrlEnter,
    CtrlSpace,
    CtrlN,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CreateSettings {
    pub(crate) items: Vec<CreateDefinition>,
    pub(crate) picker_bindings: Vec<ActionBinding>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CreateDefinition {
    pub(crate) label: String,
    pub(crate) icon: Option<String>,
    pub(crate) prompts: Vec<CreatePrompt>,
    pub(crate) shell: String,
    pub(crate) current_dir: Option<String>,
    pub(crate) success_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CreatePrompt {
    pub(crate) name: String,
    pub(crate) label: String,
    pub(crate) kind: CreatePromptKind,
    pub(crate) default: Option<String>,
    pub(crate) required: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CreatePromptKind {
    Text,
    Path,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ActionDefinition {
    pub(crate) id: Option<String>,
    pub(crate) label: String,
    pub(crate) icon: Option<String>,
    pub(crate) file_condition: Option<String>,
    pub(crate) kind: ActionKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ActionKind {
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

pub(crate) fn default_action_picker_bindings() -> Vec<ActionBinding> {
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

pub(crate) fn default_create_picker_bindings() -> Vec<ActionBinding> {
    vec![ActionBinding {
        label: "Ctrl+N".to_string(),
        key: ActionBindingKey::CtrlN,
    }]
}

pub(crate) fn default_action_definitions() -> Vec<ActionDefinition> {
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
        ActionDefinition {
            id: Some("open-opencode-session".to_string()),
            label: "Open OpenCode session".to_string(),
            icon: Some("󰘦".to_string()),
            file_condition: None,
            kind: ActionKind::Command {
                command: "opencode".to_string(),
                args: Vec::new(),
                current_dir: Some("{path}".to_string()),
            },
        },
    ]
}

pub(crate) fn default_create_definitions() -> Vec<CreateDefinition> {
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
pub(crate) struct ThemeColors {
    pub(crate) accent: Color,
    pub(crate) warm: Color,
    pub(crate) key_color: Color,
    pub(crate) text: Color,
    pub(crate) muted: Color,
}

impl ThemeColors {
    pub(crate) fn light() -> Self {
        Self::from(gator::theme::Palette::light())
    }

    pub(crate) fn dark() -> Self {
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
pub(crate) struct SortSettings {
    pub(crate) default_mode: SortMode,
    pub(crate) pin_current_project: bool,
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
    pub(crate) fn ignored_paths(
        &self,
        mode: SortMode,
    ) -> Option<&std::collections::HashSet<String>> {
        self.ignored_paths.get(&mode)
    }

    pub(crate) fn set_ignored_paths(
        &mut self,
        mode: SortMode,
        paths: std::collections::HashSet<String>,
    ) {
        self.configured_ignored_paths.insert(mode, paths.clone());
        self.ignored_paths.insert(mode, paths);
    }

    pub(crate) fn resolve_ignored_paths<'a>(
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
pub(crate) struct RemoteSettings {
    pub(crate) enabled_by_default: bool,
    pub(crate) refresh_on_toggle: bool,
    pub(crate) use_cache: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BranchSettings {
    pub(crate) on_select: BranchSelectBehavior,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BranchSelectBehavior {
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

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum SortMode {
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
    pub(crate) fn next(self) -> Self {
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

    pub(crate) fn label(self) -> &'static str {
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

    pub(crate) fn uses_filesystem_time(self) -> bool {
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
pub(crate) enum Focus {
    Search,
    Preview,
    Detail,
    TagEdit,
}

#[derive(Clone)]
pub(crate) struct HelpContext<'a> {
    pub(crate) keymap: &'a Keymap,
    pub(crate) focus: Focus,
    pub(crate) sort_mode: SortMode,
    pub(crate) remote_state: RemoteToggleState,
    pub(crate) can_delete_worktree: bool,
    pub(crate) show_detail: bool,
    pub(crate) cursor_at_end: bool,
    pub(crate) has_tag_input: bool,
    pub(crate) preview_tab_index: usize,
    pub(crate) preview_tab_count: usize,
    pub(crate) preview_scroll: usize,
    pub(crate) preview_max_scroll: usize,
    pub(crate) detail_tab_index: usize,
    pub(crate) detail_tab_count: usize,
    pub(crate) detail_scroll: usize,
}

#[derive(Clone, Copy)]
pub(crate) struct HelpColors {
    pub(crate) text: Color,
    pub(crate) accent: Color,
    pub(crate) key_color: Color,
    pub(crate) remote_color: Color,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum RemoteToggleState {
    Off,
    Fetching,
    Active,
    Error,
}

pub(crate) struct VisibleListArgs<'a> {
    pub(crate) entries: &'a [NavigateEntry],
    pub(crate) filtered: &'a [usize],
    pub(crate) selected: usize,
    pub(crate) offset: usize,
    pub(crate) height: usize,
    pub(crate) text: Color,
    pub(crate) accent: Color,
    pub(crate) muted: Color,
    pub(crate) dates: &'a std::collections::HashMap<String, String>,
    pub(crate) sort_meta: &'a std::collections::HashMap<String, SortMeta>,
    pub(crate) sort_mode: SortMode,
    pub(crate) tags: &'a std::collections::HashMap<String, Vec<String>>,
    pub(crate) inner_width: usize,
    pub(crate) tokens: &'a crate::search::QueryTokens,
    pub(crate) elapsed_ms: u64,
}

pub(crate) struct SidePanelRender<'a> {
    pub(crate) area: Rect,
    pub(crate) preview: &'a Text<'static>,
    pub(crate) detail_tabs: &'a [DetailTab],
    pub(crate) detail_tab_index: usize,
    pub(crate) preview_title: &'a str,
    pub(crate) preview_tab_labels: &'a [String],
    pub(crate) preview_tab_index: usize,
    pub(crate) preview_settings: PreviewSettings,
    pub(crate) focus: Focus,
    pub(crate) accent: Color,
    pub(crate) text: Color,
    pub(crate) preview_scroll: u16,
    pub(crate) detail_scroll: u16,
}

#[derive(Clone, Copy)]
pub(crate) struct UiLayout {
    pub(crate) list_area: Rect,
    pub(crate) detail_area: Rect,
    pub(crate) search_area: Rect,
    pub(crate) results_area: Rect,
    pub(crate) preview_area: Rect,
    pub(crate) detail_panel_area: Option<Rect>,
    pub(crate) help_area: Rect,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub(crate) enum NavigateEntryKind {
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
pub(crate) struct NavigateEntry {
    pub(crate) id: String,
    pub(crate) display: String,
    pub(crate) context: Option<String>,
    pub(crate) preview_root_path: String,
    pub(crate) preferred_preview_path: Option<String>,
    pub(crate) selection_path: String,
    pub(crate) metadata_path: String,
    pub(crate) search_text: Vec<String>,
    pub(crate) kind: NavigateEntryKind,
}
