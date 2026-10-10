//! navgator-web: the navgator project navigator as a local web app.
//! It reads the same config, caches and usage history as the terminal UI.

mod api;
mod app;
mod assets;
mod guard;
mod progress;
mod render;
mod theme;

use axum::{
    http::{header, StatusCode},
    middleware,
    response::IntoResponse,
    routing::{get, post, put},
    Router,
};
use std::{env, net::Ipv4Addr, process::ExitCode, sync::Arc};

const DEFAULT_PORT: u16 = 4747;

const HELP: &str = "\
🐊 navgator-web — navgator in the browser

Usage:
  navgator-web [serve] [OPTIONS]

Options:
  --port PORT              Listen on 127.0.0.1:PORT (default 4747)
  --open                   Open the page in the default browser after start
  --config-entry TOML      Override one config entry, as in `navgator`;
                           repeatable, applied in order
  -h, --help               Show this help
  -V, --version            Show the version

The server reads the navgator config (NAVGATOR_CONFIG or the default path)
and listens on the loopback interface only.";

struct Options {
    port: u16,
    open: bool,
    config_entries: Vec<String>,
}

enum Command {
    Serve(Options),
    Help,
    Version,
}

fn parse_args(args: &[String]) -> Result<Command, String> {
    let mut options = Options {
        port: DEFAULT_PORT,
        open: false,
        config_entries: Vec::new(),
    };
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "serve" => {}
            "-h" | "--help" | "help" => return Ok(Command::Help),
            "-V" | "--version" | "version" => return Ok(Command::Version),
            "--open" => options.open = true,
            "--port" => {
                let value = args.next().ok_or("--port requires a value")?;
                options.port = value
                    .parse()
                    .map_err(|_| format!("--port expects a port number, got {value:?}"))?;
            }
            "--config-entry" => {
                let value = args
                    .next()
                    .filter(|value| !value.trim().is_empty())
                    .ok_or("--config-entry requires a non-empty TOML assignment")?;
                options.config_entries.push(value.clone());
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(Command::Serve(options))
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let options = match parse_args(&args) {
        Ok(Command::Serve(options)) => options,
        Ok(Command::Help) => {
            println!("{HELP}");
            return ExitCode::SUCCESS;
        }
        Ok(Command::Version) => {
            println!("navgator-web {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        Err(error) => {
            eprintln!("{error}\nRun `navgator-web --help` for usage.");
            return ExitCode::from(2);
        }
    };
    match serve(options).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("navgator-web: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn serve(options: Options) -> Result<(), String> {
    let app = app::App::start(options.config_entries)?;
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, options.port))
        .await
        .map_err(|error| format!("cannot listen on 127.0.0.1:{}: {error}", options.port))?;
    let url = format!("http://127.0.0.1:{}", options.port);
    println!("navgator-web listens on {url}");
    if options.open {
        navgator::commands::open_url(&url)?;
    }
    axum::serve(listener, router(app, options.port))
        .await
        .map_err(|error| format!("server stopped: {error}"))
}

fn router(app: Arc<app::App>, port: u16) -> Router {
    Router::new()
        .route("/api/bootstrap", get(api::bootstrap))
        .route("/api/results", get(api::results))
        .route("/api/events", get(api::events))
        .route("/api/remotes", post(api::load_remotes))
        .route("/api/preview", get(api::preview))
        .route("/api/tree", get(api::tree))
        .route("/api/git", get(api::git))
        .route("/api/github", get(api::github))
        .route("/api/target", get(api::target))
        .route("/api/actions", get(api::actions))
        .route("/api/actions/run", post(api::run_action))
        .route("/api/branches/select", post(api::select_branch))
        .route("/api/worktrees/delete", post(api::delete_worktree))
        .route("/api/create/values", post(api::create_values))
        .route("/api/create/run", post(api::run_create))
        .route("/api/paths", get(api::path_completions))
        .route("/api/tags", put(api::save_tags))
        .route("/api/tags/suggestions", get(api::tag_suggestions))
        .route("/theme.css", get(theme_css))
        .fallback(assets::serve)
        .with_state(app)
        .layer(middleware::from_fn_with_state(
            guard::AllowedHosts::for_port(port),
            guard::check,
        ))
}

async fn theme_css() -> impl IntoResponse {
    let css = gator::config::home_dir()
        .map_err(|error| format!("cannot find the home folder: {error}"))
        .and_then(|home| theme::t3_theme_css(&home));
    match css {
        Ok(css) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "text/css"),
                (header::CACHE_CONTROL, "no-cache"),
            ],
            css.unwrap_or_default(),
        ),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            [
                (header::CONTENT_TYPE, "text/plain"),
                (header::CACHE_CONTROL, "no-cache"),
            ],
            error,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_args, Command};

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn serve_is_the_default_command() {
        let Ok(Command::Serve(options)) = parse_args(&args(&["--port", "5000", "--open"])) else {
            panic!("expected serve");
        };
        assert_eq!(options.port, 5000);
        assert!(options.open);
    }

    #[test]
    fn config_entries_are_repeatable() {
        let Ok(Command::Serve(options)) = parse_args(&args(&[
            "--config-entry",
            "sort.default=\"alpha-asc\"",
            "--config-entry",
            "remote.enabled=true",
        ])) else {
            panic!("expected serve");
        };
        assert_eq!(options.config_entries.len(), 2);
    }

    #[test]
    fn rejects_unknown_arguments() {
        assert!(parse_args(&args(&["--nope"])).is_err());
    }
}
