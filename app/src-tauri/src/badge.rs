//! What needs the user, from the app (T-518, T-520; docs/app-desktop-ui-design.md §7): the workspace's needs-you count
//! (in review, blocked on the user, work done together) on the app's icon, and a system notification when a project's
//! count goes up; Settings turns each off (T-517). macOS and Linux show a count (`set_badge_count`; Linux where the
//! desktop shows one); Windows, where Tauri has no count, a small icon of it over the taskbar button
//! (`set_overlay_icon`). The counts and the projects' names are the app's own, read from the user's project list: the
//! page only says which workspace it shows, so no text of the page's reaches a notification.
// The overlay icon is Windows' only; its tests run everywhere
#![cfg_attr(not(target_os = "windows"), allow(dead_code))]

use std::collections::HashMap;
use std::sync::Mutex;

use serde_json::Value;
use tauri::{Manager, Runtime, State, WebviewWindow};
use tauri_plugin_notification::NotificationExt;

use crate::commands::{AppError, AppState, listed_projects};

/// A notification's title: the app's name (T-531).
const TITLE: &str = crate::menu::APP_NAME;
/// A project's name in a notification is at most this long.
const MAX_NAME: usize = 80;

/// A project with tasks that need the user, and how many.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Waiting {
    pub id: String,
    pub name: String,
    pub count: u32,
}

/// How many things need the user in each project of a listing (`list_projects`'s): tasks in review, blocked on the
/// user, and work done together, and code reviews waiting on them (ATM-606: a task in review through one of those is
/// counted once, by atm-lib), as the dashboard counts them (`needsYou`); a missing project has none.
#[must_use]
pub fn waiting_of(projects: &[Value]) -> Vec<Waiting> {
    let count = |n: &Value, key: &str| n.get(key).and_then(Value::as_u64).unwrap_or(0);
    projects
        .iter()
        .filter_map(|p| {
            let n = p.get("summary")?.get("needs_you")?;
            let total = count(n, "review")
                + count(n, "blocked_on_user")
                + count(n, "together")
                + count(n, "code_reviews");
            Some(Waiting {
                id: p.get("id")?.as_str()?.to_owned(),
                name: p
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                count: u32::try_from(total).unwrap_or(u32::MAX),
            })
        })
        .collect()
}

/// What a notification says of a project whose `count` went up.
#[must_use]
pub fn notice(name: &str, count: u32) -> String {
    let name: String = name
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_NAME)
        .collect();
    // Tasks and code reviews alike (ATM-606)
    if count == 1 {
        format!("{name}: 1 thing needs you")
    } else {
        format!("{name}: {count} things need you")
    }
}

/// The counts seen last, per workspace, so a notification says only what is new; and each window's workspace and its
/// total, for the badge.
#[derive(Default)]
pub struct NeedsYou {
    seen: Mutex<HashMap<String, HashMap<String, u32>>>,
    shown: Mutex<Shown>,
}

/// Which workspace each window shows, and each workspace's total as last read.
#[derive(Default)]
struct Shown {
    windows: HashMap<String, String>,
    totals: HashMap<String, u32>,
}

impl NeedsYou {
    /// `window` shows `workspace`, `total` things needing the user: what the app's one badge shows, the totals of the
    /// workspaces the windows show, each workspace once (ATM-677, audit A9: the badge is the app's, not the window's
    /// that asked last).
    pub fn badge_total(&self, window: &str, workspace: &str, total: u32) -> u32 {
        let mut shown = self
            .shown
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        shown
            .windows
            .insert(window.to_owned(), workspace.to_owned());
        shown.totals.insert(workspace.to_owned(), total);
        // A workspace in two windows counts once, as its latest total (the same projects)
        let Shown { windows, totals } = &mut *shown;
        totals.retain(|w, _| windows.values().any(|v| v == w));
        totals.values().fold(0, |sum, n| sum.saturating_add(*n))
    }

    /// `window` closed: its workspace counts no more, unless another window shows it.
    pub fn forget(&self, window: &str) {
        self.shown
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .windows
            .remove(window);
    }
    /// `workspace`'s projects wait as `now` says: what to notify, a notice per project whose count went up since the
    /// last time. The first time for a workspace (the app's start, a switch) only sets what was seen.
    pub fn update(&self, workspace: &str, now: &[Waiting]) -> Vec<String> {
        let mut seen = self
            .seen
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let counts: HashMap<String, u32> = now.iter().map(|w| (w.id.clone(), w.count)).collect();
        let notices = seen.get(workspace).map_or_else(Vec::new, |before| {
            now.iter()
                .filter(|w| w.count > before.get(&w.id).copied().unwrap_or(0))
                .map(|w| notice(&w.name, w.count))
                .collect()
        });
        seen.insert(workspace.to_owned(), counts);
        notices
    }
}

/// The overlay icon's size, in pixels (Windows draws it at 16×16).
const SIZE: u32 = 16;
/// The badge's colour and its text's (RGBA).
const RED: [u8; 4] = [0xd9, 0x30, 0x25, 0xff];
const WHITE: [u8; 4] = [0xff, 0xff, 0xff, 0xff];

/// The count a badge shows: none for nothing (no badge).
#[must_use]
pub fn badge_count(count: Option<u32>) -> Option<i64> {
    count.filter(|n| *n > 0).map(i64::from)
}

/// A 3×5 glyph per character the overlay icon writes, a row per entry, its three bits left to right.
fn glyph(c: char) -> [u8; 5] {
    match c {
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
        '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
        '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
        '7' => [0b111, 0b001, 0b001, 0b001, 0b001],
        '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
        '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
        _ => [0b000, 0b010, 0b111, 0b010, 0b000],
    }
}

/// Windows' overlay icon for `count` (at least 1): a red disc with the count in white, "9+" past nine. Its RGBA
/// pixels, `SIZE` × `SIZE`.
#[must_use]
pub fn overlay_icon(count: u32) -> Vec<u8> {
    let size = SIZE as usize;
    let mut pixels = vec![0u8; size * size * 4];
    let mut put = |x: usize, y: usize, colour: [u8; 4]| {
        if x < size && y < size {
            let at = (y * size + x) * 4;
            pixels[at..at + 4].copy_from_slice(&colour);
        }
    };
    // The disc, in half pixels from its centre (each pixel's middle), so it stays whole numbers
    let diameter = i64::from(SIZE);
    for (y, row) in (0..diameter).zip(0..size) {
        for (x, col) in (0..diameter).zip(0..size) {
            let (dx, dy) = (2 * x + 1 - diameter, 2 * y + 1 - diameter);
            if dx * dx + dy * dy <= diameter * diameter {
                put(col, row, RED);
            }
        }
    }
    // The count: one digit twice the glyphs' size, two characters at their size
    let text = if count > 9 {
        "9+".to_owned()
    } else {
        count.to_string()
    };
    let (scale, gap) = if text.len() == 1 { (2, 0) } else { (1, 1) };
    let width = text.len() * 3 * scale + (text.len() - 1) * gap;
    let (left, top) = ((size - width) / 2, (size - 5 * scale) / 2);
    for (n, c) in text.chars().enumerate() {
        let x0 = left + n * (3 * scale + gap);
        for (row, bits) in glyph(c).iter().enumerate() {
            for col in 0..3 {
                if bits & (0b100 >> col) != 0 {
                    for dy in 0..scale {
                        for dx in 0..scale {
                            put(x0 + col * scale + dx, top + row * scale + dy, WHITE);
                        }
                    }
                }
            }
        }
    }
    pixels
}

/// Show `count` (none, or 0, for no badge) on the app's icon, through `window`.
fn show_badge<R: Runtime>(window: &WebviewWindow<R>, count: Option<u32>) -> tauri::Result<()> {
    #[cfg(target_os = "windows")]
    return window.set_overlay_icon(
        badge_count(count)
            .and(count)
            .map(|n| tauri::image::Image::new_owned(overlay_icon(n), SIZE, SIZE)),
    );
    #[cfg(not(target_os = "windows"))]
    window.set_badge_count(badge_count(count))
}

/// The calling window's page shows `workspace`: the app reads how many tasks need the user in its projects, shows the
/// total on its icon (`badge`), and notifies a project whose count went up (`notify`; not while one of the app's
/// windows is in front, where the user sees it). The page calls it as the workspace's projects change.
///
/// # Errors
/// No such workspace; a badge the platform can't show.
#[tauri::command]
#[specta::specta]
pub async fn needs_you<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
    workspace: String,
    badge: bool,
    notify: bool,
) -> Result<(), AppError> {
    let now = waiting_of(&listed_projects(state.workspaces(), &workspace)?);
    let notices = state.needs_you().update(&workspace, &now);
    let total = now.iter().fold(0u32, |sum, w| sum.saturating_add(w.count));
    let all = state
        .needs_you()
        .badge_total(window.label(), &workspace, total);
    show_badge(&window, badge.then_some(all)).map_err(|e| AppError::Failed(e.to_string()))?;
    let app = window.app_handle();
    let in_front = app
        .webview_windows()
        .values()
        .any(|w| w.is_focused().unwrap_or(false));
    if notify && !in_front {
        for text in notices {
            let _ = app.notification().builder().title(TITLE).body(text).show();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listed(id: &str, name: &str, needs_you: Option<Value>) -> Value {
        serde_json::json!({"id": id, "name": name, "summary": needs_you.map(|n| serde_json::json!({"needs_you": n}))})
    }

    #[test]
    fn the_counts_are_the_dashboards() {
        let projects = [
            listed(
                "a",
                "Alpha",
                // Code reviews waiting on the user count too (ATM-606)
                Some(
                    serde_json::json!({"review": 2, "blocked_on_user": 1, "together": 1, "code_reviews": 2}),
                ),
            ),
            listed(
                "b",
                "Beta",
                Some(serde_json::json!({"review": 0, "blocked_on_user": 0})),
            ),
            // A missing project: no summary
            listed("c", "Gone", None),
        ];
        assert_eq!(
            waiting_of(&projects),
            vec![
                Waiting {
                    id: "a".into(),
                    name: "Alpha".into(),
                    count: 6
                },
                Waiting {
                    id: "b".into(),
                    name: "Beta".into(),
                    count: 0
                },
            ]
        );
    }

    /// A listing as atm may hand it: no projects; an entry without an ID or with one that isn't text (skipped); a
    /// count missing or not a number (none); no name (an empty one); more than a `u32` holds (its largest)
    #[test]
    fn a_listing_of_any_shape_counts_what_it_can() {
        assert_eq!(waiting_of(&[]), Vec::new());
        let projects = [
            serde_json::json!({"name": "No ID", "summary": {"needs_you": {"review": 1}}}),
            serde_json::json!({"id": 7, "summary": {"needs_you": {"review": 1}}}),
            serde_json::json!({"id": "a", "summary": {"needs_you": {"review": "2", "together": 3}}}),
            serde_json::json!({"id": "b", "summary": {"needs_you": {"blocked_on_user": u64::MAX}}}),
            serde_json::json!({"id": "c", "summary": {}}),
        ];
        assert_eq!(
            waiting_of(&projects),
            vec![
                Waiting {
                    id: "a".into(),
                    name: String::new(),
                    count: 3
                },
                Waiting {
                    id: "b".into(),
                    name: String::new(),
                    count: u32::MAX
                },
            ]
        );
    }

    fn waiting(id: &str, count: u32) -> Waiting {
        Waiting {
            id: id.into(),
            name: id.to_uppercase(),
            count,
        }
    }

    #[test]
    fn a_notice_only_for_a_count_that_went_up_and_none_at_the_start() {
        let seen = NeedsYou::default();
        assert_eq!(
            seen.update("default", &[waiting("a", 2)]),
            Vec::<String>::new()
        );
        assert_eq!(
            seen.update("default", &[waiting("a", 2)]),
            Vec::<String>::new()
        );
        assert_eq!(
            seen.update("default", &[waiting("a", 3), waiting("b", 1)]),
            ["A: 3 things need you", "B: 1 thing needs you"]
        );
        assert_eq!(
            seen.update("default", &[waiting("a", 1)]),
            Vec::<String>::new(),
            "fewer"
        );
        // Another workspace starts on its own
        assert_eq!(
            seen.update("work", &[waiting("a", 9)]),
            Vec::<String>::new()
        );
    }

    /// A project gone from the listing and back counts from nothing, as a new one does; a new one with nothing
    /// waiting says nothing
    #[test]
    fn a_project_new_to_the_listing_counts_from_nothing() {
        let seen = NeedsYou::default();
        seen.update("default", &[waiting("a", 2)]);
        assert_eq!(
            seen.update("default", &[waiting("b", 0)]),
            Vec::<String>::new()
        );
        assert_eq!(
            seen.update("default", &[waiting("a", 2), waiting("b", 0)]),
            ["A: 2 things need you"]
        );
    }

    #[test]
    fn a_notice_shows_a_projects_name_without_control_characters_and_cut_short() {
        // At the limit it is whole; its characters, not its bytes, are counted
        let at_limit = "é".repeat(MAX_NAME);
        assert_eq!(
            notice(&at_limit, 1),
            format!("{at_limit}: 1 thing needs you")
        );
        assert_eq!(notice("", 3), ": 3 things need you");
        assert_eq!(notice("App\n\u{7}", 2), "App: 2 things need you");
        assert_eq!(
            notice(&"x".repeat(200), 1).chars().count(),
            MAX_NAME + ": 1 thing needs you".len()
        );
    }

    /// ATM-677, audit A9: the badge is the sum of what the windows show, a workspace in two windows once, a closed
    /// window's no more, and never past what a `u32` holds
    #[test]
    fn the_badge_sums_the_windows_workspaces_each_once() {
        let needs = NeedsYou::default();
        assert_eq!(needs.badge_total("main", "default", 3), 3);
        assert_eq!(needs.badge_total("window-1", "work", 2), 5);
        // Another window on the same workspace: counted once, at its latest total
        assert_eq!(needs.badge_total("window-2", "work", 4), 7);
        // The window that asks shows a smaller count now
        assert_eq!(needs.badge_total("main", "default", 1), 5);
        needs.forget("window-1");
        assert_eq!(
            needs.badge_total("main", "default", 1),
            5,
            "window-2 shows work"
        );
        needs.forget("window-2");
        assert_eq!(needs.badge_total("main", "default", 1), 1);
        // A window switching workspace counts only the new one
        assert_eq!(needs.badge_total("main", "work", 6), 6);
        assert_eq!(needs.badge_total("window-3", "big", u32::MAX), u32::MAX);
    }

    #[test]
    fn nothing_waiting_is_no_badge() {
        assert_eq!(badge_count(None), None);
        assert_eq!(badge_count(Some(0)), None);
        assert_eq!(badge_count(Some(3)), Some(3));
    }

    /// The pixel at (x, y)
    fn at(pixels: &[u8], x: usize, y: usize) -> [u8; 4] {
        let i = (y * SIZE as usize + x) * 4;
        [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
    }

    #[test]
    fn the_overlay_icon_is_a_red_disc_with_the_count() {
        for count in [1, 7, 12, 400] {
            let pixels = overlay_icon(count);
            assert_eq!(pixels.len(), (SIZE * SIZE * 4) as usize);
            // Clear corners, red edge, white text in the middle
            assert_eq!(at(&pixels, 0, 0)[3], 0, "{count}");
            assert_eq!(at(&pixels, 8, 0), RED, "{count}");
            let white = (0..SIZE as usize * SIZE as usize)
                .filter(|i| at(&pixels, i % 16, i / 16) == WHITE)
                .count();
            assert!(white > 5, "{count}: the count is drawn");
        }
        // Past nine, every count looks the same ("9+")
        assert_eq!(overlay_icon(12), overlay_icon(400));
        assert_ne!(overlay_icon(1), overlay_icon(7));
    }

    /// Each count from one to nine draws its own digit, and ten is the first "9+"
    #[test]
    fn each_count_to_nine_looks_its_own() {
        let icons: Vec<Vec<u8>> = (1..=10).map(overlay_icon).collect();
        for (a, first) in icons.iter().enumerate() {
            for (b, second) in icons.iter().enumerate().skip(a + 1) {
                assert_ne!(first, second, "{} and {} look the same", a + 1, b + 1);
            }
        }
        assert_eq!(overlay_icon(10), overlay_icon(u32::MAX));
    }
}
