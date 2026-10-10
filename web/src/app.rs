//! Server state. The same background providers as the terminal UI feed one
//! store; every change bumps a broadcast so browsers refetch.

use navgator::metadata::spawn_bulk_metadata_fetch;
use navgator::model::keybindings::Keymap;
use navgator::model::{
    ActionSettings, BranchSettings, CreateSettings, MetaResult, NavigateEntry, NavigateEntryKind,
    PreviewSettings, RemoteSettings, ResultUpdate, SortMeta, SortMode, SortSettings, TagResult,
    DATE_PLACEHOLDER,
};
use navgator::results::{
    build_items_with_config_entries, spawn_remote_branch_result_provider,
    spawn_worktree_result_provider, REMOTE_BRANCH_PROVIDER_PREFIX,
};
use navgator::tags::spawn_bulk_tag_fetch;
use navgator::usage::AccessHistory;
use std::{
    collections::{HashMap, HashSet},
    sync::{mpsc, Arc, Mutex, MutexGuard},
    thread,
};
use tokio::sync::broadcast;

/// Config values that stay fixed while the server runs.
// Limit: config edits need a server restart. Revisit if config edits become
// frequent enough that a reload endpoint pays off.
pub struct Settings {
    pub config_entries: Vec<String>,
    pub preview: PreviewSettings,
    pub remote: RemoteSettings,
    pub branch: BranchSettings,
    pub actions: ActionSettings,
    pub create: CreateSettings,
    pub keymap: Keymap,
    pub default_sort: SortMode,
}

#[derive(Default)]
pub struct RemoteStatus {
    pub message: Option<String>,
    pub fetching: bool,
    pub error: bool,
}

pub struct Store {
    pub entries: Vec<NavigateEntry>,
    pub sort_settings: SortSettings,
    pub meta: HashMap<String, SortMeta>,
    pub dates: HashMap<String, String>,
    pub tags: HashMap<String, Vec<String>>,
    pub remote: RemoteStatus,
    dates_in_flight: HashSet<String>,
    tags_in_flight: HashSet<String>,
}

pub struct App {
    pub settings: Settings,
    store: Mutex<Store>,
    changes: broadcast::Sender<()>,
    results_tx: mpsc::Sender<ResultUpdate>,
    meta_tx: mpsc::Sender<MetaResult>,
    tag_tx: mpsc::Sender<TagResult>,
}

impl App {
    /// Load the config, build the first entries and start the providers.
    pub fn start(config_entries: Vec<String>) -> Result<Arc<Self>, String> {
        let built = build_items_with_config_entries(&config_entries)
            .map_err(|error| format!("failed to load navgator config: {error}"))?;
        let (results_tx, results_rx) = mpsc::channel();
        let (meta_tx, meta_rx) = mpsc::channel();
        let (tag_tx, tag_rx) = mpsc::channel();
        let (changes, _) = broadcast::channel(64);
        let app = Arc::new(Self {
            settings: Settings {
                config_entries,
                preview: built.preview_settings,
                remote: built.remote_settings,
                branch: built.branch_settings,
                actions: built.action_settings,
                create: built.create_settings,
                keymap: built.keymap,
                default_sort: built.sort_settings.default_mode,
            },
            store: Mutex::new(Store {
                entries: Vec::new(),
                sort_settings: built.sort_settings,
                meta: HashMap::new(),
                dates: HashMap::new(),
                tags: HashMap::new(),
                remote: RemoteStatus::default(),
                dates_in_flight: HashSet::new(),
                tags_in_flight: HashSet::new(),
            }),
            changes,
            results_tx,
            meta_tx,
            tag_tx,
        });

        pump(&app, results_rx, |app, store, update| {
            app.apply_result_update(store, update)
        });
        pump(&app, meta_rx, |_, store, result| {
            store.dates.insert(
                result.path.clone(),
                result
                    .display
                    .unwrap_or_else(|| DATE_PLACEHOLDER.to_string()),
            );
            let meta = store.meta.entry(result.path.clone()).or_default();
            meta.modified_epoch = result.modified_epoch;
            meta.created_epoch = result.created_epoch;
            store.dates_in_flight.remove(&result.path);
        });
        pump(&app, tag_rx, |_, store, result| {
            store.tags_in_flight.remove(&result.path);
            store.tags.insert(result.path, result.tags);
        });

        {
            let mut store = app.store();
            store.entries = built.entries;
            app.refresh_derived(&mut store);
            spawn_worktree_result_provider(&store.entries, app.results_tx.clone());
        }
        Ok(app)
    }

    pub fn store(&self) -> MutexGuard<'_, Store> {
        self.store
            .lock()
            .expect("navgator-web store mutex is poisoned")
    }

    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.changes.subscribe()
    }

    pub fn notify(&self) {
        // An error only means that no browser listens right now.
        let _ = self.changes.send(());
    }

    pub fn entry(&self, id: &str) -> Option<NavigateEntry> {
        self.store()
            .entries
            .iter()
            .find(|entry| entry.id == id)
            .cloned()
    }

    /// Start the remote branch provider for `entry` and drop older remote rows.
    pub fn load_remote_branches(&self, entry: NavigateEntry) {
        {
            let mut store = self.store();
            store
                .entries
                .retain(|candidate| !candidate.id.starts_with(REMOTE_BRANCH_PROVIDER_PREFIX));
            store.remote = RemoteStatus {
                message: Some("loading local remotes...".to_string()),
                fetching: self.settings.remote.refresh_on_toggle,
                error: false,
            };
        }
        self.notify();
        spawn_remote_branch_result_provider(entry, self.results_tx.clone(), self.settings.remote);
    }

    /// Re-read the project folders and rescan worktrees, for example after a
    /// create recipe or a new worktree. Remote branch rows stay.
    pub fn rescan(&self) -> Result<(), String> {
        let built = build_items_with_config_entries(&self.settings.config_entries)
            .map_err(|error| format!("failed to reload navgator entries: {error}"))?;
        {
            let mut store = self.store();
            let remote_entries = store
                .entries
                .drain(..)
                .filter(|entry| matches!(entry.kind, NavigateEntryKind::RemoteBranch { .. }))
                .collect::<Vec<_>>();
            store.entries = built.entries;
            store.entries.extend(remote_entries);
            self.refresh_derived(&mut store);
            spawn_worktree_result_provider(&store.entries, self.results_tx.clone());
        }
        self.notify();
        Ok(())
    }

    pub fn remove_entries_at(&self, path: &str) {
        self.store()
            .entries
            .retain(|entry| entry.selection_path != path);
        self.notify();
    }

    pub fn set_tags(&self, path: String, tags: Vec<String>) {
        self.store().tags.insert(path, tags);
        self.notify();
    }

    /// Reload usage history after `record_access` wrote to it.
    pub fn reload_access_history(&self) -> Result<(), String> {
        let history = AccessHistory::load()
            .map_err(|error| format!("failed to read navgator usage history: {error}"))?;
        {
            let mut store = self.store();
            let Store { entries, meta, .. } = &mut *store;
            history
                .apply_to_sort_meta(entries, meta)
                .map_err(|error| format!("failed to apply usage history: {error}"))?;
        }
        self.notify();
        Ok(())
    }

    fn apply_result_update(&self, store: &mut Store, update: ResultUpdate) {
        match update {
            ResultUpdate::Entries { entries } => {
                for entry in entries {
                    if !store
                        .entries
                        .iter()
                        .any(|candidate| candidate.id == entry.id)
                    {
                        store.entries.push(entry);
                    }
                }
                self.refresh_derived(store);
            }
            ResultUpdate::ReplaceProviderEntries {
                provider_prefix,
                entries,
            } => {
                store
                    .entries
                    .retain(|entry| !entry.id.starts_with(&provider_prefix));
                store.entries.extend(entries);
                self.refresh_derived(store);
            }
            ResultUpdate::Status {
                provider_id,
                message,
            } => {
                if provider_id == REMOTE_BRANCH_PROVIDER_PREFIX {
                    let fetching = message.starts_with("refreshing ");
                    store.remote = RemoteStatus {
                        error: !fetching && message != "remote branches loaded",
                        fetching,
                        message: Some(message),
                    };
                }
            }
        }
    }

    /// Recompute what depends on the entry list: usage times, ignored paths,
    /// and metadata and tags for paths that have none yet.
    fn refresh_derived(&self, store: &mut Store) {
        let history = AccessHistory::load().expect("failed to read navgator usage history");
        let Store {
            entries,
            sort_settings,
            meta,
            dates,
            tags,
            dates_in_flight,
            tags_in_flight,
            ..
        } = store;
        history
            .apply_to_sort_meta(entries, meta)
            .expect("failed to apply navgator usage history");
        sort_settings
            .resolve_ignored_paths(entries.iter().map(|entry| entry.metadata_path.as_str()))
            .expect("failed to resolve ignored sort paths");
        let mut seen = HashSet::new();
        let paths = entries
            .iter()
            .filter(|entry| seen.insert(entry.metadata_path.as_str()))
            .map(|entry| entry.metadata_path.clone())
            .collect::<Vec<_>>();
        spawn_bulk_metadata_fetch(&paths, dates, dates_in_flight, &self.meta_tx);
        spawn_bulk_tag_fetch(&paths, tags, tags_in_flight, &self.tag_tx);
    }
}

/// Apply every message from `rx` to the store on a dedicated thread.
fn pump<T: Send + 'static>(
    app: &Arc<App>,
    rx: mpsc::Receiver<T>,
    apply: impl Fn(&App, &mut Store, T) + Send + 'static,
) {
    let app = Arc::clone(app);
    thread::spawn(move || {
        for message in rx {
            apply(&app, &mut app.store(), message);
            app.notify();
        }
    });
}
