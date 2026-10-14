//! `navgator web`: the navigator as a local web app. It reads the same
//! config, caches, tags and usage history as the terminal UI.

mod api;
mod app;
mod assets;
mod guard;
mod progress;
mod render;
mod settings;
mod theme;

use axum::{
    http::{header, StatusCode},
    middleware,
    response::IntoResponse,
    routing::{get, post, put},
    Router,
};
use std::{
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpStream},
    sync::Arc,
    time::{Duration, SystemTime},
};

pub const DEFAULT_PORT: u16 = 4747;

/// The marker that `/api/health` answers with, so a probe can tell this
/// server from another program on the same port.
const HEALTH_MARKER: &str = "navgator web";

pub struct WebOptions {
    pub port: u16,
    /// Open the page in the default browser.
    pub open: bool,
    pub config_entries: Vec<String>,
}

enum PortState {
    Free,
    Navgator,
    OtherProgram,
}

/// Open the page of a running server, or start one and open its page.
/// A started server runs in the foreground until the process stops or the
/// navgator binary on disk changes.
pub fn run(options: WebOptions) -> Result<(), String> {
    let url = format!("http://127.0.0.1:{}", options.port);
    match probe(options.port) {
        PortState::Navgator => {
            println!("navgator web already runs on {url}");
            if options.open {
                crate::commands::open_url(&url)?;
            }
            Ok(())
        }
        PortState::OtherProgram => Err(format!(
            "another program uses port {}; pick a free one with --port",
            options.port
        )),
        PortState::Free => {
            let runtime = tokio::runtime::Runtime::new()
                .map_err(|error| format!("cannot start the async runtime: {error}"))?;
            let result = runtime.block_on(serve(options, url));
            // Do not wait for open event streams or running actions.
            runtime.shutdown_background();
            result
        }
    }
}

/// Ask `127.0.0.1:port` for `/api/health` with a short timeout.
fn probe(port: u16) -> PortState {
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let timeout = Duration::from_millis(500);
    let Ok(mut stream) = TcpStream::connect_timeout(&address, timeout) else {
        return PortState::Free;
    };
    let request =
        format!("GET /api/health HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
    let mut response = String::new();
    let answered = stream.set_read_timeout(Some(timeout)).is_ok()
        && stream.write_all(request.as_bytes()).is_ok()
        && stream.take(4096).read_to_string(&mut response).is_ok();
    if answered && response.starts_with("HTTP/1.1 200") && response.contains(HEALTH_MARKER) {
        PortState::Navgator
    } else {
        PortState::OtherProgram
    }
}

async fn serve(options: WebOptions, url: String) -> Result<(), String> {
    let app = app::App::start(options.config_entries)?;
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, options.port))
        .await
        .map_err(|error| format!("cannot listen on 127.0.0.1:{}: {error}", options.port))?;
    println!("navgator web runs on {url} (Ctrl+C stops it)");
    if options.open {
        crate::commands::open_url(&url)?;
    }
    tokio::select! {
        result = axum::serve(listener, router(app, options.port)) => {
            result.map_err(|error| format!("server stopped: {error}"))
        }
        () = binary_replaced() => {
            println!("navgator web stops: the navgator binary changed, start it again to run the new version");
            Ok(())
        }
    }
}

/// Resolves when the navgator binary on disk changes, for example after
/// `brew upgrade` or `cargo install`. A host that starts the server when the
/// port is free (T3 Code) then starts the new version.
///
/// Limit: a Homebrew upgrade with cleanup turned off keeps the old keg, so a
/// server started from the keg path sees no change. Watch the `bin` link too
/// if that setup comes up.
async fn binary_replaced() {
    let Some(started) = binary_stamp() else {
        eprintln!(
            "navgator web: cannot read the navgator binary, so it does not stop on an upgrade"
        );
        return std::future::pending().await;
    };
    let mut tick = tokio::time::interval(Duration::from_secs(5));
    loop {
        tick.tick().await;
        if binary_stamp() != Some(started) {
            return;
        }
    }
}

/// The modification time of the running binary, or `None` when it is gone.
fn binary_stamp() -> Option<SystemTime> {
    std::env::current_exe()
        .ok()?
        .metadata()
        .ok()?
        .modified()
        .ok()
}

fn router(app: Arc<app::App>, port: u16) -> Router {
    Router::new()
        .route("/api/health", get(health))
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
        .route("/api/settings", get(settings::read).put(settings::update))
        .route("/theme.css", get(theme_css))
        .fallback(assets::serve)
        .with_state(app)
        .layer(middleware::from_fn_with_state(
            guard::AllowedHosts::for_port(port),
            guard::check,
        ))
}

async fn health() -> String {
    format!("{HEALTH_MARKER} {}", env!("CARGO_PKG_VERSION"))
}

async fn theme_css() -> impl IntoResponse {
    let css = gator::config::home_dir()
        .map_err(|error| format!("cannot find the home folder: {error}"))
        .and_then(|home| theme::t3_theme_css(&home));
    let headers = |content_type| {
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "no-cache"),
        ]
    };
    match css {
        Ok(css) => (StatusCode::OK, headers("text/css"), css.unwrap_or_default()),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            headers("text/plain"),
            error,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{probe, PortState};
    use std::{io::Write, net::TcpListener, thread};

    #[test]
    fn probe_sees_a_free_port() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind a free port");
        let port = listener.local_addr().expect("local address").port();
        drop(listener);
        assert!(matches!(probe(port), PortState::Free));
    }

    #[test]
    fn probe_tells_another_program_apart() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind a free port");
        let port = listener.local_addr().expect("local address").port();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\n\r\nsomething else");
            }
        });
        assert!(matches!(probe(port), PortState::OtherProgram));
    }
}
