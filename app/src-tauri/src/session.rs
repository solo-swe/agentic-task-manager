//! The app's windows come back as they were (T-518, docs/app-desktop-ui-design.md §7): each window's frame (its size
//! and place, maximized or not) and its tabs (their addresses, in order, and the active one). The app keeps them as
//! they change and writes them to `windows.json` in its own data folder; at the start it opens them again. Tauri's
//! window-state plugin keeps frames, not tabs, so this is the app's own.
//!
//! The file is the app's, but anything can write to it: every address in it is checked as a deep link is before a
//! page gets it, a frame off every screen is dropped, and a file atm can't read is ignored (a fresh start).

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, Runtime, State, WebviewWindow};

use crate::commands::{AppError, AppState};
use crate::windows::{self, page_address};

/// The file, in the app's data folder.
pub const FILE: &str = "windows.json";
/// Its format's version.
const VERSION: u32 = 1;
/// At most this many windows come back…
const MAX_WINDOWS: usize = 20;
/// …each with at most this many tabs.
const MAX_TABS: usize = 50;
/// A frame smaller than this, or larger than that, is not one the app made.
const FRAME_SIZES: std::ops::RangeInclusive<u32> = 200..=20_000;
/// How much of a window's top-left corner must be on a screen for it to come back where it was.
const ON_SCREEN: i32 = 100;

/// A window's tabs: their addresses, in order, and which is active.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Tabs {
    /// Each tab's address, one of the dashboard's pages (`/projects/abc/list`)
    pub addresses: Vec<String>,
    /// The active tab's place in `addresses`
    pub active: u32,
}

impl Tabs {
    /// These tabs, checked: only the dashboard's pages (each checked as a deep link is), at most `MAX_TABS`, the active
    /// one still active (else the first); none when no tab is left.
    #[must_use]
    pub fn checked(self) -> Option<Self> {
        let mut addresses = Vec::new();
        let mut active = 0;
        for (at, address) in self.addresses.iter().enumerate() {
            if addresses.len() == MAX_TABS {
                break;
            }
            if let Some(address) = page_address(address) {
                if u32::try_from(at).is_ok_and(|at| at == self.active) {
                    active = addresses.len();
                }
                addresses.push(address);
            }
        }
        (!addresses.is_empty()).then(|| Self {
            addresses,
            active: u32::try_from(active).unwrap_or(0),
        })
    }
}

/// A window's frame, in physical pixels: where its top-left corner is, the size of its content, and whether it is
/// maximized (then the rest is where it goes back to).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Frame {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub maximized: bool,
}

/// A screen's place and size, in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Screen {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Frame {
    /// Whether the window's top-left corner (enough of it to grab) is on one of `screens`.
    #[must_use]
    pub fn on_screen(&self, screens: &[Screen]) -> bool {
        let inside = |s: &Screen| {
            let right = i64::from(s.x) + i64::from(s.width);
            let bottom = i64::from(s.y) + i64::from(s.height);
            i64::from(self.x) >= i64::from(s.x)
                && i64::from(self.y) >= i64::from(s.y)
                && i64::from(self.x) + i64::from(ON_SCREEN) <= right
                && i64::from(self.y) + i64::from(ON_SCREEN) <= bottom
        };
        screens.iter().any(inside)
    }
}

/// A window as it is kept: its tabs and its frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedWindow {
    pub tabs: Tabs,
    pub frame: Option<Frame>,
}

/// The file.
#[derive(Debug, Serialize, Deserialize)]
struct Saved {
    version: u32,
    windows: Vec<SavedWindow>,
}

/// The windows `text` (the file) keeps, checked: their tabs (`Tabs::checked`), a frame only of a size the app makes,
/// at most `MAX_WINDOWS`; none for a file atm can't read or of another version.
#[must_use]
pub fn parse(text: &str) -> Vec<SavedWindow> {
    let Ok(saved) = serde_json::from_str::<Saved>(text) else {
        return Vec::new();
    };
    if saved.version != VERSION {
        return Vec::new();
    }
    saved
        .windows
        .into_iter()
        .filter_map(|w| {
            Some(SavedWindow {
                tabs: w.tabs.checked()?,
                frame: w
                    .frame
                    .filter(|f| FRAME_SIZES.contains(&f.width) && FRAME_SIZES.contains(&f.height)),
            })
        })
        .take(MAX_WINDOWS)
        .collect()
}

/// The file's text for `windows`.
#[must_use]
pub fn text_of(windows: Vec<SavedWindow>) -> String {
    serde_json::to_string_pretty(&Saved {
        version: VERSION,
        windows,
    })
    .unwrap_or_default()
}

/// The windows as they are now, and where they are kept.
#[derive(Default)]
pub struct Session {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    /// The file, once the app knows its data folder
    path: Option<PathBuf>,
    /// The open windows, by label, in the order they opened
    order: Vec<String>,
    /// Each window's tabs, as its page last said
    tabs: HashMap<String, Tabs>,
    /// Each window's frame, as it last moved or changed size
    frames: HashMap<String, Frame>,
    /// The windows being closed (by the user, or the page's last tab): gone for good once destroyed. A window
    /// destroyed without closing (the app quitting) is kept
    closing: HashSet<String>,
    /// The tabs each window comes back with, until its page asks
    restored: HashMap<String, Tabs>,
}

impl Session {
    fn inner(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Keep the windows in `path`.
    pub fn keep_in(&self, path: PathBuf) {
        self.inner().path = Some(path);
    }

    /// `window` opened.
    pub fn opened(&self, window: &str) {
        let mut inner = self.inner();
        if !inner.order.iter().any(|w| w == window) {
            inner.order.push(window.to_owned());
        }
    }

    /// `window`'s page shows `tabs`. Whether that changed what is kept.
    pub fn show(&self, window: &str, tabs: Tabs) -> bool {
        let mut inner = self.inner();
        if !inner.order.iter().any(|w| w == window) {
            inner.order.push(window.to_owned());
        }
        inner.tabs.insert(window.to_owned(), tabs.clone()) != Some(tabs)
    }

    /// `window` moved, changed size, or was maximized or restored. A maximized window keeps the frame it goes back
    /// to.
    pub fn framed(&self, window: &str, frame: Frame) {
        let mut inner = self.inner();
        let kept = match inner.frames.get(window) {
            Some(before) if frame.maximized => Frame {
                maximized: true,
                ..*before
            },
            _ => frame,
        };
        inner.frames.insert(window.to_owned(), kept);
    }

    /// `window` is being closed.
    pub fn closing(&self, window: &str) {
        self.inner().closing.insert(window.to_owned());
    }

    /// `window` is gone: forgotten when it was closed, kept when the app is quitting. Whether it was forgotten.
    pub fn destroyed(&self, window: &str) -> bool {
        let mut inner = self.inner();
        if !inner.closing.remove(window) {
            return false;
        }
        inner.order.retain(|w| w != window);
        inner.tabs.remove(window);
        inner.frames.remove(window);
        inner.restored.remove(window);
        true
    }

    /// `window` comes back with `tabs`, for its page to ask.
    pub fn restore(&self, window: &str, tabs: Tabs) {
        self.inner().restored.insert(window.to_owned(), tabs);
    }

    /// The tabs `window` comes back with, once.
    pub fn restored(&self, window: &str) -> Option<Tabs> {
        self.inner().restored.remove(window)
    }

    /// The windows to keep, in the order they opened: those whose page has said what it shows.
    #[must_use]
    pub fn saved(&self) -> Vec<SavedWindow> {
        let inner = self.inner();
        inner
            .order
            .iter()
            .filter_map(|w| {
                Some(SavedWindow {
                    tabs: inner.tabs.get(w)?.clone(),
                    frame: inner.frames.get(w).copied(),
                })
            })
            .collect()
    }

    /// Write the windows to the file (through a file beside it, so a crash leaves the old one); nothing before the app
    /// knows where.
    ///
    /// # Errors
    /// When the file can't be written.
    pub fn write(&self) -> std::io::Result<()> {
        let Some(path) = self.inner().path.clone() else {
            return Ok(());
        };
        if let Some(folder) = path.parent() {
            std::fs::create_dir_all(folder)?;
        }
        let next = path.with_extension("json.next");
        std::fs::write(&next, text_of(self.saved()))?;
        std::fs::rename(&next, &path)
    }
}

/// `window`'s frame now; none when Tauri can't say.
pub fn frame_of<R: Runtime>(window: &WebviewWindow<R>) -> Option<Frame> {
    let place = window.outer_position().ok()?;
    let size = window.inner_size().ok()?;
    Some(Frame {
        x: place.x,
        y: place.y,
        width: size.width,
        height: size.height,
        maximized: window.is_maximized().unwrap_or(false),
    })
}

/// Bring the windows back (at the start): the first in the main window, the others in new ones, each at its frame
/// when that is on a screen, its tabs waiting for its page.
pub fn restore<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<AppState>();
    let session = state.session();
    let Ok(folder) = app.path().app_data_dir() else {
        return;
    };
    let path = folder.join(FILE);
    let saved = std::fs::read_to_string(&path)
        .map(|text| parse(&text))
        .unwrap_or_default();
    session.keep_in(path);
    let screens: Vec<Screen> = app
        .available_monitors()
        .unwrap_or_default()
        .iter()
        .map(|m| Screen {
            x: m.position().x,
            y: m.position().y,
            width: m.size().width,
            height: m.size().height,
        })
        .collect();
    for (at, kept) in saved.into_iter().enumerate() {
        let window = if at == 0 {
            app.get_webview_window(windows::MAIN)
        } else {
            windows::build_window(app, &state.new_window_label()).ok()
        };
        let Some(window) = window else {
            continue;
        };
        let label = window.label().to_owned();
        session.opened(&label);
        session.restore(&label, kept.tabs.clone());
        session.show(&label, kept.tabs);
        if let Some(frame) = kept.frame.filter(|f| f.on_screen(&screens)) {
            let _ = window.set_position(PhysicalPosition::new(frame.x, frame.y));
            let _ = window.set_size(PhysicalSize::new(frame.width, frame.height));
            session.framed(
                &label,
                Frame {
                    maximized: false,
                    ..frame
                },
            );
            if frame.maximized {
                let _ = window.maximize();
                session.framed(&label, frame);
            }
        }
    }
}

/// The calling window's page shows `tabs` (it says so whenever they change): kept for the next start.
///
/// # Errors
/// Tabs none of which is one of the dashboard's pages; a file that can't be written.
#[tauri::command]
#[specta::specta]
pub async fn save_tabs<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
    tabs: Tabs,
) -> Result<(), AppError> {
    let tabs = tabs
        .checked()
        .ok_or_else(|| AppError::Invalid("no tab is one of the dashboard's pages".into()))?;
    if state.session().show(window.label(), tabs) {
        state
            .session()
            .write()
            .map_err(|e| AppError::Failed(e.to_string()))?;
    }
    Ok(())
}

/// The tabs the calling window comes back with (the app's start), once; none for a new window or a reload.
///
/// # Errors
/// None: a command with the app's state answers a result.
#[tauri::command]
#[specta::specta]
pub async fn restored_tabs<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
) -> Result<Option<Tabs>, AppError> {
    Ok(state.session().restored(window.label()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tabs(addresses: &[&str], active: u32) -> Tabs {
        Tabs {
            addresses: addresses.iter().map(|a| (*a).to_owned()).collect(),
            active,
        }
    }

    const FRAME: Frame = Frame {
        x: 40,
        y: 60,
        width: 1200,
        height: 800,
        maximized: false,
    };

    #[test]
    fn tabs_keep_only_the_dashboards_pages_and_their_active_one() {
        let kept = tabs(&["/projects/abc", "https://example.com", "/board"], 2).checked();
        assert_eq!(kept, Some(tabs(&["/projects/abc", "/board"], 1)));
        // The active one gone: the first is
        let kept = tabs(&["/projects/abc", "/api/v2/workspaces", "/board"], 1).checked();
        assert_eq!(kept, Some(tabs(&["/projects/abc", "/board"], 0)));
        // Out of range: the first
        assert_eq!(tabs(&["/board"], 7).checked(), Some(tabs(&["/board"], 0)));
        assert_eq!(tabs(&["/api/x", "//example.com"], 0).checked(), None);
        assert_eq!(tabs(&[], 0).checked(), None);
        let many: Vec<String> = (0..80).map(|n| format!("/projects/p{n}")).collect();
        let many = Tabs {
            addresses: many,
            active: 70,
        }
        .checked()
        .expect("tabs");
        assert_eq!((many.addresses.len(), many.active), (MAX_TABS, 0));
    }

    #[test]
    fn the_file_is_written_and_read_back() {
        let windows = vec![
            SavedWindow {
                tabs: tabs(&["/projects/abc/list?task=T-1", "/"], 1),
                frame: Some(FRAME),
            },
            SavedWindow {
                tabs: tabs(&["/w/work/board"], 0),
                frame: None,
            },
        ];
        assert_eq!(parse(&text_of(windows.clone())), windows);
    }

    #[test]
    fn a_bad_file_is_a_fresh_start_and_bad_entries_are_dropped() {
        for text in [
            "",
            "not json",
            "[]",
            r#"{"version": 2, "windows": []}"#,
            r#"{"windows": 3}"#,
        ] {
            assert_eq!(parse(text), Vec::new(), "{text}");
        }
        let text = r#"{"version": 1, "windows": [
            {"tabs": {"addresses": ["/projects/../api"], "active": 0}, "frame": null},
            {"tabs": {"addresses": ["/board", "atm://x"], "active": 0},
             "frame": {"x": 0, "y": 0, "width": 5, "height": 99999, "maximized": false}}
        ]}"#;
        assert_eq!(
            parse(text),
            vec![SavedWindow {
                tabs: tabs(&["/board"], 0),
                frame: None,
            }]
        );
        let many = text_of(
            (0..30)
                .map(|_| SavedWindow {
                    tabs: tabs(&["/board"], 0),
                    frame: None,
                })
                .collect(),
        );
        assert_eq!(parse(&many).len(), MAX_WINDOWS);
    }

    #[test]
    fn a_frame_comes_back_only_on_a_screen() {
        let screens = [
            Screen {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            },
            Screen {
                x: -1440,
                y: 0,
                width: 1440,
                height: 900,
            },
        ];
        assert!(FRAME.on_screen(&screens));
        assert!(Frame { x: -1000, ..FRAME }.on_screen(&screens));
        assert!(
            !Frame { x: 1900, ..FRAME }.on_screen(&screens),
            "its corner off the right edge"
        );
        assert!(!Frame { y: 5000, ..FRAME }.on_screen(&screens));
        assert!(
            !Frame { x: -3000, ..FRAME }.on_screen(&screens),
            "a screen no longer there"
        );
        assert!(!FRAME.on_screen(&[]));
    }

    #[test]
    fn windows_are_kept_in_order_until_closed_and_kept_when_the_app_quits() {
        let session = Session::default();
        session.opened("main");
        session.opened("window-1");
        session.opened("window-2");
        assert!(session.show("main", tabs(&["/"], 0)));
        assert!(
            !session.show("main", tabs(&["/"], 0)),
            "the same tabs again"
        );
        assert!(session.show("window-2", tabs(&["/board"], 0)));
        assert!(session.show("window-1", tabs(&["/projects/abc"], 0)));
        session.framed("window-1", FRAME);
        // Maximized: it keeps the frame it goes back to
        session.framed(
            "window-1",
            Frame {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
                maximized: true,
            },
        );
        let saved = session.saved();
        assert_eq!(
            saved
                .iter()
                .map(|w| w.tabs.addresses[0].as_str())
                .collect::<Vec<_>>(),
            ["/", "/projects/abc", "/board"]
        );
        assert_eq!(
            saved[1].frame,
            Some(Frame {
                maximized: true,
                ..FRAME
            })
        );
        // Closed by the user: forgotten
        session.closing("window-2");
        assert!(session.destroyed("window-2"));
        // Destroyed as the app quits: kept
        assert!(!session.destroyed("window-1"));
        assert_eq!(session.saved().len(), 2);
    }

    #[test]
    fn a_window_comes_back_with_its_tabs_once() {
        let session = Session::default();
        session.restore("main", tabs(&["/board", "/"], 1));
        assert_eq!(session.restored("main"), Some(tabs(&["/board", "/"], 1)));
        assert_eq!(
            session.restored("main"),
            None,
            "a reload starts from its own"
        );
        assert_eq!(session.restored("window-1"), None);
    }

    #[test]
    fn the_file_is_written_where_the_app_keeps_it() {
        let folder = std::env::temp_dir().join(format!("atm-app-session-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        let session = Session::default();
        session.show("main", tabs(&["/board"], 0));
        session.write().expect("nowhere yet: nothing written");
        session.keep_in(folder.join(FILE));
        session.write().expect("written");
        let text = std::fs::read_to_string(folder.join(FILE)).expect("the file");
        assert_eq!(parse(&text), session.saved());
        assert!(!folder.join("windows.json.next").exists());
        std::fs::remove_dir_all(&folder).expect("clean up");
    }
}
