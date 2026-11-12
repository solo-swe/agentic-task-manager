//! The app's windows (T-511, docs/app-desktop-ui-design.md §1a, §3): `main` and `window-<n>`, each one page with its
//! own subscriptions and links; the window in front (the last focused) is the one deep links and a second instance go
//! to. The page's commands about its window act on the window that calls them, never on one it names.

use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Manager, Runtime, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::commands::{AppError, AppState};
use crate::links::{SCHEME, link_address};

/// The window the app starts with.
pub const MAIN: &str = "main";
/// The label of every other window, before its number (the capability names them `window-*`).
pub const PREFIX: &str = "window-";
/// A window's title is at most this long: the tab's title, never a document.
const MAX_TITLE: usize = 200;

/// What the window has of the platform's own, so the page draws only the rest (§1a): it asks about features, not the
/// OS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub struct Shell {
    /// The system's menu bar has the app's menus (macOS): the page draws none.
    pub system_menu_bar: bool,
    /// The system draws the window's buttons over the page (macOS's traffic lights): the page leaves room for them;
    /// otherwise it draws its own (Windows and Linux, once their windows have no decorations, T-513).
    pub system_window_buttons: bool,
}

/// This platform's shell.
#[must_use]
pub const fn shell_of() -> Shell {
    Shell {
        system_menu_bar: cfg!(target_os = "macos"),
        system_window_buttons: cfg!(target_os = "macos"),
    }
}

/// A title the window can show: no control characters, at most `MAX_TITLE` characters.
#[must_use]
pub fn title_of(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control())
        .take(MAX_TITLE)
        .collect()
}

/// `address` when it is one of the dashboard's pages (`/projects/abc/list?task=T-1`), checked as a deep link is
/// (`link_address`, T-459): else none.
#[must_use]
pub fn page_address(address: &str) -> Option<String> {
    let rest = address.strip_prefix('/')?;
    link_address(&format!("{SCHEME}://{rest}"))
}

/// A new window labelled `label`, as the main window is configured (`tauri.conf.json`), on the dashboard's page.
///
/// # Errors
/// When Tauri can't make it.
pub fn build_window<R: Runtime>(
    app: &AppHandle<R>,
    label: &str,
) -> tauri::Result<WebviewWindow<R>> {
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == MAIN)
        .cloned();
    let window = match config {
        Some(mut config) => {
            label.clone_into(&mut config.label);
            WebviewWindowBuilder::from_config(app, &config)?.build()
        }
        None => {
            WebviewWindowBuilder::new(app, label, WebviewUrl::App("tasks-dashboard.html".into()))
                .build()
        }
    }?;
    dress(&window);
    app.state::<AppState>().session().opened(label);
    Ok(window)
}

/// The window's frame as the page's title bar wants it (T-513, §2): on macOS the configuration's overlay title bar
/// (the system's buttons over the page); on Windows and Linux no decorations, the page draws its own buttons, and
/// Windows 11 gives it rounded corners and a border with a shadow.
pub fn dress<R: Runtime>(window: &WebviewWindow<R>) {
    if !shell_of().system_window_buttons {
        let _ = window.set_decorations(false);
        let _ = window.set_shadow(true);
    }
}

/// The window in front: the last focused, else `main`, else any; none when no window is open.
pub fn front<R: Runtime>(app: &AppHandle<R>) -> Option<WebviewWindow<R>> {
    let state = app.state::<AppState>();
    state
        .front()
        .and_then(|label| app.get_webview_window(&label))
        .or_else(|| app.get_webview_window(MAIN))
        .or_else(|| app.webview_windows().into_values().next())
}

/// The window in front, or a new one when none is open (`main` again if it is gone).
pub fn front_or_new<R: Runtime>(app: &AppHandle<R>) -> Option<WebviewWindow<R>> {
    front(app).or_else(|| {
        let state = app.state::<AppState>();
        let label = if app.get_webview_window(MAIN).is_none() {
            MAIN.to_owned()
        } else {
            state.new_window_label()
        };
        build_window(app, &label).ok()
    })
}

/// Bring `window` to the front.
pub fn focus<R: Runtime>(window: &WebviewWindow<R>) {
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
}

fn failed(e: &tauri::Error) -> AppError {
    AppError::Failed(e.to_string())
}

/// Minimize the calling window (the page's button, on Windows and Linux).
///
/// # Errors
/// When the window can't.
#[tauri::command]
#[specta::specta]
pub async fn minimize<R: Runtime>(window: WebviewWindow<R>) -> Result<(), AppError> {
    window.minimize().map_err(|e| failed(&e))
}

/// Maximize the calling window, or restore it when it is maximized (the page's button, a double click on its bars):
/// Tauri has no toggle.
///
/// # Errors
/// When the window can't.
#[tauri::command]
#[specta::specta]
pub async fn toggle_maximize<R: Runtime>(window: WebviewWindow<R>) -> Result<(), AppError> {
    if window.is_maximized().map_err(|e| failed(&e))? {
        window.unmaximize()
    } else {
        window.maximize()
    }
    .map_err(|e| failed(&e))
}

/// Close the calling window, as a click on the system's close button would (its close handling runs).
///
/// # Errors
/// When the window can't.
#[tauri::command]
#[specta::specta]
pub async fn close_window<R: Runtime>(window: WebviewWindow<R>) -> Result<(), AppError> {
    window.close().map_err(|e| failed(&e))
}

/// Start dragging the calling window (a mouse-down on its bars outside their controls): the app's own command, as
/// Tauri's drag region needs a core permission the page doesn't have (I25).
///
/// # Errors
/// When the window can't.
#[tauri::command]
#[specta::specta]
pub async fn start_dragging<R: Runtime>(window: WebviewWindow<R>) -> Result<(), AppError> {
    window.start_dragging().map_err(|e| failed(&e))
}

/// What the calling window has of the platform's own (§1a).
#[tauri::command]
#[specta::specta]
#[must_use]
pub fn shell() -> Shell {
    shell_of()
}

/// The calling window's title (the window switcher's, the taskbar's): the page's title, as it sets
/// `document.title`.
///
/// # Errors
/// When the window can't take it.
#[tauri::command]
#[specta::specta]
pub async fn set_title<R: Runtime>(
    window: WebviewWindow<R>,
    title: String,
) -> Result<(), AppError> {
    window
        .set_title(&title_of(&title))
        .map_err(|e| AppError::Failed(e.to_string()))
}

/// Open a new window at `address`, one of the dashboard's pages, checked as a deep link is; its page gets the address
/// when it subscribes to its links.
///
/// # Errors
/// An address that isn't one of the dashboard's pages; a window Tauri can't make.
#[tauri::command]
#[specta::specta]
pub async fn open_window<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    address: String,
) -> Result<(), AppError> {
    let address = page_address(&address)
        .ok_or_else(|| AppError::Invalid("not one of the dashboard's pages".into()))?;
    let label = state.new_window_label();
    state.links().send(&label, address);
    build_window(&app, &label)
        .map(|_| ())
        .map_err(|e| AppError::Failed(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shell_is_the_platforms() {
        let shell = shell_of();
        assert_eq!(shell.system_menu_bar, cfg!(target_os = "macos"));
        assert_eq!(shell.system_window_buttons, cfg!(target_os = "macos"));
    }

    #[test]
    fn a_title_has_no_control_characters_and_a_limit() {
        assert_eq!(title_of("atm (2)"), "atm (2)");
        assert_eq!(title_of("a\nb\u{7}c"), "abc");
        assert_eq!(title_of(&"x".repeat(500)).chars().count(), MAX_TITLE);
    }

    #[test]
    fn a_window_opens_only_at_one_of_the_dashboards_pages() {
        for (address, page) in [
            ("/projects/abc/list?task=T-1", "/projects/abc/list?task=T-1"),
            ("/w/work/board", "/w/work/board"),
            ("/", "/"),
        ] {
            assert_eq!(page_address(address).as_deref(), Some(page), "{address}");
        }
        for address in [
            "projects/abc",
            "//example.com/projects",
            "/api/v2/workspaces",
            "/projects/../api",
            "https://example.com",
            "/projects/abc#x",
            "",
        ] {
            assert_eq!(page_address(address), None, "{address}");
        }
    }
}
