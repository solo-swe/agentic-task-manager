//! The atm app (T-455, docs/guides/tauri.md): the dashboard in a window of its own, on atm-lib as `atm serve` is.
//!
//! Its commands are in `commands`; tauri-specta writes their TypeScript types (`.generated/app/bindings.ts`, never
//! edited by hand). The window may call only the commands its capability lists (`capabilities/main.json`).

pub mod assets;
pub mod badge;
pub mod commands;
pub mod external;
pub mod links;
pub mod menu;
pub mod navigation;
pub mod session;
pub mod updates;
pub mod windows;

use tauri::Manager;

use badge::needs_you;
use session::{reload_window, restored_tabs, save_tabs};

use commands::{
    AppState, agents_status, claims, connect_agents, document, file, list_projects,
    list_workspaces, logo, project_tasks, search, subscribe_links, subscribe_project,
    subscribe_working_copy, subscribe_workspace, unsubscribe, version, worktrees,
};
use commands::{
    add_review_comment, add_review_draft, close_review, create_review, list_review_drafts,
    poke_review, project_review, project_reviews, remove_review_draft, reply_to_review_comment,
    set_review_tasks, submit_review_decision, update_review_comment, update_review_draft,
};
use external::open_url;
use menu::{menu, menu_state, subscribe_menu};
use updates::{check_update, install_update, restart_app};
use windows::{
    close_window, minimize, open_window, set_title, shell, start_dragging, toggle_maximize,
};

// What the app serves on its own `atm:` scheme (`assets`), embedded by build.rs
include!(concat!(env!("OUT_DIR"), "/atm_assets.rs"));

/// The app's commands, for Tauri and for the TypeScript types (each also in `build.rs`'s manifest and the main
/// window's capability).
#[must_use]
pub fn commands() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new().commands(tauri_specta::collect_commands![
        version,
        list_workspaces,
        list_projects,
        search,
        project_tasks,
        claims,
        worktrees,
        document,
        file,
        logo,
        project_reviews,
        project_review,
        subscribe_workspace::<tauri::Wry>,
        subscribe_project::<tauri::Wry>,
        subscribe_working_copy::<tauri::Wry>,
        unsubscribe::<tauri::Wry>,
        agents_status,
        connect_agents,
        create_review,
        close_review,
        set_review_tasks,
        submit_review_decision,
        add_review_comment,
        update_review_comment,
        reply_to_review_comment,
        poke_review,
        list_review_drafts,
        add_review_draft,
        update_review_draft,
        remove_review_draft,
        subscribe_links::<tauri::Wry>,
        check_update,
        install_update,
        restart_app,
        shell,
        set_title::<tauri::Wry>,
        open_window::<tauri::Wry>,
        subscribe_menu::<tauri::Wry>,
        menu::<tauri::Wry>,
        menu_state::<tauri::Wry>,
        minimize::<tauri::Wry>,
        toggle_maximize::<tauri::Wry>,
        close_window::<tauri::Wry>,
        start_dragging::<tauri::Wry>,
        save_tabs::<tauri::Wry>,
        restored_tabs::<tauri::Wry>,
        reload_window::<tauri::Wry>,
        needs_you::<tauri::Wry>,
        open_url::<tauri::Wry>,
    ])
}

/// Start the app: one instance (opening it again focuses the window in front), the user's registry in its state, the
/// `atm://` links it is opened with sent to the page of the window in front (T-459, T-511).
///
/// # Panics
/// When Tauri can't start (no window system, a broken build).
pub fn run() {
    let commands = commands();
    // The single instance first, so a second instance stops before it sets anything up
    let first =
        tauri::Builder::default().plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = windows::front_or_new(app) {
                windows::focus(&window);
            }
        }));
    let app = with_plugins(first)
        .manage(AppState::new(atm_lib::Workspaces::open()))
        // Only the app's page calls the commands (ATM-677, audit A6): Tauri counts CompMD's frames as local too
        .invoke_handler(navigation::guarded(commands.invoke_handler()))
        // A window's page loading again (a reload) ends that page's subscriptions (T-506, T-511): it can't say so
        // itself; other windows' stay. Its links and menu channels too (ATM-677, audit A8), so a link sent meanwhile
        // waits for the new page
        .on_page_load(|webview, payload| {
            if payload.event() == tauri::webview::PageLoadEvent::Started {
                let state = webview.state::<AppState>();
                state.unwatch_window(webview.label());
                state.links().page_started(webview.label());
                state.menus().page_started(webview.label());
            }
        })
        .setup(move |app| {
            commands.mount_events(app);
            // The menus (T-512): macOS's menu bar; on Windows and Linux hidden, for the page to draw
            menu::refresh(app.handle());
            // The main window's frame for the page's title bar (T-513); new windows get theirs as they are made
            if let Some(window) = app.get_webview_window(windows::MAIN) {
                windows::dress(&window);
                app.state::<AppState>().session().opened(window.label());
            }
            // The windows as they were when the app last quit, with their tabs (T-518)
            session::restore(app.handle());
            Ok(())
        })
        .on_menu_event(|app, event| menu::chosen(app, event.id().as_ref()))
        .build(context())
        .expect("the atm app");
    app.run(|app, event| match event {
        // The window in front is the one links, a second instance and the menu go to; a closed one's subscriptions end
        tauri::RunEvent::WindowEvent { label, event, .. } => {
            let state = app.state::<AppState>();
            match event {
                tauri::WindowEvent::Focused(true) => {
                    let changed = state.front().as_deref() != Some(label.as_str());
                    state.focused(&label);
                    if changed {
                        menu::refresh(app);
                    }
                }
                // A window closed by the user (or its page's last tab) is forgotten; one destroyed as the app quits
                // is kept, to come back (T-518)
                tauri::WindowEvent::CloseRequested { .. } => state.session().closing(&label),
                tauri::WindowEvent::Destroyed => {
                    state.window_closed(&label);
                    if state.session().destroyed(&label) {
                        let _ = state.session().write();
                    }
                }
                tauri::WindowEvent::Moved(_) | tauri::WindowEvent::Resized(_) => {
                    if let Some(frame) = app
                        .get_webview_window(&label)
                        .as_ref()
                        .and_then(session::frame_of)
                    {
                        state.session().framed(&label, frame);
                    }
                }
                _ => {}
            }
        }
        // The windows' last frames, as the app quits
        tauri::RunEvent::Exit => {
            let _ = app.state::<AppState>().session().write();
        }
        // macOS hands the app the links it is opened with (the scheme is in Info.plist), started by one or running
        #[cfg(target_os = "macos")]
        tauri::RunEvent::Opened { urls } => {
            let Some(window) = windows::front_or_new(app) else {
                return;
            };
            let state = app.state::<AppState>();
            if urls
                .iter()
                .any(|url| state.links().open(window.label(), url.as_str()))
            {
                windows::focus(&window);
            }
        }
        _ => {}
    });
}

/// The plugins and schemes the app starts with, after the single instance: the same in the tests, which build the
/// app's context with them, so a plugin that can't start (its configuration, T-483) fails a test, not the app.
fn with_plugins<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder
        // Every webview loads only the app's own origins (ATM-677, audit A6, A10)
        .plugin(navigation::plugin())
        // CompMD's bundle (the sandbox's frames, the packs' modules), embedded by build.rs, on a scheme of its own,
        // `compmd:`, so the frames keep their own policy and origin; the plugin gives the page that origin (ADR-006)
        .plugin(tauri_plugin_compmd::init(
            tauri_plugin_compmd::include_bundle!(),
        ))
        // What atm ships for its page itself (CompMD's processor and its WebAssembly) on `atm:`
        .register_uri_scheme_protocol(assets::SCHEME, |_context, request| {
            let response = assets::respond(ATM_ASSETS, request.method(), request.uri().path());
            if !response.status().is_success() {
                eprintln!(
                    "atm-app: {} {} {}",
                    response.status().as_u16(),
                    request.method(),
                    request.uri().path()
                );
            }
            response
        })
        // Its own updates (T-483), through the two commands in `updates`: the page has no permission of the plugin's.
        // The plugin reads `plugins.updater` in tauri.conf.json when it starts (the key, as `updates.rs` has it)
        .plugin(tauri_plugin_updater::Builder::new().build())
        // System notifications of what needs the user (T-520), sent by the app from Rust (`badge`): the page has no
        // permission of the plugin's
        .plugin(tauri_plugin_notification::init())
        // The page's external links in the user's browser (T-551), opened by the app's `open_url` after it checks them
        // (`external`): the page has no permission of the plugin's, and the plugin's own script, which would send the
        // page's link clicks to its command, is left out
        .plugin(
            tauri_plugin_opener::Builder::new()
                .open_js_links_on_click(false)
                .build(),
        )
}

/// The app's context (`tauri.conf.json`, the capabilities, the dashboard's files), made once: it can be generated only
/// once in a crate.
fn context<R: tauri::Runtime>() -> tauri::Context<R> {
    tauri::generate_context!()
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod integration_tests;
