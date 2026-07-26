use crate::{
    model::{NavigateEntry, SortMeta},
    path_identity::path_key,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const STATE_DIR_NAME: &str = "navgator";
const ACCESS_FILE_NAME: &str = "usage.json";
const ACCESS_FILE_VERSION: u32 = 1;

#[derive(Debug, Default, Deserialize, Serialize)]
struct AccessState {
    version: u32,
    items: BTreeMap<String, AccessEntry>,
}

#[derive(Debug, Deserialize, Serialize)]
struct AccessEntry {
    last_accessed_epoch: i64,
}

pub(crate) struct AccessHistory {
    state: AccessState,
}

impl AccessHistory {
    pub(crate) fn load() -> io::Result<Self> {
        Ok(Self {
            state: load_state(&state_file_path()?)?,
        })
    }

    pub(crate) fn apply_to_sort_meta(
        &self,
        entries: &[NavigateEntry],
        meta_cache: &mut HashMap<String, SortMeta>,
    ) -> io::Result<()> {
        for entry in entries {
            let key = path_key(&entry.metadata_path)?;
            let Some(access) = self.state.items.get(&key) else {
                continue;
            };
            meta_cache
                .entry(entry.metadata_path.clone())
                .or_default()
                .accessed_epoch = Some(access.last_accessed_epoch);
        }
        Ok(())
    }
}

pub(crate) fn record_access(path: &str) -> io::Result<()> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| io::Error::other(format!("System clock is before Unix epoch: {error}")))?
        .as_secs();
    let timestamp = i64::try_from(timestamp)
        .map_err(|_| io::Error::other("Current Unix timestamp does not fit in i64"))?;
    record_access_at(&state_file_path()?, path, timestamp)
}

fn record_access_at(state_path: &Path, target_path: &str, timestamp: i64) -> io::Result<()> {
    let _lock = gator::xdg::lock_sibling(state_path)?;

    let mut state = load_state(state_path)?;
    let key = path_key(target_path)?;
    let entry = state.items.entry(key).or_insert(AccessEntry {
        last_accessed_epoch: timestamp,
    });
    entry.last_accessed_epoch = entry.last_accessed_epoch.max(timestamp);
    save_state(state_path, &state)
}

fn load_state(path: &Path) -> io::Result<AccessState> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(AccessState {
                version: ACCESS_FILE_VERSION,
                items: BTreeMap::new(),
            });
        }
        Err(error) => return Err(error),
    };
    let state: AccessState = serde_json::from_str(&contents)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if state.version != ACCESS_FILE_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Unsupported access state version {}; expected {ACCESS_FILE_VERSION}",
                state.version
            ),
        ));
    }
    Ok(state)
}

fn save_state(path: &Path, state: &AccessState) -> io::Result<()> {
    gator::xdg::write_json_atomic(path, state)
}

fn state_file_path() -> io::Result<PathBuf> {
    gator::xdg::state_file(STATE_DIR_NAME, ACCESS_FILE_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::NavigateEntryKind;
    use std::env;

    #[test]
    fn records_only_the_latest_access_for_a_target() {
        let directory = test_directory("latest");
        let state_path = directory.join(ACCESS_FILE_NAME);
        let target = directory.join("project");
        fs::create_dir_all(&target).expect("target directory");

        record_access_at(&state_path, target.to_str().expect("target path"), 200)
            .expect("newer access");
        record_access_at(&state_path, target.to_str().expect("target path"), 100)
            .expect("older access");

        let state = load_state(&state_path).expect("access state");
        let key = path_key(target.to_str().expect("target path")).expect("access key");
        assert_eq!(state.items[&key].last_accessed_epoch, 200);
        fs::remove_dir_all(directory).expect("remove test directory");
    }

    #[test]
    fn rejects_unknown_state_versions_without_replacing_them() {
        let directory = test_directory("version");
        let state_path = directory.join(ACCESS_FILE_NAME);
        fs::write(&state_path, r#"{"version":2,"items":{}}"#).expect("state file");

        let error = load_state(&state_path).expect_err("unsupported version");

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            fs::read_to_string(&state_path).expect("unchanged state"),
            r#"{"version":2,"items":{}}"#
        );
        fs::remove_dir_all(directory).expect("remove test directory");
    }

    #[test]
    fn normalizes_nonexistent_target_paths_lexically() {
        let directory = test_directory("normalize");
        let target = directory.join("project").join("..").join("future-worktree");

        let key = path_key(target.to_str().expect("target path")).expect("access key");

        assert_eq!(key, directory.join("future-worktree").to_string_lossy());
        fs::remove_dir_all(directory).expect("remove test directory");
    }

    #[test]
    fn applies_access_only_to_the_matching_target() {
        let directory = test_directory("target-only");
        let state_path = directory.join(ACCESS_FILE_NAME);
        let project = directory.join("project");
        let worktree = directory.join("worktree");
        fs::create_dir_all(&project).expect("project directory");
        fs::create_dir_all(&worktree).expect("worktree directory");
        record_access_at(&state_path, worktree.to_str().expect("worktree path"), 200)
            .expect("worktree access");
        let history = AccessHistory {
            state: load_state(&state_path).expect("access state"),
        };
        let entries = [
            test_entry("project", &project),
            test_entry("worktree", &worktree),
        ];
        let mut meta_cache = HashMap::new();

        history
            .apply_to_sort_meta(&entries, &mut meta_cache)
            .expect("apply access history");

        assert!(!meta_cache.contains_key(entries[0].metadata_path.as_str()));
        assert_eq!(
            meta_cache[entries[1].metadata_path.as_str()].accessed_epoch,
            Some(200)
        );
        fs::remove_dir_all(directory).expect("remove test directory");
    }

    #[test]
    fn rejects_malformed_state_without_replacing_it() {
        let directory = test_directory("malformed");
        let state_path = directory.join(ACCESS_FILE_NAME);
        fs::write(&state_path, "not json").expect("state file");

        let error = load_state(&state_path).expect_err("malformed state");

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            fs::read_to_string(&state_path).expect("unchanged state"),
            "not json"
        );
        fs::remove_dir_all(directory).expect("remove test directory");
    }

    fn test_entry(id: &str, path: &Path) -> NavigateEntry {
        let path = path.to_string_lossy().into_owned();
        NavigateEntry {
            id: id.to_string(),
            display: id.to_string(),
            context: None,
            preview_root_path: path.clone(),
            preferred_preview_path: None,
            selection_path: path.clone(),
            metadata_path: path,
            search_text: vec![id.to_string()],
            kind: NavigateEntryKind::Project,
        }
    }

    fn test_directory(label: &str) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("current timestamp")
            .as_nanos();
        let path = env::temp_dir().join(format!(
            "navgator-usage-{label}-{}-{timestamp}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("test directory");
        path
    }
}
