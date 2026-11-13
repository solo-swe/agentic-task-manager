//! atm's own files the dashboard loads beside it (the `CompMD` runtime; docs/compmd-rendering-design.md §2, §3c),
//! embedded when atm is built (`build.rs`): read-only, each path segment a plain name, only `.js`, `.wasm` and
//! `.html`. The `CompMD` sandbox's frames have an opaque origin, so every asset may be read from anywhere (they are atm's
//! published files), and a frame document is sandboxed even when opened alone.

use std::collections::HashMap;
use std::sync::OnceLock;

use atm_common::sha256::sha256_hex;
use axum::http::{HeaderMap, Method, StatusCode};
use axum::response::Response;

use super::{send_conditional, send_error};

include!(concat!(env!("OUT_DIR"), "/assets.rs"));

/// Where the assets are served.
pub const PREFIX: &str = "/assets/";

fn plain(segment: &str) -> bool {
    let mut chars = segment.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '@')
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '@' | '-'))
}

fn content_type(name: &str) -> Option<&'static str> {
    match name.rsplit_once('.')?.1 {
        "js" => Some("text/javascript; charset=utf-8"),
        "wasm" => Some("application/wasm"),
        "html" => Some("text/html; charset=utf-8"),
        _ => None,
    }
}

/// The embedded file at `path` (under `/assets/`) with its content type, if the path is one atm serves.
fn find(path: &str) -> Option<(&'static [u8], &'static str)> {
    let segments: Vec<&str> = path.split('/').collect();
    if !segments.iter().all(|s| plain(s)) {
        return None;
    }
    let kind = content_type(segments.last()?)?;
    ASSETS
        .iter()
        .find(|(served, _)| *served == path)
        .map(|(_, bytes)| (*bytes, kind))
}

/// A strong `ETag` per asset, from its bytes, worked out once.
fn etag(path: &str, bytes: &[u8]) -> String {
    static TAGS: OnceLock<std::sync::Mutex<HashMap<String, String>>> = OnceLock::new();
    let tags = TAGS.get_or_init(Default::default);
    let mut tags = tags
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    tags.entry(path.to_string())
        .or_insert_with(|| format!("\"{}\"", &sha256_hex(bytes)[..16]))
        .clone()
}

/// `CompMD`'s bundle (ADR-006), read once from what `build.rs` embedded; none if it can't be read (the build checks
/// it has a manifest, so only a broken build), and then every bundle path is a 404.
fn bundle() -> Option<&'static compmd_host::Bundle> {
    static BUNDLE: OnceLock<Option<compmd_host::Bundle>> = OnceLock::new();
    BUNDLE
        .get_or_init(|| match compmd_host::Bundle::embedded(COMPMD_BUNDLE) {
            Ok(bundle) => Some(bundle),
            Err(e) => {
                eprintln!("atm: CompMD's bundle can't be read: {e}");
                None
            }
        })
        .as_ref()
}

/// A file of `CompMD`'s bundle, `compmd/<version>/compmd/<rest>`: answered by `compmd-host`'s `respond`, the serving
/// contract (`frames/<hash>/…` and `packs/<place>`, `GET` only, each refusal saying why, the headers that sandbox
/// them, cached for good as their paths name their content), as `CompMD`'s plugin answers it in the app.
fn serve_bundle(rest: &str, method: &Method) -> Response {
    let Some(bundle) = bundle() else {
        return send_error(StatusCode::NOT_FOUND, "not found");
    };
    let Ok(request) = axum::http::Request::builder()
        .method(method.clone())
        .uri(format!("/{rest}"))
        .body(())
    else {
        return send_error(StatusCode::NOT_FOUND, "not found");
    };
    compmd_host::respond(bundle, &compmd_host::NoStore, &request).map(axum::body::Body::from)
}

/// The asset at `path` (after `/assets/`): `CompMD`'s bundle by its contract (`serve_bundle`); any other asked again
/// each time (no-cache), a 304 while it is unchanged. Its folder is named for the version, so a page never gets
/// another version's file.
pub fn serve(path: &str, method: &Method, request: &HeaderMap) -> Response {
    if let Some(rest) = path
        .strip_prefix("compmd/")
        .and_then(|p| p.strip_prefix(COMPMD_VERSION))
        .and_then(|p| p.strip_prefix("/compmd/"))
    {
        return serve_bundle(rest, method);
    }
    let Some((bytes, kind)) = find(path) else {
        return send_error(StatusCode::NOT_FOUND, "not found");
    };
    let mut headers = vec![
        ("Cache-Control", "no-cache"),
        ("Access-Control-Allow-Origin", "*"),
    ];
    if kind.starts_with("text/html") {
        headers.push(("Content-Security-Policy", "sandbox allow-scripts"));
    }
    send_conditional(
        request,
        &etag(path, bytes),
        || bytes.to_vec(),
        kind,
        &headers,
    )
}

#[cfg(test)]
mod tests {
    use super::find;

    #[test]
    fn only_plain_paths_to_embedded_files_are_served() {
        let (served, _) = super::ASSETS
            .first()
            .expect("the CompMD runtime is embedded");
        assert!(find(served).is_some());
        for path in [
            "",
            "compmd/../x.js",
            "compmd/%2e%2e/x.js",
            "compmd//compmd.js",
            "compmd/.hidden.js",
            "compmd/missing.js",
        ] {
            assert!(find(path).is_none(), "{path}");
        }
    }
}
