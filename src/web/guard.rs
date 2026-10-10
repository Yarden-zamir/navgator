//! The API runs git and configured commands, so only this machine's pages
//! may call it.
//!
//! - A `Host` check blocks DNS rebinding: a foreign site that resolves its
//!   own name to 127.0.0.1 still sends its own name as the host.
//! - An `Origin` check on every non-GET request blocks cross-site form posts.
//! - The server sends no CORS headers, so foreign pages cannot read responses.

use axum::{
    extract::{Request, State},
    http::{header, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::sync::Arc;

pub struct AllowedHosts {
    hosts: Vec<String>,
    origins: Vec<String>,
}

impl AllowedHosts {
    pub fn for_port(port: u16) -> Arc<Self> {
        let hosts = ["127.0.0.1", "localhost", "[::1]"]
            .iter()
            .map(|name| format!("{name}:{port}"))
            .collect::<Vec<_>>();
        let origins = hosts.iter().map(|host| format!("http://{host}")).collect();
        Arc::new(Self { hosts, origins })
    }
}

pub async fn check(
    State(allowed): State<Arc<AllowedHosts>>,
    request: Request,
    next: Next,
) -> Response {
    if let Err(message) = allowed.verify(&request) {
        return (StatusCode::FORBIDDEN, message).into_response();
    }
    next.run(request).await
}

impl AllowedHosts {
    fn verify(&self, request: &Request) -> Result<(), &'static str> {
        let header_value = |name| {
            request
                .headers()
                .get(name)
                .and_then(|value| value.to_str().ok())
        };
        if !header_value(header::HOST).is_some_and(|host| self.hosts.iter().any(|h| h == host)) {
            return Err("unexpected Host header");
        }
        let reads_only = matches!(*request.method(), Method::GET | Method::HEAD);
        if !reads_only
            && !header_value(header::ORIGIN)
                .is_some_and(|origin| self.origins.iter().any(|o| o == origin))
        {
            return Err("cross-origin requests are not allowed");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::AllowedHosts;
    use axum::{body::Body, extract::Request, http::Method};

    fn request(method: Method, host: &str, origin: Option<&str>) -> Request {
        let mut builder = Request::builder()
            .method(method)
            .uri("/api/results")
            .header("host", host);
        if let Some(origin) = origin {
            builder = builder.header("origin", origin);
        }
        builder.body(Body::empty()).expect("valid test request")
    }

    #[test]
    fn allows_same_origin_reads_and_writes() {
        let allowed = AllowedHosts::for_port(4747);
        assert!(allowed
            .verify(&request(Method::GET, "127.0.0.1:4747", None))
            .is_ok());
        assert!(allowed
            .verify(&request(
                Method::POST,
                "localhost:4747",
                Some("http://localhost:4747")
            ))
            .is_ok());
    }

    #[test]
    fn rejects_rebound_hosts() {
        let allowed = AllowedHosts::for_port(4747);
        assert!(allowed
            .verify(&request(Method::GET, "evil.example:4747", None))
            .is_err());
        assert!(allowed
            .verify(&request(Method::GET, "127.0.0.1:9999", None))
            .is_err());
    }

    #[test]
    fn rejects_cross_site_and_origin_less_writes() {
        let allowed = AllowedHosts::for_port(4747);
        assert!(allowed
            .verify(&request(
                Method::POST,
                "127.0.0.1:4747",
                Some("https://evil.example")
            ))
            .is_err());
        assert!(allowed
            .verify(&request(Method::POST, "127.0.0.1:4747", None))
            .is_err());
    }
}
