# navgator

Rust TUI project navigator with Git worktree and preview support.

## Install

Homebrew tap: https://github.com/Yarden-zamir/homebrew-tap

```sh
brew install yarden-zamir/tap/navgator
```

## Run

```sh
navgator
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

Press `Ctrl+Enter` to open the action picker. A newly generated config writes the built-in actions explicitly so they can be edited in place:

```toml
[actions]
defaults = false

[[actions.items]]
label = "Navigate to"
type = "navigate"

[[actions.items]]
label = "Open terminal here"
type = "command"
command = "zsh"
current_dir = "{path}"
```

Runtime defaults still exist when no actions are configured. Use `defaults = true` to include built-ins before custom actions, or `defaults = false` to use only listed actions. Built-in actions navigate to the target, open GitHub Desktop, open VS Code, run `idea .`, open the repo online, start `claude`, and start `opencode`. Custom command and URL actions can use `{path}` and `{github_url}` placeholders.

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
