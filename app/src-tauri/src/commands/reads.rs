//! The reads: the workspaces, projects, tasks, claims, worktrees, documents, files and logos, each the same atm-lib
//! call as its REST route (I27); and the plain functions both use.

use std::sync::Arc;

use atm_lib::Workspaces;
use atm_lib::query::Filters;
use atm_lib::workspaces::{Member, Workspace, too_large};
use serde_json::{Value, json};
use tauri::State;

use super::reviews::review_ref;
use super::state::{AppError, AppState, FileData, Fresh, Json, ReviewFilters, TaskFilters};

pub(super) fn workspace(workspaces: &Workspaces, id: &str) -> Result<Arc<Workspace>, AppError> {
    workspaces
        .get(id)
        .ok_or_else(|| AppError::NotFound("unknown workspace".into()))
}

pub(super) fn member(workspaces: &Workspaces, w: &str, p: &str) -> Result<Member, AppError> {
    Ok(workspace(workspaces, w)?.member(p)?)
}

/// Workspace `w`'s projects as `list_projects` lists them, without making it the one opened last (the badge and the
/// notifications read them, T-520).
///
/// # Errors
/// No such workspace.
pub fn listed_projects(workspaces: &Workspaces, w: &str) -> Result<Vec<Value>, AppError> {
    Ok(workspace(workspaces, w)?.listing().projects())
}

/// `GET /api/v2/workspaces`: the workspaces, how many projects each lists, and the one opened last.
#[must_use]
pub fn workspaces_of(workspaces: &Workspaces) -> Value {
    workspaces.summary()
}

/// `GET /api/v2/workspaces/<w>/projects`: its projects with their summaries, worktrees and logos; opening it records
/// it as the last opened.
///
/// # Errors
/// No such workspace.
pub fn projects_of(workspaces: &Workspaces, w: &str) -> Result<Value, AppError> {
    let found = workspace(workspaces, w)?;
    let _ = workspaces.opened(w);
    Ok(json!({"projects": found.listing().projects()}))
}

/// [`projects_of`] for a reload (ATM-768): none when `known` is still the tag of what the list depends on (the
/// listing's stamp, as the route's `ETag`), and then the list isn't worked out.
///
/// # Errors
/// No such workspace.
pub fn fresh_projects(
    workspaces: &Workspaces,
    w: &str,
    known: Option<&str>,
) -> Result<Fresh, AppError> {
    let found = workspace(workspaces, w)?;
    let _ = workspaces.opened(w);
    let listing = found.listing();
    Fresh::of(known, &listing.stamp, || {
        Ok(json!({"projects": listing.projects()}))
    })
}

/// [`tasks_of`] for a reload: none when its task data, its archive, the filters and the other projects its tasks
/// name are as they were (the tasks route's `ETag`). Task data too large is refused before anything else.
///
/// # Errors
/// As [`tasks_of`].
pub fn fresh_tasks(
    workspaces: &Workspaces,
    w: &str,
    p: &str,
    filters: TaskFilters,
    known: Option<&str>,
) -> Result<Fresh, AppError> {
    let member = member(workspaces, w, p)?;
    if too_large(&member.stamp) {
        member.tasks(&Filters::default())?;
    }
    let parts = json!([
        member.stamp["tasks"],
        member.stamp["archive"],
        filters,
        member.others_version()
    ]);
    Fresh::of(known, &parts, || Ok(member.tasks(&filters.into())?))
}

/// [`claims_of`] for a reload: none when the claims store and the claims held now are as they were (an expired lease
/// is left out without the store changing).
///
/// # Errors
/// As [`claims_of`].
pub fn fresh_claims(
    workspaces: &Workspaces,
    w: &str,
    p: &str,
    known: Option<&str>,
) -> Result<Fresh, AppError> {
    let member = member(workspaces, w, p)?;
    let held = member.claims();
    let parts = json!(["claims", member.stamp["claims"], held]);
    Fresh::of(known, &parts, || Ok(json!({"claims": held})))
}

/// [`worktrees_of`] for a reload: none when the working copies and the claims are as they were.
///
/// # Errors
/// As [`worktrees_of`].
pub fn fresh_worktrees(
    workspaces: &Workspaces,
    w: &str,
    p: &str,
    known: Option<&str>,
) -> Result<Fresh, AppError> {
    let member = member(workspaces, w, p)?;
    let parts = json!(["worktrees", member.stamp["copies"], member.stamp["claims"]]);
    Fresh::of(known, &parts, || {
        Ok(json!({"worktrees": member.worktrees()}))
    })
}

/// `GET /api/v2/workspaces/<w>/search?q=`.
///
/// # Errors
/// No such workspace.
pub fn search_in(workspaces: &Workspaces, w: &str, q: &str) -> Result<Value, AppError> {
    Ok(json!(workspace(workspaces, w)?.search(q)))
}

/// `GET …/projects/<p>/tasks`.
///
/// # Errors
/// No such workspace or project, task data too large, a filter naming no such sprint or release.
pub fn tasks_of(
    workspaces: &Workspaces,
    w: &str,
    p: &str,
    filters: TaskFilters,
) -> Result<Value, AppError> {
    Ok(member(workspaces, w, p)?.tasks(&filters.into())?)
}

/// `GET …/projects/<p>/stats` (ATM-722): the slice's IDs or keys (none, the whole project) and the period, in ms.
///
/// # Errors
/// No such workspace or project, task data too large, a period that ends before it starts.
pub fn stats_of(
    workspaces: &Workspaces,
    w: &str,
    p: &str,
    tasks: Option<Vec<String>>,
    from: Option<f64>,
    to: Option<f64>,
) -> Result<Value, AppError> {
    #[allow(clippy::cast_possible_truncation)]
    // reason: ms since 1970 from the page, far inside i64
    let ms = |t: Option<f64>| t.filter(|v| v.is_finite()).map(|v| v as i64);
    let (from, to) = (ms(from), ms(to));
    if let (Some(f), Some(t)) = (from, to)
        && t < f
    {
        return Err(atm_common::Error::Invalid(format!("to ({t}) is before from ({f})")).into());
    }
    let query = atm_lib::stats::StatsQuery {
        tasks: tasks.map(|t| t.into_iter().filter(|id| !id.is_empty()).collect()),
        from,
        to,
    };
    Ok(member(workspaces, w, p)?.stats(&query)?)
}

/// `GET …/projects/<p>/claims`.
///
/// # Errors
/// No such workspace or project.
pub fn claims_of(workspaces: &Workspaces, w: &str, p: &str) -> Result<Value, AppError> {
    Ok(json!({"claims": member(workspaces, w, p)?.claims()}))
}

/// `GET …/projects/<p>/worktrees`.
///
/// # Errors
/// No such workspace or project.
pub fn worktrees_of(workspaces: &Workspaces, w: &str, p: &str) -> Result<Value, AppError> {
    Ok(json!({"worktrees": member(workspaces, w, p)?.worktrees()}))
}

/// `GET …/projects/<p>/documents?path=` (`&at=`: as a commit or tree has it, a review round's side).
///
/// # Errors
/// No such workspace or project; a path atm refuses or can't find; a revision that isn't one.
pub fn document_of(
    workspaces: &Workspaces,
    w: &str,
    p: &str,
    path: &str,
    at: Option<&str>,
) -> Result<Value, AppError> {
    let member = member(workspaces, w, p)?;
    Ok(match at.filter(|a| !a.is_empty()) {
        Some(revision) => member.document_at(path, revision)?,
        None => member.document(path)?,
    })
}

/// `GET …/projects/<p>/documents` without a path (`?q=`): its documents, its submodules, and how many more past the
/// cap (ATM-765).
///
/// # Errors
/// No such workspace or project; a query longer than 200 characters.
pub fn documents_of(workspaces: &Workspaces, w: &str, p: &str, q: &str) -> Result<Value, AppError> {
    Ok(member(workspaces, w, p)?.documents(q)?)
}

/// `GET …/projects/<p>/reviews`: its reviews the filters keep, which need the user.
///
/// # Errors
/// No such workspace or project; a status that isn't one; reviews that can't be read.
pub fn reviews_of(
    workspaces: &Workspaces,
    w: &str,
    p: &str,
    filters: ReviewFilters,
) -> Result<Value, AppError> {
    Ok(member(workspaces, w, p)?.reviews(&filters.into())?)
}

/// `GET …/projects/<p>/reviews/<CR>?round=&against=`: a review's page for one view.
///
/// # Errors
/// No such workspace, project, review or round; a round's tree that can't be had here.
pub fn review_of(
    workspaces: &Workspaces,
    w: &str,
    p: &str,
    review: &str,
    round: Option<u32>,
    against: Option<u32>,
) -> Result<Value, AppError> {
    Ok(member(workspaces, w, p)?.review(&review_ref(review), round, against)?)
}

/// `GET …/projects/<p>/files?path=` (`&at=`: as a commit or tree has it, a review's image file's side, ATM-689).
///
/// # Errors
/// As [`document_of`], and a file that isn't an image.
pub fn file_of(
    workspaces: &Workspaces,
    w: &str,
    p: &str,
    path: &str,
    at: Option<&str>,
) -> Result<FileData, AppError> {
    let member = member(workspaces, w, p)?;
    let (data, content_type) = match at.filter(|a| !a.is_empty()) {
        Some(revision) => member.image_at(path, revision)?,
        None => member.image(path)?,
    };
    Ok(FileData {
        content_type: content_type.into(),
        data,
    })
}

/// `GET …/projects/<p>/logo`: none without one.
///
/// # Errors
/// No such workspace or project; a logo that can't be read.
pub fn logo_of(workspaces: &Workspaces, w: &str, p: &str) -> Result<Option<FileData>, AppError> {
    let Some((file, content_type)) = member(workspaces, w, p)?.logo() else {
        return Ok(None);
    };
    let data = std::fs::read(&file)?;
    Ok(Some(FileData {
        content_type: content_type.into(),
        data,
    }))
}

/// atm's version.
#[tauri::command]
#[specta::specta]
#[must_use]
pub fn version() -> String {
    atm_common::VERSION.to_string()
}

/// The workspaces, how many projects each lists, and the one opened last.
///
/// # Errors
/// None: a `Result`, as every async command with state is.
#[tauri::command]
#[specta::specta]
pub async fn list_workspaces(state: State<'_, AppState>) -> Result<Json, AppError> {
    Ok(Json(workspaces_of(&state.workspaces)))
}

/// A workspace's projects, with their summaries; it becomes the last opened. `known`: the tag the page last got, so
/// an unchanged list comes back without its body (ATM-768).
///
/// # Errors
/// No such workspace.
#[tauri::command]
#[specta::specta]
pub async fn list_projects(
    state: State<'_, AppState>,
    workspace: String,
    known: Option<String>,
) -> Result<Fresh, AppError> {
    fresh_projects(&state.workspaces, &workspace, known.as_deref())
}

/// Search a workspace's projects' tasks.
///
/// # Errors
/// No such workspace.
#[tauri::command]
#[specta::specta]
pub async fn search(
    state: State<'_, AppState>,
    workspace: String,
    q: String,
) -> Result<Json, AppError> {
    search_in(&state.workspaces, &workspace, &q).map(Json)
}

/// A project's tasks, with what atm derives for them; without them when `known` is still their tag.
///
/// # Errors
/// As [`tasks_of`].
#[tauri::command]
#[specta::specta]
pub async fn project_tasks(
    state: State<'_, AppState>,
    workspace: String,
    project: String,
    filters: TaskFilters,
    known: Option<String>,
) -> Result<Fresh, AppError> {
    fresh_tasks(
        &state.workspaces,
        &workspace,
        &project,
        filters,
        known.as_deref(),
    )
}

/// A project's stats, progress and forecast for a slice of its tasks and a period (ATM-722).
///
/// # Errors
/// As [`stats_of`].
#[tauri::command]
#[specta::specta]
pub async fn project_stats(
    state: State<'_, AppState>,
    workspace: String,
    project: String,
    tasks: Option<Vec<String>>,
    from: Option<f64>,
    to: Option<f64>,
) -> Result<Json, AppError> {
    stats_of(&state.workspaces, &workspace, &project, tasks, from, to).map(Json)
}

/// A project's live claims; none when `known` is still their tag.
///
/// # Errors
/// As [`claims_of`].
#[tauri::command]
#[specta::specta]
pub async fn claims(
    state: State<'_, AppState>,
    workspace: String,
    project: String,
    known: Option<String>,
) -> Result<Fresh, AppError> {
    fresh_claims(&state.workspaces, &workspace, &project, known.as_deref())
}

/// A project's working copies; none when `known` is still their tag.
///
/// # Errors
/// As [`worktrees_of`].
#[tauri::command]
#[specta::specta]
pub async fn worktrees(
    state: State<'_, AppState>,
    workspace: String,
    project: String,
    known: Option<String>,
) -> Result<Fresh, AppError> {
    fresh_worktrees(&state.workspaces, &workspace, &project, known.as_deref())
}

/// A document of a project, as the viewer shows it: from the working copy, or `at` a commit or tree.
///
/// # Errors
/// As [`document_of`].
#[tauri::command]
#[specta::specta]
pub async fn document(
    state: State<'_, AppState>,
    workspace: String,
    project: String,
    path: String,
    at: Option<String>,
) -> Result<Json, AppError> {
    document_of(
        &state.workspaces,
        &workspace,
        &project,
        &path,
        at.as_deref(),
    )
    .map(Json)
}

/// A project's documents, for the Documents page (ATM-765): those `q` names (titles and paths), at most 500.
///
/// # Errors
/// As [`documents_of`].
#[tauri::command]
#[specta::specta]
pub async fn project_documents(
    state: State<'_, AppState>,
    workspace: String,
    project: String,
    q: Option<String>,
) -> Result<Json, AppError> {
    documents_of(
        &state.workspaces,
        &workspace,
        &project,
        q.as_deref().unwrap_or_default(),
    )
    .map(Json)
}

/// A project's reviews, for the reviews page.
///
/// # Errors
/// As [`reviews_of`].
#[tauri::command]
#[specta::specta]
pub async fn project_reviews(
    state: State<'_, AppState>,
    workspace: String,
    project: String,
    filters: ReviewFilters,
) -> Result<Json, AppError> {
    reviews_of(&state.workspaces, &workspace, &project, filters).map(Json)
}

/// A review's page, for one view of it.
///
/// # Errors
/// As [`review_of`].
#[tauri::command]
#[specta::specta]
pub async fn project_review(
    state: State<'_, AppState>,
    workspace: String,
    project: String,
    review: String,
    round: Option<u32>,
    against: Option<u32>,
) -> Result<Json, AppError> {
    review_of(
        &state.workspaces,
        &workspace,
        &project,
        &review,
        round,
        against,
    )
    .map(Json)
}

/// An image of a project (`at`: as a commit or tree has it).
///
/// # Errors
/// As [`file_of`].
#[tauri::command]
#[specta::specta]
pub async fn file(
    state: State<'_, AppState>,
    workspace: String,
    project: String,
    path: String,
    at: Option<String>,
) -> Result<FileData, AppError> {
    file_of(
        &state.workspaces,
        &workspace,
        &project,
        &path,
        at.as_deref(),
    )
}

/// A project's logo, if it has one.
///
/// # Errors
/// As [`logo_of`].
#[tauri::command]
#[specta::specta]
pub async fn logo(
    state: State<'_, AppState>,
    workspace: String,
    project: String,
) -> Result<Option<FileData>, AppError> {
    logo_of(&state.workspaces, &workspace, &project)
}
