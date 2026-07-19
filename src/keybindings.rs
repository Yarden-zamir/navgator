use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum BindingContext {
    Global,
    Navigator,
    Preview,
    Detail,
    TagEditor,
    ActionPicker,
    CreatePicker,
    CreateForm,
    CreateCompletions,
    ProgressOverlay,
    ErrorOverlay,
}

impl BindingContext {
    pub(crate) const ORDERED: [Self; 11] = [
        Self::Global,
        Self::Navigator,
        Self::Preview,
        Self::Detail,
        Self::TagEditor,
        Self::ActionPicker,
        Self::CreatePicker,
        Self::CreateForm,
        Self::CreateCompletions,
        Self::ProgressOverlay,
        Self::ErrorOverlay,
    ];

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Navigator => "navigator",
            Self::Preview => "preview",
            Self::Detail => "detail",
            Self::TagEditor => "tag-editor",
            Self::ActionPicker => "action-picker",
            Self::CreatePicker => "create-picker",
            Self::CreateForm => "create-form",
            Self::CreateCompletions => "create-completions",
            Self::ProgressOverlay => "progress-overlay",
            Self::ErrorOverlay => "error-overlay",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "global" => Some(Self::Global),
            "navigator" => Some(Self::Navigator),
            "preview" => Some(Self::Preview),
            "detail" => Some(Self::Detail),
            "tag-editor" => Some(Self::TagEditor),
            "action-picker" => Some(Self::ActionPicker),
            "create-picker" => Some(Self::CreatePicker),
            "create-form" => Some(Self::CreateForm),
            "create-completions" => Some(Self::CreateCompletions),
            "progress-overlay" => Some(Self::ProgressOverlay),
            "error-overlay" => Some(Self::ErrorOverlay),
            _ => None,
        }
    }

    pub(crate) const fn ordered() -> &'static [Self] {
        &Self::ORDERED
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum CoreAction {
    Navigate,
    Actions,
    Create,
    Run,
    RunAndClose,
    Cancel,
    Back,
    Confirm,
    Accept,
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    PageUp,
    PageDown,
    MoveHome,
    MoveEnd,
    CopyPath,
    DeleteWorktree,
    ToggleRemotes,
    EditTags,
    CycleSort,
    ClearInput,
    RemoveLastTag,
    DismissOverlay,
}

impl CoreAction {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Navigate => "navigate",
            Self::Actions => "actions",
            Self::Create => "create",
            Self::Run => "run",
            Self::RunAndClose => "run-and-close",
            Self::Cancel => "cancel",
            Self::Back => "back",
            Self::Confirm => "confirm",
            Self::Accept => "accept",
            Self::MoveUp => "move-up",
            Self::MoveDown => "move-down",
            Self::MoveLeft => "move-left",
            Self::MoveRight => "move-right",
            Self::PageUp => "page-up",
            Self::PageDown => "page-down",
            Self::MoveHome => "move-home",
            Self::MoveEnd => "move-end",
            Self::CopyPath => "copy-path",
            Self::DeleteWorktree => "delete-worktree",
            Self::ToggleRemotes => "toggle-remotes",
            Self::EditTags => "edit-tags",
            Self::CycleSort => "cycle-sort",
            Self::ClearInput => "clear-input",
            Self::RemoveLastTag => "remove-last-tag",
            Self::DismissOverlay => "dismiss-overlay",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "navigate" => Some(Self::Navigate),
            "actions" => Some(Self::Actions),
            "create" => Some(Self::Create),
            "run" => Some(Self::Run),
            "run-and-close" => Some(Self::RunAndClose),
            "cancel" => Some(Self::Cancel),
            "back" => Some(Self::Back),
            "confirm" => Some(Self::Confirm),
            "accept" => Some(Self::Accept),
            "move-up" => Some(Self::MoveUp),
            "move-down" => Some(Self::MoveDown),
            "move-left" => Some(Self::MoveLeft),
            "move-right" => Some(Self::MoveRight),
            "page-up" => Some(Self::PageUp),
            "page-down" => Some(Self::PageDown),
            "move-home" => Some(Self::MoveHome),
            "move-end" => Some(Self::MoveEnd),
            "copy-path" => Some(Self::CopyPath),
            "delete-worktree" => Some(Self::DeleteWorktree),
            "toggle-remotes" => Some(Self::ToggleRemotes),
            "edit-tags" => Some(Self::EditTags),
            "cycle-sort" => Some(Self::CycleSort),
            "clear-input" => Some(Self::ClearInput),
            "remove-last-tag" => Some(Self::RemoveLastTag),
            "dismiss-overlay" => Some(Self::DismissOverlay),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum BindingTarget {
    Core(CoreAction),
    Configured(String),
    Disabled,
}

impl BindingTarget {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        if value == "none" {
            return Ok(Self::Disabled);
        }
        if let Some(action) = CoreAction::parse(value) {
            return Ok(Self::Core(action));
        }
        if is_valid_action_id(value) {
            return Ok(Self::Configured(value.to_string()));
        }
        Err(format!("invalid action identifier: {value}"))
    }

    pub(crate) fn as_str(&self) -> &str {
        match self {
            Self::Core(action) => action.as_str(),
            Self::Configured(action) => action,
            Self::Disabled => "none",
        }
    }
}

pub(crate) fn is_valid_action_id(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('-')
        && !value.ends_with('-')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && !value.as_bytes().windows(2).any(|pair| pair == b"--")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct KeyChord {
    pub(crate) code: KeyCode,
    pub(crate) modifiers: KeyModifiers,
}

impl KeyChord {
    pub(crate) fn new(code: KeyCode, modifiers: KeyModifiers) -> Self {
        let (code, modifiers) = normalize_chord(code, modifiers);
        Self { code, modifiers }
    }

    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        let value = value.trim();
        if value.is_empty() {
            return Err("key chord cannot be empty".to_string());
        }

        if value.chars().count() == 1 {
            return Ok(Self::new(
                KeyCode::Char(value.chars().next().expect("one character was checked")),
                KeyModifiers::NONE,
            ));
        }

        let normalized = value.replace('+', "-");
        let mut remaining = normalized.as_str();
        let mut modifiers = KeyModifiers::NONE;
        while let Some((prefix, rest)) = remaining.split_once('-') {
            let modifier = match prefix.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => KeyModifiers::CONTROL,
                "alt" | "option" => KeyModifiers::ALT,
                "shift" => KeyModifiers::SHIFT,
                "super" | "cmd" | "command" => KeyModifiers::SUPER,
                _ => break,
            };
            if modifiers.contains(modifier) {
                return Err(format!("duplicate key modifier: {prefix}"));
            }
            modifiers.insert(modifier);
            remaining = rest;
        }

        let code = parse_key_code(remaining)
            .ok_or_else(|| format!("invalid or unsupported key: {remaining}"))?;
        Ok(Self::new(code, modifiers))
    }

    pub(crate) fn as_str(self) -> String {
        let mut parts = Vec::with_capacity(5);
        if self.modifiers.contains(KeyModifiers::CONTROL) {
            parts.push("ctrl".to_string());
        }
        if self.modifiers.contains(KeyModifiers::ALT) {
            parts.push("alt".to_string());
        }
        if self.modifiers.contains(KeyModifiers::SHIFT) {
            parts.push("shift".to_string());
        }
        if self.modifiers.contains(KeyModifiers::SUPER) {
            parts.push("super".to_string());
        }
        parts.push(format_key_code(self.code));
        parts.join("+")
    }

    pub(crate) fn matches_event(self, event: &KeyEvent) -> bool {
        matches!(event.kind, KeyEventKind::Press | KeyEventKind::Repeat)
            && self == Self::new(event.code, event.modifiers)
    }
}

impl Ord for KeyChord {
    fn cmp(&self, other: &Self) -> Ordering {
        self.code
            .partial_cmp(&other.code)
            .expect("crossterm key code ordering is total")
            .then_with(|| {
                self.modifiers
                    .partial_cmp(&other.modifiers)
                    .expect("crossterm modifier ordering is total")
            })
    }
}

impl PartialOrd for KeyChord {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for KeyChord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.as_str())
    }
}

fn normalize_chord(code: KeyCode, mut modifiers: KeyModifiers) -> (KeyCode, KeyModifiers) {
    match code {
        KeyCode::BackTab => {
            modifiers.insert(KeyModifiers::SHIFT);
            (KeyCode::Tab, modifiers)
        }
        KeyCode::Char(character) if character.is_ascii_uppercase() => {
            modifiers.insert(KeyModifiers::SHIFT);
            (KeyCode::Char(character.to_ascii_lowercase()), modifiers)
        }
        _ => (code, modifiers),
    }
}

fn parse_key_code(value: &str) -> Option<KeyCode> {
    if value.chars().count() == 1 {
        return value.chars().next().map(KeyCode::Char);
    }

    let normalized = value.to_ascii_lowercase();
    let named = match normalized.as_str() {
        "enter" => KeyCode::Enter,
        "space" => KeyCode::Char(' '),
        "tab" => KeyCode::Tab,
        "backtab" => KeyCode::BackTab,
        "esc" => KeyCode::Esc,
        "backspace" => KeyCode::Backspace,
        "delete" => KeyCode::Delete,
        "insert" => KeyCode::Insert,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" => KeyCode::PageUp,
        "pagedown" => KeyCode::PageDown,
        "null" => KeyCode::Null,
        "caps-lock" => KeyCode::CapsLock,
        "scroll-lock" => KeyCode::ScrollLock,
        "num-lock" => KeyCode::NumLock,
        "print-screen" => KeyCode::PrintScreen,
        "pause" => KeyCode::Pause,
        "menu" => KeyCode::Menu,
        "keypad-begin" => KeyCode::KeypadBegin,
        "plus" => KeyCode::Char('+'),
        "equals" => KeyCode::Char('='),
        "colon" => KeyCode::Char(':'),
        "semicolon" => KeyCode::Char(';'),
        "comma" => KeyCode::Char(','),
        "period" => KeyCode::Char('.'),
        "minus" => KeyCode::Char('-'),
        "slash" => KeyCode::Char('/'),
        "backslash" => KeyCode::Char('\\'),
        "quote" => KeyCode::Char('\''),
        "backtick" => KeyCode::Char('`'),
        "left-bracket" => KeyCode::Char('['),
        "right-bracket" => KeyCode::Char(']'),
        _ => {
            if let Some(number) = normalized
                .strip_prefix('f')
                .and_then(|number| number.parse().ok())
            {
                if (1..=35).contains(&number) {
                    return Some(KeyCode::F(number));
                }
            }
            return None;
        }
    };
    Some(named)
}

fn format_key_code(code: KeyCode) -> String {
    match code {
        KeyCode::Backspace => "backspace".to_string(),
        KeyCode::Enter => "enter".to_string(),
        KeyCode::Left => "left".to_string(),
        KeyCode::Right => "right".to_string(),
        KeyCode::Up => "up".to_string(),
        KeyCode::Down => "down".to_string(),
        KeyCode::Home => "home".to_string(),
        KeyCode::End => "end".to_string(),
        KeyCode::PageUp => "pageup".to_string(),
        KeyCode::PageDown => "pagedown".to_string(),
        KeyCode::Tab => "tab".to_string(),
        KeyCode::BackTab => "backtab".to_string(),
        KeyCode::Delete => "delete".to_string(),
        KeyCode::Insert => "insert".to_string(),
        KeyCode::F(number) => format!("f{number}"),
        KeyCode::Char(' ') => "space".to_string(),
        KeyCode::Char('+') => "plus".to_string(),
        KeyCode::Char('=') => "equals".to_string(),
        KeyCode::Char(':') => "colon".to_string(),
        KeyCode::Char(';') => "semicolon".to_string(),
        KeyCode::Char(',') => "comma".to_string(),
        KeyCode::Char('.') => "period".to_string(),
        KeyCode::Char('-') => "minus".to_string(),
        KeyCode::Char('/') => "slash".to_string(),
        KeyCode::Char('\\') => "backslash".to_string(),
        KeyCode::Char('\'') => "quote".to_string(),
        KeyCode::Char('`') => "backtick".to_string(),
        KeyCode::Char('[') => "left-bracket".to_string(),
        KeyCode::Char(']') => "right-bracket".to_string(),
        KeyCode::Char(character) => character.to_string(),
        KeyCode::Null => "null".to_string(),
        KeyCode::Esc => "esc".to_string(),
        KeyCode::CapsLock => "caps-lock".to_string(),
        KeyCode::ScrollLock => "scroll-lock".to_string(),
        KeyCode::NumLock => "num-lock".to_string(),
        KeyCode::PrintScreen => "print-screen".to_string(),
        KeyCode::Pause => "pause".to_string(),
        KeyCode::Menu => "menu".to_string(),
        KeyCode::KeypadBegin => "keypad-begin".to_string(),
        KeyCode::Media(_) | KeyCode::Modifier(_) => "unsupported".to_string(),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Binding {
    pub(crate) chord: KeyChord,
    pub(crate) target: BindingTarget,
}

impl Binding {
    pub(crate) const fn new(chord: KeyChord, target: BindingTarget) -> Self {
        Self { chord, target }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Keymap {
    bindings: BTreeMap<BindingContext, Vec<Binding>>,
}

impl Keymap {
    pub(crate) fn set(&mut self, context: BindingContext, binding: Binding) {
        let bindings = self.bindings.entry(context).or_default();
        if let Some(existing) = bindings
            .iter_mut()
            .find(|existing| existing.chord == binding.chord)
        {
            *existing = binding;
        } else {
            bindings.push(binding);
        }
    }

    pub(crate) fn remove_target(&mut self, context: BindingContext, target: &BindingTarget) {
        if let Some(bindings) = self.bindings.get_mut(&context) {
            bindings.retain(|binding| &binding.target != target);
        }
    }

    pub(crate) fn apply_layer(&mut self, layer: &Self) {
        for (context, binding) in layer.iter() {
            self.set(context, binding.clone());
        }
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (BindingContext, &Binding)> {
        self.bindings
            .iter()
            .flat_map(|(&context, bindings)| bindings.iter().map(move |binding| (context, binding)))
    }

    pub(crate) fn validate_targets(
        &self,
        mut validate: impl FnMut(BindingContext, &BindingTarget) -> Result<(), String>,
    ) -> Result<(), String> {
        for (context, binding) in self.iter() {
            validate(context, &binding.target).map_err(|error| {
                format!(
                    "invalid keybinding {}.{}: {error}",
                    context.as_str(),
                    binding.chord
                )
            })?;
        }
        Ok(())
    }

    pub(crate) fn resolve(
        &self,
        context: BindingContext,
        event: &KeyEvent,
    ) -> Option<&BindingTarget> {
        if event.kind == KeyEventKind::Release {
            return None;
        }
        for &candidate in fallback_contexts(context) {
            if let Some(binding) = self
                .bindings_for_context(candidate)
                .iter()
                .find(|binding| binding.chord.matches_event(event))
            {
                return Some(&binding.target);
            }
        }
        None
    }

    pub(crate) fn first_chord_for_target(
        &self,
        context: BindingContext,
        target: &BindingTarget,
    ) -> Option<KeyChord> {
        let contexts = fallback_contexts(context);
        for (index, &candidate) in contexts.iter().enumerate() {
            for binding in self.bindings_for_context(candidate) {
                let shadowed = contexts[..index].iter().any(|&more_specific| {
                    self.bindings_for_context(more_specific)
                        .iter()
                        .any(|other| other.chord == binding.chord)
                });
                if !shadowed && &binding.target == target {
                    return Some(binding.chord);
                }
            }
        }
        None
    }

    pub(crate) fn bindings_for_context(&self, context: BindingContext) -> &[Binding] {
        self.bindings
            .get(&context)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }
}

pub(crate) fn target_is_compatible(context: BindingContext, target: &BindingTarget) -> bool {
    use BindingContext::*;
    use CoreAction::*;

    let BindingTarget::Core(action) = target else {
        return matches!(target, BindingTarget::Disabled)
            || matches!(context, Navigator | Preview | Detail);
    };

    match context {
        Global => matches!(action, Cancel),
        Navigator => matches!(
            action,
            Navigate
                | Actions
                | Create
                | Cancel
                | MoveUp
                | MoveDown
                | MoveRight
                | MoveHome
                | MoveEnd
                | CopyPath
                | DeleteWorktree
                | ToggleRemotes
                | EditTags
                | CycleSort
                | ClearInput
        ),
        Preview => matches!(
            action,
            Navigate
                | Actions
                | Create
                | Cancel
                | MoveUp
                | MoveDown
                | MoveLeft
                | MoveRight
                | PageUp
                | PageDown
                | MoveHome
                | MoveEnd
                | CopyPath
                | DeleteWorktree
                | ToggleRemotes
                | EditTags
                | CycleSort
                | ClearInput
        ),
        Detail => matches!(
            action,
            Navigate
                | Actions
                | Create
                | Cancel
                | MoveUp
                | MoveDown
                | MoveLeft
                | MoveRight
                | PageUp
                | PageDown
                | MoveHome
                | MoveEnd
                | CopyPath
                | DeleteWorktree
                | ToggleRemotes
                | EditTags
                | CycleSort
        ),
        TagEditor => matches!(
            action,
            Confirm | Accept | RemoveLastTag | Cancel | CopyPath | CycleSort | ClearInput
        ),
        ActionPicker => matches!(action, Run | RunAndClose | Cancel | MoveUp | MoveDown),
        CreatePicker => matches!(action, Confirm | Cancel | MoveUp | MoveDown),
        CreateForm | CreateCompletions => matches!(
            action,
            Back | Cancel
                | Confirm
                | Accept
                | MoveUp
                | MoveDown
                | MoveLeft
                | MoveRight
                | ClearInput
        ),
        ProgressOverlay => matches!(action, Cancel),
        ErrorOverlay => matches!(action, Back | Cancel | DismissOverlay),
    }
}

fn fallback_contexts(context: BindingContext) -> &'static [BindingContext] {
    use BindingContext::*;
    match context {
        Global => &[Global],
        Navigator => &[Navigator, Global],
        Preview => &[Preview, Navigator, Global],
        Detail => &[Detail, Navigator, Global],
        TagEditor => &[TagEditor, Global],
        ActionPicker => &[ActionPicker, Global],
        CreatePicker => &[CreatePicker, Global],
        CreateForm => &[CreateForm, Global],
        CreateCompletions => &[CreateCompletions, Global],
        ProgressOverlay => &[ProgressOverlay, Global],
        ErrorOverlay => &[ErrorOverlay, Global],
    }
}

pub(crate) fn default_keymap() -> Keymap {
    use BindingContext::*;
    use CoreAction::*;

    let mut keymap = Keymap::default();
    let mut set = |context, code, modifiers, action| {
        keymap.set(
            context,
            Binding::new(KeyChord::new(code, modifiers), BindingTarget::Core(action)),
        );
    };
    let none = KeyModifiers::NONE;
    let ctrl = KeyModifiers::CONTROL;

    set(Navigator, KeyCode::Enter, none, Navigate);
    set(Navigator, KeyCode::Enter, ctrl, Actions);
    set(Navigator, KeyCode::Char(' '), ctrl, Actions);
    set(Navigator, KeyCode::Char('n'), ctrl, Create);
    set(Navigator, KeyCode::Char('y'), ctrl, CopyPath);
    set(Navigator, KeyCode::Char('d'), ctrl, DeleteWorktree);
    set(Navigator, KeyCode::Char('o'), ctrl, ToggleRemotes);
    set(Navigator, KeyCode::Char('t'), ctrl, EditTags);
    set(Navigator, KeyCode::Char('s'), ctrl, CycleSort);
    set(Navigator, KeyCode::Up, none, MoveUp);
    set(Navigator, KeyCode::Down, none, MoveDown);
    set(Navigator, KeyCode::Right, none, MoveRight);
    set(Navigator, KeyCode::Left, KeyModifiers::SUPER, MoveHome);
    set(Navigator, KeyCode::Right, KeyModifiers::SUPER, MoveEnd);
    set(Navigator, KeyCode::Char('u'), ctrl, ClearInput);
    set(Navigator, KeyCode::Esc, none, Cancel);
    set(Navigator, KeyCode::Char('c'), ctrl, Cancel);

    for context in [Preview, Detail] {
        set(context, KeyCode::Up, none, MoveUp);
        set(context, KeyCode::Down, none, MoveDown);
        set(context, KeyCode::Left, none, MoveLeft);
        set(context, KeyCode::Right, none, MoveRight);
        set(context, KeyCode::PageUp, none, PageUp);
        set(context, KeyCode::PageDown, none, PageDown);
        set(context, KeyCode::Home, none, MoveHome);
        set(context, KeyCode::End, none, MoveEnd);
    }
    set(Preview, KeyCode::Char('u'), ctrl, ClearInput);

    set(TagEditor, KeyCode::Enter, none, Confirm);
    set(TagEditor, KeyCode::Tab, none, Accept);
    set(TagEditor, KeyCode::Backspace, none, RemoveLastTag);
    set(TagEditor, KeyCode::Esc, none, Cancel);
    set(TagEditor, KeyCode::Char('c'), ctrl, Cancel);
    set(TagEditor, KeyCode::Char('y'), ctrl, CopyPath);
    set(TagEditor, KeyCode::Char('s'), ctrl, CycleSort);

    set(ActionPicker, KeyCode::Enter, none, Run);
    set(ActionPicker, KeyCode::Enter, ctrl, RunAndClose);
    set(ActionPicker, KeyCode::Char(' '), ctrl, RunAndClose);
    set(ActionPicker, KeyCode::Esc, none, Cancel);
    set(ActionPicker, KeyCode::Char('c'), ctrl, Cancel);
    set(ActionPicker, KeyCode::Up, none, MoveUp);
    set(ActionPicker, KeyCode::Char('k'), none, MoveUp);
    set(ActionPicker, KeyCode::Down, none, MoveDown);
    set(ActionPicker, KeyCode::Char('j'), none, MoveDown);

    set(CreatePicker, KeyCode::Enter, none, Confirm);
    set(CreatePicker, KeyCode::Esc, none, Cancel);
    set(CreatePicker, KeyCode::Char('c'), ctrl, Cancel);
    set(CreatePicker, KeyCode::Up, none, MoveUp);
    set(CreatePicker, KeyCode::Char('k'), none, MoveUp);
    set(CreatePicker, KeyCode::Down, none, MoveDown);
    set(CreatePicker, KeyCode::Char('j'), none, MoveDown);

    for context in [CreateForm, CreateCompletions] {
        set(context, KeyCode::Esc, none, Back);
        set(context, KeyCode::Char('c'), ctrl, Cancel);
        set(context, KeyCode::Enter, none, Confirm);
        set(context, KeyCode::Tab, none, Accept);
        set(context, KeyCode::Left, none, MoveLeft);
        set(context, KeyCode::Right, none, MoveRight);
        set(context, KeyCode::Up, none, MoveUp);
        set(context, KeyCode::Down, none, MoveDown);
        set(context, KeyCode::Char('u'), ctrl, ClearInput);
    }
    set(CreateCompletions, KeyCode::Char('k'), none, MoveUp);
    set(CreateCompletions, KeyCode::Char('j'), none, MoveDown);

    set(ProgressOverlay, KeyCode::Char('c'), ctrl, Cancel);

    set(ErrorOverlay, KeyCode::Esc, none, Back);
    set(ErrorOverlay, KeyCode::Char('c'), ctrl, Cancel);
    set(ErrorOverlay, KeyCode::Enter, none, DismissOverlay);

    keymap.set(
        ProgressOverlay,
        Binding::new(KeyChord::new(KeyCode::Esc, none), BindingTarget::Disabled),
    );

    keymap
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeSet, HashSet};

    fn core(action: CoreAction) -> BindingTarget {
        BindingTarget::Core(action)
    }

    fn event(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn contexts_and_core_actions_round_trip() {
        assert_eq!(BindingContext::ordered().len(), 11);
        for context in BindingContext::ordered() {
            assert_eq!(BindingContext::parse(context.as_str()), Some(*context));
        }

        let actions = [
            CoreAction::Navigate,
            CoreAction::Actions,
            CoreAction::Create,
            CoreAction::Run,
            CoreAction::RunAndClose,
            CoreAction::Cancel,
            CoreAction::Back,
            CoreAction::Confirm,
            CoreAction::Accept,
            CoreAction::MoveUp,
            CoreAction::MoveDown,
            CoreAction::MoveLeft,
            CoreAction::MoveRight,
            CoreAction::PageUp,
            CoreAction::PageDown,
            CoreAction::MoveHome,
            CoreAction::MoveEnd,
            CoreAction::CopyPath,
            CoreAction::DeleteWorktree,
            CoreAction::ToggleRemotes,
            CoreAction::EditTags,
            CoreAction::CycleSort,
            CoreAction::ClearInput,
            CoreAction::RemoveLastTag,
            CoreAction::DismissOverlay,
        ];
        for action in actions {
            assert_eq!(CoreAction::parse(action.as_str()), Some(action));
        }
    }

    #[test]
    fn parses_and_formats_named_keys_and_function_keys() {
        let values = [
            "enter",
            "space",
            "tab",
            "esc",
            "backspace",
            "delete",
            "insert",
            "left",
            "right",
            "up",
            "down",
            "home",
            "end",
            "pageup",
            "pagedown",
            "null",
            "caps-lock",
            "scroll-lock",
            "num-lock",
            "print-screen",
            "pause",
            "menu",
            "keypad-begin",
            "f1",
            "f35",
        ];
        for value in values {
            let chord = KeyChord::parse(value).unwrap();
            assert_eq!(KeyChord::parse(&chord.as_str()).unwrap(), chord);
        }
        assert!(KeyChord::parse("f0").is_err());
        assert!(KeyChord::parse("f36").is_err());
    }

    #[test]
    fn punctuation_aliases_are_canonical_and_round_trip() {
        let aliases = [
            ("plus", '+'),
            ("equals", '='),
            ("colon", ':'),
            ("semicolon", ';'),
            ("comma", ','),
            ("period", '.'),
            ("minus", '-'),
            ("slash", '/'),
            ("backslash", '\\'),
            ("quote", '\''),
            ("backtick", '`'),
            ("left-bracket", '['),
            ("right-bracket", ']'),
        ];
        for (alias, character) in aliases {
            let chord = KeyChord::parse(alias).unwrap();
            assert_eq!(chord.code, KeyCode::Char(character));
            assert_eq!(chord.as_str(), alias);
            assert_eq!(KeyChord::parse(&chord.as_str()).unwrap(), chord);
        }
        assert_eq!(KeyChord::parse("+").unwrap().as_str(), "plus");
    }

    #[test]
    fn modifiers_aliases_and_uppercase_are_normalized() {
        let chord = KeyChord::parse("control+option+shift+command+A").unwrap();
        assert_eq!(chord.code, KeyCode::Char('a'));
        assert_eq!(
            chord.modifiers,
            KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SHIFT | KeyModifiers::SUPER
        );
        assert_eq!(chord.as_str(), "ctrl+alt+shift+super+a");
        assert_eq!(KeyChord::parse(&chord.as_str()).unwrap(), chord);

        assert_eq!(
            KeyChord::new(KeyCode::Char('Z'), KeyModifiers::CONTROL),
            KeyChord::parse("ctrl-shift-z").unwrap()
        );
        assert_eq!(KeyChord::parse("ctrl-A").unwrap().as_str(), "ctrl+shift+a");
        assert!(KeyChord::parse("ctrl-control-a").is_err());
    }

    #[test]
    fn backtab_is_shift_tab() {
        let backtab = KeyChord::parse("backtab").unwrap();
        assert_eq!(backtab, KeyChord::parse("shift-tab").unwrap());
        assert_eq!(backtab.code, KeyCode::Tab);
        assert_eq!(backtab.modifiers, KeyModifiers::SHIFT);
        assert_eq!(backtab.as_str(), "shift+tab");
    }

    #[test]
    fn chords_support_hashing_and_total_ordering() {
        let chords = [
            KeyChord::parse("a").unwrap(),
            KeyChord::parse("ctrl-a").unwrap(),
        ];
        assert_eq!(HashSet::from(chords).len(), 2);
        assert_eq!(BTreeSet::from(chords).len(), 2);
    }

    #[test]
    fn release_is_ignored_and_repeat_is_allowed_with_exact_modifiers() {
        let keymap = default_keymap();
        let release =
            KeyEvent::new_with_kind(KeyCode::Down, KeyModifiers::NONE, KeyEventKind::Release);
        let repeat =
            KeyEvent::new_with_kind(KeyCode::Down, KeyModifiers::NONE, KeyEventKind::Repeat);
        assert_eq!(keymap.resolve(BindingContext::Navigator, &release), None);
        assert_eq!(
            keymap.resolve(BindingContext::Navigator, &repeat),
            Some(&core(CoreAction::MoveDown))
        );
        assert_eq!(
            keymap.resolve(
                BindingContext::Navigator,
                &event(KeyCode::Down, KeyModifiers::SHIFT)
            ),
            None
        );
    }

    #[test]
    fn set_replaces_in_place_without_changing_hint_priority() {
        let mut keymap = Keymap::default();
        let a = KeyChord::parse("a").unwrap();
        let b = KeyChord::parse("b").unwrap();
        keymap.set(
            BindingContext::Global,
            Binding::new(a, core(CoreAction::Navigate)),
        );
        keymap.set(
            BindingContext::Global,
            Binding::new(b, core(CoreAction::Cancel)),
        );
        keymap.set(
            BindingContext::Global,
            Binding::new(a, core(CoreAction::Confirm)),
        );

        let bindings = keymap.bindings_for_context(BindingContext::Global);
        assert_eq!(bindings.len(), 2);
        assert_eq!(bindings[0], Binding::new(a, core(CoreAction::Confirm)));
        assert_eq!(bindings[1].chord, b);
        assert_eq!(
            keymap.first_chord_for_target(BindingContext::Global, &core(CoreAction::Confirm)),
            Some(a)
        );
    }

    #[test]
    fn contexts_use_only_their_documented_fallbacks() {
        let mut keymap = Keymap::default();
        let chord = KeyChord::parse("x").unwrap();
        keymap.set(
            BindingContext::Global,
            Binding::new(chord, core(CoreAction::Cancel)),
        );
        keymap.set(
            BindingContext::Navigator,
            Binding::new(chord, core(CoreAction::Navigate)),
        );

        let key = event(KeyCode::Char('x'), KeyModifiers::NONE);
        assert_eq!(
            keymap.resolve(BindingContext::Preview, &key),
            Some(&core(CoreAction::Navigate))
        );
        assert_eq!(
            keymap.resolve(BindingContext::Detail, &key),
            Some(&core(CoreAction::Navigate))
        );
        assert_eq!(
            keymap.resolve(BindingContext::TagEditor, &key),
            Some(&core(CoreAction::Cancel))
        );
        assert_eq!(
            keymap.resolve(BindingContext::ActionPicker, &key),
            Some(&core(CoreAction::Cancel))
        );
        assert_eq!(
            keymap.first_chord_for_target(BindingContext::Preview, &core(CoreAction::Navigate)),
            Some(chord)
        );
        assert_eq!(
            keymap.first_chord_for_target(BindingContext::Preview, &core(CoreAction::Cancel)),
            None
        );
    }

    #[test]
    fn disabled_binding_consumes_key_and_stops_fallback() {
        let mut keymap = Keymap::default();
        let chord = KeyChord::parse("x").unwrap();
        keymap.set(
            BindingContext::Global,
            Binding::new(chord, core(CoreAction::Cancel)),
        );
        keymap.set(
            BindingContext::Preview,
            Binding::new(chord, BindingTarget::Disabled),
        );

        assert_eq!(
            keymap.resolve(
                BindingContext::Preview,
                &event(KeyCode::Char('x'), KeyModifiers::NONE)
            ),
            Some(&BindingTarget::Disabled)
        );
        assert_eq!(
            keymap.first_chord_for_target(BindingContext::Preview, &core(CoreAction::Cancel)),
            None
        );
    }

    #[test]
    fn layers_replace_chords_and_remove_targets_in_only_one_context() {
        let chord = KeyChord::parse("x").unwrap();
        let actions = core(CoreAction::Actions);
        let mut keymap = Keymap::default();
        keymap.set(
            BindingContext::Navigator,
            Binding::new(chord, actions.clone()),
        );
        keymap.set(
            BindingContext::ActionPicker,
            Binding::new(chord, actions.clone()),
        );
        let mut layer = Keymap::default();
        layer.set(
            BindingContext::Navigator,
            Binding::new(chord, BindingTarget::Disabled),
        );

        keymap.apply_layer(&layer);
        keymap.remove_target(BindingContext::ActionPicker, &actions);

        assert_eq!(
            keymap.bindings_for_context(BindingContext::Navigator),
            &[Binding::new(chord, BindingTarget::Disabled)]
        );
        assert!(keymap
            .bindings_for_context(BindingContext::ActionPicker)
            .is_empty());
    }

    #[test]
    fn every_default_target_is_context_compatible() {
        default_keymap()
            .validate_targets(|context, target| {
                target_is_compatible(context, target)
                    .then_some(())
                    .ok_or_else(|| format!("{} is incompatible", target.as_str()))
            })
            .unwrap();
    }

    #[test]
    fn defaults_cover_primary_application_behavior() {
        let keymap = default_keymap();
        let cases = [
            (BindingContext::Navigator, "enter", CoreAction::Navigate),
            (BindingContext::Navigator, "ctrl-enter", CoreAction::Actions),
            (BindingContext::Navigator, "ctrl-n", CoreAction::Create),
            (BindingContext::Navigator, "ctrl-y", CoreAction::CopyPath),
            (BindingContext::Preview, "pageup", CoreAction::PageUp),
            (BindingContext::Detail, "end", CoreAction::MoveEnd),
            (BindingContext::TagEditor, "tab", CoreAction::Accept),
            (
                BindingContext::TagEditor,
                "backspace",
                CoreAction::RemoveLastTag,
            ),
            (BindingContext::ActionPicker, "enter", CoreAction::Run),
            (
                BindingContext::ActionPicker,
                "ctrl-space",
                CoreAction::RunAndClose,
            ),
            (BindingContext::CreatePicker, "k", CoreAction::MoveUp),
            (BindingContext::CreateForm, "esc", CoreAction::Back),
            (BindingContext::CreateCompletions, "j", CoreAction::MoveDown),
            (
                BindingContext::ProgressOverlay,
                "ctrl-c",
                CoreAction::Cancel,
            ),
            (
                BindingContext::ErrorOverlay,
                "enter",
                CoreAction::DismissOverlay,
            ),
        ];

        for (context, chord, action) in cases {
            let chord = KeyChord::parse(chord).unwrap();
            assert_eq!(
                keymap.resolve(context, &event(chord.code, chord.modifiers)),
                Some(&core(action)),
                "{context:?} {chord}"
            );
        }
        assert_eq!(
            keymap.resolve(
                BindingContext::Navigator,
                &event(KeyCode::Char('u'), KeyModifiers::CONTROL)
            ),
            Some(&core(CoreAction::ClearInput))
        );
    }

    #[test]
    fn binding_targets_validate_identifiers() {
        assert_eq!(
            BindingTarget::parse("navigate").unwrap(),
            core(CoreAction::Navigate)
        );
        assert_eq!(
            BindingTarget::parse("custom-action-2").unwrap(),
            BindingTarget::Configured("custom-action-2".to_string())
        );
        let disabled = BindingTarget::parse("none").unwrap();
        assert_eq!(disabled, BindingTarget::Disabled);
        assert_eq!(disabled.as_str(), "none");

        for invalid in [
            "",
            "-action",
            "action-",
            "two--dashes",
            "Upper",
            "a_b",
            "a b",
        ] {
            assert!(!is_valid_action_id(invalid), "{invalid}");
            assert!(BindingTarget::parse(invalid).is_err(), "{invalid}");
        }
    }
}
