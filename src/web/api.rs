//! JSON API for the web UI. Every request and response type derives its
//! TypeScript twin into `web/ui/src/api/` (`cargo test` writes them).

use crate::actions::{
    action_file_condition_matches, filtered_action_indexes, resolve_action, ResolvedAction,
    RunnableAction,
};
use crate::commands::open_url;
use crate::content::{
    display_path_for_user, erd_output, git_summary_for_path, preview_targets_for_path,
    remote_branch_recent_commits, GitSummary,
};
use crate::create::{
    initial_prompt_values, path_suggestions_for_input, refill_dependent_defaults,
    run_create_recipe, validate_create_values, PathSuggestion,
};
use crate::git::{
    add_worktree_for_remote_with_progress, checkout_remote_branch_with_progress,
    git_command_dir_for_path, git_is_bare_repository, remove_worktree_safely_with_progress,
    selectable_worktree_path_for_project,
};
use crate::github::{github_content_for_path, GithubRepoSummary};
use crate::metadata::{format_date_display, format_epoch_minutes};
use crate::model::keybindings::{BindingContext, BindingTarget, CoreAction};
use crate::model::{
    ActionKind, BranchSelectBehavior, CreatePromptKind, NavigateEntry, NavigateEntryKind, SortMode,
    DATE_PLACEHOLDER,
};
use crate::search::{
    entry_match_context, filter_and_sort_visible, highlight_mask, parse_query_tokens,
    visible_entry_count, ResultSortContext,
};
use crate::tags::{collect_tag_suggestions, save_tags_for_path};
use crate::usage::record_access;
use crate::web::app::App;
use crate::web::progress::stream_progress;
use crate::web::render::{markdown_to_html, tree_lines, StyledSegment};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    Json,
};
use serde::{Deserialize, Serialize};
use std::{convert::Infallible, path::Path, process::Stdio, sync::Arc, time::Duration};
use tokio_stream::{wrappers::BroadcastStream, Stream, StreamExt};

type AppState = State<Arc<App>>;

/// An error the UI shows as it is.
pub struct ApiError {
    status: StatusCode,
    message: String,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ErrorBody {
    pub error: String,
}

impl ApiError {
    pub(super) fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    pub(super) fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorBody {
                error: self.message,
            }),
        )
            .into_response()
    }
}

pub(super) type ApiResult<T> = Result<Json<T>, ApiError>;

/// Run blocking work (git, gh, the file system) off the async runtime.
pub(super) async fn blocking<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    tokio::task::spawn_blocking(work)
        .await
        .expect("navgator web blocking task panicked")
}

fn entry_or_404(app: &App, id: &str) -> Result<NavigateEntry, ApiError> {
    app.entry(id)
        .ok_or_else(|| ApiError::not_found(format!("No result with id {id:?}; it may be gone")))
}

// ---------------------------------------------------------------- bootstrap

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct Bootstrap {
    pub version: String,
    pub keymap: Vec<KeyContext>,
    pub sort_modes: Vec<SortModeView>,
    pub default_sort: SortMode,
    pub remotes_by_default: bool,
    /// What selecting a remote branch does before the chosen action runs.
    pub branch_on_select: BranchOnSelect,
    pub actions: Vec<ActionView>,
    pub creates: Vec<CreateView>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct KeyContext {
    pub context: BindingContext,
    /// Contexts to search in order, starting with `context` itself.
    pub fallbacks: Vec<BindingContext>,
    pub bindings: Vec<KeyBinding>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct KeyBinding {
    /// Canonical chord, for example `ctrl+enter` or `shift+tab`.
    pub chord: String,
    /// Display label, for example `Ctrl+Enter`.
    pub label: String,
    pub target: KeyTarget,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum KeyTarget {
    Core { action: CoreAction },
    Configured { id: String },
    Disabled,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct SortModeView {
    pub id: SortMode,
    pub label: String,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum BranchOnSelect {
    Worktree,
    Checkout,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ActionView {
    pub index: usize,
    pub id: Option<String>,
    pub label: String,
    pub icon: Option<String>,
    /// `navigate` actions hand a path to a shell; the web app cannot run them.
    pub navigate: bool,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CreateView {
    pub index: usize,
    pub label: String,
    pub icon: Option<String>,
    pub prompts: Vec<CreatePromptView>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CreatePromptView {
    pub name: String,
    pub label: String,
    pub path: bool,
    pub required: bool,
}

pub async fn bootstrap(State(app): AppState) -> Json<Bootstrap> {
    let settings = app.settings();
    let keymap = BindingContext::ORDERED
        .iter()
        .map(|&context| KeyContext {
            context,
            fallbacks: gator::keymap::BindingContext::fallback_contexts(context).to_vec(),
            bindings: settings
                .keymap
                .bindings_for_context(context)
                .iter()
                .map(|binding| KeyBinding {
                    chord: binding.chord.as_str(),
                    label: gator::keymap::format_chord_label(binding.chord),
                    target: match &binding.target {
                        BindingTarget::Core(action) => KeyTarget::Core { action: *action },
                        BindingTarget::Configured(id) => KeyTarget::Configured { id: id.clone() },
                        BindingTarget::Disabled => KeyTarget::Disabled,
                    },
                })
                .collect(),
        })
        .collect();
    Json(Bootstrap {
        version: env!("CARGO_PKG_VERSION").to_string(),
        keymap,
        sort_modes: SortMode::ALL
            .iter()
            .map(|&id| SortModeView {
                id,
                label: id.label().to_string(),
            })
            .collect(),
        default_sort: settings.default_sort,
        remotes_by_default: settings.remote.enabled_by_default,
        branch_on_select: match settings.branch.on_select {
            BranchSelectBehavior::Worktree => BranchOnSelect::Worktree,
            BranchSelectBehavior::Checkout => BranchOnSelect::Checkout,
        },
        actions: settings
            .actions
            .items
            .iter()
            .enumerate()
            .map(|(index, action)| ActionView {
                index,
                id: action.id.clone(),
                label: action.label.clone(),
                icon: action.icon.clone(),
                navigate: matches!(action.kind, ActionKind::Navigate),
            })
            .collect(),
        creates: settings
            .create
            .items
            .iter()
            .enumerate()
            .map(|(index, item)| CreateView {
                index,
                label: item.label.clone(),
                icon: item.icon.clone(),
                prompts: item
                    .prompts
                    .iter()
                    .map(|prompt| CreatePromptView {
                        name: prompt.name.clone(),
                        label: prompt.label.clone(),
                        path: prompt.kind == CreatePromptKind::Path,
                        required: prompt.required,
                    })
                    .collect(),
            })
            .collect(),
    })
}

// ------------------------------------------------------------------ results

#[derive(Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ResultsQuery {
    #[serde(default)]
    pub q: String,
    pub sort: SortMode,
    #[serde(default)]
    pub remotes: bool,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct Results {
    pub rows: Vec<ResultRow>,
    /// Entries that could show with the current remote toggle, before the query.
    pub total: usize,
    pub remote: RemoteView,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct RemoteView {
    pub message: Option<String>,
    pub fetching: bool,
    pub error: bool,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ResultRow {
    pub id: String,
    pub kind: EntryKind,
    /// The display text split into runs that match the query or not.
    pub display: Vec<TextRun>,
    pub context: Option<String>,
    pub tags: Vec<TagView>,
    pub date: Option<String>,
    pub metadata_path: String,
}

#[derive(Clone, Copy, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum EntryKind {
    Project,
    Worktree,
    RemoteBranch,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct TextRun {
    pub text: String,
    pub hit: bool,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct TagView {
    pub name: String,
    pub matched: bool,
}

fn entry_kind(entry: &NavigateEntry) -> EntryKind {
    match entry.kind {
        NavigateEntryKind::Project => EntryKind::Project,
        NavigateEntryKind::Worktree { .. } => EntryKind::Worktree,
        NavigateEntryKind::RemoteBranch { .. } => EntryKind::RemoteBranch,
    }
}

fn text_runs(value: &str, mask: &[bool]) -> Vec<TextRun> {
    let mut runs: Vec<TextRun> = Vec::new();
    for (index, ch) in value.chars().enumerate() {
        let hit = mask.get(index).copied().unwrap_or(false);
        match runs.last_mut() {
            Some(run) if run.hit == hit => run.text.push(ch),
            _ => runs.push(TextRun {
                text: ch.to_string(),
                hit,
            }),
        }
    }
    runs
}

pub async fn results(State(app): AppState, Query(query): Query<ResultsQuery>) -> Json<Results> {
    let store = app.store();
    let filtered = filter_and_sort_visible(
        &store.entries,
        &query.q,
        query.sort,
        &store.meta,
        &store.tags,
        ResultSortContext {
            show_remote_branches: query.remotes,
            sort_settings: &store.sort_settings,
            // The browser has no working directory to pin.
            current_project_path: None,
        },
    );
    let tokens = parse_query_tokens(&query.q);
    let text_tokens = tokens
        .folder
        .iter()
        .chain(tokens.any.iter())
        .map(String::as_str)
        .collect::<Vec<_>>();
    let rows = filtered
        .iter()
        .map(|&index| {
            let entry = &store.entries[index];
            let tags = store
                .tags
                .get(&entry.metadata_path)
                .map(Vec::as_slice)
                .unwrap_or_default();
            let date = match query.sort {
                SortMode::AccessedDesc | SortMode::Recents => store
                    .meta
                    .get(&entry.metadata_path)
                    .and_then(|meta| match query.sort {
                        SortMode::AccessedDesc => meta.accessed_epoch,
                        _ => meta.recent_epoch(),
                    })
                    .map(format_epoch_minutes),
                _ => store.dates.get(&entry.metadata_path).cloned(),
            }
            .filter(|date| date != DATE_PLACEHOLDER)
            .map(|date| format_date_display(&date).trim().to_string());
            ResultRow {
                id: entry.id.clone(),
                kind: entry_kind(entry),
                display: text_runs(
                    &entry.display,
                    &highlight_mask(&entry.display, &text_tokens),
                ),
                context: entry
                    .context
                    .clone()
                    .or_else(|| entry_match_context(entry, tags, &tokens)),
                tags: tags
                    .iter()
                    .map(|tag| TagView {
                        name: tag.clone(),
                        matched: tokens
                            .tags
                            .iter()
                            .any(|token| gator::fuzzy_match(token, tag)),
                    })
                    .collect(),
                date,
                metadata_path: entry.metadata_path.clone(),
            }
        })
        .collect();
    Json(Results {
        rows,
        total: visible_entry_count(&store.entries, query.remotes),
        remote: RemoteView {
            message: store.remote.message.clone(),
            fetching: store.remote.fetching,
            error: store.remote.error,
        },
    })
}

/// Server-sent events: one `change` event per store update.
pub async fn events(State(app): AppState) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    // Browsers drop events with empty data, so each event carries a token.
    // A lagged receiver missed events; the refetch covers them as well.
    let stream = BroadcastStream::new(app.subscribe())
        .map(|_| Ok(Event::default().event("change").data("1")));
    Sse::new(stream).keep_alive(KeepAlive::default())
}

#[derive(Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct RemotesRequest {
    pub entry_id: String,
}

pub async fn load_remotes(
    State(app): AppState,
    Json(request): Json<RemotesRequest>,
) -> Result<StatusCode, ApiError> {
    let entry = entry_or_404(&app, &request.entry_id)?;
    app.load_remote_branches(entry);
    Ok(StatusCode::NO_CONTENT)
}

// ------------------------------------------------------------------ preview

#[derive(Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct EntryQuery {
    pub entry: String,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Preview {
    Local(LocalPreview),
    Remote(RemotePreview),
}

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LocalPreview {
    pub root_path: String,
    pub selected_repo_is_bare: bool,
    pub tabs: Vec<PreviewTabView>,
    /// The tab to select first: the worktree of a worktree result.
    pub preferred_path: Option<String>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct PreviewTabView {
    pub path: String,
    pub label: String,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct RemotePreview {
    pub remote_branch: String,
    pub target_path: String,
    pub target_display: String,
    pub recent_commits: Option<Vec<String>>,
}

pub async fn preview(State(app): AppState, Query(query): Query<EntryQuery>) -> ApiResult<Preview> {
    let entry = entry_or_404(&app, &query.entry)?;
    let shorten = app.settings().preview.shorten_worktree_tab_labels;
    Ok(Json(
        blocking(move || match &entry.kind {
            NavigateEntryKind::RemoteBranch {
                remote_branch,
                bare_path,
                ..
            } => Preview::Remote(RemotePreview {
                remote_branch: remote_branch.clone(),
                target_display: display_path_for_user(&entry.selection_path),
                target_path: entry.selection_path.clone(),
                recent_commits: remote_branch_recent_commits(bare_path, remote_branch)
                    .map(|output| output.lines().map(str::to_string).collect()),
            }),
            NavigateEntryKind::Project | NavigateEntryKind::Worktree { .. } => {
                let path = entry.preview_root_path.clone();
                Preview::Local(LocalPreview {
                    selected_repo_is_bare: git_command_dir_for_path(Path::new(&path))
                        .is_some_and(|dir| git_is_bare_repository(&dir)),
                    tabs: preview_targets_for_path(&path, shorten)
                        .into_iter()
                        .map(|target| PreviewTabView {
                            path: target.path,
                            label: target.label,
                        })
                        .collect(),
                    preferred_path: entry.preferred_preview_path.clone(),
                    root_path: path,
                })
            }
        })
        .await,
    ))
}

#[derive(Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct PathQuery {
    pub path: String,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct TreeView {
    pub display_path: String,
    pub is_dir: bool,
    /// `None` when `erd` is not installed or fails.
    pub lines: Option<Vec<Vec<StyledSegment>>>,
}

const TREE_MAX_LINES: usize = 200;

pub async fn tree(Query(query): Query<PathQuery>) -> Json<TreeView> {
    Json(
        blocking(move || {
            let path = Path::new(&query.path);
            let is_dir = path.is_dir();
            TreeView {
                display_path: display_path_for_user(&query.path),
                is_dir,
                lines: is_dir
                    .then(|| erd_output(path))
                    .flatten()
                    .map(|output| tree_lines(&output, TREE_MAX_LINES)),
            }
        })
        .await,
    )
}

#[derive(Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct GitQuery {
    pub path: String,
    pub selected_repo_is_bare: bool,
}

pub async fn git(Query(query): Query<GitQuery>) -> Json<Option<GitSummary>> {
    Json(blocking(move || git_summary_for_path(&query.path, query.selected_repo_is_bare)).await)
}

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct GithubView {
    pub summary: Option<GithubRepoSummary>,
    /// README rendered to HTML. Raw HTML in the README is dropped.
    pub readme_html: Option<String>,
}

pub async fn github(Query(query): Query<PathQuery>) -> Json<Option<GithubView>> {
    Json(
        blocking(move || {
            github_content_for_path(&query.path).and_then(|content| {
                (content.summary.is_some() || content.readme.is_some()).then(|| GithubView {
                    summary: content.summary,
                    readme_html: content.readme.as_deref().map(markdown_to_html),
                })
            })
        })
        .await,
    )
}

// ------------------------------------------------------------------ actions

#[derive(Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct TargetQuery {
    pub entry: String,
    /// The preview tab path, when the preview or detail panel has focus.
    pub tab: Option<String>,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Target {
    pub path: String,
}

/// The path that actions and create recipes run on, as in the terminal UI:
/// the focused preview tab, else the default worktree of a project, else the
/// result path.
pub async fn target(State(app): AppState, Query(query): Query<TargetQuery>) -> ApiResult<Target> {
    let entry = entry_or_404(&app, &query.entry)?;
    if let Some(tab) = query.tab {
        return Ok(Json(Target { path: tab }));
    }
    let path = blocking(move || match entry.kind {
        NavigateEntryKind::Project => {
            selectable_worktree_path_for_project(Path::new(&entry.selection_path))
                .unwrap_or(entry.selection_path)
        }
        NavigateEntryKind::Worktree { .. } | NavigateEntryKind::RemoteBranch { .. } => {
            entry.selection_path
        }
    })
    .await;
    Ok(Json(Target { path }))
}

#[derive(Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ActionsQuery {
    pub path: String,
    #[serde(default)]
    pub q: String,
}

/// Indexes into `Bootstrap.actions` that the picker shows for `path`.
pub async fn actions(State(app): AppState, Query(query): Query<ActionsQuery>) -> Json<Vec<usize>> {
    let settings = app.settings();
    let actions = &settings.actions;
    Json(
        filtered_action_indexes(
            &actions.items,
            actions.picker.as_deref(),
            &query.q,
            Some(&query.path),
        )
        .into_iter()
        .filter(|&index| !matches!(actions.items[index].kind, ActionKind::Navigate))
        .collect(),
    )
}

#[derive(Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct RunRequest {
    pub action: ActionRef,
    pub path: String,
}

/// Picker rows name an action by index; configured keybindings name it by id.
#[derive(Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(tag = "by", rename_all = "kebab-case")]
pub enum ActionRef {
    Index { index: usize },
    Id { id: String },
}

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct RunResult {
    /// Set when the command still runs after the wait limit.
    pub still_running: bool,
    pub output: Option<String>,
}

/// How long a run request waits for its command. `t3-open-path` can take
/// up to 30 s when it starts T3, so the limit sits above that.
const RUN_WAIT: Duration = Duration::from_secs(45);

pub async fn run_action(
    State(app): AppState,
    Json(request): Json<RunRequest>,
) -> ApiResult<RunResult> {
    let settings = app.settings();
    let items = &settings.actions.items;
    let action = match &request.action {
        ActionRef::Index { index } => items.get(*index),
        ActionRef::Id { id } => items
            .iter()
            .find(|action| action.id.as_deref() == Some(id.as_str())),
    }
    .ok_or_else(|| ApiError::bad_request("Unknown action"))?
    .clone();
    if !action_file_condition_matches(&action, Some(&request.path)) {
        return Err(ApiError::bad_request(format!(
            "Action {:?} is unavailable because its file condition is not met",
            action.label
        )));
    }
    let resolved = resolve_action(&action, Some(&request.path)).ok_or_else(|| {
        ApiError::bad_request(format!(
            "Action {:?} requires target metadata that is unavailable",
            action.label
        ))
    })?;
    let runnable = match resolved {
        ResolvedAction::Navigate(_) => {
            return Err(ApiError::bad_request(format!(
                "Action {:?} changes the shell directory; the web app has no shell",
                action.label
            )))
        }
        ResolvedAction::Run(runnable) => runnable,
    };

    let handle = tokio::task::spawn_blocking(move || run_runnable(runnable));
    let result = match tokio::time::timeout(RUN_WAIT, handle).await {
        Err(_elapsed) => RunResult {
            still_running: true,
            output: None,
        },
        Ok(joined) => {
            let output = joined
                .expect("navgator web action task panicked")
                .map_err(ApiError::internal)?;
            RunResult {
                still_running: false,
                output: Some(output).filter(|output| !output.is_empty()),
            }
        }
    };

    let path = request.path.clone();
    blocking(move || record_access(&path))
        .await
        .map_err(|error| ApiError::internal(format!("Failed to record access: {error}")))?;
    app.reload_access_history().map_err(ApiError::internal)?;
    Ok(Json(result))
}

/// Run without a terminal: stdin is closed, output is captured.
fn run_runnable(runnable: RunnableAction) -> Result<String, String> {
    match runnable {
        RunnableAction::OpenUrl(url) => open_url(&url).map(|()| String::new()),
        RunnableAction::Command {
            command,
            args,
            current_dir,
        } => {
            let mut cmd = std::process::Command::new(&command);
            cmd.args(&args).stdin(Stdio::null());
            if let Some(dir) = current_dir {
                cmd.current_dir(dir);
            }
            let output = cmd
                .output()
                .map_err(|error| format!("Failed to run {command}: {error}"))?;
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            if output.status.success() {
                Ok(stdout)
            } else {
                Err(format!(
                    "{command} exited with {}{}",
                    output.status,
                    if stderr.is_empty() {
                        String::new()
                    } else {
                        format!(": {stderr}")
                    }
                ))
            }
        }
    }
}

// --------------------------------------------------- branches and worktrees

#[derive(Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct EntryRequest {
    pub entry_id: String,
}

/// Create a worktree for (or check out) a remote branch. Streams progress;
/// `done` carries the checkout path.
pub async fn select_branch(
    State(app): AppState,
    Json(request): Json<EntryRequest>,
) -> Result<Response, ApiError> {
    let entry = entry_or_404(&app, &request.entry_id)?;
    let NavigateEntryKind::RemoteBranch {
        remote_branch,
        bare_path,
        container_path,
        ..
    } = entry.kind
    else {
        return Err(ApiError::bad_request("Not a remote branch"));
    };
    let behavior = app.settings().branch.on_select;
    Ok(stream_progress(move |progress| {
        let bare = Path::new(&bare_path);
        let result = match behavior {
            BranchSelectBehavior::Worktree => add_worktree_for_remote_with_progress(
                bare,
                Path::new(&container_path),
                &remote_branch,
                progress,
            ),
            BranchSelectBehavior::Checkout => {
                checkout_remote_branch_with_progress(bare, &remote_branch, progress)
            }
        };
        if result.is_ok() {
            app.reload()?;
        }
        result
    }))
}

pub async fn delete_worktree(
    State(app): AppState,
    Json(request): Json<EntryRequest>,
) -> Result<Response, ApiError> {
    let entry = entry_or_404(&app, &request.entry_id)?;
    if !matches!(entry.kind, NavigateEntryKind::Worktree { .. }) {
        return Err(ApiError::bad_request(
            "Only worktree results can be deleted",
        ));
    }
    Ok(stream_progress(move |progress| {
        let path =
            remove_worktree_safely_with_progress(Path::new(&entry.selection_path), progress)?;
        app.remove_entries_at(&path);
        Ok(path)
    }))
}

// ------------------------------------------------------------------- create

#[derive(Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CreateValuesRequest {
    pub index: usize,
    pub path: Option<String>,
    /// Current values; omit them to get the initial defaults.
    pub values: Option<Vec<String>>,
    #[serde(default)]
    pub active_prompt: usize,
}

pub async fn create_values(
    State(app): AppState,
    Json(request): Json<CreateValuesRequest>,
) -> ApiResult<Vec<String>> {
    let settings = app.settings();
    let item = settings
        .create
        .items
        .get(request.index)
        .ok_or_else(|| ApiError::bad_request("Unknown create recipe"))?;
    let path = request.path.as_deref();
    Ok(Json(match request.values {
        None => initial_prompt_values(item, path),
        Some(values) => refill_dependent_defaults(item, path, &values, request.active_prompt),
    }))
}

#[derive(Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CompletionQuery {
    pub input: String,
}

pub async fn path_completions(Query(query): Query<CompletionQuery>) -> Json<Vec<PathSuggestion>> {
    Json(blocking(move || path_suggestions_for_input(&query.input)).await)
}

#[derive(Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CreateRunRequest {
    pub index: usize,
    pub path: Option<String>,
    pub values: Vec<String>,
}

/// Validate, then run the recipe shell. A missing value answers 422 with a
/// `MissingCreateValue`; otherwise the response streams progress.
pub async fn run_create(
    State(app): AppState,
    Json(request): Json<CreateRunRequest>,
) -> Result<Response, ApiError> {
    let item = app
        .settings()
        .create
        .items
        .get(request.index)
        .ok_or_else(|| ApiError::bad_request("Unknown create recipe"))?
        .clone();
    let values = match validate_create_values(&item, &request.values) {
        Ok(values) => values,
        Err(missing) => {
            return Ok((StatusCode::UNPROCESSABLE_ENTITY, Json(missing)).into_response())
        }
    };
    Ok(stream_progress(move |progress| {
        let path = run_create_recipe(&item, request.path.as_deref(), &values, progress)?;
        app.reload()?;
        Ok(path)
    }))
}

// --------------------------------------------------------------------- tags

#[derive(Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct TagsRequest {
    pub path: String,
    pub tags: Vec<String>,
}

pub async fn save_tags(
    State(app): AppState,
    Json(request): Json<TagsRequest>,
) -> Result<StatusCode, ApiError> {
    let TagsRequest { path, tags } = request;
    let (path, tags) = blocking(move || {
        save_tags_for_path(&path, &tags)
            .map(|()| (path, tags))
            .map_err(|error| format!("Failed to save tags: {error}"))
    })
    .await
    .map_err(ApiError::internal)?;
    app.set_tags(path, tags);
    Ok(StatusCode::NO_CONTENT)
}

pub async fn tag_suggestions(State(app): AppState) -> Json<Vec<String>> {
    Json(collect_tag_suggestions(&app.store().tags))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::progress::Progress;
    use ts_rs::TS;

    /// Write the TypeScript types that `ui/src/api/` imports. Run
    /// `cargo test` after an API change and commit the result.
    #[test]
    fn export_typescript_bindings() {
        let config = ts_rs::Config::new()
            .with_out_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/web/ui/src/api"))
            .with_large_int("number");
        type Export = fn(&ts_rs::Config) -> Result<(), ts_rs::ExportError>;
        let exports: [Export; 33] = [
            Bootstrap::export_all,
            ErrorBody::export_all,
            ResultsQuery::export_all,
            Results::export_all,
            RemotesRequest::export_all,
            EntryQuery::export_all,
            Preview::export_all,
            PathQuery::export_all,
            TreeView::export_all,
            GitQuery::export_all,
            GitSummary::export_all,
            GithubView::export_all,
            TargetQuery::export_all,
            Target::export_all,
            ActionsQuery::export_all,
            RunRequest::export_all,
            RunResult::export_all,
            EntryRequest::export_all,
            CreateValuesRequest::export_all,
            CompletionQuery::export_all,
            PathSuggestion::export_all,
            CreateRunRequest::export_all,
            crate::create::MissingCreateValue::export_all,
            TagsRequest::export_all,
            Progress::export_all,
            SortMode::export_all,
            CoreAction::export_all,
            BindingContext::export_all,
            KeyTarget::export_all,
            EntryKind::export_all,
            crate::web::settings::SettingsView::export_all,
            crate::web::settings::SettingsUpdate::export_all,
            crate::web::settings::DefaultBinding::export_all,
        ];
        for export in exports {
            export(&config).expect("TypeScript bindings export");
        }
    }
}
