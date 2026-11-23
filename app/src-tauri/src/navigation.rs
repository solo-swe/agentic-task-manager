//! Where the app's webviews may go, and which of their pages may call the app (ATM-677, audit A6, A10). Tauri counts
//! every scheme the app registers as local, so a window's capability covers the `CompMD` frames (`compmd:`) and atm's
//! own files (`atm:`) as well as the app's page: these checks keep the commands to the page, and the webviews to the
//! app's own origins.
//!
//! - **Navigation:** a webview loads only the app's page, the `CompMD` frames, atm's files and `about:` (an empty or
//!   `srcdoc` frame): the origins the page's policy lets it frame or load. A link elsewhere opens in the user's browser
//!   through `open_url` (`external`), never in the window.
//! - **Invokes:** a command is answered only for a call from the app's page. A call says where it comes from in its
//!   `Origin` header; one from a frame (the `CompMD` sandbox) or from atm's files is refused.

use tauri::{Runtime, Url};

/// The app's page, as each platform serves it (`tauri://localhost`; on Windows and Android `http(s)://tauri.localhost`).
const APP_PAGE: [(&str, &str); 3] = [
    ("tauri", "localhost"),
    ("http", "tauri.localhost"),
    ("https", "tauri.localhost"),
];

/// The other origins a webview loads: the `CompMD` frames and atm's files (each platform's form).
const FRAMES_AND_FILES: [(&str, &str); 4] = [
    ("compmd", "localhost"),
    ("http", "compmd.localhost"),
    ("atm", "localhost"),
    ("http", "atm.localhost"),
];

/// A development build's page, from the dev server on this machine.
fn dev_page(url: &Url) -> bool {
    cfg!(debug_assertions)
        && url.scheme() == "http"
        && matches!(url.host_str(), Some("localhost" | "127.0.0.1"))
}

fn is(url: &Url, origins: &[(&str, &str)]) -> bool {
    origins
        .iter()
        .any(|(scheme, host)| url.scheme() == *scheme && url.host_str() == Some(host))
}

/// Whether a webview may load `url`: the app's page, the `CompMD` frames, atm's files, or `about:`.
#[must_use]
pub fn may_load(url: &Url) -> bool {
    url.scheme() == "about" || is(url, &APP_PAGE) || is(url, &FRAMES_AND_FILES) || dev_page(url)
}

/// Whether a call from `origin` (its `Origin` header, when it has one) may reach a command: only the app's page. A
/// call without one is the app's page's own (Tauri's IPC sends the header for every call from a page).
#[must_use]
pub fn may_invoke(origin: Option<&str>) -> bool {
    let Some(origin) = origin else {
        return true;
    };
    Url::parse(origin).is_ok_and(|url| is(&url, &APP_PAGE) || dev_page(&url))
}

/// `handler`, answering only calls from the app's page ([`may_invoke`]); any other is refused, saying so.
pub fn guarded<R: Runtime>(
    handler: impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static,
) -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static {
    move |invoke| {
        let origin = invoke.message.headers().get("origin");
        if may_invoke(origin.and_then(|o| o.to_str().ok())) {
            handler(invoke)
        } else {
            invoke
                .resolver
                .reject("only the app's page calls the app's commands");
            true
        }
    }
}

/// The guard on every webview's navigation, as a plugin, so the windows made from the configuration have it too.
#[must_use]
pub fn plugin<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("atm-navigation")
        .on_navigation(|_webview, url| {
            let allowed = may_load(url);
            if !allowed {
                eprintln!("atm-app: a webview's navigation to {url} was refused");
            }
            allowed
        })
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(text: &str) -> Url {
        Url::parse(text).expect("a URL")
    }

    #[test]
    fn a_webview_loads_only_the_apps_page_compmds_frames_atms_files_and_about() {
        for allowed in [
            "tauri://localhost/tasks-dashboard.html",
            "tauri://localhost/projects/abc?task=T-1",
            "http://tauri.localhost/tasks-dashboard.html",
            "https://tauri.localhost/",
            "compmd://localhost/frames/abc123/frame.html",
            "http://compmd.localhost/frames/abc123/frame.html",
            "atm://localhost/assets/compmd/0.7.0/compmd.js",
            "http://atm.localhost/assets/compmd/0.7.0/compmd.js",
            "about:blank",
            "about:srcdoc",
        ] {
            assert!(may_load(&url(allowed)), "{allowed}");
        }
        for refused in [
            "https://example.com/",
            "http://example.com/",
            "https://localhost.example.com/",
            "tauri://example.com/",
            "compmd://example.com/",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "data:text/html,<script>alert(1)</script>",
            "blob:tauri://localhost/abc",
            "http://tauri.localhost.example.com/",
        ] {
            assert!(!may_load(&url(refused)), "{refused}");
        }
    }

    #[test]
    fn only_the_apps_page_calls_the_commands() {
        for allowed in [
            None,
            Some("tauri://localhost"),
            Some("http://tauri.localhost"),
            Some("https://tauri.localhost"),
        ] {
            assert!(may_invoke(allowed), "{allowed:?}");
        }
        for refused in [
            "compmd://localhost",
            "http://compmd.localhost",
            "atm://localhost",
            "http://atm.localhost",
            "https://example.com",
            "null",
            "",
            "tauri://example.com",
        ] {
            assert!(!may_invoke(Some(refused)), "{refused}");
        }
    }

    /// A development build's page comes from the dev server; a release build's never does
    #[test]
    fn the_dev_server_is_the_page_only_in_a_development_build() {
        let dev = url("http://localhost:1430/tasks-dashboard.html");
        assert_eq!(may_load(&dev), cfg!(debug_assertions));
        assert_eq!(
            may_invoke(Some("http://127.0.0.1:1430")),
            cfg!(debug_assertions)
        );
        assert!(!may_load(&url("https://localhost:1430/")));
    }
}
