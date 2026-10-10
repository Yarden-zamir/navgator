use crate::model::{
    MatchScore, NavigateEntry, NavigateEntryKind, SortMeta, SortMode, SortSettings,
};
use gator::fuzzy_match;
use gator::search::match_score;
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    path::Path,
};

#[derive(Default)]
pub struct QueryTokens {
    pub folder: Vec<String>,
    pub tags: Vec<String>,
    pub any: Vec<String>,
}

impl QueryTokens {
    pub fn is_empty(&self) -> bool {
        self.folder.is_empty() && self.tags.is_empty() && self.any.is_empty()
    }

    pub fn needs_tags(&self) -> bool {
        !self.tags.is_empty()
    }
}

pub fn parse_query_tokens(query: &str) -> QueryTokens {
    let mut tokens = QueryTokens::default();
    for raw in query.split_whitespace() {
        if let Some(rest) = raw.strip_prefix('@') {
            if !rest.is_empty() {
                tokens.folder.push(rest.to_string());
            }
        } else if let Some(rest) = raw.strip_prefix('#') {
            if !rest.is_empty() {
                tokens.tags.push(rest.to_string());
            }
        } else if !raw.is_empty() {
            tokens.any.push(raw.to_string());
        }
    }
    tokens
}

pub fn filter_and_sort(
    entries: &[NavigateEntry],
    query: &str,
    sort_mode: SortMode,
    meta_cache: &HashMap<String, SortMeta>,
    tag_cache: &HashMap<String, Vec<String>>,
    ignored_paths: Option<&std::collections::HashSet<String>>,
) -> Vec<usize> {
    let mut indices = if sort_mode == SortMode::Match {
        filter_and_sort_by_match(entries, query, tag_cache)
    } else {
        let mut indices = filter_indices(entries, query, tag_cache);
        sort_indices(&mut indices, entries, sort_mode, meta_cache);
        indices
    };
    if let Some(ignored_paths) = ignored_paths {
        indices.sort_by_key(|index| ignored_paths.contains(&entries[*index].metadata_path));
    }
    indices
}

#[derive(Clone, Copy)]
pub struct ResultSortContext<'a> {
    pub show_remote_branches: bool,
    pub sort_settings: &'a SortSettings,
    pub current_project_path: Option<&'a str>,
}

pub fn filter_and_sort_visible(
    entries: &[NavigateEntry],
    query: &str,
    sort_mode: SortMode,
    meta_cache: &HashMap<String, SortMeta>,
    tag_cache: &HashMap<String, Vec<String>>,
    context: ResultSortContext<'_>,
) -> Vec<usize> {
    let ignored_paths = context.sort_settings.ignored_paths(sort_mode);
    let mut indices: Vec<usize> = filter_and_sort(
        entries,
        query,
        sort_mode,
        meta_cache,
        tag_cache,
        ignored_paths,
    )
    .into_iter()
    .filter(|index| context.show_remote_branches || !is_remote_branch_entry(&entries[*index]))
    .collect();
    if query.trim().is_empty() {
        let pinned_path = context
            .current_project_path
            .filter(|_| context.sort_settings.pin_current_project);
        pin_current_project(&mut indices, entries, pinned_path, ignored_paths);
    }
    indices
}

pub fn pin_current_project(
    indices: &mut Vec<usize>,
    entries: &[NavigateEntry],
    pinned_path: Option<&str>,
    ignored_paths: Option<&HashSet<String>>,
) {
    let Some(pinned_path) = pinned_path else {
        return;
    };
    let Some(position) = indices.iter().position(|index| {
        let entry = &entries[*index];
        let ignored = ignored_paths
            .map(|paths| paths.contains(&entry.metadata_path))
            .unwrap_or(false);
        !ignored && entry_matches_path(entry, pinned_path)
    }) else {
        return;
    };
    let index = indices.remove(position);
    indices.insert(0, index);
}

pub fn entry_matches_path(entry: &NavigateEntry, path: &str) -> bool {
    entry.selection_path == path
        || entry.metadata_path == path
        || entry.preview_root_path == path
        || entry.preferred_preview_path.as_deref() == Some(path)
}

pub fn visible_entry_count(entries: &[NavigateEntry], show_remote_branches: bool) -> usize {
    entries
        .iter()
        .filter(|entry| show_remote_branches || !is_remote_branch_entry(entry))
        .count()
}

pub fn is_remote_branch_entry(entry: &NavigateEntry) -> bool {
    matches!(entry.kind, NavigateEntryKind::RemoteBranch { .. })
}

pub fn index_for_entry_id(
    entries: &[NavigateEntry],
    filtered: &[usize],
    id: &str,
) -> Option<usize> {
    filtered.iter().position(|index| {
        entries
            .get(*index)
            .map(|candidate| candidate.id == id)
            .unwrap_or(false)
    })
}

/// One flag per char of `value`: true where a query token matches.
pub fn highlight_mask(value: &str, tokens: &[&str]) -> Vec<bool> {
    let mut highlighted = vec![false; value.chars().count()];
    for token in tokens {
        for index in match_indices(token, value) {
            if let Some(slot) = highlighted.get_mut(index) {
                *slot = true;
            }
        }
    }
    highlighted
}

pub fn match_indices(query: &str, value: &str) -> Vec<usize> {
    exact_match_indices(query, value).unwrap_or_else(|| fuzzy_match_indices(query, value))
}

fn exact_match_indices(query: &str, value: &str) -> Option<Vec<usize>> {
    if query.is_empty() {
        return Some(Vec::new());
    }
    let value_lower = value.to_lowercase();
    let query_lower = query.to_lowercase();
    let start_byte = value_lower.find(&query_lower)?;
    let start = value
        .char_indices()
        .take_while(|(byte_index, _)| *byte_index < start_byte)
        .count();
    let len = query.chars().count();
    Some((start..start + len).collect())
}

fn fuzzy_match_indices(query: &str, value: &str) -> Vec<usize> {
    let mut query_chars = query.chars().filter(|ch| !ch.is_whitespace());
    let mut current = query_chars.next();
    if current.is_none() {
        return Vec::new();
    }

    let mut matches = Vec::new();
    for (index, ch) in value.chars().enumerate() {
        let Some(expected) = current else {
            break;
        };
        if expected.eq_ignore_ascii_case(&ch) {
            matches.push(index);
            current = query_chars.next();
            if current.is_none() {
                return matches;
            }
        }
    }
    Vec::new()
}

pub fn entry_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|part| part.to_str())
        .unwrap_or(path)
        .to_string()
}

pub fn entry_match_context(
    entry: &NavigateEntry,
    tags: &[String],
    tokens: &QueryTokens,
) -> Option<String> {
    if tokens.is_empty() {
        return None;
    }

    let mut contexts = Vec::new();
    for token in tokens.folder.iter().chain(tokens.any.iter()) {
        if fuzzy_match(token, &entry.display) {
            continue;
        }
        if let Some(context) = entry
            .context
            .as_ref()
            .filter(|context| fuzzy_match(token, context))
        {
            contexts.push(context.clone());
            continue;
        }
    }
    for token in &tokens.tags {
        if let Some(tag) = tags.iter().find(|tag| fuzzy_match(token, tag)) {
            contexts.push(format!("tag: {tag}"));
        }
    }

    contexts.dedup();
    contexts.into_iter().next()
}

fn filter_indices(
    entries: &[NavigateEntry],
    query: &str,
    tag_cache: &HashMap<String, Vec<String>>,
) -> Vec<usize> {
    let tokens = parse_query_tokens(query);
    if tokens.is_empty() {
        return (0..entries.len()).collect();
    }
    entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            let tags = tag_cache
                .get(&entry.metadata_path)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            if matches_entry_tokens(entry, tags, &tokens) {
                Some(index)
            } else {
                None
            }
        })
        .collect()
}

fn filter_and_sort_by_match(
    entries: &[NavigateEntry],
    query: &str,
    tag_cache: &HashMap<String, Vec<String>>,
) -> Vec<usize> {
    let tokens = parse_query_tokens(query);
    if tokens.is_empty() {
        return (0..entries.len()).collect();
    }
    let mut scored: Vec<(usize, MatchScore)> = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        let tags = tag_cache
            .get(&entry.metadata_path)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        if !matches_entry_tokens(entry, tags, &tokens) {
            continue;
        }
        if let Some(score) = match_score_entry_tokens(&tokens, entry, tags) {
            scored.push((index, score));
        }
    }
    scored.sort_by(|(left_idx, left), (right_idx, right)| {
        left.cmp(right).then_with(|| {
            compare_entry_names(&entries[*left_idx], &entries[*right_idx])
                .then_with(|| left_idx.cmp(right_idx))
        })
    });
    scored.into_iter().map(|(index, _)| index).collect()
}

fn matches_entry_tokens(entry: &NavigateEntry, tags: &[String], tokens: &QueryTokens) -> bool {
    for token in &tokens.folder {
        if !matches_entry_text_token(token, entry) {
            return false;
        }
    }
    for token in &tokens.tags {
        if !tags.iter().any(|tag| fuzzy_match(token, tag)) {
            return false;
        }
    }
    for token in &tokens.any {
        let entry_match = matches_entry_text_token(token, entry);
        if !entry_match {
            return false;
        }
    }
    true
}

fn matches_entry_text_token(token: &str, entry: &NavigateEntry) -> bool {
    searchable_text(entry).any(|text| fuzzy_match(token, text))
}

fn match_score_entry_tokens(
    tokens: &QueryTokens,
    entry: &NavigateEntry,
    tags: &[String],
) -> Option<MatchScore> {
    let mut penalty_sum = 0usize;
    let mut span_sum = 0usize;
    let mut gap_sum = 0usize;
    let mut start_sum = 0usize;
    let mut len_sum = 0usize;

    for token in &tokens.folder {
        let score = match_score_for_entry(token, entry)?;
        penalty_sum += score.0;
        span_sum += score.1;
        gap_sum += score.2;
        start_sum += score.3;
        len_sum += score.4;
    }
    for token in &tokens.tags {
        let score = best_tag_score(token, tags)?;
        penalty_sum += score.0;
        span_sum += score.1;
        gap_sum += score.2;
        start_sum += score.3;
        len_sum += score.4;
    }
    for token in &tokens.any {
        let score = match_score_for_entry(token, entry)?;
        penalty_sum += score.0;
        span_sum += score.1;
        gap_sum += score.2;
        start_sum += score.3;
        len_sum += score.4;
    }

    Some((penalty_sum, span_sum, gap_sum, start_sum, len_sum))
}

fn match_score_for_entry(token: &str, entry: &NavigateEntry) -> Option<MatchScore> {
    searchable_text(entry)
        .filter_map(|text| match_score(token, text))
        .min()
}

fn searchable_text(entry: &NavigateEntry) -> impl Iterator<Item = &str> {
    entry
        .search_text
        .iter()
        .map(String::as_str)
        .chain(entry.context.as_deref())
}

fn best_tag_score(token: &str, tags: &[String]) -> Option<MatchScore> {
    let mut best: Option<MatchScore> = None;
    for tag in tags {
        if let Some(score) = match_score(token, tag) {
            best = match best {
                Some(existing) => Some(existing.min(score)),
                None => Some(score),
            };
        }
    }
    best
}

fn sort_indices(
    indices: &mut [usize],
    entries: &[NavigateEntry],
    sort_mode: SortMode,
    meta_cache: &HashMap<String, SortMeta>,
) {
    indices.sort_by(|left, right| compare_indices(*left, *right, entries, sort_mode, meta_cache));
}

fn compare_indices(
    left: usize,
    right: usize,
    entries: &[NavigateEntry],
    sort_mode: SortMode,
    meta_cache: &HashMap<String, SortMeta>,
) -> Ordering {
    let left_entry = &entries[left];
    let right_entry = &entries[right];
    let left_path = &left_entry.metadata_path;
    let right_path = &right_entry.metadata_path;
    match sort_mode {
        SortMode::Match => Ordering::Equal,
        SortMode::AlphaAsc => {
            compare_entry_names(left_entry, right_entry).then_with(|| left.cmp(&right))
        }
        SortMode::AlphaDesc => {
            compare_entry_names(right_entry, left_entry).then_with(|| left.cmp(&right))
        }
        SortMode::CreatedAsc => {
            compare_time(left_path, right_path, meta_cache, TimeField::Created, false)
                .then_with(|| compare_entry_names(left_entry, right_entry))
        }
        SortMode::CreatedDesc => {
            compare_time(left_path, right_path, meta_cache, TimeField::Created, true)
                .then_with(|| compare_entry_names(left_entry, right_entry))
        }
        SortMode::ModifiedAsc => compare_time(
            left_path,
            right_path,
            meta_cache,
            TimeField::Modified,
            false,
        )
        .then_with(|| compare_entry_names(left_entry, right_entry)),
        SortMode::ModifiedDesc => {
            compare_time(left_path, right_path, meta_cache, TimeField::Modified, true)
                .then_with(|| compare_entry_names(left_entry, right_entry))
        }
        SortMode::AccessedDesc => {
            compare_time(left_path, right_path, meta_cache, TimeField::Accessed, true)
                .then_with(|| compare_entry_names(left_entry, right_entry))
        }
        SortMode::Recents => {
            compare_time(left_path, right_path, meta_cache, TimeField::Recent, true)
                .then_with(|| compare_entry_names(left_entry, right_entry))
        }
    }
}

fn compare_entry_names(left: &NavigateEntry, right: &NavigateEntry) -> Ordering {
    left.display
        .to_lowercase()
        .cmp(&right.display.to_lowercase())
}

enum TimeField {
    Created,
    Modified,
    Accessed,
    Recent,
}

fn compare_time(
    left_path: &str,
    right_path: &str,
    meta_cache: &HashMap<String, SortMeta>,
    field: TimeField,
    descending: bool,
) -> Ordering {
    let value = |path: &str| -> Option<i64> {
        let meta = meta_cache.get(path)?;
        match field {
            TimeField::Created => meta.created_epoch,
            TimeField::Modified => meta.modified_epoch,
            TimeField::Accessed => meta.accessed_epoch,
            TimeField::Recent => meta.recent_epoch(),
        }
    };

    let left_value = value(left_path);
    let right_value = value(right_path);
    match (left_value, right_value) {
        (Some(left), Some(right)) if descending => right.cmp(&left),
        (Some(left), Some(right)) => left.cmp(&right),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

#[cfg(test)]
mod tests {
    use super::{filter_and_sort, pin_current_project};
    use crate::model::{NavigateEntry, NavigateEntryKind, SortMeta, SortMode};
    use std::collections::{HashMap, HashSet};

    #[test]
    fn searches_visible_context_text() {
        let entry = NavigateEntry {
            id: "remote".to_string(),
            display: " repo origin/feature".to_string(),
            context: Some("12m - Alice".to_string()),
            preview_root_path: "remote:/repo.git:origin/feature".to_string(),
            preferred_preview_path: None,
            selection_path: "/repo-feature".to_string(),
            metadata_path: "/repo-feature".to_string(),
            search_text: vec![
                " repo origin/feature".to_string(),
                "12m - Alice".to_string(),
            ],
            kind: NavigateEntryKind::RemoteBranch {
                repo_label: "repo".to_string(),
                branch: "feature".to_string(),
                remote_branch: "origin/feature".to_string(),
                bare_path: "/repo.git".to_string(),
                container_path: "/".to_string(),
            },
        };

        assert_eq!(
            filter_and_sort(
                &[entry],
                "Alice",
                SortMode::Match,
                &HashMap::new(),
                &HashMap::new(),
                None
            ),
            vec![0]
        );
    }

    #[test]
    fn modified_desc_keeps_unknown_metadata_after_known_paths() {
        let entries = vec![
            test_entry("recycle", "$RECYCLE.BIN", "/repos/$RECYCLE.BIN"),
            test_entry("project", "project", "/repos/project"),
        ];
        let mut meta_cache = HashMap::new();
        meta_cache.insert(
            "/repos/project".to_string(),
            SortMeta {
                modified_epoch: Some(200),
                created_epoch: Some(100),
                accessed_epoch: None,
            },
        );
        meta_cache.insert(
            "/repos/$RECYCLE.BIN".to_string(),
            SortMeta {
                modified_epoch: None,
                created_epoch: None,
                accessed_epoch: None,
            },
        );

        assert_eq!(
            filter_and_sort(
                &entries,
                "",
                SortMode::ModifiedDesc,
                &meta_cache,
                &HashMap::new(),
                None
            ),
            vec![1, 0]
        );
    }

    #[test]
    fn accessed_desc_uses_only_successful_target_access_time() {
        let entries = vec![
            test_entry("modified", "modified", "/repos/modified"),
            test_entry("accessed", "accessed", "/repos/accessed"),
            test_entry("unknown", "unknown", "/repos/unknown"),
        ];
        let meta_cache = HashMap::from([
            (
                "/repos/modified".to_string(),
                SortMeta {
                    modified_epoch: Some(500),
                    created_epoch: None,
                    accessed_epoch: Some(100),
                },
            ),
            (
                "/repos/accessed".to_string(),
                SortMeta {
                    modified_epoch: Some(200),
                    created_epoch: None,
                    accessed_epoch: Some(400),
                },
            ),
        ]);

        assert_eq!(
            filter_and_sort(
                &entries,
                "",
                SortMode::AccessedDesc,
                &meta_cache,
                &HashMap::new(),
                None
            ),
            vec![1, 0, 2]
        );
    }

    #[test]
    fn recents_uses_the_latest_modified_or_accessed_time() {
        let entries = vec![
            test_entry("modified", "modified", "/repos/modified"),
            test_entry("accessed", "accessed", "/repos/accessed"),
            test_entry("unknown", "unknown", "/repos/unknown"),
        ];
        let meta_cache = HashMap::from([
            (
                "/repos/modified".to_string(),
                SortMeta {
                    modified_epoch: Some(500),
                    created_epoch: None,
                    accessed_epoch: Some(100),
                },
            ),
            (
                "/repos/accessed".to_string(),
                SortMeta {
                    modified_epoch: Some(200),
                    created_epoch: None,
                    accessed_epoch: Some(600),
                },
            ),
        ]);

        assert_eq!(
            filter_and_sort(
                &entries,
                "",
                SortMode::Recents,
                &meta_cache,
                &HashMap::new(),
                None
            ),
            vec![1, 0, 2]
        );
    }

    #[test]
    fn ignored_paths_sort_after_non_ignored_paths() {
        let entries = vec![
            test_entry("ignored-new", "ignored-new", "/repos/ignored-new"),
            test_entry("normal-old", "normal-old", "/repos/normal-old"),
            test_entry("ignored-old", "ignored-old", "/repos/ignored-old"),
            test_entry("normal-new", "normal-new", "/repos/normal-new"),
        ];
        let meta_cache = HashMap::from([
            (
                "/repos/ignored-new".to_string(),
                SortMeta {
                    modified_epoch: Some(700),
                    created_epoch: None,
                    accessed_epoch: Some(800),
                },
            ),
            (
                "/repos/normal-old".to_string(),
                SortMeta {
                    modified_epoch: Some(100),
                    created_epoch: None,
                    accessed_epoch: Some(200),
                },
            ),
            (
                "/repos/ignored-old".to_string(),
                SortMeta {
                    modified_epoch: Some(300),
                    created_epoch: None,
                    accessed_epoch: Some(400),
                },
            ),
            (
                "/repos/normal-new".to_string(),
                SortMeta {
                    modified_epoch: Some(500),
                    created_epoch: None,
                    accessed_epoch: Some(600),
                },
            ),
        ]);
        let ignored_paths = std::collections::HashSet::from([
            "/repos/ignored-new".to_string(),
            "/repos/ignored-old".to_string(),
        ]);

        assert_eq!(
            filter_and_sort(
                &entries,
                "",
                SortMode::Recents,
                &meta_cache,
                &HashMap::new(),
                Some(&ignored_paths)
            ),
            vec![3, 1, 0, 2]
        );
    }

    #[test]
    fn ignored_current_project_is_not_pinned_above_other_results() {
        let entries = vec![
            test_entry("normal", "normal", "/repos/normal"),
            test_entry("ignored", "ignored", "/repos/ignored"),
        ];
        let ignored_paths = HashSet::from(["/repos/ignored".to_string()]);
        let mut indices = vec![0, 1];

        pin_current_project(
            &mut indices,
            &entries,
            Some("/repos/ignored"),
            Some(&ignored_paths),
        );

        assert_eq!(indices, vec![0, 1]);
    }

    fn test_entry(id: &str, display: &str, path: &str) -> NavigateEntry {
        NavigateEntry {
            id: id.to_string(),
            display: display.to_string(),
            context: None,
            preview_root_path: path.to_string(),
            preferred_preview_path: None,
            selection_path: path.to_string(),
            metadata_path: path.to_string(),
            search_text: vec![display.to_string()],
            kind: NavigateEntryKind::Project,
        }
    }
}
