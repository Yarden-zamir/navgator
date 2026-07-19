# navgator

🐊 Rust TUI project navigator with Git worktree and preview support.

Nerd Font is recommended for gator-family CLIs so built-in icons render correctly.

## Install

Homebrew tap: https://github.com/Yarden-zamir/homebrew-tap

```sh
brew install yarden-zamir/tap/navgator
```

## Run

```sh
navgator
```

Open directly to the action picker for the first result or a specific path:

```sh
navgator actions
navgator actions ~/Github/navgator/main
```

Open directly to create flows from the current directory:

```sh
navgator create
navgator create new-project
navgator create new-branch-worktree ~/Github/navgator/main
```

Print the JSON config schema:

```sh
navgator config-schema
```

Override one or more config entries for a single invocation with valid TOML dotted assignments:

```sh
navgator --config-entry 'keybindings.navigator.enter="actions"'
navgator --config-entry 'sort.default="alpha-asc"' \
  --config-entry 'remote.enabled_by_default=true'
```

Config behavior, discovery order, runtime defaults, written defaults, and schema rules are specified in `docs/config-behavior-spec.md`.

## Zsh Widget

Choose one setup path.

Homebrew manages both the binary and wrapper:

```zsh
brew install yarden-zamir/tap/navgator
source "$(brew --prefix navgator)/share/navgator/navgator.zsh"
bindkey '^T' navigate
bindkey '^N' navgator-create-new-project
```

Alternatively, [gh-source](https://github.com/Yarden-zamir/gh-source) clones the repository, builds a missing local release binary, and sources the local wrapper:

```zsh
gh_source Yarden-zamir/navgator/scripts/navgator.zsh \
  --skip-build-if-present target/release/navgator \
  --build cargo build --release
bindkey '^T' navigate
bindkey '^N' navgator-create-new-project
```

The wrapper prefers `$NAVGATOR_BIN`, then adjacent local release and debug builds, then `navgator` on `PATH`.
The wrapper writes selections through `GATOR_OUTPUT`; otherwise `navgator` prints the selected path to stdout.

## Actions

Press `Ctrl+Enter` or `Ctrl+Space` to open the action picker. A newly generated config writes stable action IDs and the built-in actions explicitly so they can be edited or bound directly:

```toml
[actions]
picker = ["open-vs-code", "navigate-to"]

[[actions.items]]
id = "navigate-to"
label = "Navigate to"
icon = "󰁔"
type = "navigate"

[[actions.items]]
id = "open-vs-code"
label = "Open VS Code"
type = "command"
command = "open"
args = ["-a", "Visual Studio Code", "{path}"]
```

Runtime defaults still exist when no actions are configured. Listing `actions.items` replaces the built-ins; set `include_defaults = true` only when you want built-ins prepended before listed actions. Optional `picker` lists the action IDs shown in the picker and controls their display order; omit it to show every effective action. Hidden actions remain available as direct keybinding targets. Built-in actions navigate to the target, open GitHub Desktop, open VS Code, run `idea .`, open the repo online, start `claude`, and start `opencode`. Custom command and URL actions can use `{path}` and `{github_url}` placeholders. Optional `icon` text supports Nerd Font glyphs or emoji. Optional `file_condition` hides an action unless the given file or directory exists under the selected target. In the action picker, type to filter actions, `Enter` runs the action and keeps the shell session open, and `run-and-close` asks the zsh wrapper to close the shell session after success.

## Key Bindings

Bindings map a single key chord to a core action or configured action ID within one UI context:

```toml
[keybindings.navigator]
enter = "navigate"
"ctrl+enter" = "actions"
"ctrl+space" = "actions"
"ctrl+n" = "create"
"ctrl+v" = "open-vs-code"

[keybindings.action-picker]
enter = "run"
"ctrl+enter" = "run-and-close"
esc = "cancel"
```

Available contexts are `global`, `navigator`, `preview`, `detail`, `tag-editor`, `action-picker`, `create-picker`, `create-form`, `create-completions`, `progress-overlay`, and `error-overlay`. Use `none` to disable a default chord. Modifiers use `ctrl`, `alt`, `shift`, and `super`; named keys include arrows, `enter`, `space`, `tab`, `esc`, paging keys, and function keys. Existing `[actions].bindings` and `[create].bindings` remain supported but are deprecated.

Custom action IDs are optional unless the action is a binding target. IDs must be unique lowercase ASCII words separated by single dashes and cannot reuse core action names such as `navigate`, `actions`, or `create`.

For an ephemeral Ghostty project launcher, bind Enter to the action picker only for that process:

```sh
open -na Ghostty.app --args \
  --config-default-files=false \
  --font-size=25 \
  --maximize=true \
  --background-opacity=0.9 \
  --background-blur=true \
  --window-save-state=never \
  --macos-titlebar-style=hidden \
  --confirm-close-surface=false \
  -e /opt/homebrew/bin/navgator \
  --config-entry 'ui.theme="dark"' \
  --config-entry 'actions.picker=["open-vs-code","open-github-desktop","open-repo-online","open-intellij"]' \
  --config-entry 'keybindings.navigator.enter="actions"'
```

Navgator's `ui.theme = "auto"` follows the operating-system appearance, not the terminal background. Set `ui.theme` to `dark` or `light` for isolated terminal launchers whose colors may differ from the operating system.
An `actions.picker` config entry changes only picker visibility for that invocation and preserves action definitions loaded from config files.

## Branches

Remote branch selection defaults to the worktree workflow:

```toml
[branches]
on_select = "worktree"
```

Set `on_select = "checkout"` to fetch and checkout the selected remote branch in an existing worktree instead. Checkout mode does not create a working tree; it requires one to already exist for the selected bare repo.

## Create

Press `Ctrl+N` to open the create picker. Create recipes collect typed prompts, run arbitrary shell, then navigate to `success_path` after the shell succeeds:

```toml
[create]

[[create.items]]
label = "New project"
icon = "󰉋"
shell = "mkdir -p \"$NAVGATOR_CREATE_PARENT/$NAVGATOR_CREATE_NAME\""
success_path = "{parent}/{name}"

[[create.items.prompts]]
name = "name"
label = "Project name"
type = "text"
required = true

[[create.items.prompts]]
name = "parent"
label = "Parent folder"
type = "path"
default = "~/Github"
required = true
```

Prompt types are `text` and `path`. Path prompts represent filesystem paths; for new projects, use a parent-folder prompt plus the project name for clearer shell and success paths. `Up`/`Down` move between fields. Press `Right` to focus completions, `Left` to return to fields, `j`/`k` or arrows to move focused completions, and `Tab` to accept. Prompt values expand in `current_dir` and `success_path` as `{prompt_name}`; `{path}` expands to the currently selected navgator target. Shell recipes receive prompt values as environment variables like `NAVGATOR_CREATE_PARENT` and the selected target as `NAVGATOR_SELECTED_PATH`.

Terminal launch selectors match recipe labels directly or by slug, so `New project` can be launched with `navgator create new-project`. When no path is provided, create launches use the current directory as the selected path for `{path}` and `NAVGATOR_SELECTED_PATH`.

## Build

```sh
cargo build --release
```

## Check

```sh
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

## License

MIT
