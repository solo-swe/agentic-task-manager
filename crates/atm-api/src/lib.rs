//! The dashboard server (the REST API; the Python atm's `api/server.py`): one per user, a detached process serving
//! every registered project, on axum. It holds the dashboard page and its assets in memory (embedded in the binary)
//! and answers GET and HEAD (and POST, PATCH and PUT on a project's reviews, from its own page only: ATM-572), only for the Host `localhost:<port>` or `127.0.0.1:<port>` (DNS rebinding), with
//! the security headers and the page's Content-Security-Policy of docs/security-architecture.md I12–I13. Its routes
//! and record are described in docs/multi-project-api.md. It never depends on the CLI: `atm service` starts it as a
//! process (`atm _serve <home> <port>`).
//!
//! Part of atm's Rust port (T-415): docs/porting-to-rust.md, docs/layered-architecture.md, docs/guides/rust.md.

/// Where the page's framework looks for a new deployment of it (`_app/version.json`, `SvelteKit`'s `appDir`)
const APP_VERSION: &str = "/_app/version.json";

mod assets;
mod pages;
/// The read API's routes: public so the app's tests can check its commands answer the same (ADR-003 §5).
pub mod routes;

use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, TcpStream};

use atm_common::Shown;
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
    projects: routes::Projects,
    /// This start's session token (ATM-572): in the page it serves, asked of every write
    session: String,
    /// The page, with the session token in it
    page: Vec<u8>,
}

/// The header a review change carries the session token in.
const SESSION_HEADER: &str = "x-atm-session";
/// The largest body a review change may send.
const MAX_CHANGE_BYTES: usize = 1_000_000;

/// The methods of the review routes that change something (`routes::serve_review_change`); DELETE only a draft
/// (ATM-612), with the same checks and a JSON body (`{}`).
fn changes_something(method: &Method) -> bool {
    *method == Method::POST
        || *method == Method::PATCH
        || *method == Method::PUT
        || *method == Method::DELETE
}

/// A random token for this start: 244 bits from the system's source, as two v4 UUIDs.
fn session_token() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

/// The page with the session token in a `meta` tag the page reads (`atm-session`), right after `<head>`. Another
/// site's page can't read it (the same-origin policy), so it can't change anything.
fn with_session(page: &[u8], token: &str) -> Vec<u8> {
    let text = String::from_utf8_lossy(page);
    let tag = format!(r#"<meta name="atm-session" content="{token}">"#);
    match text.find("<head>") {
        Some(at) => {
            let end = at + "<head>".len();
            format!("{}{tag}{}", &text[..end], &text[end..]).into_bytes()
        }
        None => page.to_vec(),
    }
}

/// Whether two tokens are equal, comparing every byte (no early exit to time).
fn same_token(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

/// Whether a request comes from another site's page (ATM-671): the browser says so (`Sec-Fetch-Site: cross-site`), or
/// its `Origin` isn't on this machine (`localhost` or `127.0.0.1`, any port: the dashboard's dev server, on another
/// port, sends its requests on). A client without these headers (atm itself, a script) is none.
fn from_another_site(headers: &HeaderMap) -> bool {
    let value = |name: &str| headers.get(name).and_then(|v| v.to_str().ok());
    if value("sec-fetch-site").is_some_and(|s| s.eq_ignore_ascii_case("cross-site")) {
        return true;
    }
    value(header::ORIGIN.as_str()).is_some_and(|origin| {
        let host = origin
            .strip_prefix("http://")
            .and_then(|rest| rest.split(':').next())
            .unwrap_or_default();
        !(host == "localhost" || host == "127.0.0.1")
    })
}

/// Why a review change is refused before anything is read (docs/code-review-design.md §3.12): it must come from the
/// page this server served, with its token, as JSON. None when it may go on.
fn review_change_refused(served: &Served, host: &str, headers: &HeaderMap) -> Option<Response> {
    let value = |name: header::HeaderName| headers.get(name).and_then(|v| v.to_str().ok());
    // The request's Origin is this server's own (the page atm serves; never another site, never none)
    let port = served.port;
    let origin_ok = value(header::ORIGIN).is_some_and(|o| {
        o == format!("http://{host}")
            && (o == format!("http://localhost:{port}") || o == format!("http://127.0.0.1:{port}"))
    });
    if !origin_ok {
        return Some(send_error(
            StatusCode::FORBIDDEN,
            "a change comes from atm's own page",
        ));
    }
    let token = headers.get(SESSION_HEADER).and_then(|v| v.to_str().ok());
    if !token.is_some_and(|t| same_token(t, &served.session)) {
        return Some(send_error(
            StatusCode::FORBIDDEN,
            "no session token, or not this server's",
        ));
    }
    // JSON, which a plain form can't send
    let json = value(header::CONTENT_TYPE).is_some_and(|t| {
        let kind = t.split(';').next().unwrap_or_default().trim();
        kind.eq_ignore_ascii_case("application/json")
    });
    if !json {
        return Some(send_error(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "the body is JSON",
        ));
    }
    None
}

/// A review change (ATM-572): checked, its body read (at most [`MAX_CHANGE_BYTES`]), then its route's atm-lib call
/// off the async workers.
async fn serve_checked_review_change(
    served: Arc<Served>,
    host: String,
    request: Request,
) -> Response {
    if let Some(refused) = review_change_refused(&served, &host, request.headers()) {
        return refused;
    }
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let Ok(bytes) = axum::body::to_bytes(request.into_body(), MAX_CHANGE_BYTES).await else {
        return send_error(StatusCode::PAYLOAD_TOO_LARGE, "the body is too large");
    };
    let Ok(body) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return send_error(StatusCode::BAD_REQUEST, "the body isn't JSON");
    };
    tokio::task::spawn_blocking(move || {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            routes::serve_review_change(&served.projects, &method, &path, body)
        }))
        .unwrap_or_else(|_| send_error(StatusCode::INTERNAL_SERVER_ERROR, "internal error"))
    })
    .await
    .unwrap_or_else(|_| send_error(StatusCode::INTERNAL_SERVER_ERROR, "internal error"))
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
    response.body(Body::from(body)).unwrap_or_else(|_| {
        // A header the builder refused (ATM-672, audit H11): a bare 500, never a 200 without the security headers
        let mut failed = Response::new(Body::empty());
        *failed.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
        let headers = failed.headers_mut();
        for (name, value) in [
            ("cache-control", "no-store"),
            ("x-content-type-options", "nosniff"),
            ("cross-origin-resource-policy", "same-origin"),
        ] {
            headers.insert(name, header::HeaderValue::from_static(value));
        }
        failed
    })
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

/// Every request the server takes: the Host checked, then the page, its assets, whoami, the read API, or a review
/// change.
async fn serve_request(State(served): State<Arc<Served>>, request: Request) -> Response {
    let method = request.method().clone();
    let changes_reviews =
        changes_something(&method) && routes::is_reviews_path(request.uri().path());
    if method != Method::GET && method != Method::HEAD && !changes_reviews {
        // OPTIONS among them: no CORS preflight is ever answered, so no other site can send the session header; and
        // every path but a project's reviews is read-only
        return send_error(
            StatusCode::METHOD_NOT_ALLOWED,
            "only GET and HEAD, and POST, PATCH, PUT or DELETE on reviews",
        );
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
    if changes_reviews {
        return serve_checked_review_change(Arc::clone(&served), host, request).await;
    }
    let path = request.uri().path();
    // A read another site's page sends (ATM-671, audit H5): reads write too (`recent.json`) and run git, so a GET
    // from elsewhere is refused. Opening the page itself from another site is fine, and so are its assets
    if !pages::is_page(path)
        && !path.starts_with(assets::PREFIX)
        && from_another_site(request.headers())
    {
        return send_error(StatusCode::FORBIDDEN, "a read comes from atm's own page");
    }
    if pages::is_page(path) {
        let policy = page_csp(&served.hashes, &format!("http://{host}"));
        return send(
            StatusCode::OK,
            served.page.clone(),
            "text/html; charset=utf-8",
            None,
            &[("Content-Security-Policy", &policy)],
        );
    }
    if let Some(rest) = path.strip_prefix(assets::PREFIX) {
        return assets::serve(rest, request.method(), request.headers());
    }
    if path == APP_VERSION {
        // SvelteKit's own check for a new deployment (when the tab regains focus): the page's version is atm's
        // (vite.config.ts `version.name`), and atm's own whoami decides when to offer a reload
        return send(
            StatusCode::OK,
            to_json(&json!({"version": served.version})).into_bytes(),
            "application/json",
            None,
            &[],
        );
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
    // The read API works on the projects' files: off the async workers
    let (path, query) = (
        path.to_string(),
        request.uri().query().unwrap_or_default().to_string(),
    );
    let headers = request.headers().clone();
    let worker = Arc::clone(&served);
    tokio::task::spawn_blocking(move || {
        // One bad request, or one project's broken files, gets a 500, never a dead connection
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            routes::serve(&worker.projects, &path, &query, &headers)
        }))
        .unwrap_or_else(|_| send_error(StatusCode::INTERNAL_SERVER_ERROR, "internal error"))
    })
    .await
    .unwrap_or_else(|_| send_error(StatusCode::INTERNAL_SERVER_ERROR, "internal error"))
}

/// Something accepts connections on `127.0.0.1:port` or `[::1]:port` (`localhost` may resolve to either). Both asked
/// at once: on a free port each can take the whole timeout (Windows tries again before giving up: ATM-482).
fn port_in_use(port: u16) -> bool {
    let addresses = [
        SocketAddr::from((Ipv4Addr::LOCALHOST, port)),
        SocketAddr::from((Ipv6Addr::LOCALHOST, port)),
    ];
    std::thread::scope(|scope| {
        let asks: Vec<_> = addresses
            .iter()
            .map(|a| {
                scope.spawn(move || {
                    TcpStream::connect_timeout(a, Duration::from_millis(500)).is_ok()
                })
            })
            .collect();
        asks.into_iter().any(|ask| ask.join().unwrap_or(false))
    })
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
    let session = session_token();
    let served = Arc::new(Served {
        port,
        version: version.clone(),
        build: build.clone(),
        home: home_id(home),
        hashes: script_hashes(PAGE),
        projects: routes::Projects::new(home),
        page: with_session(PAGE, &session),
        session,
    });
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    let record_path: PathBuf = home.join(SERVER_RECORD);
    runtime.block_on(async move {
        let app = Router::new()
            .fallback(serve_request)
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
            .map(|p| p.shown())
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
