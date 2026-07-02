# navgator

Rust TUI project navigator with Git worktree and preview support.

## Install

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

## Zsh Widget

```sh
source "$(brew --prefix)/share/navgator/navgator.zsh"
bindkey '^T' navigate
```

The wrapper writes selections through `GATOR_OUTPUT`; otherwise `navgator` prints the selected path to stdout.

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
