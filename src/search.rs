use crate::model::{MatchScore, NavigateEntry, SortMeta, SortMode};
use gator::fuzzy_match;
use std::{cmp::Ordering, collections::HashMap, path::Path};

#[derive(Default)]
pub(crate) struct QueryTokens {
    pub(crate) folder: Vec<String>,
    pub(crate) tags: Vec<String>,
    pub(crate) any: Vec<String>,
}

impl QueryTokens {
    pub(crate) fn is_empty(&self) -> bool {
        self.folder.is_empty() && self.tags.is_empty() && self.any.is_empty()
    }

    pub(crate) fn needs_tags(&self) -> bool {
        !self.tags.is_empty()
    }
}

pub(crate) fn parse_query_tokens(query: &str) -> QueryTokens {
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

pub(crate) fn filter_and_sort(
    entries: &[NavigateEntry],
    query: &str,
    sort_mode: SortMode,
    meta_cache: &HashMap<String, SortMeta>,
    tag_cache: &HashMap<String, Vec<String>>,
) -> Vec<usize> {
    if sort_mode == SortMode::Match {
        return filter_and_sort_by_match(entries, query, tag_cache);
    }
    let mut indices = filter_indices(entries, query, tag_cache);
    sort_indices(&mut indices, entries, sort_mode, meta_cache);
    indices
}

pub(crate) fn index_for_entry_id(
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

pub(crate) fn entry_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|part| part.to_str())
        .unwrap_or(path)
        .to_string()
}

pub(crate) fn entry_match_context(
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

fn match_score(query: &str, text: &str) -> Option<MatchScore> {
    let qchars: Vec<char> = query.chars().filter(|c| !c.is_whitespace()).collect();
    if qchars.is_empty() {
        return Some((0, 0, 0, 0, text.chars().count()));
    }

    if let Some(start) = find_case_insensitive(text, query) {
        let span = qchars.len().saturating_sub(1);
        return Some((0, span, 0, start, text.chars().count()));
    }

    let mut positions: Vec<usize> = Vec::with_capacity(qchars.len());
    let mut qi = 0usize;
    for (ti, t) in text.chars().enumerate() {
        if qi >= qchars.len() {
            break;
        }
        if qchars[qi].eq_ignore_ascii_case(&t) {
            positions.push(ti);
            qi += 1;
        }
    }

    if qi < qchars.len() {
        return None;
    }

    let start = *positions.first().unwrap_or(&0);
    let end = *positions.last().unwrap_or(&start);
    let span = end.saturating_sub(start);
    let mut gaps = 0usize;
    for window in positions.windows(2) {
        if let [prev, next] = window {
            gaps = gaps.saturating_add(next.saturating_sub(prev + 1));
        }
    }
    let text_len = text.chars().count();
    Some((1, span, gaps, start, text_len))
}

fn find_case_insensitive(text: &str, needle: &str) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    let text_lower = text.to_lowercase();
    let needle_lower = needle.to_lowercase();
    let byte_index = text_lower.find(&needle_lower)?;
    Some(char_index_from_byte(text, byte_index))
}

fn char_index_from_byte(text: &str, byte_index: usize) -> usize {
    text.char_indices()
        .take_while(|(idx, _)| *idx < byte_index)
        .count()
}

#[cfg(test)]
mod tests {
    use super::filter_and_sort;
    use crate::model::{NavigateEntry, NavigateEntryKind, SortMeta, SortMode};
    use std::collections::HashMap;

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
                &HashMap::new()
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
            },
        );
        meta_cache.insert(
            "/repos/$RECYCLE.BIN".to_string(),
            SortMeta {
                modified_epoch: None,
                created_epoch: None,
            },
        );

        assert_eq!(
            filter_and_sort(
                &entries,
                "",
                SortMode::ModifiedDesc,
                &meta_cache,
                &HashMap::new()
            ),
            vec![1, 0]
        );
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
