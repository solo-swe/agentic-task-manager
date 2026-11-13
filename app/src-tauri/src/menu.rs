//! The app's menus (T-512, docs/app-desktop-ui-design.md §6): one definition, here, for every platform. On macOS it is
//! the menu bar; on Windows and Linux it is attached to each window and hidden, and the page draws the same menus from
//! the `menu` command (T-513). An item sends the window in front a [`MenuAction`] on its channel (`subscribe_menu`);
//! the page does what the same control does. Quit and New Window the app does itself. What the menu shows checked (the
//! view, the sidebar, the filters, archived tasks) the page reports with `menu_state`.

use std::collections::HashMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use specta::Type;
use tauri::ipc::Channel;
use tauri::menu::{
    CheckMenuItem, IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu, WINDOW_SUBMENU_ID,
};
use tauri::{AppHandle, Manager, Runtime, State, WebviewWindow};

use crate::commands::{AppError, AppState};

/// The name users see (T-531): the menus, the window, the notifications, macOS's menu bar (`Info.plist`'s
/// `CFBundleName`) and the bundle's file (`productName`: `Agentic Task Manager.app`, which the Dock and ⌘-Tab show,
/// T-537). The command stays `atm`, and the app's program `atm-app`.
pub const APP_NAME: &str = "Agentic Task Manager";

/// What a menu item asks for: the page does it (all but `quit` and `new_window`, which the app does).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum MenuAction {
    About,
    CheckForUpdates,
    Settings,
    ConnectAgents,
    NewTab,
    NewWindow,
    CloseTab,
    CopyLink,
    Find,
    ShowGraph,
    ShowKanban,
    ShowList,
    ShowAgents,
    ShowProgress,
    ShowProject,
    ToggleSidebar,
    ToggleFilters,
    ToggleArchived,
    Reload,
    Back,
    Forward,
    Overview,
    /// Open a project of the workspace, by its ID
    Project(String),
    /// Open a workspace, by its ID
    Workspace(String),
    NextTab,
    PreviousTab,
    MoveTabToNewWindow,
    Help,
    WhatsNew,
    #[serde(rename = "github")]
    GitHub,
    #[serde(rename = "compmd")]
    CompMd,
    Quit,
}

/// The actions without an ID, every one of them (the tests check each has an item).
pub const ACTIONS: [MenuAction; 30] = [
    MenuAction::About,
    MenuAction::CheckForUpdates,
    MenuAction::Settings,
    MenuAction::ConnectAgents,
    MenuAction::NewTab,
    MenuAction::NewWindow,
    MenuAction::CloseTab,
    MenuAction::CopyLink,
    MenuAction::Find,
    MenuAction::ShowGraph,
    MenuAction::ShowKanban,
    MenuAction::ShowList,
    MenuAction::ShowAgents,
    MenuAction::ShowProgress,
    MenuAction::ShowProject,
    MenuAction::ToggleSidebar,
    MenuAction::ToggleFilters,
    MenuAction::ToggleArchived,
    MenuAction::Reload,
    MenuAction::Back,
    MenuAction::Forward,
    MenuAction::Overview,
    MenuAction::NextTab,
    MenuAction::PreviousTab,
    MenuAction::MoveTabToNewWindow,
    MenuAction::Help,
    MenuAction::WhatsNew,
    MenuAction::GitHub,
    MenuAction::CompMd,
    MenuAction::Quit,
];

impl MenuAction {
    /// The menu item's ID: the action's name, with the project's or workspace's ID after a colon.
    #[must_use]
    pub fn id(&self) -> String {
        let value = serde_json::to_value(self).unwrap_or(Value::Null);
        let kind = value["kind"].as_str().unwrap_or_default();
        match value["id"].as_str() {
            Some(id) => format!("{kind}:{id}"),
            None => kind.to_owned(),
        }
    }

    /// The action a menu item's ID stands for; none for the system's own items.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        let value = match id.split_once(':') {
            Some((kind, id)) => serde_json::json!({"kind": kind, "id": id}),
            None => serde_json::json!({"kind": id}),
        };
        serde_json::from_value(value).ok()
    }
}

/// The system's own items, which the OS (macOS) or the page (Windows, Linux) carries out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum SystemItem {
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
    Services,
    Hide,
    HideOthers,
    ShowAll,
    Minimize,
    Zoom,
    Fullscreen,
}

/// One entry of a menu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MenuEntry {
    /// An item of the app's: its action, label, shortcut (`CmdOrCtrl+T`) and, for a toggle, whether it is checked
    Item {
        action: MenuAction,
        label: String,
        accelerator: Option<String>,
        checked: Option<bool>,
    },
    /// One of the system's own
    System {
        item: SystemItem,
    },
    Separator,
    /// A menu inside the menu
    Submenu {
        label: String,
        entries: Vec<MenuEntry>,
    },
}

/// A menu of the bar: File, Edit, …
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct MenuDef {
    pub label: String,
    /// macOS's Window menu, which lists the open windows
    pub windows: bool,
    pub entries: Vec<MenuEntry>,
}

/// What the page reports for its window, so the menu shows it checked.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Type)]
#[serde(default)]
pub struct MenuState {
    /// The view shown (`kanban`, `list`, …), if a project's page is open
    pub view: Option<String>,
    pub sidebar: bool,
    pub filters: bool,
    pub archived: bool,
    /// The workspace the window shows: its projects are the Go menu's
    pub workspace: Option<String>,
}

/// The platforms whose conventions differ (§1a).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Mac,
    Windows,
    Linux,
}

impl Platform {
    /// This one.
    #[must_use]
    pub const fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::Mac
        } else if cfg!(windows) {
            Self::Windows
        } else {
            Self::Linux
        }
    }
}

/// What the Go menu lists: the workspaces, and the shown workspace's projects (ID, name).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Places {
    pub workspaces: Vec<String>,
    pub projects: Vec<(String, String)>,
}

fn item(action: MenuAction, label: &str, accelerator: Option<&str>) -> MenuEntry {
    MenuEntry::Item {
        action,
        label: label.to_owned(),
        accelerator: accelerator.map(str::to_owned),
        checked: None,
    }
}

fn toggle(action: MenuAction, label: &str, accelerator: Option<&str>, on: bool) -> MenuEntry {
    MenuEntry::Item {
        action,
        label: label.to_owned(),
        accelerator: accelerator.map(str::to_owned),
        checked: Some(on),
    }
}

const fn system(item: SystemItem) -> MenuEntry {
    MenuEntry::System { item }
}

fn bar_menu(label: &str, entries: Vec<MenuEntry>) -> MenuDef {
    MenuDef {
        label: label.to_owned(),
        windows: false,
        entries,
    }
}

/// The menus, for `platform`, showing `state` checked and `places` in the Go menu (§6's table).
#[must_use]
#[allow(clippy::too_many_lines)] // reason: the table of §6, one menu after the other
pub fn definition(platform: Platform, state: &MenuState, places: &Places) -> Vec<MenuDef> {
    use MenuAction as A;
    let mac = platform == Platform::Mac;
    let pick =
        |on_mac: &'static str, elsewhere: &'static str| Some(if mac { on_mac } else { elsewhere });
    let view = |name: &str| state.view.as_deref() == Some(name);
    let mut menus = Vec::new();
    if mac {
        menus.push(bar_menu(
            APP_NAME,
            vec![
                item(A::About, &format!("About {APP_NAME}"), None),
                item(A::CheckForUpdates, "Check for Updates…", None),
                MenuEntry::Separator,
                item(A::Settings, "Settings…", Some("CmdOrCtrl+Comma")),
                item(A::ConnectAgents, "Connect Your Agents…", None),
                MenuEntry::Separator,
                system(SystemItem::Services),
                MenuEntry::Separator,
                system(SystemItem::Hide),
                system(SystemItem::HideOthers),
                system(SystemItem::ShowAll),
                MenuEntry::Separator,
                item(A::Quit, &format!("Quit {APP_NAME}"), Some("CmdOrCtrl+Q")),
            ],
        ));
    }
    let mut file = vec![
        item(A::NewTab, "New Tab", Some("CmdOrCtrl+T")),
        item(A::NewWindow, "New Window", Some("CmdOrCtrl+N")),
        item(A::CloseTab, "Close Tab", Some("CmdOrCtrl+W")),
        MenuEntry::Separator,
        item(
            A::CopyLink,
            "Copy Link to This Page",
            Some("Alt+CmdOrCtrl+C"),
        ),
    ];
    if !mac {
        file.extend([
            MenuEntry::Separator,
            item(A::Settings, "Settings…", Some("CmdOrCtrl+Comma")),
            item(A::ConnectAgents, "Connect Your Agents…", None),
            MenuEntry::Separator,
            // Windows quits with the system's Alt+F4
            item(
                A::Quit,
                "Exit",
                (platform == Platform::Linux).then_some("Ctrl+Q"),
            ),
        ]);
    }
    menus.push(bar_menu("File", file));
    menus.push(bar_menu(
        "Edit",
        vec![
            system(SystemItem::Undo),
            system(SystemItem::Redo),
            MenuEntry::Separator,
            system(SystemItem::Cut),
            system(SystemItem::Copy),
            system(SystemItem::Paste),
            system(SystemItem::SelectAll),
            MenuEntry::Separator,
            item(A::Find, "Find", Some("CmdOrCtrl+F")),
        ],
    ));
    let mut view_menu = vec![
        toggle(
            A::ShowGraph,
            "Graph",
            Some("CmdOrCtrl+Digit1"),
            view("graph"),
        ),
        toggle(
            A::ShowKanban,
            "Kanban",
            Some("CmdOrCtrl+Digit2"),
            view("kanban"),
        ),
        toggle(A::ShowList, "List", Some("CmdOrCtrl+Digit3"), view("list")),
        toggle(
            A::ShowAgents,
            "Agents",
            Some("CmdOrCtrl+Digit4"),
            view("agents"),
        ),
        toggle(
            A::ShowProgress,
            "Progress",
            Some("CmdOrCtrl+Digit5"),
            view("progress"),
        ),
        toggle(
            A::ShowProject,
            "Project",
            Some("CmdOrCtrl+Digit6"),
            view("project"),
        ),
        MenuEntry::Separator,
        toggle(
            A::ToggleSidebar,
            "Show Sidebar",
            pick("Ctrl+CmdOrCtrl+S", "Ctrl+B"),
            state.sidebar,
        ),
        toggle(
            A::ToggleFilters,
            "Show Filters",
            Some("Alt+CmdOrCtrl+F"),
            state.filters,
        ),
        toggle(
            A::ToggleArchived,
            "Include Archived Tasks",
            None,
            state.archived,
        ),
        MenuEntry::Separator,
        item(A::Reload, "Reload", Some("CmdOrCtrl+R")),
    ];
    if mac {
        view_menu.extend([MenuEntry::Separator, system(SystemItem::Fullscreen)]);
    }
    menus.push(bar_menu("View", view_menu));
    let mut go = vec![
        item(
            A::Back,
            "Back",
            pick("CmdOrCtrl+BracketLeft", "Alt+ArrowLeft"),
        ),
        item(
            A::Forward,
            "Forward",
            pick("CmdOrCtrl+BracketRight", "Alt+ArrowRight"),
        ),
        MenuEntry::Separator,
        item(A::Overview, "Projects Overview", Some("Shift+CmdOrCtrl+O")),
    ];
    if !places.projects.is_empty() {
        go.push(MenuEntry::Separator);
        go.extend(
            places
                .projects
                .iter()
                .map(|(id, name)| item(A::Project(id.clone()), name, None)),
        );
    }
    if !places.workspaces.is_empty() {
        go.extend([
            MenuEntry::Separator,
            MenuEntry::Submenu {
                label: "Workspace".into(),
                entries: places
                    .workspaces
                    .iter()
                    .map(|id| {
                        toggle(
                            A::Workspace(id.clone()),
                            id,
                            None,
                            state.workspace.as_deref() == Some(id),
                        )
                    })
                    .collect(),
            },
        ]);
    }
    menus.push(bar_menu("Go", go));
    let mut window = vec![
        system(SystemItem::Minimize),
        system(SystemItem::Zoom),
        MenuEntry::Separator,
        item(A::NextTab, "Next Tab", Some("Ctrl+Tab")),
        item(A::PreviousTab, "Previous Tab", Some("Ctrl+Shift+Tab")),
        item(A::MoveTabToNewWindow, "Move Tab to New Window", None),
    ];
    if !mac {
        window.remove(1); // Zoom is macOS's
    }
    menus.push(MenuDef {
        label: "Window".into(),
        windows: mac,
        entries: window,
    });
    let mut help = vec![
        item(
            A::Help,
            &format!("{APP_NAME} Help"),
            pick("CmdOrCtrl+Shift+Slash", "F1"),
        ),
        item(A::WhatsNew, "What's New", None),
        MenuEntry::Separator,
        item(A::GitHub, &format!("{APP_NAME} on GitHub"), None),
        item(A::CompMd, "CompMD", None),
    ];
    if !mac {
        help.extend([
            MenuEntry::Separator,
            item(A::About, &format!("About {APP_NAME}"), None),
            item(A::CheckForUpdates, "Check for Updates…", None),
        ]);
    }
    menus.push(bar_menu("Help", help));
    menus
}

/// The Go menu's places: the workspaces, and the projects of `workspace` (else the one opened last).
#[must_use]
pub fn places_of(workspaces: &atm_lib::Workspaces, workspace: Option<&str>) -> Places {
    let summary = workspaces.summary();
    let ids: Vec<String> = summary["workspaces"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|w| w["id"].as_str().map(str::to_owned))
        .collect();
    let shown = workspace
        .map(str::to_owned)
        .or_else(|| summary["recent"].as_str().map(str::to_owned));
    let projects = shown
        .and_then(|id| workspaces.get(&id))
        .map(|found| {
            let listing = serde_json::json!(found.listing().projects());
            listing
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|p| {
                    let id = p["id"].as_str()?;
                    let name = p["name"].as_str().unwrap_or(id);
                    Some((id.to_owned(), name.to_owned()))
                })
                .collect()
        })
        .unwrap_or_default();
    Places {
        workspaces: ids,
        projects,
    }
}

/// Each window's page's menu channel, and what its menu shows.
type ByWindow = (
    HashMap<String, Channel<MenuAction>>,
    HashMap<String, MenuState>,
);

/// The page's menu channels and states, by window.
#[derive(Default)]
pub struct Menus {
    inner: Mutex<ByWindow>,
}

impl Menus {
    fn inner(&self) -> std::sync::MutexGuard<'_, ByWindow> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// `window`'s page listens on `channel`.
    pub fn subscribe(&self, window: &str, channel: Channel<MenuAction>) {
        self.inner().0.insert(window.to_owned(), channel);
    }

    /// Send `action` to `window`'s page. Whether it listens.
    pub fn send(&self, window: &str, action: MenuAction) -> bool {
        self.inner()
            .0
            .get(window)
            .is_some_and(|channel| channel.send(action).is_ok())
    }

    /// `window`'s page reports what its menu shows.
    pub fn set_state(&self, window: &str, state: MenuState) {
        self.inner().1.insert(window.to_owned(), state);
    }

    /// What `window`'s menu shows (nothing checked before its page reports).
    #[must_use]
    pub fn state(&self, window: &str) -> MenuState {
        self.inner().1.get(window).cloned().unwrap_or_default()
    }

    /// `window` closed.
    pub fn forget(&self, window: &str) {
        let mut inner = self.inner();
        inner.0.remove(window);
        inner.1.remove(window);
    }
}

fn add<R: Runtime>(
    app: &AppHandle<R>,
    entries: &[MenuEntry],
    to: &dyn Fn(&dyn IsMenuItem<R>) -> tauri::Result<()>,
) -> tauri::Result<()> {
    for entry in entries {
        match entry {
            MenuEntry::Item {
                action,
                label,
                accelerator,
                checked: None,
            } => to(&MenuItem::with_id(
                app,
                action.id(),
                label,
                true,
                accelerator.as_deref(),
            )?)?,
            MenuEntry::Item {
                action,
                label,
                accelerator,
                checked: Some(on),
            } => to(&CheckMenuItem::with_id(
                app,
                action.id(),
                label,
                true,
                *on,
                accelerator.as_deref(),
            )?)?,
            MenuEntry::System { item } => to(&predefined(app, *item)?)?,
            MenuEntry::Separator => to(&PredefinedMenuItem::separator(app)?)?,
            MenuEntry::Submenu { label, entries } => {
                let submenu = Submenu::new(app, label, true)?;
                add(app, entries, &|item| submenu.append(item))?;
                to(&submenu)?;
            }
        }
    }
    Ok(())
}

fn predefined<R: Runtime>(
    app: &AppHandle<R>,
    item: SystemItem,
) -> tauri::Result<PredefinedMenuItem<R>> {
    use SystemItem as S;
    match item {
        S::Undo => PredefinedMenuItem::undo(app, None),
        S::Redo => PredefinedMenuItem::redo(app, None),
        S::Cut => PredefinedMenuItem::cut(app, None),
        S::Copy => PredefinedMenuItem::copy(app, None),
        S::Paste => PredefinedMenuItem::paste(app, None),
        S::SelectAll => PredefinedMenuItem::select_all(app, None),
        S::Services => PredefinedMenuItem::services(app, None),
        S::Hide => PredefinedMenuItem::hide(app, Some(&format!("Hide {APP_NAME}"))),
        S::HideOthers => PredefinedMenuItem::hide_others(app, None),
        S::ShowAll => PredefinedMenuItem::show_all(app, None),
        S::Minimize => PredefinedMenuItem::minimize(app, None),
        S::Zoom => PredefinedMenuItem::maximize(app, Some("Zoom")),
        S::Fullscreen => PredefinedMenuItem::fullscreen(app, None),
    }
}

/// The native menu of `menus`.
///
/// # Errors
/// When Tauri can't make an item (an accelerator it can't read).
pub fn build<R: Runtime>(app: &AppHandle<R>, menus: &[MenuDef]) -> tauri::Result<Menu<R>> {
    let bar = Menu::new(app)?;
    for def in menus {
        let submenu = if def.windows {
            Submenu::with_id(app, WINDOW_SUBMENU_ID, &def.label, true)?
        } else {
            Submenu::new(app, &def.label, true)?
        };
        add(app, &def.entries, &|item| submenu.append(item))?;
        bar.append(&submenu)?;
    }
    Ok(bar)
}

/// Show the menu of the window in front: build it from its page's state and set it; on Windows and Linux, hidden in
/// every window (the page draws it).
pub fn refresh<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<AppState>();
    let front = crate::windows::front(app);
    let shown = front
        .as_ref()
        .map(|w| state.menus().state(w.label()))
        .unwrap_or_default();
    let places = places_of(state.workspaces(), shown.workspace.as_deref());
    let menus = definition(Platform::current(), &shown, &places);
    match build(app, &menus) {
        Ok(menu) => {
            let _ = app.set_menu(menu);
            if Platform::current() != Platform::Mac {
                for window in app.webview_windows().values() {
                    let _ = window.hide_menu();
                }
            }
        }
        Err(e) => eprintln!("atm-app: the menu: {e}"),
    }
}

/// A menu item was chosen: Quit and New Window the app does; the rest goes to the page of the window in front.
pub fn chosen<R: Runtime>(app: &AppHandle<R>, id: &str) {
    let Some(action) = MenuAction::from_id(id) else {
        return;
    };
    match action {
        MenuAction::Quit => app.exit(0),
        MenuAction::NewWindow => {
            let state = app.state::<AppState>();
            let label = state.new_window_label();
            if let Err(e) = crate::windows::build_window(app, &label) {
                eprintln!("atm-app: a new window: {e}");
            }
        }
        action => {
            if let Some(window) = crate::windows::front(app) {
                app.state::<AppState>().menus().send(window.label(), action);
            }
        }
    }
}

/// The calling window's page listens for its menu's actions on `channel` (the window in front gets them).
///
/// # Errors
/// None: a `Result`, as every async command with state is.
#[tauri::command]
#[specta::specta]
pub async fn subscribe_menu<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
    channel: Channel<MenuAction>,
) -> Result<(), AppError> {
    state.menus().subscribe(window.label(), channel);
    Ok(())
}

/// The calling window's menus, for the page to draw (Windows, Linux), with what it shows checked.
///
/// # Errors
/// None: a `Result`, as every async command with state is.
#[tauri::command]
#[specta::specta]
pub async fn menu<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
) -> Result<Vec<MenuDef>, AppError> {
    let shown = state.menus().state(window.label());
    let places = places_of(state.workspaces(), shown.workspace.as_deref());
    Ok(definition(Platform::current(), &shown, &places))
}

/// The calling window's page says what its menu shows checked; the menu bar shows it while that window is in front.
///
/// # Errors
/// None: a `Result`, as every async command with state is.
#[tauri::command]
#[specta::specta]
pub async fn menu_state<R: Runtime>(
    app: AppHandle<R>,
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
    shown: MenuState,
) -> Result<(), AppError> {
    let changed = state.menus().state(window.label()) != shown;
    state.menus().set_state(window.label(), shown);
    let in_front = crate::windows::front(&app).is_some_and(|w| w.label() == window.label());
    if changed && in_front {
        refresh(&app);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every_item(entries: &[MenuEntry], out: &mut Vec<(MenuAction, Option<String>)>) {
        for entry in entries {
            match entry {
                MenuEntry::Item {
                    action,
                    accelerator,
                    ..
                } => out.push((action.clone(), accelerator.clone())),
                MenuEntry::Submenu { entries, .. } => every_item(entries, out),
                _ => {}
            }
        }
    }

    fn items(platform: Platform) -> Vec<(MenuAction, Option<String>)> {
        let places = Places {
            workspaces: vec!["default".into(), "work".into()],
            projects: vec![("abcd1234".into(), "App".into())],
        };
        let mut out = Vec::new();
        for def in definition(platform, &MenuState::default(), &places) {
            every_item(&def.entries, &mut out);
        }
        out
    }

    /// The labels name the app as users know it (T-531): Agentic Task Manager, never atm (the command's name)
    #[test]
    fn the_menus_call_the_app_agentic_task_manager() {
        fn labels(entries: &[MenuEntry], out: &mut Vec<String>) {
            for entry in entries {
                match entry {
                    MenuEntry::Item { label, .. } => out.push(label.clone()),
                    MenuEntry::Submenu { label, entries, .. } => {
                        out.push(label.clone());
                        labels(entries, out);
                    }
                    _ => {}
                }
            }
        }
        for platform in [Platform::Mac, Platform::Windows, Platform::Linux] {
            let mut all = Vec::new();
            for def in definition(platform, &MenuState::default(), &Places::default()) {
                all.push(def.label.clone());
                labels(&def.entries, &mut all);
            }
            assert!(
                all.iter()
                    .all(|l| !l.split(|c: char| !c.is_alphanumeric()).any(|w| w == "atm")),
                "{platform:?}: {all:?}"
            );
            for wanted in [
                "About Agentic Task Manager",
                "Agentic Task Manager Help",
                "Agentic Task Manager on GitHub",
            ] {
                assert!(all.iter().any(|l| l == wanted), "{platform:?}: no {wanted}");
            }
        }
        let mac = definition(Platform::Mac, &MenuState::default(), &Places::default());
        assert_eq!(mac[0].label, APP_NAME);
    }

    #[test]
    fn every_action_has_one_item_and_no_shortcut_is_used_twice() {
        for platform in [Platform::Mac, Platform::Windows, Platform::Linux] {
            let items = items(platform);
            for action in &ACTIONS {
                let n = items.iter().filter(|(a, _)| a == action).count();
                assert_eq!(n, 1, "{platform:?}: {action:?} has {n} items");
            }
            let mut keys: Vec<&String> = items.iter().filter_map(|(_, k)| k.as_ref()).collect();
            let all = keys.len();
            keys.sort();
            keys.dedup();
            assert_eq!(keys.len(), all, "{platform:?}: a shortcut is used twice");
        }
    }

    /// Each shortcut is modifiers and one key, by the names Tauri's menus (muda) read: a native menu can only be made
    /// on the main thread, so the app's by-hand check makes the real one
    #[test]
    fn every_shortcut_is_one_tauri_reads() {
        const MODIFIERS: [&str; 4] = ["CmdOrCtrl", "Ctrl", "Alt", "Shift"];
        let keys: Vec<String> = ('A'..='Z')
            .map(String::from)
            .chain((1..=6).map(|n| format!("Digit{n}")))
            .chain(
                [
                    "Comma",
                    "Slash",
                    "BracketLeft",
                    "BracketRight",
                    "ArrowLeft",
                    "ArrowRight",
                    "Tab",
                    "F1",
                ]
                .map(String::from),
            )
            .collect();
        for platform in [Platform::Mac, Platform::Windows, Platform::Linux] {
            for (action, shortcut) in items(platform) {
                let Some(shortcut) = shortcut else { continue };
                let parts: Vec<&str> = shortcut.split('+').collect();
                let (key, modifiers) = parts.split_last().expect("a key");
                assert!(keys.iter().any(|k| k == key), "{action:?}: {shortcut}");
                assert!(
                    modifiers.iter().all(|m| MODIFIERS.contains(m)),
                    "{action:?}: {shortcut}"
                );
            }
        }
    }

    #[test]
    fn the_go_menu_lists_the_projects_and_workspaces() {
        let items = items(Platform::Mac);
        assert!(items.contains(&(MenuAction::Project("abcd1234".into()), None)));
        assert!(items.contains(&(MenuAction::Workspace("work".into()), None)));
    }

    #[test]
    fn the_platforms_own_shortcuts() {
        let key = |platform, action: MenuAction| {
            items(platform)
                .into_iter()
                .find(|(a, _)| *a == action)
                .and_then(|(_, k)| k)
        };
        assert_eq!(
            key(Platform::Mac, MenuAction::Back).as_deref(),
            Some("CmdOrCtrl+BracketLeft")
        );
        assert_eq!(
            key(Platform::Windows, MenuAction::Back).as_deref(),
            Some("Alt+ArrowLeft")
        );
        assert_eq!(
            key(Platform::Linux, MenuAction::ToggleSidebar).as_deref(),
            Some("Ctrl+B")
        );
        assert_eq!(
            key(Platform::Linux, MenuAction::Help).as_deref(),
            Some("F1")
        );
        assert_eq!(key(Platform::Windows, MenuAction::Quit), None);
        assert_eq!(
            key(Platform::Linux, MenuAction::Quit).as_deref(),
            Some("Ctrl+Q")
        );
    }

    fn walk(entries: &[MenuEntry], out: &mut Vec<MenuAction>) {
        for entry in entries {
            match entry {
                MenuEntry::Item {
                    action,
                    checked: Some(true),
                    ..
                } => out.push(action.clone()),
                MenuEntry::Submenu { entries, .. } => walk(entries, out),
                _ => {}
            }
        }
    }

    #[test]
    fn what_the_page_reports_is_checked() {
        let state = MenuState {
            view: Some("list".into()),
            archived: true,
            workspace: Some("work".into()),
            ..MenuState::default()
        };
        let places = Places {
            workspaces: vec!["default".into(), "work".into()],
            projects: vec![],
        };
        let mut checked = Vec::new();
        for def in definition(Platform::Mac, &state, &places) {
            walk(&def.entries, &mut checked);
        }
        assert_eq!(
            checked,
            vec![
                MenuAction::ShowList,
                MenuAction::ToggleArchived,
                MenuAction::Workspace("work".into())
            ]
        );
    }

    #[test]
    fn an_items_id_is_its_action() {
        for action in ACTIONS.iter().cloned().chain([
            MenuAction::Project("abcd1234".into()),
            MenuAction::Workspace("w:x".into()),
        ]) {
            assert_eq!(
                MenuAction::from_id(&action.id()),
                Some(action.clone()),
                "{action:?}"
            );
        }
        assert_eq!(
            MenuAction::id(&MenuAction::CheckForUpdates),
            "check_for_updates"
        );
        assert_eq!(MenuAction::GitHub.id(), "github");
        assert_eq!(MenuAction::CompMd.id(), "compmd");
        assert_eq!(MenuAction::from_id("nope"), None);
        assert_eq!(MenuAction::from_id("project"), None);
    }

    #[test]
    fn the_windows_menu_states_are_their_own() {
        let menus = Menus::default();
        let shown = MenuState {
            view: Some("kanban".into()),
            ..MenuState::default()
        };
        menus.set_state("main", shown.clone());
        assert_eq!(menus.state("main"), shown);
        assert_eq!(menus.state("window-1"), MenuState::default());
        let (sent, received) = std::sync::mpsc::channel();
        menus.subscribe(
            "main",
            Channel::new(move |body| {
                let _ = sent.send(body);
                Ok(())
            }),
        );
        assert!(menus.send("main", MenuAction::Reload));
        assert!(!menus.send("window-1", MenuAction::Reload));
        let tauri::ipc::InvokeResponseBody::Json(text) = received.recv().expect("an action") else {
            panic!("a JSON message");
        };
        assert_eq!(text, r#"{"kind":"reload"}"#);
        menus.forget("main");
        assert!(!menus.send("main", MenuAction::Reload));
        assert_eq!(menus.state("main"), MenuState::default());
    }
}
