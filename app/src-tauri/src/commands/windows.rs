//! The window's live parts: its subscriptions to a workspace, a project or a working copy, its links, and the agents'
//! skills (their status, and connecting them).

use serde_json::json;
use tauri::ipc::Channel;
use tauri::{Runtime, State, WebviewWindow};

use super::reads::{member, workspace};
use super::state::{AppError, AppState, Changed, Json};

/// Be told on `channel` when a workspace's list or any of its projects changes. The subscription's ID.
///
/// # Errors
/// No such workspace; a list or project that can't be watched.
#[tauri::command]
#[specta::specta]
pub async fn subscribe_workspace<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
    workspace: String,
    channel: Channel<Changed>,
) -> Result<u32, AppError> {
    let found = self::workspace(&state.workspaces, &workspace)?;
    let message = Changed {
        workspace,
        project: None,
    };
    state.watch(window.label(), |c| found.subscribe(c), channel, message)
}

/// Be told on `channel` when a project's data changes. The subscription's ID.
///
/// # Errors
/// No such workspace or project; a project that can't be watched.
#[tauri::command]
#[specta::specta]
pub async fn subscribe_project<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
    workspace: String,
    project: String,
    channel: Channel<Changed>,
) -> Result<u32, AppError> {
    let found = member(&state.workspaces, &workspace, &project)?;
    let message = Changed {
        workspace,
        project: Some(project),
    };
    state.watch(window.label(), |c| found.subscribe(c), channel, message)
}

/// Be told on `channel` when a project's working copy changes as a live review shows it (ATM-607): a live review's
/// page asks again on each, instead of every 2 s. The subscription's ID.
///
/// # Errors
/// No such workspace or project; a project not in git, or a folder that can't be watched.
#[tauri::command]
#[specta::specta]
pub async fn subscribe_working_copy<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
    workspace: String,
    project: String,
    channel: Channel<Changed>,
) -> Result<u32, AppError> {
    let found = member(&state.workspaces, &workspace, &project)?;
    let message = Changed {
        workspace,
        project: Some(project),
    };
    state.watch(
        window.label(),
        |c| found.subscribe_working_copy(c),
        channel,
        message,
    )
}

/// `atm agents status`: per harness atm knows, whether it is installed here, and each of atm's skills' state in the
/// user's own folder for it (linked, copied, missing, stale, not atm's).
///
/// # Errors
/// None: a `Result`, as every async command is.
#[tauri::command]
#[specta::specta]
pub async fn agents_status() -> Result<Json, AppError> {
    Ok(Json(json!(atm_lib::agents::skills_status(
        None,
        atm_common::VERSION
    ))))
}

/// `atm agents install --harness <harness>` (T-458): this version's skills where the harness reads the user's, the
/// same atm-lib code as the command; `{written: [{harness, skill, path, how, action}], refused: [{path, reason}]}`. Only
/// ever run by the user's click on the app's button: nothing calls it on its own.
///
/// # Errors
/// A harness atm doesn't know; a folder or file that can't be written.
#[tauri::command]
#[specta::specta]
pub async fn connect_agents(harness: String) -> Result<Json, AppError> {
    let installed =
        atm_lib::agents::install_skills(Some(&harness), None, false, atm_common::VERSION)?;
    Ok(Json(
        json!({"written": installed.written, "refused": installed.refused}),
    ))
}

/// Be told on `channel` the dashboard address of each `atm://` link the app opens in the calling window (T-459,
/// T-511); the answer is the one waiting for it, if any (a link the app was started with, a new window's first page).
///
/// # Errors
/// None: a `Result`, as every async command with state is.
#[tauri::command]
#[specta::specta]
pub async fn subscribe_links<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
    channel: Channel<String>,
) -> Result<Option<String>, AppError> {
    Ok(state.links.subscribe(window.label(), channel))
}

/// End one of the calling window's subscriptions. Whether there was one.
///
/// # Errors
/// None: a `Result`, as every async command with state is.
#[tauri::command]
#[specta::specta]
pub async fn unsubscribe<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
    id: u32,
) -> Result<bool, AppError> {
    Ok(state.unwatch(window.label(), id))
}
