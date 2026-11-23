//! The review and draft commands (ATM-572, ATM-612), each the REST route of the same name, acting as the user.

use atm_lib::Workspaces;
use atm_lib::{review_drafts, review_requests};
use serde::Deserialize;
use serde_json::{Value, json};
use specta::Type;
use tauri::State;

use super::reads::member;
use super::state::{AppError, AppState, Json, JsonShape};

/// A review request's body, as the page sends it and the REST route takes it (ATM-572; atm-lib's `review_requests`
/// reads it, refusing what doesn't fit).
#[derive(Debug, Clone, PartialEq, Deserialize, Type)]
#[serde(transparent)]
pub struct ReviewRequest(#[specta(type = JsonShape)] pub Value);

/// Project `p` of workspace `w`, opened to act as the user (never an agent's identity).
fn as_user(workspaces: &Workspaces, w: &str, p: &str) -> Result<atm_lib::Project, AppError> {
    Ok(member(workspaces, w, p)?.project_as_user()?)
}

fn header_json(header: &atm_lib::reviews::ReviewHeader) -> Result<Value, AppError> {
    Ok(serde_json::to_value(header)?)
}

pub(super) fn review_ref(review: &str) -> atm_lib::reviews::ReviewRef {
    atm_lib::reviews::ReviewRef(review.to_string())
}

fn comment_id(comment: &str) -> atm_lib::reviews::CommentId {
    atm_lib::reviews::CommentId(comment.to_string())
}

// The app's review commands (docs/code-review-design.md §3.12, ATM-572): each the REST route of the same name, the same
// atm-lib call as its `atm review …` command, acting as the user. Only ever on the user's action in the page (a new
// exception to I27, beside connecting agents and installing an update); CompMD's frames, which show the comments,
// still call nothing (I25).

/// `POST …/reviews`: a review opened (or a draft, or a live review).
///
/// # Errors
/// No such workspace or project; a body that isn't one; what the review's rules refuse.
#[tauri::command]
#[specta::specta]
pub async fn create_review(
    state: State<'_, AppState>,
    workspace: String,
    project: String,
    opening: ReviewRequest,
) -> Result<Json, AppError> {
    let mut project = as_user(&state.workspaces, &workspace, &project)?;
    let opening = review_requests::request_from(opening.0)?;
    header_json(&review_requests::open_review(&mut project, opening)?).map(Json)
}

/// `PATCH …/reviews/<CR>`: the review closed.
///
/// # Errors
/// As [`create_review`].
#[tauri::command]
#[specta::specta]
pub async fn close_review(
    state: State<'_, AppState>,
    workspace: String,
    project: String,
    review: String,
    closing: ReviewRequest,
) -> Result<Json, AppError> {
    let mut project = as_user(&state.workspaces, &workspace, &project)?;
    let closing = review_requests::request_from(closing.0)?;
    header_json(&review_requests::close_review(
        &mut project,
        &review_ref(&review),
        &closing,
    )?)
    .map(Json)
}

/// `PUT …/reviews/<CR>/tasks`: the tasks it reviews, as a whole.
///
/// # Errors
/// As [`create_review`].
#[tauri::command]
#[specta::specta]
pub async fn set_review_tasks(
    state: State<'_, AppState>,
    workspace: String,
    project: String,
    review: String,
    tasks: Vec<String>,
) -> Result<Json, AppError> {
    let mut project = as_user(&state.workspaces, &workspace, &project)?;
    let wanted = review_requests::ReviewTasks { tasks };
    header_json(&review_requests::set_review_tasks(
        &mut project,
        &review_ref(&review),
        &wanted,
    )?)
    .map(Json)
}

/// `POST …/reviews/<CR>/decisions`: a reviewer's decision.
///
/// # Errors
/// As [`create_review`].
#[tauri::command]
#[specta::specta]
pub async fn submit_review_decision(
    state: State<'_, AppState>,
    workspace: String,
    project: String,
    review: String,
    decision: ReviewRequest,
) -> Result<Json, AppError> {
    let mut project = as_user(&state.workspaces, &workspace, &project)?;
    let decision = review_requests::request_from(decision.0)?;
    header_json(&review_requests::submit_decision(
        &mut project,
        &review_ref(&review),
        &decision,
    )?)
    .map(Json)
}

/// `POST …/reviews/<CR>/comments`: a new chain.
///
/// # Errors
/// As [`create_review`].
#[tauri::command]
#[specta::specta]
pub async fn add_review_comment(
    state: State<'_, AppState>,
    workspace: String,
    project: String,
    review: String,
    comment: ReviewRequest,
) -> Result<Json, AppError> {
    let project = as_user(&state.workspaces, &workspace, &project)?;
    let comment = review_requests::request_from(comment.0)?;
    let id = review_requests::add_comment(&project, &review_ref(&review), comment)?;
    Ok(Json(json!({ "review": review, "comment": id })))
}

/// `PATCH …/reviews/<CR>/comments/<c>`: a comment's text (one's own), or its resolved box.
///
/// # Errors
/// As [`create_review`].
#[tauri::command]
#[specta::specta]
pub async fn update_review_comment(
    state: State<'_, AppState>,
    workspace: String,
    project: String,
    review: String,
    comment: String,
    update: ReviewRequest,
) -> Result<Json, AppError> {
    let project = as_user(&state.workspaces, &workspace, &project)?;
    let update = review_requests::request_from(update.0)?;
    review_requests::update_comment(
        &project,
        &review_ref(&review),
        &comment_id(&comment),
        update,
    )?;
    Ok(Json(json!({ "review": review, "comment": comment })))
}

/// `POST …/reviews/<CR>/comments/<c>/replies`: a reply.
///
/// # Errors
/// As [`create_review`].
#[tauri::command]
#[specta::specta]
pub async fn reply_to_review_comment(
    state: State<'_, AppState>,
    workspace: String,
    project: String,
    review: String,
    comment: String,
    body: String,
) -> Result<Json, AppError> {
    let project = as_user(&state.workspaces, &workspace, &project)?;
    let reply = review_requests::NewReply { body };
    let id = review_requests::reply_to_comment(
        &project,
        &review_ref(&review),
        &comment_id(&comment),
        &reply,
    )?;
    Ok(Json(
        json!({ "review": review, "comment": id, "reply_to": comment }),
    ))
}

/// `POST …/reviews/<CR>/pokes` (ATM-611): a poke, with an optional message.
///
/// # Errors
/// As [`create_review`].
#[tauri::command]
#[specta::specta]
pub async fn poke_review(
    state: State<'_, AppState>,
    workspace: String,
    project: String,
    review: String,
    body: Option<String>,
) -> Result<Json, AppError> {
    let project = as_user(&state.workspaces, &workspace, &project)?;
    let poke = review_requests::NewPoke { body };
    let id = review_requests::poke_review(&project, &review_ref(&review), &poke)?;
    Ok(Json(json!({ "review": review, "poke": id })))
}

fn draft_json(value: &impl serde::Serialize) -> Result<serde_json::Value, AppError> {
    serde_json::to_value(value)
        .map_err(|e| AppError::from(atm_common::Error::failed(e.to_string())))
}

/// `GET …/reviews/<CR>/drafts` (ATM-612): the user's drafts on the review.
///
/// # Errors
/// As [`create_review`].
#[tauri::command]
#[specta::specta]
pub async fn list_review_drafts(
    state: State<'_, AppState>,
    workspace: String,
    project: String,
    review: String,
) -> Result<Json, AppError> {
    let project = as_user(&state.workspaces, &workspace, &project)?;
    let drafts = review_drafts::list_drafts(&project, &review_ref(&review))?;
    Ok(Json(
        json!({ "review": review, "drafts": draft_json(&drafts)? }),
    ))
}

/// `POST …/reviews/<CR>/drafts`: a draft of the user's.
///
/// # Errors
/// As [`create_review`].
#[tauri::command]
#[specta::specta]
pub async fn add_review_draft(
    state: State<'_, AppState>,
    workspace: String,
    project: String,
    review: String,
    draft: ReviewRequest,
) -> Result<Json, AppError> {
    let project = as_user(&state.workspaces, &workspace, &project)?;
    let draft = review_requests::request_from(draft.0)?;
    let made = review_drafts::add_draft(&project, &review_ref(&review), draft)?;
    Ok(Json(draft_json(&made)?))
}

/// `PATCH …/reviews/<CR>/drafts/<d>`: a draft's text.
///
/// # Errors
/// As [`create_review`].
#[tauri::command]
#[specta::specta]
pub async fn update_review_draft(
    state: State<'_, AppState>,
    workspace: String,
    project: String,
    review: String,
    draft: String,
    body: String,
) -> Result<Json, AppError> {
    let project = as_user(&state.workspaces, &workspace, &project)?;
    let changed = review_drafts::update_draft(
        &project,
        &review_ref(&review),
        &draft,
        &review_drafts::DraftUpdate { body },
    )?;
    Ok(Json(draft_json(&changed)?))
}

/// `DELETE …/reviews/<CR>/drafts/<d>`: a draft thrown away.
///
/// # Errors
/// As [`create_review`].
#[tauri::command]
#[specta::specta]
pub async fn remove_review_draft(
    state: State<'_, AppState>,
    workspace: String,
    project: String,
    review: String,
    draft: String,
) -> Result<Json, AppError> {
    let project = as_user(&state.workspaces, &workspace, &project)?;
    review_drafts::remove_draft(&project, &review_ref(&review), &draft)?;
    Ok(Json(
        json!({ "review": review, "draft": draft, "removed": true }),
    ))
}
