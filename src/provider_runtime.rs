use serde::{de::DeserializeOwned, Serialize};
use std::{
    collections::VecDeque,
    env, fs, io,
    path::PathBuf,
    sync::{mpsc, Arc, Mutex},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const CACHE_DIR_NAME: &str = "navgator";

pub(crate) fn cache_path(file_name: &str) -> PathBuf {
    let base = env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .unwrap_or_else(env::temp_dir);
    base.join(CACHE_DIR_NAME).join(file_name)
}

pub(crate) fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

pub(crate) fn load_json_cache<T: DeserializeOwned>(file_name: &str) -> Option<T> {
    let path = cache_path(file_name);
    let contents = fs::read_to_string(path).ok()?;
    serde_json::from_str(&contents).ok()
}

pub(crate) fn save_json_cache<T: Serialize>(file_name: &str, value: &T) -> io::Result<()> {
    let path = cache_path(file_name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    let contents = serde_json::to_string(value)?;
    fs::write(&tmp, contents)?;
    fs::rename(tmp, path)
}

pub(crate) fn worker_count(job_count: usize) -> usize {
    let parallel = thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(4);
    job_count.min(parallel).clamp(1, 8)
}

pub(crate) fn spawn_batched_jobs<Job, Output, Run>(
    jobs: Vec<Job>,
    batch_size: usize,
    batch_delay: Duration,
    batch_tx: mpsc::Sender<Vec<Output>>,
    run_job: Run,
) where
    Job: Send + 'static,
    Output: Send + 'static,
    Run: Fn(Job) -> Vec<Output> + Send + Sync + 'static,
{
    let worker_count = worker_count(jobs.len());
    let queue = Arc::new(Mutex::new(VecDeque::from(jobs)));
    let run_job = Arc::new(run_job);
    let (item_tx, item_rx) = mpsc::channel::<Output>();

    for _ in 0..worker_count {
        let queue = Arc::clone(&queue);
        let run_job = Arc::clone(&run_job);
        let item_tx = item_tx.clone();
        thread::spawn(move || loop {
            let job = {
                let mut queue = queue.lock().expect("job queue lock should not be poisoned");
                queue.pop_front()
            };
            let Some(job) = job else {
                break;
            };
            for item in run_job(job) {
                let _ = item_tx.send(item);
            }
        });
    }
    drop(item_tx);

    thread::spawn(move || {
        let mut batch = Vec::new();
        loop {
            match item_rx.recv_timeout(batch_delay) {
                Ok(item) => {
                    batch.push(item);
                    if batch.len() >= batch_size {
                        let _ = batch_tx.send(std::mem::take(&mut batch));
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if !batch.is_empty() {
                        let _ = batch_tx.send(std::mem::take(&mut batch));
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    if !batch.is_empty() {
                        let _ = batch_tx.send(batch);
                    }
                    break;
                }
            }
        }
    });
}
