use crossterm::event::{self, Event, KeyCode, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Clear, List, ListState, Paragraph, Wrap},
};
use std::{
    collections::{HashMap, HashSet},
    env,
    path::Path,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
use tui_input::backend::crossterm::EventHandler;
use tui_input::{Input, InputRequest};

mod commands;
mod compositor;
mod config;
mod content;
mod git;
mod github;
mod metadata;
mod model;
mod provider_runtime;
mod results;
mod search;
mod tags;
mod ui;

use compositor::{CurrentCompositor, FocusChange, NavigationCompositor};
use config::config_schema_json;
use content::{
    apply_git_result, apply_github_readme_result, build_placeholder_text, build_preview_data,
    build_remote_branch_preview_data, ensure_git_for_preview,
};
use gator::{copy_to_clipboard, ensure_tty_stdin, input_at_end, setup_terminal, write_selection};
use github::ensure_github_readme_for_preview;
use metadata::{ensure_dates_for_paths, spawn_bulk_metadata_fetch};
use model::{
    AppResult, Focus, GitResult, GithubReadmeResult, HelpColors, HelpContext, MetaResult,
    NavigateEntry, NavigateEntryKind, PreviewColors, PreviewData, PreviewResult, PreviewSettings,
    RemoteSettings, RemoteToggleState, ResultUpdate, SidePanelRender, SortMeta, SortMode,
    SortSettings, TagResult, ThemeColors, VisibleListArgs, DATE_PLACEHOLDER,
};
use results::{
    build_items, spawn_remote_branch_result_provider, spawn_worktree_result_provider,
    REMOTE_BRANCH_PROVIDER_PREFIX,
};
use search::{filter_and_sort, index_for_entry_id, parse_query_tokens};
use tags::{
    collect_tag_suggestions, commit_tag_input, ensure_tags_for_paths, read_tags_for_path,
    save_tags_for_path, spawn_bulk_tag_fetch,
};
use ui::{
    build_help_line, build_visible_list_items, compose_preview_text,
    compose_preview_text_with_input, compute_ui_layout, preview_content_area, rect_contains,
    render_side_panels, text_line_count,
};

fn main() -> AppResult<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() || args[0] == "navigate" {
        ensure_tty_stdin()?;
        return run_navigate();
    }
    if args[0] == "config-schema" || args[0] == "schema" {
        return print_config_schema();
    }
    if args[0] == "--help" || args[0] == "-h" {
        print_usage();
        return Ok(());
    }

    eprintln!("Unknown command.");
    print_usage();
    std::process::exit(2);
}

fn print_usage() {
    eprintln!("Usage:\n  navgator [navigate|config-schema]");
}

fn print_config_schema() -> AppResult<()> {
    let json = config_schema_json()?;
    println!("{json}");
    Ok(())
}

enum WorktreeProgress {
    Message(String),
    Select(String),
    Deleted(String),
    Error(String),
}

struct WorktreeOverlay {
    title: String,
    messages: Vec<String>,
    error: Option<String>,
}

impl WorktreeOverlay {
    fn new(title: String, first_message: &str) -> Self {
        Self {
            title,
            messages: vec![first_message.to_string()],
            error: None,
        }
    }

    fn is_error(&self) -> bool {
        self.error.is_some()
    }

    fn push_message(&mut self, message: String) {
        self.messages.push(message);
    }

    fn fail(&mut self, message: String) {
        self.error = Some(message);
    }
}

fn run_navigate() -> AppResult<()> {
    let result = build_items()?;
    match select_from_list(
        "Navigate",
        result.entries,
        result.preview_settings,
        result.sort_settings,
        result.remote_settings,
        result.theme_colors,
    )? {
        Some(choice) => write_selection(&choice),
        None => std::process::exit(1),
    }
}

fn select_from_list(
    _title: &str,
    mut entries: Vec<NavigateEntry>,
    preview_settings: PreviewSettings,
    sort_settings: SortSettings,
    remote_settings: RemoteSettings,
    theme_colors: ThemeColors,
) -> AppResult<Option<String>> {
    if entries.is_empty() {
        return Ok(None);
    }

    let (mut terminal, _guard) = setup_terminal()?;
    let mut input = Input::default();
    let mut selected = 0usize;
    let mut sort_mode = sort_settings.default_mode;
    let mut focus = Focus::Search;
    let mut meta_cache: HashMap<String, SortMeta> = HashMap::new();
    let mut list_offset = 0usize;
    let accent = theme_colors.accent;
    let warm = theme_colors.warm;
    let key_color = theme_colors.key_color;
    let text = theme_colors.text;
    let muted = theme_colors.muted;
    let (preview_tx, preview_rx) = mpsc::channel::<PreviewResult>();
    let (git_tx, git_rx) = mpsc::channel::<GitResult>();
    let (github_tx, github_rx) = mpsc::channel::<GithubReadmeResult>();
    let (date_tx, date_rx) = mpsc::channel::<MetaResult>();
    let (tag_tx, tag_rx) = mpsc::channel::<TagResult>();
    let (result_tx, result_rx) = mpsc::channel::<ResultUpdate>();
    let (worktree_tx, worktree_rx) = mpsc::channel::<WorktreeProgress>();
    let mut preview_cache: HashMap<String, PreviewData> = HashMap::new();
    let mut git_in_flight: HashSet<String> = HashSet::new();
    let mut github_in_flight: HashSet<String> = HashSet::new();
    let mut date_cache: HashMap<String, String> = HashMap::new();
    let mut date_in_flight: HashSet<String> = HashSet::new();
    let mut tag_cache: HashMap<String, Vec<String>> = HashMap::new();
    let mut tag_in_flight: HashSet<String> = HashSet::new();
    let mut tag_scan_started = false;
    let mut show_remote_branches = remote_settings.enabled_by_default;
    let mut remote_fetching =
        remote_settings.enabled_by_default && remote_settings.refresh_on_toggle;
    let mut remote_error = false;
    let mut remote_status: Option<String> = None;
    let current_project_path = env::current_dir()
        .ok()
        .and_then(|path| git::git_worktree_root_for_path(&path));
    let mut filtered = filter_and_sort_visible(
        &entries,
        input.value(),
        sort_mode,
        &meta_cache,
        &tag_cache,
        show_remote_branches,
        pinned_path(
            sort_settings.pin_current_project,
            current_project_path.as_deref(),
        ),
    );
    let mut preview_path: Option<String> = None;
    let mut preview_entry_id: Option<String> = None;
    let mut in_flight: Option<String> = None;
    let mut compositor = CurrentCompositor::new(build_placeholder_text(
        None,
        accent,
        muted,
        text,
        "No selection",
    ));
    let start_time = Instant::now();
    let mut tag_edit_path: Option<String> = None;
    let mut tag_edit_tags: Vec<String> = Vec::new();
    let mut tag_input = Input::default();
    let mut tag_suggestions: Vec<String> = Vec::new();
    let mut worktree_overlay: Option<WorktreeOverlay> = None;
    spawn_worktree_result_provider(&entries, result_tx.clone());
    if show_remote_branches {
        if let Some(entry) = filtered
            .first()
            .and_then(|index| entries.get(*index))
            .cloned()
        {
            remote_status = Some("loading local remotes...".to_string());
            spawn_remote_branch_result_provider(entry, result_tx.clone(), remote_settings);
        }
    }
    if sort_mode.uses_time() {
        let paths = all_metadata_paths(&entries);
        spawn_bulk_metadata_fetch(&paths, &date_cache, &mut date_in_flight, &date_tx);
    }

    loop {
        let current_entry = current_selection_entry(&entries, &filtered, selected).cloned();
        let current = current_entry
            .as_ref()
            .map(|entry| entry.preview_root_path.clone());
        let query_value = input.value();
        let tokens = parse_query_tokens(query_value);

        while let Ok(progress) = worktree_rx.try_recv() {
            match progress {
                WorktreeProgress::Message(message) => {
                    if let Some(overlay) = worktree_overlay.as_mut() {
                        overlay.push_message(message);
                    }
                }
                WorktreeProgress::Select(path) => {
                    terminal.show_cursor()?;
                    return Ok(Some(path));
                }
                WorktreeProgress::Deleted(path) => {
                    entries.retain(|entry| entry.selection_path != path);
                    filtered = filter_and_sort_visible(
                        &entries,
                        input.value(),
                        sort_mode,
                        &meta_cache,
                        &tag_cache,
                        show_remote_branches,
                        pinned_path(
                            sort_settings.pin_current_project,
                            current_project_path.as_deref(),
                        ),
                    );
                    selected = adjust_selected_index(selected, filtered.len());
                    preview_cache.remove(&path);
                    worktree_overlay = None;
                }
                WorktreeProgress::Error(message) => {
                    if let Some(overlay) = worktree_overlay.as_mut() {
                        overlay.fail(message);
                    }
                }
            }
        }

        let mut entries_changed = false;
        while let Ok(update) = result_rx.try_recv() {
            match update {
                ResultUpdate::Entries { entries: incoming } => {
                    for entry in incoming {
                        if entries.iter().any(|candidate| candidate.id == entry.id) {
                            continue;
                        }
                        entries.push(entry);
                        entries_changed = true;
                    }
                }
                ResultUpdate::ReplaceProviderEntries {
                    provider_prefix,
                    entries: replacement,
                } => {
                    entries.retain(|entry| !entry.id.starts_with(&provider_prefix));
                    entries.extend(replacement);
                    entries_changed = true;
                }
                ResultUpdate::Status {
                    provider_id,
                    message,
                } => {
                    if provider_id == REMOTE_BRANCH_PROVIDER_PREFIX {
                        remote_fetching = message.starts_with("refreshing ");
                        remote_error = !remote_fetching && message != "remote branches loaded";
                        remote_status = Some(message);
                    }
                }
            }
        }

        if entries_changed {
            let selected_id = current_entry.as_ref().map(|entry| entry.id.clone());
            filtered = filter_and_sort_visible(
                &entries,
                input.value(),
                sort_mode,
                &meta_cache,
                &tag_cache,
                show_remote_branches,
                pinned_path(
                    sort_settings.pin_current_project,
                    current_project_path.as_deref(),
                ),
            );
            selected = selected_id
                .as_deref()
                .and_then(|id| index_for_entry_id(&entries, &filtered, id))
                .unwrap_or_else(|| adjust_selected_index(selected, filtered.len()));
        }

        while let Ok(result) = preview_rx.try_recv() {
            preview_cache.insert(result.path.clone(), result.data.clone());
            if let Some(data) = preview_cache.get(&result.path) {
                ensure_git_for_preview(
                    &result.path,
                    data,
                    &mut git_in_flight,
                    &git_tx,
                    compositor.active_content_index(),
                    PreviewColors {
                        accent,
                        muted,
                        text,
                    },
                );
                ensure_github_readme_for_preview(
                    &result.path,
                    data,
                    &mut github_in_flight,
                    &github_tx,
                    compositor.active_content_index(),
                    PreviewColors {
                        accent,
                        muted,
                        text,
                    },
                );
            }
            if current.as_deref() == Some(result.path.as_str()) {
                apply_preview_for_entry(&mut compositor, &result.data, current_entry.as_ref());
                preview_path = Some(result.path.clone());
                preview_entry_id = current_entry.as_ref().map(|entry| entry.id.clone());
            }
            if in_flight.as_deref() == Some(result.path.as_str()) {
                in_flight = None;
            }
        }

        while let Ok(result) = git_rx.try_recv() {
            if result.done {
                git_in_flight.remove(&result.path);
            }
            let mut updated = false;
            if let Some(data) = preview_cache.get_mut(&result.path) {
                apply_git_result(data, result.tab_index, result.git, result.done);
                updated = true;
            }
            if updated && current.as_deref() == Some(result.path.as_str()) {
                if let Some(data) = preview_cache.get(&result.path) {
                    apply_preview_for_entry(&mut compositor, data, current_entry.as_ref());
                }
            }
        }

        while let Ok(result) = github_rx.try_recv() {
            if result.done {
                github_in_flight.remove(&result.path);
            }
            let mut updated = false;
            if let Some(data) = preview_cache.get_mut(&result.path) {
                apply_github_readme_result(data, result.tab_index, result.readme, result.done);
                updated = true;
            }
            if updated && current.as_deref() == Some(result.path.as_str()) {
                if let Some(data) = preview_cache.get(&result.path) {
                    apply_preview_for_entry(&mut compositor, data, current_entry.as_ref());
                }
            }
        }

        let mut resort_needed = false;
        while let Ok(result) = date_rx.try_recv() {
            let display = result
                .display
                .unwrap_or_else(|| DATE_PLACEHOLDER.to_string());
            date_cache.insert(result.path.clone(), display);
            meta_cache.insert(
                result.path.clone(),
                SortMeta {
                    modified_epoch: result.modified_epoch,
                    created_epoch: result.created_epoch,
                },
            );
            date_in_flight.remove(&result.path);
            if sort_mode.uses_time() {
                resort_needed = true;
            }
        }

        let mut tags_changed = false;
        while let Ok(result) = tag_rx.try_recv() {
            tag_cache.insert(result.path.clone(), result.tags);
            tag_in_flight.remove(&result.path);
            tags_changed = true;
        }

        let query_uses_tags = tokens.needs_tags();
        if query_uses_tags && !tag_scan_started {
            let paths = all_metadata_paths(&entries);
            spawn_bulk_tag_fetch(&paths, &tag_cache, &mut tag_in_flight, &tag_tx);
            tag_scan_started = true;
        }

        if resort_needed {
            let selected_id = current_selection_entry(&entries, &filtered, selected)
                .map(|entry| entry.id.clone());
            filtered = filter_and_sort_visible(
                &entries,
                input.value(),
                sort_mode,
                &meta_cache,
                &tag_cache,
                show_remote_branches,
                pinned_path(
                    sort_settings.pin_current_project,
                    current_project_path.as_deref(),
                ),
            );
            selected = match selected_id {
                Some(id) => index_for_entry_id(&entries, &filtered, &id).unwrap_or(0),
                None => adjust_selected_index(selected, filtered.len()),
            };
        }

        if tags_changed && query_uses_tags {
            let selected_id = current_selection_entry(&entries, &filtered, selected)
                .map(|entry| entry.id.clone());
            filtered = filter_and_sort_visible(
                &entries,
                input.value(),
                sort_mode,
                &meta_cache,
                &tag_cache,
                show_remote_branches,
                pinned_path(
                    sort_settings.pin_current_project,
                    current_project_path.as_deref(),
                ),
            );
            selected = match selected_id {
                Some(id) => index_for_entry_id(&entries, &filtered, &id).unwrap_or(0),
                None => adjust_selected_index(selected, filtered.len()),
            };
        }

        match current.as_deref() {
            None => {
                if preview_path.is_some() || in_flight.is_some() {
                    let placeholder =
                        build_placeholder_text(None, accent, muted, text, "No selection");
                    compositor.reset_for_no_selection(placeholder);
                    preview_path = None;
                    preview_entry_id = None;
                    in_flight = None;
                }
            }
            Some(path) => {
                let current_entry_id = current_entry.as_ref().map(|entry| entry.id.as_str());
                if preview_path.as_deref() != Some(path)
                    || preview_entry_id.as_deref() != current_entry_id
                {
                    compositor.reset_for_new_selection();
                    if let Some(data) = preview_cache.get(path) {
                        apply_preview_for_entry(&mut compositor, data, current_entry.as_ref());
                        preview_path = Some(path.to_string());
                        preview_entry_id = current_entry.as_ref().map(|entry| entry.id.clone());
                        ensure_git_for_preview(
                            path,
                            data,
                            &mut git_in_flight,
                            &git_tx,
                            compositor.active_content_index(),
                            PreviewColors {
                                accent,
                                muted,
                                text,
                            },
                        );
                        ensure_github_readme_for_preview(
                            path,
                            data,
                            &mut github_in_flight,
                            &github_tx,
                            compositor.active_content_index(),
                            PreviewColors {
                                accent,
                                muted,
                                text,
                            },
                        );
                    } else if in_flight.as_deref() != Some(path) {
                        let placeholder = build_placeholder_text(
                            preview_placeholder_path(current_entry.as_ref()),
                            accent,
                            muted,
                            text,
                            "Loading preview...",
                        );
                        compositor.set_loading(placeholder);
                        preview_path = Some(path.to_string());
                        preview_entry_id = current_entry.as_ref().map(|entry| entry.id.clone());
                        in_flight = Some(path.to_string());
                        let tx = preview_tx.clone();
                        let path_owned = path.to_string();
                        let entry_owned = current_entry.clone();
                        thread::spawn(move || {
                            let data = build_preview_data_for_entry(
                                &path_owned,
                                entry_owned.as_ref(),
                                accent,
                                muted,
                                text,
                                preview_settings,
                            );
                            let _ = tx.send(PreviewResult {
                                path: path_owned,
                                data,
                            });
                        });
                    }
                }
            }
        }

        focus = compositor.ensure_valid_focus(focus);
        if focus == Focus::TagEdit && tag_edit_path.is_none() {
            focus = Focus::Preview;
        }

        let show_detail = compositor.show_detail();
        let size = terminal.size()?;
        let ui = compute_ui_layout(size.into(), show_detail);

        terminal.draw(|frame| {
            let list_area = ui.list_area;
            let detail_area = ui.detail_area;

            let remote_state =
                remote_toggle_state(show_remote_branches, remote_fetching, remote_error);
            let can_delete_worktree = current_entry
                .as_ref()
                .is_some_and(|entry| matches!(entry.kind, NavigateEntryKind::Worktree { .. }));
            let list_title = if show_remote_branches {
                match remote_status.as_deref() {
                    Some(status) => format!(
                        "Results {}/{} + remote {status}",
                        filtered.len(),
                        visible_entry_count(&entries, true)
                    ),
                    None => format!(
                        "Results {}/{} + remote",
                        filtered.len(),
                        visible_entry_count(&entries, true)
                    ),
                }
            } else {
                format!(
                    "Results {}/{}",
                    filtered.len(),
                    visible_entry_count(&entries, false)
                )
            };
            let left_title = if focus == Focus::Search {
                format!("* {}", list_title)
            } else {
                list_title
            };
            let left_border_style = if focus == Focus::Search {
                Style::default().fg(accent)
            } else {
                Style::default().fg(muted)
            };
            let left_block = Block::default()
                .borders(Borders::ALL)
                .title(left_title)
                .border_style(left_border_style)
                .border_type(BorderType::Rounded);
            frame.render_widget(left_block, list_area);

            let search_area = ui.search_area;
            let results_area = ui.results_area;

            let search_width = search_area.width.saturating_sub(1) as usize;
            let scroll = if search_width > 0 {
                input.visual_scroll(search_width)
            } else {
                0
            };
            let search = Paragraph::new(input.value())
                .scroll((0, scroll as u16))
                .alignment(Alignment::Left)
                .wrap(Wrap { trim: false });
            frame.render_widget(search, search_area);
            if focus == Focus::Search && search_area.width > 0 && search_area.height > 0 {
                let cursor_x = input.visual_cursor().max(scroll).saturating_sub(scroll);
                frame.set_cursor_position((search_area.x + cursor_x as u16, search_area.y));
            }

            let list_inner_height = results_area.height as usize;
            let total = filtered.len();
            list_offset =
                compute_list_window_offset(selected, list_offset, list_inner_height, total);

            let scrollbar_space = if total > 0 { 1 } else { 0 };
            let list_inner_width = results_area.width.saturating_sub(scrollbar_space) as usize;
            let visible_paths =
                visible_paths_for_window(&entries, &filtered, list_offset, list_inner_height);
            ensure_dates_for_paths(&visible_paths, &date_cache, &mut date_in_flight, &date_tx);
            ensure_tags_for_paths(&visible_paths, &tag_cache, &mut tag_in_flight, &tag_tx);

            let (list_items, list_selected) = build_visible_list_items(VisibleListArgs {
                entries: &entries,
                filtered: &filtered,
                selected,
                offset: list_offset,
                height: list_inner_height,
                text,
                accent,
                muted,
                dates: &date_cache,
                tags: &tag_cache,
                inner_width: list_inner_width,
                tokens: &tokens,
                elapsed_ms: start_time.elapsed().as_millis() as u64,
            });

            let list = List::new(list_items).highlight_style(
                Style::default()
                    .fg(Color::Black)
                    .bg(warm)
                    .add_modifier(Modifier::BOLD),
            );

            let mut state = ListState::default();
            state.select(list_selected);
            frame.render_stateful_widget(list, results_area, &mut state);

            let preview_body_area =
                preview_content_area(ui.preview_area, compositor.preview_tab_count);
            let preview_height = preview_body_area.height as usize;
            let detail_height = ui
                .detail_panel_area
                .map(|rect| {
                    let tab_row = if compositor.detail_tabs.len() > 1 {
                        1
                    } else {
                        0
                    };
                    rect.height.saturating_sub(2).saturating_sub(tab_row) as usize
                })
                .unwrap_or(0);
            compositor.preview_page_step = preview_height.max(1);
            compositor.detail_page_step = detail_height.max(1);
            let preview_title = compositor.preview_title(current.as_deref());
            let preview_tags = if focus == Focus::TagEdit {
                tag_edit_tags.clone()
            } else {
                current
                    .as_deref()
                    .and_then(|path| tag_cache.get(path))
                    .cloned()
                    .unwrap_or_default()
            };
            let preview_width = preview_body_area.width as usize;
            let (preview_combined, tag_cursor) = if focus == Focus::TagEdit {
                compose_preview_text_with_input(
                    &compositor.preview_text,
                    &preview_tags,
                    &tag_input,
                    preview_width,
                    text,
                )
            } else {
                (
                    compose_preview_text(
                        &compositor.preview_text,
                        &preview_tags,
                        preview_width,
                        text,
                    ),
                    None,
                )
            };
            compositor.preview_max_scroll =
                text_line_count(&preview_combined).saturating_sub(preview_height);
            let active_detail = compositor.detail_tabs.get(compositor.detail_tab_index);
            compositor.detail_max_scroll = active_detail
                .map(|tab| text_line_count(&tab.text).saturating_sub(detail_height))
                .unwrap_or(0);
            if focus == Focus::TagEdit {
                if let Some((row, _)) = tag_cursor {
                    if row < compositor.preview_scroll {
                        compositor.preview_scroll = row;
                    } else if row >= compositor.preview_scroll + preview_height {
                        compositor.preview_scroll =
                            row.saturating_sub(preview_height.saturating_sub(1));
                    }
                }
            }
            compositor.preview_scroll =
                compositor.preview_scroll.min(compositor.preview_max_scroll);
            compositor.detail_scroll = compositor.detail_scroll.min(compositor.detail_max_scroll);
            render_side_panels(
                frame,
                SidePanelRender {
                    area: detail_area,
                    preview: &preview_combined,
                    detail_tabs: &compositor.detail_tabs,
                    detail_tab_index: compositor.detail_tab_index,
                    preview_title: &preview_title,
                    preview_tab_labels: &compositor.preview_tab_labels,
                    preview_tab_index: compositor.preview_tab_visible_index,
                    preview_settings,
                    focus,
                    accent,
                    text,
                    preview_scroll: compositor.preview_scroll as u16,
                    detail_scroll: compositor.detail_scroll as u16,
                },
            );
            if focus == Focus::TagEdit {
                if let Some((row, col)) = tag_cursor {
                    let visible_row = row.saturating_sub(compositor.preview_scroll);
                    if visible_row < preview_height {
                        let x = preview_body_area.x + col as u16;
                        let y = preview_body_area.y + visible_row as u16;
                        frame.set_cursor_position((x, y));
                    }
                }
            }

            let help_line = build_help_line(
                HelpContext {
                    focus,
                    sort_mode,
                    remote_state,
                    can_delete_worktree,
                    show_detail,
                    cursor_at_end: input_at_end(&input),
                    has_tag_input: !tag_input.value().trim().is_empty(),
                    preview_tab_index: compositor.preview_tab_visible_index,
                    preview_tab_count: compositor.preview_tab_count,
                    preview_scroll: compositor.preview_scroll,
                    preview_max_scroll: compositor.preview_max_scroll,
                    detail_tab_index: compositor.detail_tab_index,
                    detail_tab_count: compositor.detail_tabs.len(),
                    detail_scroll: compositor.detail_scroll,
                },
                HelpColors {
                    text,
                    accent,
                    key_color,
                    remote_color: warm,
                },
            );
            let help = Paragraph::new(Text::from(help_line))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Keys")
                        .border_style(Style::default().fg(muted))
                        .border_type(BorderType::Rounded),
                )
                .alignment(Alignment::Left)
                .wrap(Wrap { trim: true });
            frame.render_widget(help, ui.help_area);

            if let Some(overlay) = worktree_overlay.as_ref() {
                render_worktree_overlay(frame, size.into(), overlay, accent, warm, text, muted);
            }
        })?;

        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => {
                    if key.code == KeyCode::Esc {
                        if worktree_overlay.is_some() {
                            if worktree_overlay
                                .as_ref()
                                .is_some_and(WorktreeOverlay::is_error)
                            {
                                worktree_overlay = None;
                            }
                            continue;
                        }
                        terminal.show_cursor()?;
                        return Ok(None);
                    }
                    if key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        terminal.show_cursor()?;
                        return Ok(None);
                    }
                    if worktree_overlay.is_some() {
                        if key.code == KeyCode::Enter
                            && worktree_overlay
                                .as_ref()
                                .is_some_and(WorktreeOverlay::is_error)
                        {
                            worktree_overlay = None;
                        }
                        continue;
                    }
                    if key.code == KeyCode::Char('y')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        if let Some(value) = selection_path_for_action(
                            focus,
                            current.as_deref(),
                            compositor.active_content_index(),
                            &preview_cache,
                            current_selection_entry(&entries, &filtered, selected),
                        ) {
                            let _ = copy_to_clipboard(&value);
                        }
                        continue;
                    }
                    if key.code == KeyCode::Char('d')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                        && focus != Focus::TagEdit
                    {
                        let Some(entry) = current_selection_entry(&entries, &filtered, selected)
                            .filter(|entry| {
                                matches!(entry.kind, NavigateEntryKind::Worktree { .. })
                            })
                            .cloned()
                        else {
                            continue;
                        };
                        worktree_overlay = Some(WorktreeOverlay::new(
                            delete_worktree_overlay_title(&entry),
                            "Starting safety checks",
                        ));
                        start_worktree_deletion(entry, worktree_tx.clone());
                        continue;
                    }
                    if key.code == KeyCode::Char('o')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                        && focus != Focus::TagEdit
                    {
                        let selected_id = current_selection_entry(&entries, &filtered, selected)
                            .map(|entry| entry.id.clone());
                        show_remote_branches = !show_remote_branches;
                        remote_error = false;
                        if show_remote_branches {
                            if let Some(entry) =
                                current_selection_entry(&entries, &filtered, selected).cloned()
                            {
                                remote_fetching = remote_settings.refresh_on_toggle;
                                remote_status = Some("loading local remotes...".to_string());
                                entries.retain(|entry| {
                                    !entry.id.starts_with(REMOTE_BRANCH_PROVIDER_PREFIX)
                                });
                                spawn_remote_branch_result_provider(
                                    entry,
                                    result_tx.clone(),
                                    remote_settings,
                                );
                            } else {
                                remote_fetching = false;
                                remote_error = true;
                                remote_status = Some("No repo selected".to_string());
                            }
                        } else {
                            remote_fetching = false;
                            remote_status = None;
                        }
                        filtered = filter_and_sort_visible(
                            &entries,
                            input.value(),
                            sort_mode,
                            &meta_cache,
                            &tag_cache,
                            show_remote_branches,
                            pinned_path(
                                sort_settings.pin_current_project,
                                current_project_path.as_deref(),
                            ),
                        );
                        selected = selected_id
                            .as_deref()
                            .and_then(|id| index_for_entry_id(&entries, &filtered, id))
                            .unwrap_or_else(|| adjust_selected_index(selected, filtered.len()));
                        list_offset = 0;
                        continue;
                    }
                    if key.code == KeyCode::Char('t')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                        && focus != Focus::TagEdit
                    {
                        if let Some(path) = current_selection_entry(&entries, &filtered, selected)
                            .map(|entry| entry.metadata_path.clone())
                        {
                            tag_edit_path = Some(path.clone());
                            tag_edit_tags = read_tags_for_path(&path);
                            tag_input.reset();
                            tag_suggestions = collect_tag_suggestions(&tag_cache);
                            focus = Focus::TagEdit;
                            compositor.preview_scroll = 0;
                        }
                        continue;
                    }
                    if key.code == KeyCode::Enter && focus != Focus::TagEdit {
                        if let Some(entry) = current_selection_entry(&entries, &filtered, selected)
                            .filter(|entry| is_remote_branch_entry(entry))
                            .cloned()
                        {
                            worktree_overlay = Some(WorktreeOverlay::new(
                                worktree_overlay_title(&entry),
                                "Starting worktree creation",
                            ));
                            start_remote_worktree_creation(entry, worktree_tx.clone());
                            continue;
                        }
                        let value = selection_path_for_action(
                            focus,
                            current.as_deref(),
                            compositor.active_content_index(),
                            &preview_cache,
                            current_selection_entry(&entries, &filtered, selected),
                        );
                        if let Some(value) = value {
                            terminal.show_cursor()?;
                            return Ok(Some(value));
                        }
                    }
                    if key.code == KeyCode::Char('s')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        sort_mode = sort_mode.next();
                        filtered = filter_and_sort_visible(
                            &entries,
                            input.value(),
                            sort_mode,
                            &meta_cache,
                            &tag_cache,
                            show_remote_branches,
                            pinned_path(
                                sort_settings.pin_current_project,
                                current_project_path.as_deref(),
                            ),
                        );
                        selected = 0;
                        list_offset = 0;
                        if sort_mode.uses_time() {
                            let paths = all_metadata_paths(&entries);
                            spawn_bulk_metadata_fetch(
                                &paths,
                                &date_cache,
                                &mut date_in_flight,
                                &date_tx,
                            );
                        }
                        if parse_query_tokens(input.value()).needs_tags() && !tag_scan_started {
                            let paths = all_metadata_paths(&entries);
                            spawn_bulk_tag_fetch(&paths, &tag_cache, &mut tag_in_flight, &tag_tx);
                            tag_scan_started = true;
                        }
                        continue;
                    }

                    match focus {
                        Focus::Search => match key.code {
                            KeyCode::Up => {
                                selected = selected.saturating_sub(1);
                            }
                            KeyCode::Down => {
                                if selected + 1 < filtered.len() {
                                    selected += 1;
                                }
                            }
                            KeyCode::Right
                                if !key.modifiers.intersects(
                                    KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER,
                                ) && input_at_end(&input) =>
                            {
                                focus = Focus::Preview;
                            }
                            _ => {
                                let before = input.value().to_string();
                                if key.modifiers.contains(KeyModifiers::SUPER) {
                                    if key.code == KeyCode::Left {
                                        input.handle(InputRequest::GoToStart);
                                    } else if key.code == KeyCode::Right {
                                        input.handle(InputRequest::GoToEnd);
                                    }
                                } else if key.code == KeyCode::Char('u')
                                    && key.modifiers.contains(KeyModifiers::CONTROL)
                                {
                                    input.handle(InputRequest::DeleteLine);
                                } else {
                                    let _ = input.handle_event(&Event::Key(key));
                                }
                                if input.value() != before {
                                    filtered = filter_and_sort_visible(
                                        &entries,
                                        input.value(),
                                        sort_mode,
                                        &meta_cache,
                                        &tag_cache,
                                        show_remote_branches,
                                        pinned_path(
                                            sort_settings.pin_current_project,
                                            current_project_path.as_deref(),
                                        ),
                                    );
                                    selected = 0;
                                    list_offset = 0;
                                }
                            }
                        },
                        Focus::TagEdit => match key.code {
                            KeyCode::Enter => {
                                commit_tag_input(
                                    &mut tag_input,
                                    &mut tag_edit_tags,
                                    &tag_suggestions,
                                );
                                if let Some(path) = tag_edit_path.clone() {
                                    save_tags_for_path(&path, &tag_edit_tags)?;
                                    tag_cache.insert(path.clone(), tag_edit_tags.clone());
                                }
                                focus = Focus::Preview;
                                tag_edit_path = None;
                                tag_edit_tags.clear();
                                tag_input.reset();
                                let selected_id =
                                    current_selection_entry(&entries, &filtered, selected)
                                        .map(|entry| entry.id.clone());
                                filtered = filter_and_sort_visible(
                                    &entries,
                                    input.value(),
                                    sort_mode,
                                    &meta_cache,
                                    &tag_cache,
                                    show_remote_branches,
                                    pinned_path(
                                        sort_settings.pin_current_project,
                                        current_project_path.as_deref(),
                                    ),
                                );
                                selected = match selected_id {
                                    Some(value) => {
                                        index_for_entry_id(&entries, &filtered, &value).unwrap_or(0)
                                    }
                                    None => adjust_selected_index(selected, filtered.len()),
                                };
                            }
                            KeyCode::Tab => {
                                commit_tag_input(
                                    &mut tag_input,
                                    &mut tag_edit_tags,
                                    &tag_suggestions,
                                );
                            }
                            KeyCode::Backspace if tag_input.value().is_empty() => {
                                tag_edit_tags.pop();
                            }
                            KeyCode::Backspace => {
                                let _ = tag_input.handle_event(&Event::Key(key));
                            }
                            _ => {
                                let _ = tag_input.handle_event(&Event::Key(key));
                            }
                        },
                        Focus::Preview => {
                            let data = current.as_deref().and_then(|path| preview_cache.get(path));
                            if let Some(change) = compositor.handle_preview_key(key, data) {
                                focus = focus_from_change(change);
                            }
                        }
                        Focus::Detail => {
                            let data = current.as_deref().and_then(|path| preview_cache.get(path));
                            if let Some(change) = compositor.handle_detail_key(key, data) {
                                focus = focus_from_change(change);
                            }
                        }
                    }
                }
                Event::Paste(value) => match focus {
                    Focus::Search => {
                        insert_paste(&mut input, &value);
                        filtered = filter_and_sort_visible(
                            &entries,
                            input.value(),
                            sort_mode,
                            &meta_cache,
                            &tag_cache,
                            show_remote_branches,
                            pinned_path(
                                sort_settings.pin_current_project,
                                current_project_path.as_deref(),
                            ),
                        );
                        selected = 0;
                        list_offset = 0;
                    }
                    Focus::TagEdit => {
                        insert_paste(&mut tag_input, &value);
                    }
                    Focus::Preview | Focus::Detail => {}
                },
                Event::Mouse(mouse) => {
                    let col = mouse.column;
                    let row = mouse.row;
                    match mouse.kind {
                        MouseEventKind::Down(MouseButton::Left) => {
                            if rect_contains(ui.list_area, col, row) {
                                focus = Focus::Search;
                            } else if let Some(detail_panel_area) = ui.detail_panel_area {
                                if rect_contains(detail_panel_area, col, row) {
                                    focus = Focus::Detail;
                                } else if rect_contains(ui.preview_area, col, row) {
                                    focus = Focus::Preview;
                                }
                            } else if rect_contains(ui.preview_area, col, row) {
                                focus = Focus::Preview;
                            }
                        }
                        MouseEventKind::ScrollUp => {
                            if rect_contains(ui.preview_area, col, row) {
                                compositor.scroll_preview_up();
                            } else if let Some(detail_panel_area) = ui.detail_panel_area {
                                if rect_contains(detail_panel_area, col, row) {
                                    compositor.scroll_detail_up();
                                }
                            } else if rect_contains(ui.results_area, col, row) {
                                selected = selected.saturating_sub(1);
                            }
                        }
                        MouseEventKind::ScrollDown => {
                            if rect_contains(ui.preview_area, col, row) {
                                compositor.scroll_preview_down();
                            } else if let Some(detail_panel_area) = ui.detail_panel_area {
                                if rect_contains(detail_panel_area, col, row) {
                                    compositor.scroll_detail_down();
                                }
                            } else if rect_contains(ui.results_area, col, row) {
                                selected = selected
                                    .saturating_add(1)
                                    .min(filtered.len().saturating_sub(1));
                            }
                        }
                        _ => {}
                    }
                }
                Event::Resize(_, _) => {}
                _ => {}
            }
        }
    }
}

fn adjust_selected_index(current: usize, len: usize) -> usize {
    if len == 0 {
        0
    } else if current >= len {
        len - 1
    } else {
        current
    }
}

fn insert_paste(input: &mut Input, value: &str) {
    for ch in value.chars().filter(|ch| *ch != '\r') {
        input.handle(InputRequest::InsertChar(ch));
    }
}

fn filter_and_sort_visible(
    entries: &[NavigateEntry],
    query: &str,
    sort_mode: SortMode,
    meta_cache: &HashMap<String, SortMeta>,
    tag_cache: &HashMap<String, Vec<String>>,
    show_remote_branches: bool,
    pinned_path: Option<&str>,
) -> Vec<usize> {
    let mut indices: Vec<usize> = filter_and_sort(entries, query, sort_mode, meta_cache, tag_cache)
        .into_iter()
        .filter(|index| show_remote_branches || !is_remote_branch_entry(&entries[*index]))
        .collect();
    if query.trim().is_empty() {
        pin_current_project(&mut indices, entries, pinned_path);
    }
    indices
}

fn pin_current_project(
    indices: &mut Vec<usize>,
    entries: &[NavigateEntry],
    pinned_path: Option<&str>,
) {
    let Some(pinned_path) = pinned_path else {
        return;
    };
    let Some(position) = indices
        .iter()
        .position(|index| entry_matches_path(&entries[*index], pinned_path))
    else {
        return;
    };
    let index = indices.remove(position);
    indices.insert(0, index);
}

fn pinned_path(pin_current_project: bool, current_project_path: Option<&str>) -> Option<&str> {
    if pin_current_project {
        current_project_path
    } else {
        None
    }
}

fn entry_matches_path(entry: &NavigateEntry, path: &str) -> bool {
    entry.selection_path == path
        || entry.metadata_path == path
        || entry.preview_root_path == path
        || entry.preferred_preview_path.as_deref() == Some(path)
}

fn visible_entry_count(entries: &[NavigateEntry], show_remote_branches: bool) -> usize {
    entries
        .iter()
        .filter(|entry| show_remote_branches || !is_remote_branch_entry(entry))
        .count()
}

fn is_remote_branch_entry(entry: &NavigateEntry) -> bool {
    matches!(entry.kind, NavigateEntryKind::RemoteBranch { .. })
}

fn remote_toggle_state(
    show_remote_branches: bool,
    remote_fetching: bool,
    remote_error: bool,
) -> RemoteToggleState {
    if remote_fetching {
        RemoteToggleState::Fetching
    } else if remote_error {
        RemoteToggleState::Error
    } else if show_remote_branches {
        RemoteToggleState::Active
    } else {
        RemoteToggleState::Off
    }
}

fn start_remote_worktree_creation(entry: NavigateEntry, tx: mpsc::Sender<WorktreeProgress>) {
    thread::spawn(move || {
        let NavigateEntryKind::RemoteBranch {
            remote_branch,
            bare_path,
            container_path,
            ..
        } = entry.kind
        else {
            let _ = tx.send(WorktreeProgress::Error("not a remote branch".to_string()));
            return;
        };

        let result = git::add_worktree_for_remote_with_progress(
            Path::new(&bare_path),
            Path::new(&container_path),
            &remote_branch,
            progress_sender(&tx),
        );
        match result {
            Ok(path) => {
                let _ = tx.send(WorktreeProgress::Select(path));
            }
            Err(message) => {
                let _ = tx.send(WorktreeProgress::Error(message));
            }
        }
    });
}

fn start_worktree_deletion(entry: NavigateEntry, tx: mpsc::Sender<WorktreeProgress>) {
    thread::spawn(move || {
        let path = entry.selection_path.clone();
        let result =
            git::remove_worktree_safely_with_progress(Path::new(&path), progress_sender(&tx));
        match result {
            Ok(path) => {
                let _ = tx.send(WorktreeProgress::Deleted(path));
            }
            Err(message) => {
                let _ = tx.send(WorktreeProgress::Error(message));
            }
        }
    });
}

fn progress_sender(tx: &mpsc::Sender<WorktreeProgress>) -> impl FnMut(String) + '_ {
    move |message| {
        let _ = tx.send(WorktreeProgress::Message(message));
    }
}

fn worktree_overlay_title(entry: &NavigateEntry) -> String {
    if let NavigateEntryKind::RemoteBranch { remote_branch, .. } = &entry.kind {
        format!("Creating {remote_branch}")
    } else {
        "Creating worktree".to_string()
    }
}

fn delete_worktree_overlay_title(entry: &NavigateEntry) -> String {
    if let NavigateEntryKind::Worktree { branch, .. } = &entry.kind {
        format!("Deleting {branch}")
    } else {
        "Deleting worktree".to_string()
    }
}

fn render_worktree_overlay(
    frame: &mut ratatui::Frame,
    area: Rect,
    overlay: &WorktreeOverlay,
    accent: Color,
    warm: Color,
    text: Color,
    muted: Color,
) {
    let width = area.width.saturating_mul(2).saturating_div(3).clamp(48, 96);
    let height = area
        .height
        .saturating_mul(2)
        .saturating_div(5)
        .clamp(11, 18);
    let popup = centered_rect(area, width.min(area.width), height.min(area.height));
    frame.render_widget(Clear, popup);

    let border = if overlay.error.is_some() {
        warm
    } else {
        accent
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(overlay.title.clone())
        .border_style(Style::default().fg(border))
        .border_type(BorderType::Rounded);
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let progress_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: 4.min(inner.height),
    };
    let progress = if overlay.error.is_some() {
        1.0
    } else {
        (overlay.messages.len() as f64 / 5.0).clamp(0.08, 0.95)
    };
    let status = if overlay.error.is_some() {
        "Failed"
    } else {
        "Working"
    };
    let current_step = overlay
        .messages
        .last()
        .map(String::as_str)
        .unwrap_or("Starting");
    let progress_text = Text::from(vec![
        Line::from(vec![
            Span::styled(
                status,
                Style::default().fg(border).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(current_step.to_string(), Style::default().fg(text)),
        ]),
        render_progress_bar(progress, progress_area.width as usize, border, muted),
    ]);
    frame.render_widget(
        Paragraph::new(progress_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" {:>3}% ", (progress * 100.0).round() as u8))
                    .border_style(Style::default().fg(border)),
            )
            .wrap(Wrap { trim: false }),
        progress_area,
    );

    let messages_area = Rect {
        x: inner.x,
        y: inner.y.saturating_add(progress_area.height),
        width: inner.width,
        height: inner.height.saturating_sub(progress_area.height),
    };
    let max_messages = messages_area.height.saturating_sub(2) as usize;
    let start = overlay.messages.len().saturating_sub(max_messages);
    let mut lines = overlay.messages[start..].join("\n");
    if let Some(error) = &overlay.error {
        if !lines.is_empty() {
            lines.push('\n');
        }
        lines.push_str(error);
        lines.push_str("\n\nPress Enter or Esc to dismiss");
    }
    let messages = Paragraph::new(lines)
        .style(Style::default().fg(if overlay.error.is_some() { warm } else { text }))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Messages")
                .border_style(Style::default().fg(muted)),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(messages, messages_area);
}

fn render_progress_bar(progress: f64, width: usize, fill: Color, empty: Color) -> Line<'static> {
    let bar_width = width.saturating_sub(6).clamp(8, 72);
    let filled = ((bar_width as f64) * progress).round() as usize;
    let empty_count = bar_width.saturating_sub(filled);
    Line::from(vec![
        Span::styled("[", Style::default().fg(empty)),
        Span::styled("█".repeat(filled), Style::default().fg(fill)),
        Span::styled("░".repeat(empty_count), Style::default().fg(empty)),
        Span::styled("]", Style::default().fg(empty)),
    ])
}

fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

fn apply_preview_for_entry(
    compositor: &mut CurrentCompositor,
    data: &PreviewData,
    entry: Option<&NavigateEntry>,
) {
    compositor.apply_preview(data);
    if let Some(preferred_path) = entry.and_then(|entry| entry.preferred_preview_path.as_deref()) {
        compositor.select_preview_path(data, preferred_path);
    }
}

fn build_preview_data_for_entry(
    path: &str,
    entry: Option<&NavigateEntry>,
    accent: Color,
    muted: Color,
    text: Color,
    preview_settings: PreviewSettings,
) -> PreviewData {
    if let Some(NavigateEntry {
        kind:
            NavigateEntryKind::RemoteBranch {
                remote_branch,
                bare_path,
                ..
            },
        selection_path,
        ..
    }) = entry
    {
        return build_remote_branch_preview_data(
            bare_path,
            remote_branch,
            selection_path,
            accent,
            muted,
            text,
        );
    }

    build_preview_data(path, accent, muted, text, preview_settings)
}

fn preview_placeholder_path(entry: Option<&NavigateEntry>) -> Option<&str> {
    let entry = entry?;
    if is_remote_branch_entry(entry) {
        Some(entry.selection_path.as_str())
    } else {
        Some(entry.preview_root_path.as_str())
    }
}

fn focus_from_change(change: FocusChange) -> Focus {
    match change {
        FocusChange::Search => Focus::Search,
        FocusChange::Preview => Focus::Preview,
        FocusChange::Detail => Focus::Detail,
    }
}

fn compute_list_window_offset(
    selected: usize,
    current_offset: usize,
    height: usize,
    total: usize,
) -> usize {
    if total == 0 || height == 0 {
        return 0;
    }

    let mut offset = current_offset.min(total.saturating_sub(1));
    if selected < offset {
        offset = selected;
    } else if selected >= offset + height {
        offset = selected + 1 - height;
    }

    let max_offset = total.saturating_sub(height);
    if offset > max_offset {
        offset = max_offset;
    }
    offset
}

fn current_selection_entry<'a>(
    entries: &'a [NavigateEntry],
    filtered: &[usize],
    selected: usize,
) -> Option<&'a NavigateEntry> {
    filtered.get(selected).and_then(|index| entries.get(*index))
}

fn enter_selection_path(
    focus: Focus,
    current_path: Option<&str>,
    preview_tab_index: usize,
    preview_cache: &HashMap<String, PreviewData>,
) -> Option<String> {
    if !matches!(focus, Focus::Preview | Focus::Detail) {
        return None;
    }
    let current_path = current_path?;
    preview_cache
        .get(current_path)
        .and_then(|data| data.previews.get(preview_tab_index))
        .map(|tab| tab.path.clone())
}

fn selection_path_for_action(
    focus: Focus,
    current_path: Option<&str>,
    preview_tab_index: usize,
    preview_cache: &HashMap<String, PreviewData>,
    selected_entry: Option<&NavigateEntry>,
) -> Option<String> {
    enter_selection_path(focus, current_path, preview_tab_index, preview_cache).or_else(|| {
        selected_entry.map(|entry| match &entry.kind {
            NavigateEntryKind::Project => selectable_project_path(entry),
            _ => entry.selection_path.clone(),
        })
    })
}

fn selectable_project_path(entry: &NavigateEntry) -> String {
    git::selectable_worktree_path_for_project(Path::new(&entry.selection_path))
        .unwrap_or_else(|| entry.selection_path.clone())
}

fn visible_paths_for_window(
    entries: &[NavigateEntry],
    filtered: &[usize],
    offset: usize,
    height: usize,
) -> Vec<String> {
    if filtered.is_empty() || height == 0 {
        return Vec::new();
    }
    let end = (offset + height).min(filtered.len());
    filtered[offset..end]
        .iter()
        .filter_map(|index| entries.get(*index))
        .map(|entry| entry.metadata_path.clone())
        .collect()
}

fn all_metadata_paths(entries: &[NavigateEntry]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut paths = Vec::new();
    for entry in entries {
        if seen.insert(entry.metadata_path.clone()) {
            paths.push(entry.metadata_path.clone());
        }
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::default_preview_settings;
    use ratatui::text::Line;

    #[test]
    fn parses_git_worktree_porcelain_with_bare_and_branch_entries() {
        let output = "worktree /repos/example.git\nbare\n\nworktree /repos/example\nHEAD 123456\nbranch refs/heads/main\n\nworktree /repos/example-feature\nHEAD abcdef\nbranch refs/heads/feature/worktree\n";

        let worktrees = git::parse_git_worktree_list(output);

        assert_eq!(worktrees.len(), 3);
        assert_eq!(worktrees[0].path, "/repos/example.git");
        assert!(worktrees[0].bare);
        assert_eq!(worktrees[1].path, "/repos/example");
        assert_eq!(worktrees[1].branch.as_deref(), Some("main"));
        assert_eq!(worktrees[2].branch.as_deref(), Some("feature/worktree"));
    }

    #[test]
    fn labels_detached_worktree_when_no_branch_is_reported() {
        let output = "worktree /repos/example-detached\nHEAD abcdef\ndetached\n";

        let worktrees = git::parse_git_worktree_list(output);

        assert_eq!(worktrees.len(), 1);
        assert!(worktrees[0].detached);
        assert_eq!(git::git_worktree_label(&worktrees[0], true), "detached");
    }

    #[test]
    fn shortens_worktree_tab_label_after_last_slash() {
        assert_eq!(
            git::worktree_tab_label("feat/yarden/potato", true),
            "potato"
        );
        assert_eq!(
            git::worktree_tab_label("feat/yarden/potato", false),
            "feat/yarden/potato"
        );
    }

    #[test]
    fn pseudo_scrolls_tabs_from_previous_label() {
        let labels = vec![
            "main".to_string(),
            "feature-1".to_string(),
            "feature-2".to_string(),
            "feature-3".to_string(),
            "feature-4".to_string(),
        ];

        let settings = default_preview_settings();
        let (first_visible, first_selected) = ui::visible_tab_window(&labels, 0, 35, settings);
        let (middle_visible, middle_selected) = ui::visible_tab_window(&labels, 2, 40, settings);

        assert_eq!(first_selected, 0);
        assert_eq!(
            first_visible,
            vec!["main", "feature-1", "feature-2", "f..."]
        );
        assert_eq!(middle_selected, 1);
        assert_eq!(
            middle_visible,
            vec!["feature-1", "feature-2", "feature-3", "f..."]
        );
    }

    #[test]
    fn truncates_long_tab_labels_with_ellipsis() {
        assert_eq!(
            ui::truncate_tab_label("very-long-feature", 10),
            "very-lo..."
        );
        assert_eq!(ui::truncate_tab_label("abc", 10), "abc");
        assert_eq!(ui::truncate_tab_label("abcdef", 3), "...");
    }

    #[test]
    fn keeps_more_selected_tab_chars_before_ellipsis() {
        let labels = vec![
            "previous-worktree".to_string(),
            "selected-worktree".to_string(),
            "next-worktree".to_string(),
        ];
        let settings = PreviewSettings {
            shorten_worktree_tab_labels: true,
            worktree_tab_min_chars: 6,
            selected_worktree_tab_min_chars: 10,
        };

        let (visible, selected) = ui::visible_tab_window(&labels, 1, 25, settings);

        assert_eq!(selected, 1);
        assert_eq!(visible, vec!["previo...", "selected-w..."]);
    }

    #[test]
    fn enter_from_preview_returns_active_worktree_path() {
        let mut cache = HashMap::new();
        cache.insert(
            "/repos/project.git".to_string(),
            PreviewData {
                previews: vec![
                    model::PreviewTab {
                        path: "/repos/project".to_string(),
                        label: "main".to_string(),
                        text: Text::default(),
                        git: None,
                        github_readme: None,
                    },
                    model::PreviewTab {
                        path: "/repos/project-feature".to_string(),
                        label: "feature".to_string(),
                        text: Text::default(),
                        git: None,
                        github_readme: None,
                    },
                ],
                selected_repo_is_bare: false,
                git_loaded: false,
                github_readme_loaded: false,
            },
        );

        let value = enter_selection_path(Focus::Preview, Some("/repos/project.git"), 1, &cache);

        assert_eq!(value.as_deref(), Some("/repos/project-feature"));
        assert!(
            enter_selection_path(Focus::Search, Some("/repos/project.git"), 1, &cache).is_none()
        );
        let selected_entry = test_entry("project", "project.git", "/repos/project.git");
        assert_eq!(
            selection_path_for_action(
                Focus::Search,
                Some("/repos/project.git"),
                1,
                &cache,
                Some(&selected_entry),
            )
            .as_deref(),
            Some("/repos/project.git")
        );
    }

    #[test]
    fn applies_git_result_to_one_preview_tab() {
        let mut data = PreviewData {
            previews: vec![
                model::PreviewTab {
                    path: "/repos/project".to_string(),
                    label: "main".to_string(),
                    text: Text::default(),
                    git: None,
                    github_readme: None,
                },
                model::PreviewTab {
                    path: "/repos/project-feature".to_string(),
                    label: "feature".to_string(),
                    text: Text::default(),
                    git: None,
                    github_readme: None,
                },
            ],
            selected_repo_is_bare: true,
            git_loaded: false,
            github_readme_loaded: false,
        };

        apply_git_result(
            &mut data,
            1,
            Some(Text::from(Line::from("Branch: feature"))),
            false,
        );

        assert!(data.previews[0].git.is_none());
        assert!(data.previews[1].git.is_some());
        assert!(!data.git_loaded);
    }

    #[test]
    fn orders_github_detail_before_git_detail() {
        let tab = model::PreviewTab {
            path: "/repos/project".to_string(),
            label: "main".to_string(),
            text: Text::default(),
            git: Some(Text::from(Line::from("Branch: main"))),
            github_readme: Some(Text::from(Line::from("# Project"))),
        };

        let detail_tabs = content::detail_tabs_for_preview(&tab);

        assert_eq!(detail_tabs.len(), 2);
        assert_eq!(detail_tabs[0].label, "GitHub");
        assert_eq!(detail_tabs[1].label, "Git");
    }

    #[test]
    fn filters_preview_worktree_tabs_by_label_or_path() {
        let data = PreviewData {
            previews: vec![
                model::PreviewTab {
                    path: "/repos/project".to_string(),
                    label: "main".to_string(),
                    text: Text::default(),
                    git: None,
                    github_readme: None,
                },
                model::PreviewTab {
                    path: "/repos/project-feature".to_string(),
                    label: "feature".to_string(),
                    text: Text::default(),
                    git: None,
                    github_readme: None,
                },
                model::PreviewTab {
                    path: "/repos/project-hotfix".to_string(),
                    label: "hotfix".to_string(),
                    text: Text::default(),
                    git: None,
                    github_readme: None,
                },
            ],
            selected_repo_is_bare: false,
            git_loaded: false,
            github_readme_loaded: false,
        };

        assert_eq!(content::preview_tab_visible_indexes(&data, "feat"), vec![1]);
        assert_eq!(content::preview_tab_visible_indexes(&data, "hot"), vec![2]);
        assert_eq!(
            content::preview_tab_visible_indexes(&data, "missing"),
            vec![0, 1, 2]
        );
    }

    #[test]
    fn visible_name_match_does_not_include_deep_parent_path_match() {
        let entries = vec![
            test_entry(
                "child",
                "Funnel Not Working",
                "/repos/Trading Platform Location/Funnel Not Working",
            ),
            test_entry(
                "parent",
                "Trading Platform Location",
                "/repos/Trading Platform Location",
            ),
        ];
        let filtered = search::filter_and_sort(
            &entries,
            "trading platform location",
            SortMode::Match,
            &HashMap::new(),
            &HashMap::new(),
        );

        assert_eq!(filtered, vec![1]);

        let tokens = search::parse_query_tokens("trading platform location");
        assert_eq!(
            search::entry_match_context(&entries[0], &[], &tokens).as_deref(),
            None
        );
    }

    #[test]
    fn entry_match_context_explains_hidden_search_field_match() {
        let entry = model::NavigateEntry {
            id: "hidden".to_string(),
            display: "dev-on-tuesday-github-org-portal-ui".to_string(),
            context: None,
            preview_root_path: "/repos/ideda/dev-on-tuesday-github-org-portal-ui".to_string(),
            preferred_preview_path: None,
            selection_path: "/repos/ideda/dev-on-tuesday-github-org-portal-ui".to_string(),
            metadata_path: "/repos/ideda/dev-on-tuesday-github-org-portal-ui".to_string(),
            search_text: vec![
                "dev-on-tuesday-github-org-portal-ui".to_string(),
                "/repos/ideda/dev-on-tuesday-github-org-portal-ui".to_string(),
            ],
            kind: model::NavigateEntryKind::Project,
        };
        let tokens = search::parse_query_tokens("ideda");

        assert_eq!(
            search::entry_match_context(&entry, &[], &tokens).as_deref(),
            None
        );
    }

    #[test]
    fn compositor_selects_preferred_worktree_preview_tab() {
        let data = PreviewData {
            previews: vec![
                model::PreviewTab {
                    path: "/repos/project".to_string(),
                    label: "main".to_string(),
                    text: Text::from(Line::from("main")),
                    git: None,
                    github_readme: None,
                },
                model::PreviewTab {
                    path: "/repos/project-QCDI-8206".to_string(),
                    label: "QCDI-8206".to_string(),
                    text: Text::from(Line::from("worktree")),
                    git: None,
                    github_readme: None,
                },
            ],
            selected_repo_is_bare: false,
            git_loaded: false,
            github_readme_loaded: false,
        };
        let mut compositor = CurrentCompositor::new(Text::default());

        compositor.apply_preview(&data);
        compositor.select_preview_path(&data, "/repos/project-QCDI-8206");

        assert_eq!(compositor.active_content_index(), 1);
        assert_eq!(compositor.preview_tab_visible_index, 1);
    }

    fn test_entry(id: &str, display: &str, path: &str) -> model::NavigateEntry {
        model::NavigateEntry {
            id: id.to_string(),
            display: display.to_string(),
            context: None,
            preview_root_path: path.to_string(),
            preferred_preview_path: None,
            selection_path: path.to_string(),
            metadata_path: path.to_string(),
            search_text: vec![display.to_string()],
            kind: model::NavigateEntryKind::Project,
        }
    }

    #[test]
    fn sorts_default_branch_worktree_first() {
        let trunk = model::GitWorktree {
            path: "/repos/project-trunk".to_string(),
            branch: Some("trunk".to_string()),
            detached: false,
            bare: false,
        };
        let feature = model::GitWorktree {
            path: "/repos/project-feature".to_string(),
            branch: Some("feature".to_string()),
            detached: false,
            bare: false,
        };
        let mut worktrees = vec![&feature, &trunk];

        content::sort_worktrees_default_first(&mut worktrees, Some("trunk"));

        assert_eq!(worktrees[0].branch.as_deref(), Some("trunk"));
        assert_eq!(worktrees[1].branch.as_deref(), Some("feature"));
    }

    #[test]
    fn displays_home_paths_with_tilde() {
        assert_eq!(
            content::display_path_with_home("/Users/kcw", "/Users/kcw"),
            "~"
        );
        assert_eq!(
            content::display_path_with_home("/Users/kcw/Github/navgator", "/Users/kcw"),
            "~/Github/navgator"
        );
        assert_eq!(
            content::display_path_with_home("/Users/kcw-other/Github", "/Users/kcw"),
            "/Users/kcw-other/Github"
        );
    }

    #[test]
    fn resolves_dot_bare_worktree_container() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        let root = env::temp_dir().join(format!("navgator-dot-bare-test-{unique}"));
        let dot_bare = root.join(".bare");
        std::fs::create_dir_all(&root).expect("test root should be created");

        let status = std::process::Command::new("git")
            .arg("init")
            .arg("--bare")
            .arg(&dot_bare)
            .status()
            .expect("git should be available");
        assert!(status.success());

        let resolved = git::git_command_dir_for_path(&root);
        let _ = std::fs::remove_dir_all(&root);

        assert_eq!(resolved.as_deref(), Some(dot_bare.as_path()));
    }
}
