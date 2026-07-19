# Action Bindings Spec

Status: Implemented.

## Decision

- Navgator does not have a separate default-action setting.
- Keys bind directly to semantic actions within a UI context.
- Plain Enter is bound to `navigate` in the default navigator keymap.
- A launcher can override that one binding and bind Enter to `actions`.
- Action definitions describe behavior; key bindings describe how behavior is invoked.

## Goals

- Make current keyboard behavior explicit as a default keymap.
- Allow any supported single key chord to invoke a bindable action.
- Allow configured command and open URL actions to be invoked directly by key.
- Allow one invocation to override config entries without using a separate config file.
- Keep bindings context-specific so one key can safely mean different things in different UI states.
- Preserve documented behavior when no keybinding configuration or CLI override is present.
- Support an ephemeral launcher without a separate `launch` command or shell pipeline.

## Non-Goals

- This change does not add key sequences or prefix maps.
- This change does not add macros or multiple actions per binding.
- This change does not bind arbitrary shell text independently of configured actions.
- This change does not add keymap profiles.
- This change does not change action placeholder or command execution semantics.
- This change does not make action labels stable identifiers.

## Terminology

- A key chord is one key with zero or more modifiers.
- A binding maps one key chord to one action identifier in one context.
- A context is the active navigator surface, focus, picker, form, editor, or overlay.
- A core action is behavior provided by navgator, such as `navigate`, `actions`, or `create`.
- A configured action is an item from the effective `[actions].items` list.
- The effective action list is the built-in and configured action list after `include_defaults` and fallback behavior are applied.

## Action Registry

- Every bindable action has a stable, non-empty identifier.
- Core action identifiers are reserved and always provided by navgator.
- Configured action identifiers come from `actions.items.id`.
- Action identifiers use lowercase ASCII letters, numbers, and single dashes between words.
- Action identifiers cannot begin or end with a dash.
- Duplicate configured action identifiers are startup errors.
- Configured action identifiers cannot use a reserved core action identifier.
- Unknown binding targets are startup errors.
- Errors identify the invalid, duplicate, reserved, or unknown identifier.

Required core action identifiers include:

| Identifier | Behavior |
| --- | --- |
| `navigate` | Return the active target path |
| `actions` | Open the action picker for the active target |
| `create` | Open the create picker |
| `run` | Run the highlighted picker action |
| `run-and-close` | Run the highlighted picker action and request parent-session closure |
| `cancel` | Close the active modal or exit the main navigator |
| `move-up` | Move the active selection up |
| `move-down` | Move the active selection down |
| `copy-path` | Copy the active target path |
| `delete-worktree` | Start deletion for the selected worktree |
| `toggle-remotes` | Toggle remote branch results |
| `edit-tags` | Enter tag editing |
| `cycle-sort` | Select the next sort mode |
| `none` | Binding tombstone that disables the chord in that context |

- The implementation may define additional core actions for existing non-text keyboard behavior.
- All current non-text commands must have stable identifiers before their hardcoded key checks are made configurable.
- Context-specific actions such as `run` are rejected in contexts where they have no meaning.

## Configured Action Identifiers

- `id` is optional for an action that is only shown in the action picker.
- `id` is required before an action can be used as a binding target.
- Existing action items without `id` remain valid and picker-visible.
- Built-in picker actions have stable identifiers.
- Built-in identifiers are written into the starter config.
- The identifier is not shown in the action picker.
- Labels remain user-facing text and may change without changing bindings.
- Optional `[actions].picker` uses identifiers to choose and order picker-visible actions.
- Configured actions omitted from the picker allowlist remain valid binding targets.

```toml
[[actions.items]]
id = "open-vs-code"
label = "Open VS Code"
type = "command"
command = "open"
args = ["-a", "Visual Studio Code", "{path}"]
```

## Contexts

The initial configurable contexts are:

| Context | Scope |
| --- | --- |
| `global` | Commands valid regardless of focus when no modal consumes the event |
| `navigator` | Main result list and search input |
| `preview` | Preview focus |
| `detail` | Detail focus |
| `action-picker` | Action picker modal |
| `create-picker` | Create recipe picker |
| `create-form` | Create prompt fields |
| `create-completions` | Focused create path completions |
| `tag-editor` | Tag editing |
| `progress-overlay` | Active worktree or create operation |
| `error-overlay` | Failed worktree, create, or direct action operation |

- Modal contexts do not fall through to navigator bindings.
- `preview` and `detail` fall back to `navigator` when they do not bind a chord.
- Non-modal contexts fall back to `global` when they do not bind a chord.
- Modal contexts may fall back only to valid `global` bindings such as application cancellation.
- The most specific active context wins.
- Text input receives an unmatched printable key only after keybinding resolution.
- Binding a printable key in a text-accepting context prevents that key from being inserted.

## Configuration

- Key bindings are configured under context tables in `[keybindings]`.
- Each TOML key is a key chord.
- Each TOML value is one action identifier.
- User bindings replace default bindings for the same canonical context and chord.
- A `none` value disables a default binding without removing other defaults.
- Multiple chords may target the same action.

```toml
[keybindings.navigator]
enter = "navigate"
"ctrl+enter" = "actions"
"ctrl+space" = "actions"
"ctrl+n" = "create"
"ctrl+y" = "copy-path"
"ctrl+d" = "delete-worktree"
"ctrl+o" = "toggle-remotes"
"ctrl+t" = "edit-tags"
"ctrl+s" = "cycle-sort"

[keybindings.action-picker]
enter = "run"
"ctrl+enter" = "run-and-close"
"ctrl+space" = "run-and-close"
esc = "cancel"
"ctrl+c" = "cancel"
up = "move-up"
down = "move-down"
j = "move-down"
k = "move-up"
```

A configured action can be invoked directly:

```toml
[keybindings.navigator]
"ctrl+v" = "open-vs-code"
```

## Default Keymap

- Runtime defaults reproduce all current keyboard behavior.
- Plain Enter in `navigator` is bound to `navigate`.
- Ctrl+Enter and Ctrl+Space in `navigator` are bound to `actions`.
- Enter in `action-picker` is bound to `run`.
- Ctrl+Enter and Ctrl+Space in `action-picker` are bound to `run-and-close`.
- Ctrl+N in `navigator` is bound to `create`.
- Existing movement, focus, editing, modal, and cancellation keys retain their behavior through default bindings or text-input fallback.
- Missing `[keybindings]` configuration uses the complete default keymap.
- A partial `[keybindings]` configuration overrides only the listed canonical chords.
- The starter config writes the complete editable default keymap.

## Invocation Overrides

- `--config-entry <toml>` applies one TOML config fragment after discovered config files.
- `--config-entry=<toml>` is equivalent.
- The option may be repeated before or after an interactive command.
- Entries are applied in argument order, and a later entry replaces an earlier scalar or keybinding entry.
- Each entry uses normal config merge behavior as a highest-precedence synthetic config layer.
- A config entry containing only `actions.picker` is a field-level override and preserves file-defined action items.
- Modern `[keybindings]` entries always override deprecated `[actions].bindings` and `[create].bindings`, regardless of source order.
- A keybinding override uses a dotted TOML assignment such as `keybindings.navigator.enter="actions"`.
- `none` disables a chord for the invocation.
- Invalid TOML or typed values fail startup before the TUI begins and identify the failing entry.
- Unknown or context-incompatible action identifiers fail after the effective action list is loaded.
- The option is accepted by interactive `navigate`, `actions`, and `create` invocations and their aliases.
- The option is rejected by `config-schema`, `schema`, and help-only invocations.

Launcher override:

```console
navgator --config-entry 'keybindings.navigator.enter="actions"'
```

Direct VS Code override:

```console
navgator --config-entry 'keybindings.navigator.enter="open-vs-code"'
```

## Key Chord Syntax

- Canonical modifiers are `ctrl`, `alt`, `shift`, and `super`.
- Modifier order is normalized to `ctrl+alt+shift+super`.
- `control` is accepted as an alias for `ctrl`.
- `option` is accepted as an alias for `alt`.
- `cmd` and `command` are accepted as aliases for `super`.
- Named keys include `enter`, `space`, `tab`, `backtab`, `esc`, `backspace`, `delete`, `insert`, arrows, `home`, `end`, `pageup`, `pagedown`, and supported function keys.
- A single Unicode scalar value represents a character key.
- Key and modifier names are ASCII case-insensitive.
- Empty chords, unknown names, duplicate modifiers, and multiple non-modifier keys are errors.
- Matching uses the complete canonical modifier set rather than modifier containment.
- Shifted character events are normalized consistently between parsing, matching, and display.
- Canonical display labels are generated from parsed chords rather than stored separately.
- Alias spellings that resolve to the same context and canonical chord are duplicate-binding errors within one config layer.

## Terminal Keyboard Support

- Navgator can bind only key events reported distinctly by the active terminal.
- Legacy terminal input may not distinguish Enter from Ctrl+M or Tab from Ctrl+I.
- Super, standalone modifier, and some modified special-key events require an enhanced keyboard protocol.
- Ghostty supports the enhanced keyboard protocol.
- Navgator ignores key release events and accepts press and repeat events.
- Navgator uses the keyboard events reported by the terminal and Crossterm without requiring enhanced reporting.
- Terminal protocol limitations are documented and are not treated as config parsing errors.

## Navigator Action Behavior

- `navigate`, `actions`, and configured target actions resolve the active target with existing focus rules.
- Search focus uses the selected entry path.
- Preview and detail focus use the active preview tab path, then fall back to the selected entry path.
- Project entries use the selectable worktree path when one exists.
- Worktree entries use their worktree path.
- If no target resolves, the action does nothing and writes no output.
- `navigate` writes the resolved path through existing navigation output and does not request parent-session closure.
- `actions` opens the action picker with an empty query for the resolved target.
- A directly bound configured action uses its existing condition, placeholder, command, URL, and output behavior.
- An unmet condition or unavailable placeholder prevents a directly bound action from running, keeps navgator open, and shows a clear error.
- Successful command and open URL actions exit navgator without writing a selected path.
- Directly bound actions never emit the close-session marker.

## Remote Branch Continuations

- A target-dependent action bound in `navigator`, `preview`, or `detail` first applies `[branches].on_select` when the selected entry is a remote branch.
- `navigate` continues to the resulting checkout or worktree path.
- `actions` opens the picker for the resulting path.
- A configured target action executes against the resulting path.
- Branch preparation stores one explicit pending action continuation.
- A successful branch result consumes the continuation exactly once.
- A failed branch result clears the continuation and keeps the existing error overlay.
- Repeated input while preparation runs cannot execute or enqueue the continuation again.
- This replaces the current restriction that the action picker must not materialize remote branches.

## Action Picker Behavior

- Opening the picker through any `actions` binding preserves the selected navigator result and query.
- The picker starts with an empty query and its first visible action selected.
- If no action is visible, the picker shows its existing empty state with no selection.
- `run` executes the highlighted action without requesting parent-session closure.
- `run-and-close` executes the highlighted action and requests parent-session closure after success.
- `cancel` returns to the unchanged navigator state.
- Action-picker bindings do not inherit navigator bindings.
- A binding to `actions` is rejected in `action-picker` to prevent recursive picker opening.

## Ephemeral Launcher

- An external launcher invokes navgator directly rather than through the Zsh widget.
- The launcher overrides navigator Enter with `actions`.
- Selecting a project, worktree, or remote branch with Enter opens its action picker.
- Enter in the picker runs the highlighted action.
- A GUI action exits navgator after its launcher command returns.
- A terminal configured to close when its child exits then closes naturally.
- Escape in the picker returns to the navigator.
- Escape in the navigator exits without running an action.

Example Ghostty invocation:

```console
open -na Ghostty.app --args -e /opt/homebrew/bin/navgator --config-entry 'keybindings.navigator.enter="actions"'
```

## Output Protocol

- The Zsh wrapper continues to invoke `navgator navigate` without config-entry overrides.
- Default Enter therefore preserves navigation and directory changes.
- Navigate actions write a selected path.
- Command and open URL actions write no selected path.
- Only `run-and-close` requests parent-session closure.
- Direct launcher invocations do not require `GATOR_OUTPUT` or the Zsh wrapper.

## Existing Binding Configuration

- Existing `[actions].bindings` remains supported because it is shipped public configuration.
- Existing action binding values retain their currently supported syntax and meaning.
- Legacy bindings use the new exact-modifier matching rules; additional modifiers no longer match a legacy chord implicitly.
- Each existing action binding is translated to `actions` in `navigator` and `run-and-close` in `action-picker`.
- Existing `[create].bindings` remains supported and translates to `create` in `navigator`.
- New `[keybindings]` entries override translated legacy bindings for the same canonical context and chord.
- CLI config-entry keybindings override file keybindings and translated legacy configuration.
- README marks the old binding fields as deprecated after the new keymap lands.
- Starter config generation writes `[keybindings]` and stops writing the old binding fields.
- Removing the old fields requires a separately documented breaking release.

## Merge And Validation

- Later config files merge keybindings by canonical context and chord.
- Later bindings replace earlier bindings for the same canonical context and chord.
- Action item merge behavior remains unchanged.
- Action identifiers are validated after the effective action list is built.
- Keybinding targets are validated after action identifiers and core actions are registered.
- Invalid bindings fail startup before the TUI begins.
- Navgator never silently restores a default for an explicitly invalid binding.

## UI And Documentation

- Help text is generated from the resolved keymap rather than hardcoded key labels.
- Help text shows the first resolved binding for actions that currently expose one hint.
- Help text reflects CLI overrides.
- Key labels use canonical display formatting.
- README documents action identifiers, contexts, key syntax, `none`, and `--config-entry`.
- README includes the ephemeral Ghostty launcher example.
- Config behavior documentation describes binding merge, validation, and compatibility behavior.
- Action picker documentation describes context-specific `run` and `run-and-close` bindings.
- Remote branch documentation describes pending action continuations.
- `config-schema.json` includes action IDs and all keybinding context tables.

## Implementation Constraints

- Key parsing, canonical display, and event matching live in one reusable module.
- Key handling resolves a semantic action before dispatching behavior.
- Action execution is shared between picker and directly bound configured actions.
- Branch preparation carries an explicit action continuation rather than synthesizing input.
- No new navgator dependency is required.

## Verification

- Tests cover key parsing, aliases, canonical formatting, and exact modifier matching.
- Tests cover context precedence, fallback, `none`, and text-input fallback.
- Tests prove the complete default keymap preserves current behavior.
- Tests cover partial config merge and CLI precedence.
- Tests cover invalid syntax, unknown actions, duplicate IDs, reserved IDs, and context incompatibility.
- Tests cover directly bound navigate, command, and open URL actions.
- Tests cover action-picker `run`, `run-and-close`, cancel, and empty state.
- Tests cover target dispatch against a prepared branch path and exactly-once pending action consumption.
- Tests cover output paths and close-session markers.
- Tests cover key press, repeat, and release event behavior.
- Config changes regenerate `config-schema.json`.
- Implementation verification runs `cargo fmt -- --check`.
- Implementation verification runs `cargo clippy --all-targets --all-features -- -D warnings`.
- Implementation verification runs `cargo test`.
- Implementation verification runs `cargo build --release`.
