//! Long git and shell work streams its progress as newline-delimited JSON.

use axum::{body::Body, http::header, response::Response};
use serde::Serialize;
use std::convert::Infallible;
use tokio_stream::{wrappers::UnboundedReceiverStream, StreamExt};
use ts_rs::TS;

/// One line of a progress stream. The last line is `done` or `error`; a
/// stream that ends without one failed on the server.
#[derive(Serialize, TS)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Progress {
    Message { text: String },
    Done { path: String },
    Error { message: String },
}

/// Run `work` on a blocking thread and stream its messages and outcome.
pub fn stream_progress<F>(work: F) -> Response
where
    F: FnOnce(&mut dyn FnMut(String)) -> Result<String, String> + Send + 'static,
{
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Progress>();
    tokio::task::spawn_blocking(move || {
        let message_tx = tx.clone();
        let mut send_message = move |text: String| {
            // A closed channel means the browser left; the work still finishes.
            let _ = message_tx.send(Progress::Message { text });
        };
        let last = match work(&mut send_message) {
            Ok(path) => Progress::Done { path },
            Err(message) => Progress::Error { message },
        };
        let _ = tx.send(last);
    });
    let lines = UnboundedReceiverStream::new(rx).map(|progress| {
        let mut line = serde_json::to_string(&progress).expect("progress serializes to JSON");
        line.push('\n');
        Ok::<_, Infallible>(line)
    });
    Response::builder()
        .header(header::CONTENT_TYPE, "application/x-ndjson")
        .body(Body::from_stream(lines))
        .expect("static progress response headers are valid")
}
