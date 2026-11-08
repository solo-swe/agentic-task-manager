//! What the CLI and the REST API's server both know about the dashboard server (the Python atm's
//! `common/dashboard.py`): the dashboard page, embedded in the binary, with the version and build tags at its top; the
//! server's record and log in the user's atm folder; how to ask a server who it is; and the page's
//! Content-Security-Policy.

use std::path::Path;

use serde_json::{Map, Value};

use crate::py::truthy;
use crate::sha256::{base64, sha256, sha256_hex};

/// The dashboard (`plugin/templates/tasks-dashboard.html`, built from `dashboard/`), embedded when atm is built: the
/// server serves it from memory, and `atm service --refresh` compares its build with the running server's.
pub static PAGE: &[u8] = include_bytes!("../../../plugin/templates/tasks-dashboard.html");

/// In the user's atm folder: the running server's `{pid, port, version, build, plugin_root, home, started_at}`.
pub const SERVER_RECORD: &str = "server.json";
/// In the user's atm folder: what the server prints.
pub const SERVER_LOG: &str = "server.log";
/// The port the server tries first.
pub const DEFAULT_PORT: u16 = 8765;
/// How many ports from the first one `atm service status` and `stop` look at for a server the record doesn't name.
pub const SCAN_PORTS: u16 = 20;
/// The path an atm server answers who it is on.
pub const WHOAMI: &str = "/__atm/whoami";
/// The tags sit at the top of the page; this much of it finds them.
const HEAD_BYTES: usize = 16384;
/// The `CompMD` runtime's folder under the server's assets.
pub const COMPMD_ASSETS: &str = "/assets/compmd/";

/// A short ID of an atm folder: its real path, hashed. The server gives it in whoami and its record, so the CLI can
/// tell its own folder's servers from another folder's without the path going over HTTP.
#[must_use]
pub fn home_id(home: &Path) -> String {
    let real = std::fs::canonicalize(home).unwrap_or_else(|_| home.to_path_buf());
    sha256_hex(real.display().to_string().as_bytes())[..16].to_string()
}

/// The `content` of the `<meta name="…">` tag in `head`, "" without one.
fn meta_tag(head: &str, name: &str) -> String {
    let marker = format!("<meta name=\"{name}\" content=\"");
    head.find(&marker)
        .map(|at| &head[at + marker.len()..])
        .and_then(|rest| rest.split_once('"').map(|(value, _)| value.to_string()))
        .unwrap_or_default()
}

/// (version, build ID) from the start of a dashboard page, "" for a missing tag.
#[must_use]
pub fn page_tags(page: &[u8]) -> (String, String) {
    let head = String::from_utf8_lossy(&page[..page.len().min(HEAD_BYTES)]);
    (
        meta_tag(&head, "atm-dashboard-version"),
        meta_tag(&head, "atm-dashboard-build"),
    )
}

/// A version as numbers, as the Python atm compares them: `0.32.1` is `[0, 32, 1]`, a part without digits 0.
#[must_use]
pub fn parse_version(text: &str) -> Vec<u64> {
    text.split('.')
        .map(|part| {
            let digits: String = part.chars().take_while(char::is_ascii_digit).collect();
            digits.parse().unwrap_or(0)
        })
        .collect()
}

/// Ask the atm server on this port (on localhost) who it is: none if nothing, or not an atm server, answers in
/// `timeout`.
#[must_use]
pub fn whoami(port: u64, timeout: std::time::Duration) -> Option<Map<String, Value>> {
    use std::io::{Read, Write};
    use std::net::{TcpStream, ToSocketAddrs};
    let port = u16::try_from(port).ok()?;
    let mut stream = ("localhost", port)
        .to_socket_addrs()
        .ok()?
        .find_map(|address| TcpStream::connect_timeout(&address, timeout).ok())?;
    stream.set_read_timeout(Some(timeout)).ok()?;
    stream.set_write_timeout(Some(timeout)).ok()?;
    write!(
        stream,
        "GET {WHOAMI} HTTP/1.0\r\nHost: localhost:{port}\r\nConnection: close\r\n\r\n"
    )
    .ok()?;
    let mut response = Vec::new();
    stream.take(1 << 20).read_to_end(&mut response).ok()?;
    let text = String::from_utf8(response).ok()?;
    let (head, body) = text.split_once("\r\n\r\n")?;
    if head.split(' ').nth(1).is_none_or(|code| code != "200") {
        return None;
    }
    let info: Value = serde_json::from_str(body).ok()?;
    let info = info.as_object()?;
    info.get("atm").is_some_and(truthy).then(|| info.clone())
}

/// Where `needle` starts in `haystack` at or after `from`, ASCII case ignored.
fn find_ignoring_case(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    (from..=haystack.len().checked_sub(needle.len())?)
        .find(|&i| haystack[i..i + needle.len()].eq_ignore_ascii_case(needle))
}

/// The page's inline scripts (those without `src=`), as `<script …>body</script>` finds them.
fn inline_scripts(page: &[u8]) -> Vec<&[u8]> {
    let mut found = Vec::new();
    let mut at = 0;
    while let Some(start) = find_ignoring_case(page, b"<script", at) {
        let after = start + b"<script".len();
        // `\b`: the tag's name ends there
        if page
            .get(after)
            .is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_')
        {
            at = after;
            continue;
        }
        let Some(close) = page[after..].iter().position(|c| *c == b'>') else {
            break;
        };
        let attrs = &page[after..after + close];
        let body_start = after + close + 1;
        let Some(end) = find_ignoring_case(page, b"</script>", body_start) else {
            break;
        };
        if !attrs.windows(4).any(|w| w == b"src=") {
            found.push(&page[body_start..end]);
        }
        at = end + b"</script>".len();
    }
    found
}

/// The hashes of the page's inline scripts, as a CSP names them (`'sha256-…'`).
#[must_use]
pub fn script_hashes(page: &[u8]) -> Vec<String> {
    inline_scripts(page)
        .into_iter()
        .map(|body| format!("'sha256-{}'", base64(&sha256(body))))
        .collect()
}

/// The dashboard's Content-Security-Policy for the page served at `origin` (`http://localhost:<port>`, from the
/// checked Host header): only its own inline scripts run (by `hashes`), and the `CompMD` runtime from the server's asset
/// folder, which may compile `WebAssembly`; its only frames are the `CompMD` sandbox's frames from there, it talks only to
/// this server, and no page may frame it (docs/security-architecture.md I12–I13; `dashboard/tests/e2e/csp.mjs`
/// builds the same policy).
#[must_use]
pub fn page_csp(hashes: &[String], origin: &str) -> String {
    let assets = format!("{origin}{COMPMD_ASSETS}");
    let mut scripts = hashes.to_vec();
    scripts.push("'wasm-unsafe-eval'".into());
    scripts.push(assets.clone());
    [
        "default-src 'none'".to_string(),
        format!("script-src {}", scripts.join(" ")),
        format!("frame-src {assets}"),
        "style-src 'unsafe-inline'".into(),
        "img-src 'self' data:".into(),
        "connect-src 'self'".into(),
        "base-uri 'none'".into(),
        "form-action 'none'".into(),
        "frame-ancestors 'none'".into(),
    ]
    .join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_page_has_its_tags_and_scripts() {
        let (version, build) = page_tags(PAGE);
        assert!(!version.is_empty() && !build.is_empty());
        assert_ne!(script_hashes(PAGE).len(), 0);
    }

    #[test]
    fn inline_scripts_are_found_and_those_with_a_source_skipped() {
        let page = b"<SCRIPT>a()</script><scripts></scripts><script src=x.js></script><script type=module>b</SCRIPT>";
        assert_eq!(inline_scripts(page), [&b"a()"[..], &b"b"[..]]);
        let policy = page_csp(&script_hashes(b"<script></script>"), "http://localhost:1");
        assert!(policy.starts_with(
            "default-src 'none'; script-src 'sha256-47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU=' 'wasm-unsafe-eval' \
             http://localhost:1/assets/compmd/; frame-src http://localhost:1/assets/compmd/;"
        ));
    }

    #[test]
    fn versions_compare_as_numbers() {
        assert!(parse_version("0.10.0") > parse_version("0.9.9"));
        assert_eq!(parse_version("1.x"), [1, 0]);
    }
}
