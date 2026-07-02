use crate::content::{apply_preview_data, preview_tab_visible_indexes, ApplyPreviewData};
use crate::model::{DetailTab, Focus, PreviewData};
use crate::search::entry_name;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::text::Text;
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;

pub(crate) enum FocusChange {
    Search,
    Preview,
    Detail,
}

pub(crate) trait NavigationCompositor {
    fn show_detail(&self) -> bool;
    fn active_content_index(&self) -> usize;
    fn apply_preview(&mut self, data: &PreviewData);
    fn reset_for_no_selection(&mut self, placeholder: Text<'static>);
    fn reset_for_new_selection(&mut self);
    fn set_loading(&mut self, placeholder: Text<'static>);
    fn ensure_valid_focus(&self, focus: Focus) -> Focus;
    fn preview_title(&self, current_path: Option<&str>) -> String;
}

pub(crate) struct CurrentCompositor {
    pub(crate) preview_text: Text<'static>,
    pub(crate) preview_tab_index: usize,
    pub(crate) preview_tab_visible_index: usize,
    pub(crate) preview_tab_count: usize,
    pub(crate) preview_tab_labels: Vec<String>,
    pub(crate) worktree_filter: Input,
    pub(crate) detail_tabs: Vec<DetailTab>,
    pub(crate) detail_tab_index: usize,
    pub(crate) preview_scroll: usize,
    pub(crate) detail_scroll: usize,
    pub(crate) preview_max_scroll: usize,
    pub(crate) detail_max_scroll: usize,
    pub(crate) preview_page_step: usize,
    pub(crate) detail_page_step: usize,
}

impl CurrentCompositor {
    pub(crate) fn new(placeholder: Text<'static>) -> Self {
        Self {
            preview_text: placeholder,
            preview_tab_index: 0,
            preview_tab_visible_index: 0,
            preview_tab_count: 1,
            preview_tab_labels: Vec::new(),
            worktree_filter: Input::default(),
            detail_tabs: Vec::new(),
            detail_tab_index: 0,
            preview_scroll: 0,
            detail_scroll: 0,
            preview_max_scroll: 0,
            detail_max_scroll: 0,
            preview_page_step: 5,
            detail_page_step: 5,
        }
    }

    pub(crate) fn handle_preview_key(
        &mut self,
        key: KeyEvent,
        data: Option<&PreviewData>,
    ) -> Option<FocusChange> {
        match key.code {
            KeyCode::Char('u')
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && !self.worktree_filter.value().is_empty() =>
            {
                self.worktree_filter.reset();
                self.reset_active_preview_position();
                if let Some(data) = data {
                    self.apply_preview(data);
                }
            }
            KeyCode::Char(_)
                if !key.modifiers.intersects(
                    KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER,
                ) =>
            {
                let before = self.worktree_filter.value().to_string();
                let _ = self.worktree_filter.handle_event(&Event::Key(key));
                if self.worktree_filter.value() != before {
                    self.reset_active_preview_position();
                    if let Some(data) = data {
                        self.apply_preview(data);
                    }
                }
            }
            KeyCode::Backspace if !self.worktree_filter.value().is_empty() => {
                let before = self.worktree_filter.value().to_string();
                let _ = self.worktree_filter.handle_event(&Event::Key(key));
                if self.worktree_filter.value() != before {
                    self.reset_active_preview_position();
                    if let Some(data) = data {
                        self.apply_preview(data);
                    }
                }
            }
            KeyCode::Left => {
                if self.move_preview_tab(data, -1) {
                    return None;
                }
                return Some(FocusChange::Search);
            }
            KeyCode::Right => {
                if self.move_preview_tab(data, 1) {
                    return None;
                }
                if !self.detail_tabs.is_empty() {
                    return Some(FocusChange::Detail);
                }
            }
            KeyCode::Up => {
                if self.preview_scroll > 0 {
                    self.preview_scroll -= 1;
                } else if self.move_preview_tab(data, -1) {
                    return None;
                } else {
                    return Some(FocusChange::Search);
                }
            }
            KeyCode::Down => {
                if self.preview_scroll < self.preview_max_scroll {
                    self.preview_scroll += 1;
                } else if self.move_preview_tab(data, 1) {
                    return None;
                } else if !self.detail_tabs.is_empty() {
                    return Some(FocusChange::Detail);
                }
            }
            KeyCode::PageUp => {
                self.preview_scroll = self.preview_scroll.saturating_sub(self.preview_page_step);
            }
            KeyCode::PageDown => {
                self.preview_scroll =
                    (self.preview_scroll + self.preview_page_step).min(self.preview_max_scroll);
            }
            KeyCode::Home => {
                self.preview_scroll = 0;
            }
            KeyCode::End => {
                self.preview_scroll = self.preview_max_scroll;
            }
            _ => {}
        }
        None
    }

    pub(crate) fn handle_detail_key(
        &mut self,
        key: KeyEvent,
        data: Option<&PreviewData>,
    ) -> Option<FocusChange> {
        match key.code {
            KeyCode::Left => {
                if self.detail_tab_index > 0 {
                    self.detail_tab_index -= 1;
                    self.detail_scroll = 0;
                } else {
                    self.preview_tab_visible_index = self.preview_tab_count.saturating_sub(1);
                    self.preview_scroll = 0;
                    self.set_preview_from_visible_index(data);
                    if let Some(data) = data {
                        self.apply_preview(data);
                    }
                    return Some(FocusChange::Preview);
                }
            }
            KeyCode::Right => {
                if self.detail_tab_index + 1 < self.detail_tabs.len() {
                    self.detail_tab_index += 1;
                    self.detail_scroll = 0;
                } else {
                    return Some(FocusChange::Preview);
                }
            }
            KeyCode::Up => {
                if self.detail_scroll > 0 {
                    self.detail_scroll -= 1;
                } else {
                    return Some(FocusChange::Preview);
                }
            }
            KeyCode::Down if self.detail_scroll < self.detail_max_scroll => {
                self.detail_scroll += 1;
            }
            KeyCode::PageUp => {
                self.detail_scroll = self.detail_scroll.saturating_sub(self.detail_page_step);
            }
            KeyCode::PageDown => {
                self.detail_scroll =
                    (self.detail_scroll + self.detail_page_step).min(self.detail_max_scroll);
            }
            KeyCode::Home => {
                self.detail_scroll = 0;
            }
            KeyCode::End => {
                self.detail_scroll = self.detail_max_scroll;
            }
            _ => {}
        }
        None
    }

    pub(crate) fn scroll_preview_up(&mut self) {
        self.preview_scroll = self.preview_scroll.saturating_sub(1);
    }

    pub(crate) fn scroll_preview_down(&mut self) {
        self.preview_scroll = (self.preview_scroll + 1).min(self.preview_max_scroll);
    }

    pub(crate) fn scroll_detail_up(&mut self) {
        self.detail_scroll = self.detail_scroll.saturating_sub(1);
    }

    pub(crate) fn scroll_detail_down(&mut self) {
        self.detail_scroll = (self.detail_scroll + 1).min(self.detail_max_scroll);
    }

    pub(crate) fn select_preview_path(&mut self, data: &PreviewData, preferred_path: &str) {
        let Some(index) = data
            .previews
            .iter()
            .position(|tab| tab.path == preferred_path)
        else {
            return;
        };
        self.preview_tab_index = index;
        self.preview_scroll = 0;
        self.detail_tab_index = 0;
        self.detail_scroll = 0;
        self.apply_preview(data);
    }

    fn reset_active_preview_position(&mut self) {
        self.preview_tab_visible_index = 0;
        self.preview_scroll = 0;
        self.detail_tab_index = 0;
        self.detail_scroll = 0;
    }

    fn move_preview_tab(&mut self, data: Option<&PreviewData>, delta: isize) -> bool {
        let next = if delta < 0 {
            self.preview_tab_visible_index.checked_sub(1)
        } else if self.preview_tab_visible_index + 1 < self.preview_tab_count {
            Some(self.preview_tab_visible_index + 1)
        } else {
            None
        };
        let Some(next) = next else {
            return false;
        };

        self.preview_tab_visible_index = next;
        self.preview_scroll = 0;
        self.detail_tab_index = 0;
        self.detail_scroll = 0;
        self.set_preview_from_visible_index(data);
        if let Some(data) = data {
            self.apply_preview(data);
        }
        true
    }

    fn set_preview_from_visible_index(&mut self, data: Option<&PreviewData>) {
        let Some(data) = data else {
            return;
        };
        let visible_indexes = preview_tab_visible_indexes(data, self.worktree_filter.value());
        if let Some(index) = visible_indexes.get(self.preview_tab_visible_index) {
            self.preview_tab_index = *index;
        }
    }
}

impl NavigationCompositor for CurrentCompositor {
    fn show_detail(&self) -> bool {
        !self.detail_tabs.is_empty()
    }

    fn active_content_index(&self) -> usize {
        self.preview_tab_index
    }

    fn apply_preview(&mut self, data: &PreviewData) {
        apply_preview_data(
            data,
            ApplyPreviewData {
                tab_index: &mut self.preview_tab_index,
                tab_visible_index: &mut self.preview_tab_visible_index,
                tab_count: &mut self.preview_tab_count,
                tab_labels: &mut self.preview_tab_labels,
                preview_text: &mut self.preview_text,
                detail_tabs: &mut self.detail_tabs,
                detail_tab_index: &mut self.detail_tab_index,
                worktree_filter: self.worktree_filter.value(),
            },
        );
    }

    fn reset_for_no_selection(&mut self, placeholder: Text<'static>) {
        self.preview_text = placeholder;
        self.detail_tabs.clear();
        self.preview_tab_index = 0;
        self.preview_tab_visible_index = 0;
        self.detail_tab_index = 0;
        self.preview_tab_count = 1;
        self.preview_tab_labels.clear();
        self.worktree_filter.reset();
        self.preview_scroll = 0;
        self.detail_scroll = 0;
    }

    fn reset_for_new_selection(&mut self) {
        self.preview_tab_index = 0;
        self.preview_tab_visible_index = 0;
        self.detail_tab_index = 0;
        self.preview_tab_count = 1;
        self.preview_tab_labels.clear();
        self.worktree_filter.reset();
        self.preview_scroll = 0;
        self.detail_scroll = 0;
    }

    fn set_loading(&mut self, placeholder: Text<'static>) {
        self.preview_text = placeholder;
        self.detail_tabs.clear();
        self.detail_tab_index = 0;
        self.preview_tab_labels.clear();
    }

    fn ensure_valid_focus(&self, focus: Focus) -> Focus {
        if focus == Focus::Detail && self.detail_tabs.is_empty() {
            Focus::Preview
        } else {
            focus
        }
    }

    fn preview_title(&self, current_path: Option<&str>) -> String {
        let title = current_path
            .map(entry_name)
            .unwrap_or_else(|| "Preview".to_string());
        let filter = self.worktree_filter.value().trim();
        if filter.is_empty() {
            title
        } else {
            format!("{} / {}", title, filter)
        }
    }
}
