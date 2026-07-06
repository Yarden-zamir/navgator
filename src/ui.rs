use crate::metadata::format_date_display;
use crate::model::{
    Focus, HelpColors, HelpContext, PreviewSettings, RemoteToggleState, SidePanelRender, UiLayout,
    VisibleListArgs, DATE_PLACEHOLDER, MIN_PARTIAL_TAB_WIDTH, TAB_DIVIDER_WIDTH,
};
use crate::search::{entry_match_context, QueryTokens};
use gator::fuzzy_match;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, ListItem, Paragraph, Tabs, Wrap},
};
use tui_input::Input;

pub(crate) fn build_help_line(context: HelpContext, colors: HelpColors) -> Line<'static> {
    let key_style = Style::default()
        .fg(colors.key_color)
        .add_modifier(Modifier::BOLD);
    let label_style = Style::default()
        .fg(colors.accent)
        .add_modifier(Modifier::BOLD);
    let regular_style = Style::default().fg(colors.text);
    let (remote_label, remote_style) = remote_toggle_help(context.remote_state, colors);
    let mut spans: Vec<Span> = Vec::new();
    let has_prev_preview = context.preview_tab_index > 0;
    let has_next_preview = context.preview_tab_index + 1 < context.preview_tab_count;
    let has_prev_detail = context.detail_tab_index > 0;
    let has_next_detail = context.detail_tab_index + 1 < context.detail_tab_count;

    match context.focus {
        Focus::Search => {
            spans.push(Span::styled("Search", label_style));
            spans.push(Span::styled("  ", regular_style));
            if context.cursor_at_end {
                spans.push(Span::styled("Right", key_style));
                spans.push(Span::styled(" preview  ", regular_style));
            }
            spans.push(Span::styled("Ctrl+T", key_style));
            spans.push(Span::styled(" tag  ", regular_style));
            spans.push(Span::styled("Ctrl+S", key_style));
            spans.push(Span::styled(
                format!(" {}  ", context.sort_mode.label()),
                regular_style,
            ));
            spans.push(Span::styled("Ctrl+U", key_style));
            spans.push(Span::styled(" clear  ", regular_style));
            spans.push(Span::styled("Ctrl+Y", key_style));
            spans.push(Span::styled(" copy  ", regular_style));
            spans.push(Span::styled("Ctrl+Enter", key_style));
            spans.push(Span::styled(" actions  ", regular_style));
            if context.can_delete_worktree {
                spans.push(Span::styled("Ctrl+D", key_style));
                spans.push(Span::styled(" delete  ", regular_style));
            }
            spans.push(Span::styled("Ctrl+O", remote_style));
            spans.push(Span::styled(format!(" {remote_label}"), remote_style));
        }
        Focus::Preview => {
            let label = if context.preview_tab_count > 1 {
                format!(
                    "Preview {}/{}",
                    context.preview_tab_index + 1,
                    context.preview_tab_count
                )
            } else {
                "Preview".to_string()
            };
            spans.push(Span::styled(label, label_style));
            spans.push(Span::styled("  ", regular_style));
            spans.push(Span::styled("Left", key_style));
            if has_prev_preview {
                spans.push(Span::styled(" prev  ", regular_style));
            } else {
                spans.push(Span::styled(" search  ", regular_style));
            }
            if has_next_preview {
                spans.push(Span::styled("Right", key_style));
                spans.push(Span::styled(" next  ", regular_style));
            } else if context.show_detail {
                spans.push(Span::styled("Right", key_style));
                spans.push(Span::styled(" detail  ", regular_style));
            }
            spans.push(Span::styled("Ctrl+T", key_style));
            spans.push(Span::styled(" tag  ", regular_style));
            spans.push(Span::styled("Ctrl+Y", key_style));
            spans.push(Span::styled(" copy  ", regular_style));
            spans.push(Span::styled("Ctrl+Enter", key_style));
            spans.push(Span::styled(" actions  ", regular_style));
            if context.can_delete_worktree {
                spans.push(Span::styled("Ctrl+D", key_style));
                spans.push(Span::styled(" delete  ", regular_style));
            }
            spans.push(Span::styled("Ctrl+O", remote_style));
            spans.push(Span::styled(format!(" {remote_label}  "), remote_style));
            if context.preview_scroll == 0 && !has_prev_preview {
                spans.push(Span::styled("Up", key_style));
                spans.push(Span::styled(" search  ", regular_style));
            }
            if has_next_preview && context.preview_scroll >= context.preview_max_scroll {
                spans.push(Span::styled("Down", key_style));
                spans.push(Span::styled(" next", regular_style));
            } else if context.show_detail && context.preview_scroll >= context.preview_max_scroll {
                spans.push(Span::styled("Down", key_style));
                spans.push(Span::styled(" detail", regular_style));
            }
        }
        Focus::Detail => {
            let label = if context.detail_tab_count > 1 {
                format!(
                    "Detail {}/{}",
                    context.detail_tab_index + 1,
                    context.detail_tab_count
                )
            } else {
                "Detail".to_string()
            };
            spans.push(Span::styled(label, label_style));
            spans.push(Span::styled("  ", regular_style));
            spans.push(Span::styled("Left", key_style));
            if has_prev_detail {
                spans.push(Span::styled(" prev  ", regular_style));
            } else {
                spans.push(Span::styled(" preview  ", regular_style));
            }
            spans.push(Span::styled("Right", key_style));
            if has_next_detail {
                spans.push(Span::styled(" next  ", regular_style));
            } else {
                spans.push(Span::styled(" preview  ", regular_style));
            }
            spans.push(Span::styled("Ctrl+T", key_style));
            spans.push(Span::styled(" tag  ", regular_style));
            spans.push(Span::styled("Ctrl+Y", key_style));
            spans.push(Span::styled(" copy  ", regular_style));
            spans.push(Span::styled("Ctrl+Enter", key_style));
            spans.push(Span::styled(" actions  ", regular_style));
            if context.can_delete_worktree {
                spans.push(Span::styled("Ctrl+D", key_style));
                spans.push(Span::styled(" delete  ", regular_style));
            }
            spans.push(Span::styled("Ctrl+O", remote_style));
            spans.push(Span::styled(format!(" {remote_label}  "), remote_style));
            if context.detail_scroll == 0 {
                spans.push(Span::styled("Up", key_style));
                spans.push(Span::styled(" preview", regular_style));
            }
        }
        Focus::TagEdit => {
            spans.push(Span::styled("Tag", label_style));
            spans.push(Span::styled("  ", regular_style));
            spans.push(Span::styled("Tab", key_style));
            spans.push(Span::styled(" add  ", regular_style));
            spans.push(Span::styled("Enter", key_style));
            if context.has_tag_input {
                spans.push(Span::styled(" add+done", regular_style));
            } else {
                spans.push(Span::styled(" done", regular_style));
            }
        }
    }

    Line::from(spans)
}

fn remote_toggle_help(state: RemoteToggleState, colors: HelpColors) -> (&'static str, Style) {
    match state {
        RemoteToggleState::Off => (
            "remote",
            Style::default()
                .fg(colors.key_color)
                .add_modifier(Modifier::BOLD),
        ),
        RemoteToggleState::Fetching => (
            "refreshing",
            Style::default()
                .fg(colors.remote_color)
                .add_modifier(Modifier::BOLD),
        ),
        RemoteToggleState::Active => (
            "remote:on",
            Style::default()
                .fg(colors.remote_color)
                .add_modifier(Modifier::BOLD),
        ),
        RemoteToggleState::Error => (
            "remote:error",
            Style::default()
                .fg(colors.remote_color)
                .add_modifier(Modifier::BOLD),
        ),
    }
}

pub(crate) fn compute_ui_layout(size: Rect, show_detail: bool) -> UiLayout {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(3)])
        .split(size);
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(chunks[0]);

    let list_area = body[0];
    let detail_area = body[1];
    let left_inner = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .inner(list_area);
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(left_inner);
    let search_area = left_chunks[0];
    let results_area = left_chunks[1];

    let (preview_area, detail_panel_area) = if show_detail {
        let panels = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(detail_area);
        (panels[0], Some(panels[1]))
    } else {
        (detail_area, None)
    };

    UiLayout {
        list_area,
        detail_area,
        search_area,
        results_area,
        preview_area,
        detail_panel_area,
        help_area: chunks[1],
    }
}

pub(crate) fn rect_contains(rect: Rect, col: u16, row: u16) -> bool {
    col >= rect.x
        && col < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}

pub(crate) fn preview_content_area(area: Rect, tab_count: usize) -> Rect {
    let mut inner = panel_inner_area(area);
    if tab_count > 1 && inner.height > 0 {
        inner.y = inner.y.saturating_add(1);
        inner.height = inner.height.saturating_sub(1);
    }
    inner
}

pub(crate) fn text_line_count(text: &Text) -> usize {
    text.lines.len()
}

pub(crate) fn build_visible_list_items(
    args: VisibleListArgs<'_>,
) -> (Vec<ListItem<'static>>, Option<usize>) {
    if args.filtered.is_empty() || args.height == 0 {
        let item = ListItem::new(Line::from(Span::styled(
            "No matches",
            Style::default().fg(args.muted),
        )));
        return (vec![item], None);
    }

    let end = (args.offset + args.height).min(args.filtered.len());
    let visible = &args.filtered[args.offset..end];
    let mut list_items = Vec::with_capacity(visible.len());

    for item_index in visible.iter() {
        let entry = &args.entries[*item_index];
        let metadata_path = &entry.metadata_path;
        let display = &entry.display;
        let date_value = args
            .dates
            .get(metadata_path)
            .map(String::as_str)
            .unwrap_or(DATE_PLACEHOLDER);
        let date_display = format_date_display(date_value);
        let date_len = date_display.chars().count();
        let tag_list = args
            .tags
            .get(metadata_path)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let context = entry
            .context
            .clone()
            .or_else(|| entry_match_context(entry, tag_list, args.tokens));
        let context_len = context
            .as_ref()
            .map(|value| value.chars().count() + 1)
            .unwrap_or(0);
        let max_entry = args.inner_width.saturating_sub(date_len + context_len + 1);
        let mut entry_display = truncate_with_ellipsis(display, max_entry);
        let mut entry_len = entry_display.chars().count();
        if entry_len + date_len + context_len + 1 > args.inner_width {
            let new_len = args.inner_width.saturating_sub(date_len + context_len + 1);
            entry_display = truncate_with_ellipsis(display, new_len);
            entry_len = entry_display.chars().count();
        }

        let remaining = args
            .inner_width
            .saturating_sub(entry_len + date_len + context_len);
        let tag_space = if context.is_none() {
            remaining.saturating_sub(1)
        } else {
            0
        };
        let (tag_spans, tag_len) = if tag_space > 0 && context.is_none() {
            build_tag_spans(tag_list, args.tokens, tag_space, args.elapsed_ms, args.text)
        } else {
            (Vec::new(), 0)
        };
        let tag_block_len = if tag_len > 0 { tag_len + 1 } else { 0 };
        let right_block_len = date_len + tag_block_len + context_len;
        let padding = args.inner_width.saturating_sub(entry_len + right_block_len);
        let mut spans = Vec::new();
        spans.extend(highlight_match_spans(
            &entry_display,
            entry_match_tokens(args.tokens),
            args.text,
            args.accent,
        ));
        spans.push(Span::raw(" ".repeat(padding)));
        if let Some(context) = context {
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                context,
                Style::default()
                    .fg(args.accent)
                    .add_modifier(Modifier::BOLD),
            ));
        }
        if tag_len > 0 {
            spans.push(Span::raw(" "));
            spans.extend(tag_spans);
        }
        spans.push(Span::raw(" "));
        spans.push(Span::styled(date_display, Style::default().fg(args.muted)));
        let line = Line::from(spans);
        list_items.push(ListItem::new(line));
    }

    let list_selected = args.selected.checked_sub(args.offset);
    (list_items, list_selected)
}

pub(crate) fn compose_preview_text_with_input(
    base: &Text<'static>,
    tags: &[String],
    input: &Input,
    width: usize,
    text: Color,
) -> (Text<'static>, Option<(usize, usize)>) {
    let tag_lines = build_full_tag_lines(tags, width, text);
    let input_line_index = tag_lines.len();
    let scroll = input.visual_scroll(width.max(1));
    let input_slice = substring_by_char(input.value(), scroll, width.max(1));
    let input_line = Line::from(Span::styled(input_slice, Style::default().fg(text)));
    let cursor_col = input.visual_cursor().max(scroll).saturating_sub(scroll);

    let mut lines = Vec::new();
    lines.extend(tag_lines);
    lines.push(input_line);
    lines.push(Line::from(""));
    lines.extend(base.lines.clone());
    let cursor = Some((input_line_index, cursor_col));
    (Text::from(lines), cursor)
}

pub(crate) fn compose_preview_text(
    base: &Text<'static>,
    tags: &[String],
    width: usize,
    text: Color,
) -> Text<'static> {
    if tags.is_empty() {
        return base.clone();
    }

    let tag_lines = build_full_tag_lines(tags, width, text);
    if tag_lines.is_empty() {
        return base.clone();
    }

    let mut lines = Vec::new();
    lines.extend(tag_lines);
    lines.push(Line::from(""));
    lines.extend(base.lines.clone());
    Text::from(lines)
}

pub(crate) fn visible_tab_window(
    labels: &[String],
    selected: usize,
    width: usize,
    settings: PreviewSettings,
) -> (Vec<String>, usize) {
    if labels.is_empty() {
        return (Vec::new(), 0);
    }
    let selected = selected.min(labels.len() - 1);
    if width == 0 {
        return (vec![labels[selected].clone()], 0);
    }

    let mut start = selected.saturating_sub(1);
    let mut count = labels.len() - start;
    let selected_offset = selected - start;
    let selected_only_count = selected_offset + 1;
    let preferred_count = if selected + 1 < labels.len() {
        selected_offset + 2
    } else {
        selected_only_count
    };

    while count > preferred_count
        && min_tab_window_width(&labels[start..start + count], selected - start, settings) > width
    {
        count -= 1;
    }
    while count > selected_only_count
        && min_tab_window_width(&labels[start..start + count], selected - start, settings) > width
    {
        count -= 1;
    }
    if min_tab_window_width(&labels[start..start + count], selected - start, settings) > width
        && selected > start
    {
        start = selected;
        count = 1;
    }

    let selected_index = selected - start;
    let mut visible = fit_tab_labels(
        &labels[start..start + count],
        selected_index,
        width,
        settings,
    );
    let next_index = start + count;
    if next_index < labels.len() {
        let used = rendered_tab_width(&visible);
        let partial_width = width.saturating_sub(used.saturating_add(TAB_DIVIDER_WIDTH));
        if partial_width >= MIN_PARTIAL_TAB_WIDTH {
            visible.push(truncate_tab_label(&labels[next_index], partial_width));
        }
    }

    (visible, selected_index)
}

pub(crate) fn truncate_tab_label(label: &str, width: usize) -> String {
    let len = label.chars().count();
    if len <= width {
        return label.to_string();
    }
    if width == 0 {
        return String::new();
    }
    if width <= 3 {
        return ".".repeat(width);
    }
    let mut value = label.chars().take(width - 3).collect::<String>();
    value.push_str("...");
    value
}

fn truncate_with_ellipsis(value: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    let count = value.chars().count();
    if count <= max {
        return value.to_string();
    }
    if max <= 3 {
        return value.chars().take(max).collect();
    }
    let trimmed: String = value.chars().take(max - 3).collect();
    format!("{}...", trimmed)
}

fn entry_match_tokens(tokens: &QueryTokens) -> Vec<&str> {
    tokens
        .folder
        .iter()
        .chain(tokens.any.iter())
        .map(String::as_str)
        .collect()
}

fn highlight_match_spans(
    value: &str,
    tokens: Vec<&str>,
    text: Color,
    accent: Color,
) -> Vec<Span<'static>> {
    if value.is_empty() || tokens.is_empty() {
        return vec![Span::styled(value.to_string(), Style::default().fg(text))];
    }

    let mut highlighted = vec![false; value.chars().count()];
    for token in tokens {
        for index in match_indices(token, value) {
            if let Some(slot) = highlighted.get_mut(index) {
                *slot = true;
            }
        }
    }

    let normal = Style::default().fg(text);
    let matched = Style::default().fg(accent).add_modifier(Modifier::BOLD);
    let mut spans = Vec::new();
    let mut current = String::new();
    let mut current_highlight = None;

    for (index, ch) in value.chars().enumerate() {
        let is_highlighted = highlighted.get(index).copied().unwrap_or(false);
        if current_highlight == Some(is_highlighted) {
            current.push(ch);
            continue;
        }
        if !current.is_empty() {
            let style = if current_highlight.unwrap_or(false) {
                matched
            } else {
                normal
            };
            spans.push(Span::styled(current.clone(), style));
            current.clear();
        }
        current_highlight = Some(is_highlighted);
        current.push(ch);
    }

    if !current.is_empty() {
        let style = if current_highlight.unwrap_or(false) {
            matched
        } else {
            normal
        };
        spans.push(Span::styled(current, style));
    }
    spans
}

fn match_indices(query: &str, value: &str) -> Vec<usize> {
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

fn build_tag_spans(
    tags: &[String],
    tokens: &QueryTokens,
    max_width: usize,
    elapsed_ms: u64,
    text: Color,
) -> (Vec<Span<'static>>, usize) {
    if tags.is_empty() || max_width == 0 {
        return (Vec::new(), 0);
    }

    let mut ordered = Vec::new();
    let mut matching = Vec::new();
    let mut non_matching = Vec::new();
    if tokens.tags.is_empty() {
        ordered.extend_from_slice(tags);
    } else {
        for tag in tags {
            if tokens.tags.iter().any(|token| fuzzy_match(token, tag)) {
                matching.push(tag.clone());
            } else {
                non_matching.push(tag.clone());
            }
        }
        ordered.extend_from_slice(&matching);
        ordered.extend_from_slice(&non_matching);
    }
    let has_tag_query_match = !matching.is_empty();

    let segments = build_tag_segments(&ordered, text);
    let total_len = segments_total_len(&segments);
    let display_width = max_width.max(1);
    let scroll_enabled =
        total_len > display_width && !has_tag_query_match && tokens.tags.is_empty();

    if scroll_enabled && total_len > display_width {
        let max_offset = total_len.saturating_sub(display_width);
        let offset = ((elapsed_ms / 200) as usize) % (max_offset + 1);
        return slice_tag_segments(&segments, offset, display_width);
    }

    let (spans, used) = slice_tag_segments(&segments, 0, display_width.min(total_len));
    if used < total_len {
        let more = "[...]";
        let more_len = more.chars().count();
        let extra = if spans.is_empty() { 0 } else { 1 };
        if used + extra + more_len <= display_width {
            let mut spans = spans;
            if extra > 0 {
                spans.push(Span::raw(" "));
            }
            spans.push(Span::styled(
                more,
                Style::default().fg(text).add_modifier(Modifier::ITALIC),
            ));
            return (spans, used + extra + more_len);
        }
    }

    (spans, used)
}

#[derive(Clone)]
struct TagSegment {
    text: String,
    style: Style,
    len: usize,
}

fn build_tag_segments(tags: &[String], fallback: Color) -> Vec<TagSegment> {
    let mut segments = Vec::new();
    for (index, tag) in tags.iter().enumerate() {
        if index > 0 {
            segments.push(TagSegment {
                text: " ".to_string(),
                style: Style::default().fg(fallback),
                len: 1,
            });
        }
        let pill = format!("[{}]", tag);
        let color = tag_color(tag, fallback);
        let style = Style::default().fg(color).add_modifier(Modifier::ITALIC);
        segments.push(TagSegment {
            text: pill.clone(),
            style,
            len: pill.chars().count(),
        });
    }
    segments
}

fn segments_total_len(segments: &[TagSegment]) -> usize {
    segments.iter().map(|seg| seg.len).sum()
}

fn slice_tag_segments(
    segments: &[TagSegment],
    offset: usize,
    width: usize,
) -> (Vec<Span<'static>>, usize) {
    let mut spans = Vec::new();
    if width == 0 {
        return (spans, 0);
    }
    let mut skipped = 0usize;
    let mut remaining = width;
    for seg in segments {
        if remaining == 0 {
            break;
        }
        if skipped + seg.len <= offset {
            skipped += seg.len;
            continue;
        }
        let start = offset.saturating_sub(skipped);
        let take = remaining.min(seg.len.saturating_sub(start));
        let slice = substring_by_char(&seg.text, start, take);
        spans.push(Span::styled(slice, seg.style));
        remaining = remaining.saturating_sub(take);
        skipped += seg.len;
    }
    (spans, width.saturating_sub(remaining))
}

fn substring_by_char(value: &str, start: usize, len: usize) -> String {
    if len == 0 {
        return String::new();
    }
    let mut result = String::new();
    for (idx, ch) in value.chars().enumerate() {
        if idx < start {
            continue;
        }
        if idx >= start + len {
            break;
        }
        result.push(ch);
    }
    result
}

fn build_full_tag_lines(tags: &[String], width: usize, text: Color) -> Vec<Line<'static>> {
    if tags.is_empty() || width == 0 {
        return Vec::new();
    }
    let segments = build_tag_segments(tags, text);
    wrap_tag_segments(&segments, width)
}

fn wrap_tag_segments(segments: &[TagSegment], width: usize) -> Vec<Line<'static>> {
    if width == 0 {
        return Vec::new();
    }
    let mut lines = Vec::new();
    let mut current: Vec<Span<'static>> = Vec::new();
    let mut current_len = 0usize;

    for seg in segments {
        if seg.len == 0 {
            continue;
        }
        let mut offset = 0usize;
        while offset < seg.len {
            if current_len == 0 && seg.text.starts_with(' ') {
                offset = offset.saturating_add(1);
                continue;
            }
            let available = width.saturating_sub(current_len).max(1);
            let remaining = seg.len.saturating_sub(offset);
            let take = remaining.min(available);
            let slice = substring_by_char(&seg.text, offset, take);
            current.push(Span::styled(slice, seg.style));
            current_len = current_len.saturating_add(take);
            offset = offset.saturating_add(take);

            if current_len >= width {
                lines.push(Line::from(current));
                current = Vec::new();
                current_len = 0;
            }
        }
    }

    if !current.is_empty() {
        lines.push(Line::from(current));
    }

    lines
}

fn tag_color(tag: &str, fallback: Color) -> Color {
    let mut hash = 2166136261u32;
    for byte in tag.as_bytes() {
        hash ^= *byte as u32;
        hash = hash.wrapping_mul(16777619);
    }
    let hue = (hash % 360) as f32;
    hsl_to_rgb(hue, 0.6, 0.55).unwrap_or(fallback)
}

fn hsl_to_rgb(hue: f32, sat: f32, light: f32) -> Option<Color> {
    if !(0.0..=360.0).contains(&hue) {
        return None;
    }
    let c = (1.0 - (2.0 * light - 1.0).abs()) * sat;
    let h = hue / 60.0;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r1, g1, b1) = if (0.0..1.0).contains(&h) {
        (c, x, 0.0)
    } else if (1.0..2.0).contains(&h) {
        (x, c, 0.0)
    } else if (2.0..3.0).contains(&h) {
        (0.0, c, x)
    } else if (3.0..4.0).contains(&h) {
        (0.0, x, c)
    } else if (4.0..5.0).contains(&h) {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    let m = light - c / 2.0;
    let r = ((r1 + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    let g = ((g1 + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    let b = ((b1 + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    Some(Color::Rgb(r, g, b))
}

pub(crate) fn render_side_panels(frame: &mut ratatui::Frame, render: SidePanelRender<'_>) {
    let preview_focused = matches!(render.focus, Focus::Preview | Focus::TagEdit);
    let detail_focused = render.focus == Focus::Detail;
    let preview_border_style = if preview_focused {
        Style::default().fg(render.accent)
    } else {
        Style::default().fg(render.text)
    };
    let detail_border_style = if detail_focused {
        Style::default().fg(render.accent)
    } else {
        Style::default().fg(render.text)
    };
    let preview_title =
        build_preview_title_line(render.preview_title, preview_focused, render.text);

    if !render.detail_tabs.is_empty() {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(render.area);

        render_preview_panel(
            frame,
            chunks[0],
            &render,
            preview_title.clone(),
            preview_border_style,
        );

        render_detail_panel(frame, chunks[1], &render, detail_border_style);
    } else {
        render_preview_panel(
            frame,
            render.area,
            &render,
            preview_title.clone(),
            preview_border_style,
        );
    }
}

fn render_detail_panel(
    frame: &mut ratatui::Frame,
    area: Rect,
    render: &SidePanelRender<'_>,
    border_style: Style,
) {
    let focused = render.focus == Focus::Detail;
    let title = if focused { "* Details" } else { "Details" };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(title, Style::default().fg(render.text)))
        .border_style(border_style)
        .border_type(BorderType::Rounded);
    let inner = panel_inner_area(area);
    frame.render_widget(block, area);

    let mut content_area = inner;
    if render.detail_tabs.len() > 1 && inner.height > 0 {
        let tab_area = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: 1,
        };
        content_area.y = content_area.y.saturating_add(1);
        content_area.height = content_area.height.saturating_sub(1);
        let titles = render
            .detail_tabs
            .iter()
            .map(|tab| {
                Line::from(Span::styled(
                    tab.label.clone(),
                    Style::default().fg(render.text),
                ))
            })
            .collect::<Vec<Line<'static>>>();
        let tabs = Tabs::new(titles)
            .select(
                render
                    .detail_tab_index
                    .min(render.detail_tabs.len().saturating_sub(1)),
            )
            .divider(" | ")
            .padding("", "")
            .style(Style::default().fg(render.text))
            .highlight_style(
                Style::default()
                    .fg(render.accent)
                    .add_modifier(Modifier::BOLD),
            );
        frame.render_widget(tabs, tab_area);
    }

    let tab_index = render
        .detail_tab_index
        .min(render.detail_tabs.len().saturating_sub(1));
    let detail_paragraph = Paragraph::new(render.detail_tabs[tab_index].text.clone())
        .style(Style::default().fg(render.text))
        .alignment(Alignment::Left)
        .scroll((render.detail_scroll, 0))
        .wrap(Wrap { trim: false });
    frame.render_widget(detail_paragraph, content_area);
}

fn build_preview_title_line(
    title: &str,
    focused: bool,
    text: ratatui::style::Color,
) -> Line<'static> {
    let label = if focused {
        format!("* {}", title)
    } else {
        title.to_string()
    };
    Line::from(Span::styled(label, Style::default().fg(text)))
}

fn panel_inner_area(area: Rect) -> Rect {
    Rect {
        x: area.x.saturating_add(1),
        y: area.y.saturating_add(1),
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    }
}

fn min_tab_window_width(
    labels: &[String],
    selected_index: usize,
    settings: PreviewSettings,
) -> usize {
    if labels.is_empty() {
        return 0;
    }
    let label_width = labels
        .iter()
        .enumerate()
        .map(|(index, label)| {
            tab_min_width(
                label,
                if index == selected_index {
                    settings.selected_worktree_tab_min_chars
                } else {
                    settings.worktree_tab_min_chars
                },
            )
        })
        .sum::<usize>();
    label_width + labels.len().saturating_sub(1) * TAB_DIVIDER_WIDTH
}

fn rendered_tab_width(labels: &[String]) -> usize {
    if labels.is_empty() {
        return 0;
    }
    labels
        .iter()
        .map(|label| label.chars().count())
        .sum::<usize>()
        + labels.len().saturating_sub(1) * TAB_DIVIDER_WIDTH
}

fn fit_tab_labels(
    labels: &[String],
    selected_index: usize,
    width: usize,
    settings: PreviewSettings,
) -> Vec<String> {
    if labels.is_empty() {
        return Vec::new();
    }

    let divider_width = labels.len().saturating_sub(1) * TAB_DIVIDER_WIDTH;
    let budget = width.saturating_sub(divider_width);
    let mut widths = labels
        .iter()
        .map(|label| label.chars().count())
        .collect::<Vec<usize>>();
    let min_widths = labels
        .iter()
        .enumerate()
        .map(|(index, label)| {
            tab_min_width(
                label,
                if index == selected_index {
                    settings.selected_worktree_tab_min_chars
                } else {
                    settings.worktree_tab_min_chars
                },
            )
        })
        .collect::<Vec<usize>>();

    while widths.iter().sum::<usize>() > budget {
        let mut changed = false;
        for index in (0..widths.len()).rev() {
            if widths[index] > min_widths[index] {
                widths[index] -= 1;
                changed = true;
                break;
            }
        }
        if !changed {
            break;
        }
    }

    while widths.iter().sum::<usize>() > budget {
        let Some((index, _)) = widths.iter().enumerate().max_by_key(|(_, width)| **width) else {
            break;
        };
        if widths[index] == 0 {
            break;
        }
        widths[index] -= 1;
    }

    labels
        .iter()
        .zip(widths)
        .map(|(label, width)| truncate_tab_label(label, width))
        .collect()
}

fn tab_min_width(label: &str, min_chars_before_ellipsis: usize) -> usize {
    let len = label.chars().count();
    if len <= min_chars_before_ellipsis {
        len
    } else {
        min_chars_before_ellipsis.saturating_add(3)
    }
}

fn render_preview_panel(
    frame: &mut ratatui::Frame,
    area: Rect,
    render: &SidePanelRender<'_>,
    title: Line<'static>,
    border_style: Style,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(border_style)
        .border_type(BorderType::Rounded);
    let inner = panel_inner_area(area);
    frame.render_widget(block, area);

    let mut content_area = inner;
    if render.preview_tab_labels.len() > 1 && inner.height > 0 {
        let tab_area = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: 1,
        };
        content_area = preview_content_area(area, render.preview_tab_labels.len());
        let (visible_labels, visible_index) = visible_tab_window(
            render.preview_tab_labels,
            render.preview_tab_index,
            tab_area.width as usize,
            render.preview_settings,
        );
        let titles = visible_labels
            .iter()
            .map(|label| {
                Line::from(Span::styled(
                    label.clone(),
                    Style::default().fg(render.text),
                ))
            })
            .collect::<Vec<Line<'static>>>();
        let tabs = Tabs::new(titles)
            .select(visible_index)
            .divider(" | ")
            .padding("", "")
            .style(Style::default().fg(render.text))
            .highlight_style(
                Style::default()
                    .fg(render.accent)
                    .add_modifier(Modifier::BOLD),
            );
        frame.render_widget(tabs, tab_area);
    }

    let preview_paragraph = Paragraph::new(render.preview.clone())
        .style(Style::default().fg(render.text))
        .alignment(Alignment::Left)
        .scroll((render.preview_scroll, 0))
        .wrap(Wrap { trim: false });
    frame.render_widget(preview_paragraph, content_area);
}
