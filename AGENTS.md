# AGENTS.md

- This repo owns the `navgator` project navigator binary.
- Git, GitHub README, folder/worktree content, config, tags, metadata, search ranking, explanations, compositor, and app UI behavior live here.
- Generic terminal/tooling helpers should stay in `gator`.
- Regenerate `config-schema.json` after config struct changes with `cargo run -- config-schema > config-schema.json`.
- Verify with `cargo fmt -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test`.
