//! The dashboard server (the REST API; the Python atm's `api/server.py`): one per user, a detached process serving
//! every registered project, on axum. It holds the dashboard page and its assets in memory (embedded in the binary)
//! and answers only GET and HEAD, only for the Host `localhost:<port>` or `127.0.0.1:<port>` (DNS rebinding), with
//! the security headers and the page's Content-Security-Policy of docs/security-architecture.md I12–I13. Its routes
//! and record are described in docs/multi-project-api.md. It never depends on the CLI: `atm service` starts it as a
//! process (`atm _serve <home> <port>`).
//!
//! Part of atm's Rust port (T-415): docs/porting-to-rust.md, docs/layered-architecture.md, docs/guides/rust.md.

mod assets;
mod pages;

use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use atm_common::dashboard::{
    PAGE, SERVER_RECORD, WHOAMI, home_id, page_csp, page_tags, script_hashes,
};
use atm_common::home::write_text_atomic;
use atm_common::py::to_json;
use atm_common::time::now_iso;
use axum::Router;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, Method, StatusCode, header};
use axum::response::Response;
use serde_json::json;

/// How many ports from the first one the server tries.
const PORT_TRIES: u16 = 100;

/// What every request can see: the server's own facts, worked out once.
struct Served {
    port: u16,
    version: String,
    build: String,
    home: String,
    hashes: Vec<String>,
}

/// A response as atm sends them: `Cache-Control: no-store` unless `headers` say otherwise, no content sniffing, and
/// no other site may embed it (docs/security-review.md F11).
fn send(
    status: StatusCode,
    body: Vec<u8>,
    content_type: &str,
    etag: Option<&str>,
    headers: &[(&str, &str)],
) -> Response {
    let mut response = Response::builder().status(status);
    if !body.is_empty() || status == StatusCode::OK {
        response = response
            .header(header::CONTENT_TYPE, content_type)
            .header(header::CONTENT_LENGTH, body.len());
    }
    if let Some(etag) = etag {
        response = response.header(header::ETAG, etag);
    }
    let mut all: Vec<(&str, &str)> = vec![
        ("Cache-Control", "no-store"),
        ("X-Content-Type-Options", "nosniff"),
        ("Cross-Origin-Resource-Policy", "same-origin"),
    ];
    for (name, value) in headers {
        match all.iter_mut().find(|(n, _)| n.eq_ignore_ascii_case(name)) {
            Some(entry) => entry.1 = value,
            None => all.push((name, value)),
        }
    }
    for (name, value) in all {
        response = response.header(name, value);
    }
    response
        .body(Body::from(body))
        .unwrap_or_else(|_| Response::new(Body::empty()))
}

/// `{"error": message}` with `status`.
fn send_error(status: StatusCode, message: &str) -> Response {
    send(
        status,
        to_json(&json!({"error": message})).into_bytes(),
        "application/json",
        None,
        &[],
    )
}

/// 304 when the client already has this version (`If-None-Match`), else 200 with the body.
fn send_conditional(
    request: &HeaderMap,
    etag: &str,
    body: impl FnOnce() -> Vec<u8>,
    content_type: &str,
    headers: &[(&str, &str)],
) -> Response {
    let unchanged = request
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        == Some(etag);
    if unchanged {
        // Unchanged since the dashboard's last poll: it skips parsing and re-rendering (D-5.1)
        return send(
            StatusCode::NOT_MODIFIED,
            Vec::new(),
            content_type,
            Some(etag),
            headers,
        );
    }
    send(StatusCode::OK, body(), content_type, Some(etag), headers)
}

async fn handle(State(served): State<Arc<Served>>, request: Request) -> Response {
    if request.method() != Method::GET && request.method() != Method::HEAD {
        return send_error(StatusCode::METHOD_NOT_ALLOWED, "only GET and HEAD");
    }
    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let port = served.port;
    if host != format!("localhost:{port}") && host != format!("127.0.0.1:{port}") {
        // A page on another site that points its own domain at 127.0.0.1 (DNS rebinding) is refused
        return send_error(StatusCode::FORBIDDEN, "unexpected Host header");
    }
    let path = request.uri().path();
    if pages::is_page(path) {
        let policy = page_csp(&served.hashes, &format!("http://{host}"));
        return send(
            StatusCode::OK,
            PAGE.to_vec(),
            "text/html; charset=utf-8",
            None,
            &[("Content-Security-Policy", &policy)],
        );
    }
    if let Some(rest) = path.strip_prefix(assets::PREFIX) {
        return assets::serve(rest, request.headers());
    }
    if path == WHOAMI {
        let info = json!({
            "atm": true,
            "pid": std::process::id(),
            "port": port,
            "version": served.version,
            "build": served.build,
            "home": served.home,
        });
        return send(
            StatusCode::OK,
            to_json(&info).into_bytes(),
            "application/json",
            None,
            &[],
        );
    }
    send_error(StatusCode::NOT_FOUND, "not found")
}

/// Something accepts connections on `127.0.0.1:port` or `[::1]:port` (`localhost` may resolve to either).
fn port_in_use(port: u16) -> bool {
    let addresses = [
        SocketAddr::from((Ipv4Addr::LOCALHOST, port)),
        SocketAddr::from((Ipv6Addr::LOCALHOST, port)),
    ];
    addresses
        .iter()
        .any(|a| TcpStream::connect_timeout(a, Duration::from_millis(500)).is_ok())
}

/// The first free port from `first`: listening on `127.0.0.1` and, where there is IPv6, on `[::1]` too, so no
/// other local user can take `[::1]` on our port and answer for localhost (docs/security-review.md F10).
fn listen(first: u16) -> Option<(TcpListener, Option<TcpListener>)> {
    for candidate in first..first.saturating_add(PORT_TRIES) {
        if port_in_use(candidate) {
            continue;
        }
        let Ok(v4) = TcpListener::bind((Ipv4Addr::LOCALHOST, candidate)) else {
            continue;
        };
        match TcpListener::bind((Ipv6Addr::LOCALHOST, candidate)) {
            Ok(v6) => return Some((v4, Some(v6))),
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {}
            // No IPv6 on this machine: nobody can listen on [::1] then
            Err(_) => return Some((v4, None)),
        }
    }
    None
}

/// Remove the server's record when it is this process's (a newer server's record is left alone).
fn forget(record_path: &Path) {
    let ours = std::fs::read_to_string(record_path)
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .is_some_and(|r| {
            r.get("pid").and_then(serde_json::Value::as_u64) == Some(u64::from(std::process::id()))
        });
    if ours {
        let _ = std::fs::remove_file(record_path);
    }
}

/// The detached server process: serve on the first free port from `first_port`, record it in `home`, run until
/// stopped (SIGTERM or SIGINT, which remove the record).
///
/// # Errors
/// No free port, the record can't be written, or the server can't run.
pub fn serve(home: &Path, first_port: u16) -> Result<(), String> {
    let (version, build) = page_tags(PAGE);
    let Some((v4, v6)) = listen(first_port) else {
        return Err(format!(
            "no free port in {first_port}-{}",
            first_port.saturating_add(PORT_TRIES - 1)
        ));
    };
    let port = v4.local_addr().map_err(|e| e.to_string())?.port();
    let served = Arc::new(Served {
        port,
        version: version.clone(),
        build: build.clone(),
        home: home_id(home),
        hashes: script_hashes(PAGE),
    });
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    let record_path: PathBuf = home.join(SERVER_RECORD);
    runtime.block_on(async move {
        let app = Router::new()
            .fallback(handle)
            .with_state(Arc::clone(&served));
        let mut listeners = vec![v4];
        listeners.extend(v6);
        for listener in listeners {
            listener.set_nonblocking(true).map_err(|e| e.to_string())?;
            let listener =
                tokio::net::TcpListener::from_std(listener).map_err(|e| e.to_string())?;
            let app = app.clone();
            tokio::spawn(async move {
                let _ = axum::serve(listener, app).await;
            });
        }
        let plugin_root = std::env::current_exe()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        let record = json!({
            "pid": std::process::id(),
            "port": port,
            "version": version,
            "build": build,
            "plugin_root": plugin_root,
            "home": served.home,
            "started_at": now_iso(),
        });
        write_text_atomic(&record_path, &(to_json(&record) + "\n")).map_err(|e| e.to_string())?;
        stopped().await;
        forget(&record_path);
        Ok(())
    })
}

/// Until the process is asked to stop: SIGTERM, or SIGINT.
async fn stopped() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let (Ok(mut term), Ok(mut int)) = (
            signal(SignalKind::terminate()),
            signal(SignalKind::interrupt()),
        ) else {
            std::future::pending::<()>().await;
            return;
        };
        tokio::select! {
            _ = term.recv() => {}
            _ = int.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
