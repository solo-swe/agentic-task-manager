//! `CompMD` in a Tauri app ([ADR-004], `docs/tauri-compmd-design.md` §3–§4): serves `CompMD`'s built folder on a
//! scheme of its own, `compmd:`, with what `CompMD` needs from a host. Tauri puts the window's CSP on every HTML file
//! its own protocol serves, `CompMD`'s sandbox frames too, and answers with its own CORS header; a frame then runs
//! under the window's policy as well as its own, which refuses its `blob:` modules. Files from this scheme are the
//! app's to answer for: the frames keep the policy they were built with.
//!
//! What a host gives `CompMD` (the design's H1 and H2): every file readable from anywhere (the frames have an opaque
//! origin and load their modules with CORS), and a frame document sandboxed, with no policy but its own `<meta>`.
//! The app adds the scheme's origin to its CSP (`compmd://localhost`, and `http://compmd.localhost` on Windows and
//! Android), and gives `CompMD`'s sandbox its `base` on it (`convertFileSrc('', 'compmd')` in `@tauri-apps/api`).
//!
//! ```ignore
//! // build.rs
//! tauri_plugin_compmd::build::embed("../dist/assets/compmd", "assets/compmd");
//! // lib.rs
//! include!(concat!(env!("OUT_DIR"), "/compmd_files.rs"));
//! tauri::Builder::default().plugin(tauri_plugin_compmd::init(COMPMD_FILES))
//! ```
//!
//! [ADR-004]: ../../docs/adr/ADR-004-compmd-in-tauri-through-its-own-protocol.md

use std::borrow::Cow;

use tauri::Runtime;
use tauri::http::{Method, Response, StatusCode, header};
use tauri::plugin::{Builder, TauriPlugin};

pub mod build;

/// The scheme `CompMD`'s files are served on.
pub const SCHEME: &str = "compmd";

/// The files the plugin serves: (path from the scheme's root, without its leading `/`; bytes), as
/// [`build::embed`] writes them.
pub type Files = &'static [(&'static str, &'static [u8])];

/// The plugin: `CompMD`'s `files` on the `compmd:` scheme.
#[must_use]
pub fn init<R: Runtime>(files: Files) -> TauriPlugin<R> {
    Builder::new("compmd")
        .register_uri_scheme_protocol(SCHEME, move |_context, request| {
            let response = respond(files, request.method(), request.uri().path());
            if !response.status().is_success() {
                eprintln!(
                    "tauri-plugin-compmd: {} {} {}",
                    response.status().as_u16(),
                    request.method(),
                    request.uri().path()
                );
            }
            response
        })
        .build()
}

/// The content type of a file `CompMD`'s folder holds, by its extension; none for any other file.
#[must_use]
pub fn content_type(path: &str) -> Option<&'static str> {
    match path.rsplit_once('.')?.1 {
        "js" => Some("text/javascript; charset=utf-8"),
        "wasm" => Some("application/wasm"),
        "html" => Some("text/html; charset=utf-8"),
        _ => None,
    }
}

/// The headers a host sends with a file of `CompMD`'s folder (H1, H2): readable from anywhere, never sniffed, asked
/// again each time, and a frame document sandboxed even when opened alone, with no policy but its own `<meta>`.
#[must_use]
pub fn headers(path: &str) -> Vec<(&'static str, &'static str)> {
    let mut headers = vec![
        ("Access-Control-Allow-Origin", "*"),
        ("X-Content-Type-Options", "nosniff"),
        ("Cache-Control", "no-cache"),
    ];
    if content_type(path).is_some_and(|kind| kind.starts_with("text/html")) {
        headers.push(("Content-Security-Policy", "sandbox allow-scripts"));
    }
    headers
}

/// Whether a path segment is a plain name (letters, digits, `.`, `_`, `@`, `-`; not starting with `.` or `-`).
fn plain(segment: &str) -> bool {
    let mut chars = segment.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '@')
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '@' | '-'))
}

/// The answer to a request for `path`: the embedded file with its headers, or a refusal that says why (`405` for a
/// method other than `GET` and `HEAD`, `404` for a path that isn't one of the files).
#[must_use]
pub fn respond(files: Files, method: &Method, path: &str) -> Response<Cow<'static, [u8]>> {
    if method != Method::GET && method != Method::HEAD {
        return refuse(StatusCode::METHOD_NOT_ALLOWED, "only GET and HEAD");
    }
    let wanted = path.strip_prefix('/').unwrap_or(path);
    let found = (wanted.split('/').all(plain))
        .then(|| content_type(wanted))
        .flatten()
        .and_then(|kind| {
            files
                .iter()
                .find(|(served, _)| *served == wanted)
                .map(|(_, bytes)| (*bytes, kind))
        });
    let Some((bytes, kind)) = found else {
        return refuse(
            StatusCode::NOT_FOUND,
            &format!("not one of CompMD's files: {path}"),
        );
    };
    let mut response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, kind);
    for (name, value) in headers(wanted) {
        response = response.header(name, value);
    }
    let body = if method == Method::HEAD {
        &[][..]
    } else {
        bytes
    };
    response
        .body(Cow::Borrowed(body))
        .unwrap_or_else(|_| Response::new(Cow::Borrowed(&[][..])))
}

fn refuse(status: StatusCode, why: &str) -> Response<Cow<'static, [u8]>> {
    let mut response = Response::new(Cow::Owned(why.as_bytes().to_vec()));
    *response.status_mut() = status;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILES: Files = &[
        ("assets/compmd/1.0.0/compmd.js", b"export {}"),
        ("assets/compmd/1.0.0/compmd/strict.html", b"<!doctype html>"),
        ("assets/compmd/1.0.0/compmd_wasm_bg.wasm", b"\0asm"),
    ];

    fn header_of<'a>(response: &'a Response<Cow<'static, [u8]>>, name: &str) -> Option<&'a str> {
        response.headers().get(name).and_then(|v| v.to_str().ok())
    }

    #[test]
    fn a_frame_document_is_sandboxed_with_no_policy_but_its_own() {
        let response = respond(
            FILES,
            &Method::GET,
            "/assets/compmd/1.0.0/compmd/strict.html",
        );
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.body().as_ref(), b"<!doctype html>");
        assert_eq!(
            header_of(&response, "content-type"),
            Some("text/html; charset=utf-8")
        );
        assert_eq!(
            header_of(&response, "content-security-policy"),
            Some("sandbox allow-scripts")
        );
        assert_eq!(
            header_of(&response, "access-control-allow-origin"),
            Some("*")
        );
        assert_eq!(
            header_of(&response, "x-content-type-options"),
            Some("nosniff")
        );
    }

    #[test]
    fn a_module_is_readable_from_anywhere_and_never_sniffed() {
        for (path, kind) in [
            (
                "/assets/compmd/1.0.0/compmd.js",
                "text/javascript; charset=utf-8",
            ),
            (
                "/assets/compmd/1.0.0/compmd_wasm_bg.wasm",
                "application/wasm",
            ),
        ] {
            let response = respond(FILES, &Method::GET, path);
            assert_eq!(response.status(), StatusCode::OK, "{path}");
            assert_eq!(header_of(&response, "content-type"), Some(kind));
            assert_eq!(
                header_of(&response, "access-control-allow-origin"),
                Some("*")
            );
            assert_eq!(header_of(&response, "cache-control"), Some("no-cache"));
            assert_eq!(header_of(&response, "content-security-policy"), None);
        }
        let head = respond(FILES, &Method::HEAD, "/assets/compmd/1.0.0/compmd.js");
        assert_eq!(head.status(), StatusCode::OK);
        assert!(head.body().is_empty());
    }

    #[test]
    fn anything_else_is_refused_with_a_reason() {
        for path in [
            "/assets/compmd/1.0.0/missing.js",
            "/assets/compmd/1.0.0/../1.0.0/compmd.js",
            "/assets/compmd/1.0.0/.hidden.js",
            "/assets/compmd/1.0.0/compmd.json",
            "/",
            "",
        ] {
            let response = respond(FILES, &Method::GET, path);
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
            let why = String::from_utf8_lossy(response.body());
            assert!(why.contains("not one of CompMD's files"), "{why}");
        }
        let post = respond(FILES, &Method::POST, "/assets/compmd/1.0.0/compmd.js");
        assert_eq!(post.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(String::from_utf8_lossy(post.body()), "only GET and HEAD");
    }
}
