//! The inbox's commands (docs/inbox-design.md §7, ATM-761), each the REST route of the same name, acting as the user:
//! the items, an item added, its text changed, and a person's decision (discarded or restored). No command injects an
//! item: an agent records that with `atm inbox inject`.

use atm_lib::inbox;
use atm_lib::review_requests::request_from;
use serde_json::{Value, json};
use tauri::State;

use super::reads::member;
use super::reviews::ReviewRequest;
use super::state::{AppError, AppState, Json};

/// A request's body: an object, as the routes take it (a list would fill a request's fields in order)
fn object(body: ReviewRequest) -> Result<Value, AppError> {
    if body.0.is_object() {
        Ok(body.0)
    } else {
        Err(atm_common::Error::Invalid("not this request: the body is an object".into()).into())
    }
}

/// `GET …/inbox?state=&archived=`: the items `state` names (the open ones when none), of the archive with `archived`.
///
/// # Errors
/// No such workspace or project; a state that isn't one; an inbox that can't be read.
#[tauri::command]
#[specta::specta]
pub async fn project_inbox(
    state: State<'_, AppState>,
    workspace: String,
    project: String,
    which: Option<String>,
    archived: bool,
) -> Result<Json, AppError> {
    let project = member(&state.workspaces, &workspace, &project)?.project()?;
    let listed = inbox::list(&project, inbox::listing_of(which.as_deref())?, archived)?;
    Ok(Json(json!({
        "items": listed.items,
        "created": listed.created,
        "discarded": listed.discarded,
        "injected": listed.injected,
    })))
}

/// `POST …/inbox`: an item added, as the user.
///
/// # Errors
/// No such workspace or project; a body that isn't one; what the inbox's rules refuse.
#[tauri::command]
#[specta::specta]
pub async fn add_inbox_item(
    state: State<'_, AppState>,
    workspace: String,
    project: String,
    item: ReviewRequest,
) -> Result<Json, AppError> {
    let mut project = member(&state.workspaces, &workspace, &project)?.project_as_user()?;
    let item: inbox::NewItem = request_from(object(item)?)?;
    Ok(Json(
        json!({ "item": inbox::add(&mut project, &item.text)? }),
    ))
}

/// `PATCH …/inbox/<IN>`: an open item's text, as the user.
///
/// # Errors
/// As [`add_inbox_item`].
#[tauri::command]
#[specta::specta]
pub async fn edit_inbox_item(
    state: State<'_, AppState>,
    workspace: String,
    project: String,
    id: String,
    edit: ReviewRequest,
) -> Result<Json, AppError> {
    let mut project = member(&state.workspaces, &workspace, &project)?.project_as_user()?;
    let edit: inbox::ItemEdit = request_from(object(edit)?)?;
    Ok(Json(
        json!({ "item": inbox::edit(&mut project, &id, &edit.text)? }),
    ))
}

/// `POST …/inbox/<IN>/decisions`: a person's decision, discarded (with a reason) or restored.
///
/// # Errors
/// As [`add_inbox_item`].
#[tauri::command]
#[specta::specta]
pub async fn decide_inbox_item(
    state: State<'_, AppState>,
    workspace: String,
    project: String,
    id: String,
    decision: ReviewRequest,
) -> Result<Json, AppError> {
    let mut project = member(&state.workspaces, &workspace, &project)?.project_as_user()?;
    let decision: inbox::ItemDecision = request_from(object(decision)?)?;
    Ok(Json(
        json!({ "item": inbox::decide(&mut project, &id, &decision)? }),
    ))
}
