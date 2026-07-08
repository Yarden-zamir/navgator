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

Print the JSON config schema:

```sh
navgator config-schema
```

Config behavior, discovery order, runtime defaults, written defaults, and schema rules are specified in `docs/config-behavior-spec.md`.

## Zsh Widget

```sh
source "$(brew --prefix)/share/navgator/navgator.zsh"
bindkey '^T' navigate
```

The wrapper writes selections through `GATOR_OUTPUT`; otherwise `navgator` prints the selected path to stdout.

## Actions

Press `Ctrl+Enter` or `Ctrl+Space` to open the action picker. A newly generated config writes the built-in actions explicitly so they can be edited in place:

```toml
[actions]
bindings = ["ctrl-enter", "ctrl-space"]

[[actions.items]]
label = "Navigate to"
icon = "󰁔"
type = "navigate"

[[actions.items]]
label = "Open terminal here"
icon = ""
file_condition = ".git"
type = "command"
command = "zsh"
current_dir = "{path}"
```

Runtime defaults still exist when no actions are configured. Listing `actions.items` replaces the built-ins; set `include_defaults = true` only when you want built-ins prepended before listed actions. Built-in actions navigate to the target, open GitHub Desktop, open VS Code, run `idea .`, open the repo online, start `claude`, and start `opencode`. Custom command and URL actions can use `{path}` and `{github_url}` placeholders. Optional `icon` text supports Nerd Font glyphs or emoji. Optional `file_condition` hides an action unless the given file or directory exists under the selected target. In the action picker, type to filter actions, `Enter` runs the action and keeps the shell session open, and any configured picker binding runs it and asks the zsh wrapper to close the shell session afterward. Only the first configured binding is shown in the UI hints.

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
bindings = ["ctrl-n"]

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
