use crate::model::{MetaResult, DATE_WIDTH};
use std::{
    collections::{HashMap, HashSet},
    fs,
    sync::mpsc,
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

pub(crate) fn ensure_dates_for_paths(
    paths: &[String],
    cache: &HashMap<String, String>,
    in_flight: &mut HashSet<String>,
    tx: &mpsc::Sender<MetaResult>,
) {
    for path in paths {
        if cache.contains_key(path) || in_flight.contains(path) {
            continue;
        }
        in_flight.insert(path.clone());
        let path_owned = path.clone();
        let tx = tx.clone();
        thread::spawn(move || {
            let meta = fetch_metadata(&path_owned);
            let _ = tx.send(meta);
        });
    }
}

pub(crate) fn spawn_bulk_metadata_fetch(
    items: &[String],
    cache: &HashMap<String, String>,
    in_flight: &mut HashSet<String>,
    tx: &mpsc::Sender<MetaResult>,
) {
    let mut missing = Vec::new();
    for path in items {
        if cache.contains_key(path) || in_flight.contains(path) {
            continue;
        }
        in_flight.insert(path.clone());
        missing.push(path.clone());
    }
    if missing.is_empty() {
        return;
    }
    let tx = tx.clone();
    thread::spawn(move || {
        for path in missing {
            let meta = fetch_metadata(&path);
            let _ = tx.send(meta);
        }
    });
}

pub(crate) fn format_date_display(value: &str) -> String {
    let mut text = value.to_string();
    if text.len() > DATE_WIDTH {
        text.truncate(DATE_WIDTH);
    } else if text.len() < DATE_WIDTH {
        text = format!("{:>width$}", text, width = DATE_WIDTH);
    }
    text
}

fn fetch_metadata(path: &str) -> MetaResult {
    let metadata = fs::metadata(path).ok();
    let modified_epoch = metadata
        .as_ref()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(system_time_epoch);
    let created_epoch = metadata
        .as_ref()
        .and_then(|metadata| metadata.created().ok())
        .and_then(system_time_epoch);
    let display = modified_epoch.map(format_epoch_minutes);

    MetaResult {
        path: path.to_string(),
        display,
        modified_epoch,
        created_epoch,
    }
}

fn system_time_epoch(value: SystemTime) -> Option<i64> {
    value
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs() as i64)
        .filter(|value| *value > 0)
}

fn format_epoch_minutes(epoch: i64) -> String {
    let timestamp = epoch as libc::time_t;
    let mut tm = std::mem::MaybeUninit::<libc::tm>::uninit();
    let local_time = unsafe { libc::localtime_r(&timestamp, tm.as_mut_ptr()) };
    if local_time.is_null() {
        return epoch.to_string();
    }
    let tm = unsafe { tm.assume_init() };
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min
    )
}
