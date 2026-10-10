# AGENTS.md

- This repo owns the `navgator` project navigator binary: the terminal UI and the web app (`navgator web`).
- The crate is a library (`src/lib.rs`) plus the binary (`src/main.rs`). The web server is `src/web/`; its UI is `web/ui`.
- Put shared logic in the library. Keep `src/main.rs`, `ui.rs`, `compositor.rs` and `onboarding.rs` for terminal-only behavior, and `src/web/` plus `web/ui` for web-only behavior.
- `cargo test` regenerates the web API types in `web/ui/src/api/`. After a change to a type that the API sends, commit those files.
- For UI changes, run `npm install` once and `npm run typecheck` in `web/ui`.
- Git, GitHub README, folder/worktree content, config, tags, metadata, search ranking, explanations, compositor, and app UI behavior live here.
- Generic terminal/tooling helpers should stay in `gator`.
- Regenerate `config-schema.json` after config struct changes with `cargo run -- config-schema > config-schema.json`.
- When asked to build this repo, default to `cargo build --release` unless a debug build is explicitly requested.
- After making code or config-schema-affecting changes, always run `cargo build --release` before handing off.
- Verify with `cargo fmt -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test`.
