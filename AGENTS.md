# AGENTS.md

- This repo owns the `navgator` project navigator: the terminal UI binary and the `navgator-web` web app.
- The repo is a Cargo workspace. The root crate is a library (`src/lib.rs`) plus the TUI binary (`src/main.rs`). `web/` holds the `navgator-web` crate and its UI in `web/ui`.
- Put shared logic in the library. Keep `src/main.rs`, `ui.rs`, `compositor.rs` and `onboarding.rs` for terminal-only behavior, and `web/` for web-only behavior.
- After a change to an API type in `web/src`, or to a library type that the API sends, run `cargo test -p navgator-web`. It regenerates `web/ui/src/api/`; commit those files.
- For UI changes, run `npm install` once and `npm run typecheck` in `web/ui`.
- Git, GitHub README, folder/worktree content, config, tags, metadata, search ranking, explanations, compositor, and app UI behavior live here.
- Generic terminal/tooling helpers should stay in `gator`.
- Regenerate `config-schema.json` after config struct changes with `cargo run -- config-schema > config-schema.json`.
- When asked to build this repo, default to `cargo build --release` unless a debug build is explicitly requested.
- After making code or config-schema-affecting changes, always run `cargo build --release` before handing off.
- Verify with `cargo fmt -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test`.
