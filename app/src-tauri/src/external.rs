//! The page's external links (T-551, docs/guides/tauri.md): the window's webview opens none itself, so the page hands
//! each one to `open_url`, which checks it and opens it in the user's browser (or mail app) with Tauri's opener plugin,
//! from Rust. The page has no permission of the plugin's (I25): this command is the one way to it.

use tauri::{AppHandle, Runtime, Url};
use tauri_plugin_opener::OpenerExt;

use crate::commands::AppError;

/// The schemes the page may open outside the app: web pages and mail.
const SCHEMES: [&str; 3] = ["https", "http", "mailto"];

/// `url` when the page may open it outside the app: an absolute `https:` or `http:` URL with a host, or a `mailto:`
/// one. Anything else is refused: `file:`, `atm:` (the app's own links go to its router), `javascript:`, `data:`, any
/// other scheme the system would hand to an app.
///
/// # Errors
/// `Invalid` when `url` isn't a URL; `Refused` for any other scheme, or a web URL without a host.
pub fn external_url(url: &str) -> Result<Url, AppError> {
    let parsed =
        Url::parse(url.trim()).map_err(|e| AppError::Invalid(format!("not a URL: {e}")))?;
    if !SCHEMES.contains(&parsed.scheme()) {
        return Err(AppError::Refused(format!(
            "the app opens only https, http and mailto links, not {}:",
            parsed.scheme()
        )));
    }
    if parsed.scheme() != "mailto" && parsed.host_str().is_none_or(str::is_empty) {
        return Err(AppError::Refused("a web link needs a host".into()));
    }
    Ok(parsed)
}

/// Open `url`, one of the page's external links, in the user's browser (a `mailto:` one in their mail app), after
/// checking it (`external_url`).
///
/// # Errors
/// A URL `external_url` refuses; the system not opening it.
#[tauri::command]
#[specta::specta]
pub async fn open_url<R: Runtime>(app: AppHandle<R>, url: String) -> Result<(), AppError> {
    let url = external_url(&url)?;
    app.opener()
        .open_url(url.as_str(), None::<&str>)
        .map_err(|e| AppError::Failed(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_and_mail_links_open() {
        for url in [
            "https://github.com/solo-swe/agentic-task-manager",
            "https://compmd.dev/",
            "http://example.com/a?b=c#d",
            "  https://example.com  ",
            "mailto:someone@example.com",
        ] {
            assert!(external_url(url).is_ok(), "{url}");
        }
    }

    #[test]
    fn every_other_link_is_refused() {
        for url in [
            "file:///etc/passwd",
            "atm://projects/abc",
            "javascript:alert(1)",
            "data:text/html,<p>x</p>",
            "vscode://file/etc/passwd",
            "smb://host/share",
            "tauri://localhost/tasks-dashboard.html",
            "ftp://example.com/",
        ] {
            assert!(
                matches!(external_url(url), Err(AppError::Refused(_))),
                "{url}"
            );
        }
        for url in ["", "/projects/abc", "#top", "example.com", "?task=T-1"] {
            assert!(
                matches!(external_url(url), Err(AppError::Invalid(_))),
                "{url}"
            );
        }
        // A web URL without a host (`http:` alone parses with an empty host only where the URL standard allows it)
        for url in ["http://", "https:///"] {
            assert!(external_url(url).is_err(), "{url}");
        }
    }
}
