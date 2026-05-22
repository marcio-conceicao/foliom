//! Host-header allowlist — DNS rebinding mitigation (T-02-01).
//!
//! Without this middleware, an attacker page on `evil.example.com` whose
//! DNS resolves to `127.0.0.1` could reach our loopback API from the
//! victim's browser. By rejecting any `Host` header whose hostname is
//! not in `{127.0.0.1, localhost}`, we block that vector even though
//! the socket is loopback-bound.
//!
//! SECURITY: Referenced by 02-CONTEXT.md threat T-02-01 and 02-RESEARCH
//! §Security Domain. Must not be removed without an equivalent control.

use axum::{
    extract::Request,
    http::{StatusCode, header::HOST},
    middleware::Next,
    response::Response,
};

/// Hosts allowed to address this server. Anything else is rejected with
/// 421 Misdirected Request — the closest HTTP semantic ("you sent this
/// to the wrong server").
const ALLOWED_HOSTS: &[&str] = &["127.0.0.1", "localhost"];

pub async fn host_allowlist(req: Request, next: Next) -> Result<Response, StatusCode> {
    let host_header = req
        .headers()
        .get(HOST)
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::MISDIRECTED_REQUEST)?;

    // Strip optional `:port` suffix; HTTP/1.1 RFC 9110 §7.2 permits both.
    let hostname = host_header.rsplit_once(':').map_or(host_header, |(h, _)| h);

    if ALLOWED_HOSTS.contains(&hostname) {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::MISDIRECTED_REQUEST)
    }
}
