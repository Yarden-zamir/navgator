use crossterm::event::{KeyCode, KeyModifiers};

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
        binding_context_as_str(self)
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        parse_binding_context(value)
    }

    pub(crate) const fn ordered() -> &'static [Self] {
        &Self::ORDERED
    }
}

const fn binding_context_as_str(context: BindingContext) -> &'static str {
    match context {
        BindingContext::Global => "global",
        BindingContext::Navigator => "navigator",
        BindingContext::Preview => "preview",
        BindingContext::Detail => "detail",
        BindingContext::TagEditor => "tag-editor",
        BindingContext::ActionPicker => "action-picker",
        BindingContext::CreatePicker => "create-picker",
        BindingContext::CreateForm => "create-form",
        BindingContext::CreateCompletions => "create-completions",
        BindingContext::ProgressOverlay => "progress-overlay",
        BindingContext::ErrorOverlay => "error-overlay",
    }
}

fn parse_binding_context(value: &str) -> Option<BindingContext> {
    match value {
        "global" => Some(BindingContext::Global),
        "navigator" => Some(BindingContext::Navigator),
        "preview" => Some(BindingContext::Preview),
        "detail" => Some(BindingContext::Detail),
        "tag-editor" => Some(BindingContext::TagEditor),
        "action-picker" => Some(BindingContext::ActionPicker),
        "create-picker" => Some(BindingContext::CreatePicker),
        "create-form" => Some(BindingContext::CreateForm),
        "create-completions" => Some(BindingContext::CreateCompletions),
        "progress-overlay" => Some(BindingContext::ProgressOverlay),
        "error-overlay" => Some(BindingContext::ErrorOverlay),
        _ => None,
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
        core_action_as_str(self)
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        parse_core_action(value)
    }
}

const fn core_action_as_str(action: CoreAction) -> &'static str {
    match action {
        CoreAction::Navigate => "navigate",
        CoreAction::Actions => "actions",
        CoreAction::Create => "create",
        CoreAction::Run => "run",
        CoreAction::RunAndClose => "run-and-close",
        CoreAction::Cancel => "cancel",
        CoreAction::Back => "back",
        CoreAction::Confirm => "confirm",
        CoreAction::Accept => "accept",
        CoreAction::MoveUp => "move-up",
        CoreAction::MoveDown => "move-down",
        CoreAction::MoveLeft => "move-left",
        CoreAction::MoveRight => "move-right",
        CoreAction::PageUp => "page-up",
        CoreAction::PageDown => "page-down",
        CoreAction::MoveHome => "move-home",
        CoreAction::MoveEnd => "move-end",
        CoreAction::CopyPath => "copy-path",
        CoreAction::DeleteWorktree => "delete-worktree",
        CoreAction::ToggleRemotes => "toggle-remotes",
        CoreAction::EditTags => "edit-tags",
        CoreAction::CycleSort => "cycle-sort",
        CoreAction::ClearInput => "clear-input",
        CoreAction::RemoveLastTag => "remove-last-tag",
        CoreAction::DismissOverlay => "dismiss-overlay",
    }
}

fn parse_core_action(value: &str) -> Option<CoreAction> {
    match value {
        "navigate" => Some(CoreAction::Navigate),
        "actions" => Some(CoreAction::Actions),
        "create" => Some(CoreAction::Create),
        "run" => Some(CoreAction::Run),
        "run-and-close" => Some(CoreAction::RunAndClose),
        "cancel" => Some(CoreAction::Cancel),
        "back" => Some(CoreAction::Back),
        "confirm" => Some(CoreAction::Confirm),
        "accept" => Some(CoreAction::Accept),
        "move-up" => Some(CoreAction::MoveUp),
        "move-down" => Some(CoreAction::MoveDown),
        "move-left" => Some(CoreAction::MoveLeft),
        "move-right" => Some(CoreAction::MoveRight),
        "page-up" => Some(CoreAction::PageUp),
        "page-down" => Some(CoreAction::PageDown),
        "move-home" => Some(CoreAction::MoveHome),
        "move-end" => Some(CoreAction::MoveEnd),
        "copy-path" => Some(CoreAction::CopyPath),
        "delete-worktree" => Some(CoreAction::DeleteWorktree),
        "toggle-remotes" => Some(CoreAction::ToggleRemotes),
        "edit-tags" => Some(CoreAction::EditTags),
        "cycle-sort" => Some(CoreAction::CycleSort),
        "clear-input" => Some(CoreAction::ClearInput),
        "remove-last-tag" => Some(CoreAction::RemoveLastTag),
        "dismiss-overlay" => Some(CoreAction::DismissOverlay),
        _ => None,
    }
}

// The keybinding engine (chord parsing, targets, keymap resolution) lives in
// gator; navgator supplies its own contexts and actions and reuses the engine.
pub(crate) use gator::keymap::{is_valid_action_id, KeyChord};

pub(crate) type BindingTarget = gator::keymap::BindingTarget<CoreAction>;
pub(crate) type Binding = gator::keymap::Binding<CoreAction>;
pub(crate) type Keymap = gator::keymap::Keymap<BindingContext, CoreAction>;

impl gator::keymap::BindingContext for BindingContext {
    fn as_str(self) -> &'static str {
        binding_context_as_str(self)
    }

    fn parse(value: &str) -> Option<Self> {
        parse_binding_context(value)
    }

    fn ordered() -> &'static [Self] {
        &Self::ORDERED
    }

    fn fallback_contexts(self) -> &'static [Self] {
        fallback_contexts(self)
    }
}

impl gator::keymap::CoreAction for CoreAction {
    fn as_str(self) -> &'static str {
        core_action_as_str(self)
    }

    fn parse(value: &str) -> Option<Self> {
        parse_core_action(value)
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
    use crossterm::event::{KeyEvent, KeyEventKind};
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
