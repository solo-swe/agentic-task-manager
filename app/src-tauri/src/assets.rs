//! The app's own asset scheme, `atm:` (ADR-004, T-503): what atm ships for its page and Tauri's protocol mustn't
//! serve, as `atm serve` serves it at `/assets/compmd/<version>/`: `compmd.js` (`CompMD`'s processor and sandbox), its
//! `WebAssembly`, and the packs' built modules, which the page fetches and gives the sandbox as code. Nothing else: no
//! HTML (the sandbox's frames come from `CompMD`'s plugin, on `compmd:`), nothing outside the build `build.rs`
//! embedded.
//!
//! Its origin is `atm://localhost` (`http://atm.localhost` on Windows and Android). The deep links (`links`) share the
//! scheme but never meet it: macOS hands those to the app (`RunEvent::Opened`), while this handler answers only the
//! window's own loads, and `link_address` opens no `atm://localhost/…` link.

use std::borrow::Cow;

use tauri::http::{Method, Response, StatusCode, header};

/// The scheme the app serves its assets on (the deep links' too: `links::SCHEME`).
pub const SCHEME: &str = crate::links::SCHEME;

/// The files served: (path from the scheme's root, without its leading `/`; bytes), as `build.rs` embeds them.
pub type Files = &'static [(&'static str, &'static [u8])];

/// The content type of a file served, by its extension; none for any other.
#[must_use]
pub fn content_type(path: &str) -> Option<&'static str> {
    match path.rsplit_once('.')?.1 {
        "js" => Some("text/javascript; charset=utf-8"),
        "wasm" => Some("application/wasm"),
        _ => None,
    }
}

/// Whether a path segment is a plain name (letters, digits, `.`, `_`, `@`, `-`; not starting with `.` or `-`).
fn plain(segment: &str) -> bool {
    let mut chars = segment.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '@')
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '@' | '-'))
}

/// The answer to a request for `path`: the embedded file, readable from the page's origin (`tauri://localhost` is
/// another origin: the page imports and fetches these with CORS), never sniffed, asked again each time; or a refusal
/// that says why (`405` for a method other than `GET` and `HEAD`, `404` for a path that isn't one of the files).
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
            &format!("not one of atm's assets: {path}"),
        );
    };
    let body = if method == Method::HEAD {
        &[][..]
    } else {
        bytes
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, kind)
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .header(header::CACHE_CONTROL, "no-cache")
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
        ("assets/compmd/1.0.0/compmd_wasm_bg.wasm", b"\0asm"),
        (
            "assets/compmd/1.0.0/compmd/packs/@kit/mermaid@1.0.0/dist/mermaid.js",
            b"export {}",
        ),
    ];

    fn header_of<'a>(response: &'a Response<Cow<'static, [u8]>>, name: &str) -> Option<&'a str> {
        response.headers().get(name).and_then(|v| v.to_str().ok())
    }

    #[test]
    fn a_file_is_readable_from_the_page_never_sniffed_and_asked_again() {
        for (path, kind) in [
            (
                "/assets/compmd/1.0.0/compmd.js",
                "text/javascript; charset=utf-8",
            ),
            (
                "/assets/compmd/1.0.0/compmd_wasm_bg.wasm",
                "application/wasm",
            ),
            (
                "/assets/compmd/1.0.0/compmd/packs/@kit/mermaid@1.0.0/dist/mermaid.js",
                "text/javascript; charset=utf-8",
            ),
        ] {
            let response = respond(FILES, &Method::GET, path);
            assert_eq!(response.status(), StatusCode::OK, "{path}");
            assert_eq!(header_of(&response, "content-type"), Some(kind));
            assert_eq!(
                header_of(&response, "access-control-allow-origin"),
                Some("*")
            );
            assert_eq!(
                header_of(&response, "x-content-type-options"),
                Some("nosniff")
            );
            assert_eq!(header_of(&response, "cache-control"), Some("no-cache"));
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
            "/assets/compmd/1.0.0/compmd/strict.html",
            "/projects/abc/list",
            "/",
            "",
        ] {
            let response = respond(FILES, &Method::GET, path);
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
            let why = String::from_utf8_lossy(response.body());
            assert!(why.contains("not one of atm's assets"), "{why}");
        }
        let post = respond(FILES, &Method::POST, "/assets/compmd/1.0.0/compmd.js");
        assert_eq!(post.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(String::from_utf8_lossy(post.body()), "only GET and HEAD");
    }

    /// The scheme is the deep links' too, but no asset address opens a page
    #[test]
    fn no_asset_address_is_a_deep_link() {
        for (path, _) in FILES {
            assert_eq!(
                crate::links::link_address(&format!("atm://localhost/{path}")),
                None
            );
        }
        assert_eq!(crate::links::link_address("atm://localhost"), None);
    }
}
