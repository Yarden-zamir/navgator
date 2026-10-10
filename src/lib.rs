//! Shared navgator core: config, git and worktree logic, search ranking,
//! tags, usage history and preview data. The terminal UI (`src/main.rs`)
//! and the web app (`navgator web`, `src/web/`) both build on it.

pub mod actions;
pub mod commands;
pub mod config;
pub mod content;
pub mod create;
pub mod git;
pub mod github;
pub mod metadata;
pub mod model;
pub mod path_identity;
pub mod provider_runtime;
pub mod results;
pub mod search;
pub mod tags;
pub mod usage;
pub mod web;
