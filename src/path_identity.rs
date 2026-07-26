use std::{
    env, io,
    path::{Component, Path, PathBuf},
};

pub(crate) fn path_key(path: &str) -> io::Result<String> {
    let path = PathBuf::from(path);
    let absolute = if path.is_absolute() {
        path
    } else {
        env::current_dir()?.join(path)
    };
    let normalized = absolute
        .canonicalize()
        .unwrap_or_else(|_| normalize_absolute_path(&absolute));
    Ok(normalized.to_string_lossy().into_owned())
}

fn normalize_absolute_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }
    normalized
}
