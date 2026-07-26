//! Provider caches and background job plumbing, built on gator's shared XDG
//! and worker-pool primitives.

use serde::{de::DeserializeOwned, Serialize};
use std::{io, path::PathBuf};

pub(crate) use gator::xdg::{spawn_batched_jobs, unix_timestamp};

const CACHE_DIR_NAME: &str = "navgator";

fn cache_path(file_name: &str) -> PathBuf {
    gator::xdg::cache_file(CACHE_DIR_NAME, file_name)
}

pub(crate) fn load_json_cache<T: DeserializeOwned>(file_name: &str) -> Option<T> {
    gator::xdg::read_json_opt(&cache_path(file_name))
}

pub(crate) fn save_json_cache<T: Serialize>(file_name: &str, value: &T) -> io::Result<()> {
    gator::xdg::write_json_atomic(&cache_path(file_name), value)
}
