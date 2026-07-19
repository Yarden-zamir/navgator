# Config Behavior Spec

## Goals

- Runtime defaults must keep navgator usable when config files are missing, empty, or partial.
- Written defaults must be explicit and editable when navgator creates a starter config file.
- Config discovery must be deterministic and easy to explain.
- Schema generation must be side-effect free.
- Existing config files should be respected rather than overwritten.

## Commands

- `navgator config-schema` prints the JSON schema to stdout.
- `navgator schema` is an alias for `navgator config-schema`.
- Schema commands do not read user config files.
- Schema commands do not create config files.
- Schema commands do not insert `$schema` into config files.
- `navgator navigate` loads config before building the result list.
- `navgator` without arguments behaves like `navgator navigate`.
- `navgator actions` loads config and opens directly to the action picker for the first result.
- `navgator actions <path>` loads config and opens directly to the action picker for the provided path.
- Interactive commands accept repeatable `--config-entry <toml>` and `--config-entry=<toml>` options.
- Config entries are parsed as TOML fragments and applied in argument order after discovered config files.
- A CLI config entry containing only `actions.picker` preserves effective action definitions and overrides picker visibility for that invocation.
- Schema and help commands reject `--config-entry`.

## Discovery

- `NAVGATOR_CONFIG` is a single-file override when set to a non-empty value.
- When `NAVGATOR_CONFIG` is set, no other config locations are loaded.
- When `NAVGATOR_CONFIG` is set and the file is missing, navgator creates the starter config at that exact path.
- When `NAVGATOR_CONFIG` is unset or empty, navgator checks the standard config locations.
- Standard config locations are checked in this order: `/etc/navgator/config.toml`.
- Standard config locations then check `$XDG_CONFIG_HOME/navgator/config.toml`.
- If `XDG_CONFIG_HOME` is unset or empty, the XDG path is `~/.config/navgator/config.toml`.
- Standard config locations then check `~/.config/navgator/config.toml`.
- Standard config locations then check `~/.navgator.toml`.
- Standard config locations then check `.navgator.toml` in the current working directory.
- Standard config locations then check `.navgator/config.toml` in the current working directory.
- Duplicate discovered paths are removed while preserving the first occurrence.
- When no discovered config file exists, navgator creates the starter config at the default user config path.
- The default user config path is `$XDG_CONFIG_HOME/navgator/config.toml` when `XDG_CONFIG_HOME` is set and non-empty.
- The default user config path is `~/.config/navgator/config.toml` otherwise.
- The presence of any discovered config file prevents starter config creation.
- Empty config files are valid existing config files.

## Merge Rules

- All existing standard config files are loaded in discovery order.
- Later standard config files override earlier scalar settings.
- Path lists are additive across standard config files.
- Duplicate normalized paths are ignored after their first occurrence.
- `index_folders` and `static_items` are merged independently.
- Actions are replaced by the latest config file that contains an `[actions]` table.
- Keybindings merge by canonical context and chord across config files.
- Later keybinding entries replace earlier entries for the same canonical context and chord.
- CLI config-entry keybindings have the highest precedence.
- Theme settings are replaced by the latest config file that contains `[ui].theme`.
- Sort settings are replaced field by field by later config files.
- Remote settings are replaced field by field by later config files.
- Preview settings are replaced field by field by later config files.
- Invalid TOML or invalid typed values fail config loading with a user-visible error.
- Unknown config keys are ignored by deserialization unless the schema validator is used externally, except unknown contexts inside `[keybindings]`, which fail loading.

## Runtime Defaults

- Runtime defaults are applied before reading config files.
- Missing config sections keep their runtime defaults.
- Missing fields inside present sections keep their runtime defaults.
- Default sort mode is `modified-desc`.
- Pinning the current project defaults to true.
- Remote branches default to disabled on startup.
- Remote refresh on toggle defaults to true.
- Remote cache usage defaults to true.
- UI theme defaults to `auto`.
- Worktree tab label shortening defaults to true.
- Worktree tab minimum characters defaults to 6.
- Selected worktree tab minimum characters defaults to 10.
- Runtime action defaults are the built-in action list.
- Runtime action defaults are used when no `[actions]` table is configured.
- Runtime action defaults are used when action configuration produces no valid action items.
- Runtime keybinding defaults preserve the documented navigator, picker, form, editor, and overlay commands.
- Missing or partial `[keybindings]` configuration keeps unrelated runtime keybinding defaults.
- Runtime create defaults are the built-in create recipe list.
- Runtime create defaults are used when no `[create]` table is configured.
- Runtime create defaults are used when create configuration produces no valid create items.

## Written Defaults

- Written defaults are only used when navgator creates a starter config file.
- Written defaults must include `$schema` as the first line.
- Written defaults must include `[paths]`.
- Written defaults must include editable `index_folders` and `static_items` examples.
- Written defaults must include `[sort]` with explicit runtime-equivalent values.
- Written defaults must include `[remote]` with explicit runtime-equivalent values.
- Written defaults must include `[ui]` with explicit runtime-equivalent values.
- Written defaults must include `[preview]` with explicit runtime-equivalent values.
- Written defaults must include `[actions]`.
- Written defaults must omit action default toggles because listed actions replace built-ins by default.
- Written defaults must write every built-in action as an explicit `[[actions.items]]` entry.
- Written default actions include stable `id` values.
- Written default actions must be behaviorally equivalent to runtime action defaults.
- Written defaults must include `[create]`.
- Written defaults must write every built-in create recipe as an explicit `[[create.items]]` entry.
- Written default create recipes must be behaviorally equivalent to runtime create defaults.
- Written default actions are intended to be edited in place by users.
- Written defaults include every context in `[keybindings]` with the runtime default command bindings.
- Runtime defaults and written defaults may share source data, but they are different concepts.

## Paths

- Path values may use `~/` at the beginning.
- Path values may contain `$HOME`.
- Relative path values are resolved relative to the config file that contains them.
- Missing path values are ignored.
- Empty path strings are ignored.
- Existing files and directories are accepted as static items.
- `index_folders` includes each configured folder and its direct child directories.
- Path deduplication uses the normalized absolute path string.

## Actions

- Listed `actions.items` replace built-in actions by default.
- `[actions].include_defaults = true` prepends the built-in action list before listed actions.
- When `[actions]` has no valid listed action items, navgator falls back to built-in actions.
- `[actions].bindings` is deprecated compatibility configuration for opening the action picker and running with parent-session closure.
- Legacy `[actions].bindings` defaults to `ctrl-enter` and `ctrl-space` when omitted or invalid.
- Legacy bindings are translated before `[keybindings]` and CLI config-entry overrides are applied.
- Action `id` is optional for picker-only actions and required when an action is a keybinding target.
- Action IDs must be unique lowercase ASCII words separated by single dashes and cannot use reserved core action IDs.
- Optional `[actions].picker` is an ordered allowlist of action IDs shown in the action picker.
- Omitting `[actions].picker` shows every effective action.
- An empty `[actions].picker` shows the picker empty state.
- Unknown or duplicate picker IDs fail config loading.
- Actions omitted from `[actions].picker` remain available as direct keybinding targets.
- Picker allowlist order takes precedence over action item order.
- Empty action labels are ignored.
- Action icons are optional.
- Action icons may use Nerd Font glyphs or emoji.
- Action file_condition values are optional.
- Relative action file_condition values are checked under the selected target path.
- Absolute action file_condition values are checked as absolute paths.
- Actions with unmet file_condition are hidden from the picker.
- Command actions require a non-empty `command`.
- Open URL actions require a non-empty `url`.
- Navigate actions do not require additional fields.
- Action item order is preserved when no picker allowlist is configured.
- Built-in actions must remain available through runtime defaults.
- Built-in action commands must remain available through starter config generation.
- `{path}` expands to the resolved selected path when an action runs.
- `{github_url}` expands to the selected repository GitHub URL when available.
- Actions requiring an unavailable placeholder do not run.

## Branches

- `[branches].on_select` controls what happens when a remote branch result is selected.
- `[branches].on_select` defaults to `worktree`.
- `worktree` creates or reuses a worktree for the selected remote branch, then continues the bound target action with that path.
- `checkout` fetches/prepares the selected remote branch, checks it out in an existing worktree for the selected bare repo, then continues the bound target action with that path.
- `checkout` does not create a new working tree.
- `checkout` fails with a clear error when no existing worktree is available.
- Both branch selection modes share the same remote ref preparation behavior.

## Create

- Listed `create.items` replace built-in create recipes by default.
- `[create].include_defaults = true` prepends the built-in create recipe list before listed recipes.
- When `[create]` has no valid listed recipes, navgator falls back to built-in create recipes.
- `[create].bindings` is deprecated compatibility configuration for opening the create picker.
- Legacy `[create].bindings` defaults to `ctrl-n` when omitted or invalid.
- Create recipe labels must be non-empty.
- Create recipe shell values must be non-empty.
- Create recipe `success_path` values must be non-empty.
- Create recipes run arbitrary shell through the platform shell.
- Create recipe prompt values are exposed to shell as `NAVGATOR_CREATE_*` environment variables.
- The selected navgator target is exposed to shell as `NAVGATOR_SELECTED_PATH` when available.
- Prompt names may contain ASCII letters, numbers, underscore, or dash.
- Prompt names with dash are normalized to underscore for placeholders and environment variables.
- Prompt names must be unique per recipe after normalization; later duplicates are ignored.
- Prompt types are `text` and `path`.
- Prompt type defaults to `text`.
- Path prompts represent filesystem paths. Recipes that create a child folder should prefer a parent-folder path prompt plus a text name prompt over one ambiguous full path prompt.
- Path prompts support non-recursive filesystem autocomplete.
- `Tab` accepts the selected path suggestion.
- `Right` focuses path suggestions when available.
- `Left` returns focus to prompt fields.
- `j`, `k`, `Up`, and `Down` move through path suggestions while suggestions are focused.
- `Up` and `Down` move through prompt fields while fields are focused.
- Required prompts reject empty values.
- Prompt defaults support earlier prompt placeholders and `{path}`.
- Create recipe `current_dir` supports prompt placeholders and `{path}`.
- Create recipe `success_path` supports prompt placeholders and `{path}`.
- After shell success, navgator navigates to the expanded `success_path`.
- If the expanded `success_path` does not exist after shell success, navgator shows an error instead of navigating.

## Schema Behavior

- The schema URL is `https://raw.githubusercontent.com/Yarden-zamir/navgator/main/config-schema.json`.
- Starter configs include `$schema` with the schema URL.
- Existing config files missing `$schema` are updated after successful parsing.
- Schema insertion adds `$schema` as the first line.
- Schema insertion preserves the rest of the file content verbatim after the inserted line spacing.
- Schema insertion failures are ignored so read-only configs still load.
- Config files with an existing `$schema` are not modified.
- Schema generation must be regenerated after config structs change.
- The generated schema must include all public config sections.
- The generated schema must include all action item variants.
- The generated schema must include action IDs and every keybinding context.
- The generated schema must include branch selection behavior.
- The generated schema must include create settings and prompt types.

## Documentation

- README must mention `navgator config-schema`.
- README must document the zsh wrapper relationship with `GATOR_OUTPUT`.
- README must document action picker bindings.
- README must document keybinding contexts, action IDs, key chord syntax, `none`, and `--config-entry`.
- README must document the difference between runtime defaults and written defaults.
- README must document `actions.include_defaults`.
- README must document `{path}` and `{github_url}` placeholders.
- README must document branch selection behavior.
- README must document create recipes, prompt types, path autocomplete, environment variables, and automatic success navigation.
- Detailed config behavior belongs in this spec.
- Feature-specific action behavior belongs in the action picker spec.

## Verification

- Config behavior changes must regenerate `config-schema.json` when schema-bearing structs change.
- Config behavior changes must run `cargo fmt -- --check`.
- Config behavior changes must run `cargo clippy --all-targets --all-features -- -D warnings`.
- Config behavior changes must run `cargo test`.
