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
- Theme settings are replaced by the latest config file that contains `[ui].theme`.
- Sort settings are replaced field by field by later config files.
- Remote settings are replaced field by field by later config files.
- Preview settings are replaced field by field by later config files.
- Invalid TOML or invalid typed values fail config loading with a user-visible error.
- Unknown config keys are ignored by deserialization unless the schema validator is used externally.

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
- Written defaults must set `actions.defaults = false`.
- Written defaults must write every built-in action as an explicit `[[actions.items]]` entry.
- Written default actions must be behaviorally equivalent to runtime action defaults.
- Written default actions are intended to be edited in place by users.
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

- `[actions].defaults` controls whether built-in actions are included before listed actions.
- `[actions].defaults` defaults to true when omitted.
- `defaults = true` prepends the built-in action list.
- `defaults = false` uses only valid listed actions.
- If `defaults = false` and every listed action is invalid, navgator falls back to built-in actions.
- Empty action labels are ignored.
- Command actions require a non-empty `command`.
- Open URL actions require a non-empty `url`.
- Navigate actions do not require additional fields.
- Action item order is preserved.
- Built-in actions must remain available through runtime defaults.
- Built-in action commands must remain available through starter config generation.
- `{path}` expands to the resolved selected path when an action runs.
- `{github_url}` expands to the selected repository GitHub URL when available.
- Actions requiring an unavailable placeholder do not run.

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

## Documentation

- README must mention `navgator config-schema`.
- README must document the zsh wrapper relationship with `GATOR_OUTPUT`.
- README must document `Ctrl+Enter` actions.
- README must document the difference between runtime defaults and written defaults.
- README must document `actions.defaults`.
- README must document `{path}` and `{github_url}` placeholders.
- Detailed config behavior belongs in this spec.
- Feature-specific action behavior belongs in the action picker spec.

## Verification

- Config behavior changes must regenerate `config-schema.json` when schema-bearing structs change.
- Config behavior changes must run `cargo fmt -- --check`.
- Config behavior changes must run `cargo clippy --all-targets --all-features -- -D warnings`.
- Config behavior changes must run `cargo test`.
