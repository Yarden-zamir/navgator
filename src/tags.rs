use crate::model::{AppResult, TagResult};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
    sync::mpsc,
    thread,
};
use tui_input::Input;

pub(crate) fn ensure_tags_for_paths(
    paths: &[String],
    cache: &HashMap<String, Vec<String>>,
    in_flight: &mut HashSet<String>,
    tx: &mpsc::Sender<TagResult>,
) {
    for path in paths {
        if cache.contains_key(path) || in_flight.contains(path) {
            continue;
        }
        in_flight.insert(path.clone());
        let path_owned = path.clone();
        let tx = tx.clone();
        thread::spawn(move || {
            let tags = read_tags_for_path(&path_owned);
            let _ = tx.send(TagResult {
                path: path_owned,
                tags,
            });
        });
    }
}

pub(crate) fn spawn_bulk_tag_fetch(
    items: &[String],
    cache: &HashMap<String, Vec<String>>,
    in_flight: &mut HashSet<String>,
    tx: &mpsc::Sender<TagResult>,
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
            let tags = read_tags_for_path(&path);
            let _ = tx.send(TagResult { path, tags });
        }
    });
}

pub(crate) fn read_tags_for_path(path: &str) -> Vec<String> {
    let dir = Path::new(path);
    let config_path = dir.join(".navgator.toml");
    if !config_path.is_file() {
        return Vec::new();
    }
    let contents = match fs::read_to_string(config_path) {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };
    parse_tags_from_toml(&contents)
}

pub(crate) fn collect_tag_suggestions(tag_cache: &HashMap<String, Vec<String>>) -> Vec<String> {
    let mut set = HashSet::new();
    for tags in tag_cache.values() {
        for tag in tags {
            if tag.starts_with("org/") {
                continue;
            }
            set.insert(tag.clone());
        }
    }
    let mut list: Vec<String> = set.into_iter().collect();
    list.sort();
    list
}

pub(crate) fn commit_tag_input(input: &mut Input, tags: &mut Vec<String>, suggestions: &[String]) {
    let raw = input.value().trim();
    if raw.is_empty() {
        return;
    }
    let mut chosen = raw.to_string();
    let lower = raw.to_lowercase();
    if let Some(match_tag) = suggestions
        .iter()
        .find(|tag| tag.to_lowercase().starts_with(&lower))
    {
        chosen = match_tag.clone();
    }
    if !tags.iter().any(|tag| tag == &chosen) {
        tags.push(chosen);
    }
    input.reset();
}

pub(crate) fn save_tags_for_path(path: &str, tags: &[String]) -> AppResult<()> {
    let dir = Path::new(path);
    let config_path = dir.join(".navgator.toml");
    let contents = if config_path.exists() {
        fs::read_to_string(&config_path)?
    } else {
        String::new()
    };
    let updated = write_tags_into_toml(&contents, tags);
    fs::write(config_path, updated)?;
    Ok(())
}

fn parse_tags_from_toml(contents: &str) -> Vec<String> {
    let mut in_tags = false;
    let mut buffer = String::new();
    for line in contents.lines() {
        let mut cleaned = line;
        if let Some(hash) = cleaned.find('#') {
            cleaned = &cleaned[..hash];
        }
        let trimmed = cleaned.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !in_tags {
            if let Some(eq_pos) = trimmed.find('=') {
                let key = trimmed[..eq_pos].trim();
                if key == "tags" {
                    let value = trimmed[eq_pos + 1..].trim();
                    buffer.push_str(value);
                    buffer.push(' ');
                    if value.contains('[') {
                        in_tags = true;
                    }
                    if value.contains(']') {
                        break;
                    }
                }
            }
        } else {
            buffer.push_str(trimmed);
            buffer.push(' ');
            if trimmed.contains(']') {
                break;
            }
        }
    }

    if buffer.is_empty() {
        return Vec::new();
    }
    extract_quoted_strings(&buffer)
}

fn extract_quoted_strings(value: &str) -> Vec<String> {
    let mut tags = Vec::new();
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '"' {
            let mut text = String::new();
            for next in chars.by_ref() {
                if next == '"' {
                    break;
                }
                text.push(next);
            }
            if !text.is_empty() {
                tags.push(text);
            }
        }
    }
    tags
}

fn write_tags_into_toml(contents: &str, tags: &[String]) -> String {
    let line = format!("tags = [{}]", format_tags(tags));
    if contents.trim().is_empty() {
        return format!("{}\n", line);
    }

    let mut lines: Vec<String> = contents.lines().map(|line| line.to_string()).collect();
    let mut start = None;
    let mut end = None;
    for (idx, raw) in lines.iter().enumerate() {
        let cleaned = raw.split('#').next().unwrap_or("");
        if start.is_none() {
            if let Some(eq) = cleaned.find('=') {
                let key = cleaned[..eq].trim();
                if key == "tags" {
                    start = Some(idx);
                    if cleaned.contains(']') {
                        end = Some(idx);
                        break;
                    }
                }
            }
        } else if cleaned.contains(']') {
            end = Some(idx);
            break;
        }
    }

    if start.is_none() {
        let mut out = contents.trim_end().to_string();
        out.push('\n');
        out.push_str(&line);
        out.push('\n');
        return out;
    }

    let start = start.unwrap();
    let end = end.unwrap_or(start);
    lines.splice(start..=end, [line.to_string()]);
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

fn format_tags(tags: &[String]) -> String {
    tags.iter()
        .map(|tag| format!("\"{}\"", tag.replace('"', "\\\"")))
        .collect::<Vec<String>>()
        .join(", ")
}
