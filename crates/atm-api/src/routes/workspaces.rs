//! The workspaces, a workspace's projects with their summaries, and search across them (docs/multi-project-api.md
//! §2.4, §2.8, §3).

use atm_common::py::to_json;
use atm_lib::workspaces::Workspace;
use axum::http::HeaderMap;
use axum::response::Response;
use serde_json::json;

use super::query::parameter;
use super::{Projects, etag, ok_json, stamped};
use crate::send_conditional;

/// `GET /api/v2/workspaces`: the workspaces, with how many projects each lists, and the one opened last.
pub(super) fn get_workspaces(projects: &Projects) -> Response {
    ok_json(&stamped(projects.workspaces.summary()), None)
}

/// `GET …/<w>/projects` (`/api/projects` for `default`): a workspace's projects, with their summaries. The `ETag`
/// changes with its file, and with each project's task data, claims, worktrees, logo and the day.
pub(super) fn get_projects(workspace: &Workspace, request: &HeaderMap) -> Response {
    let listing = workspace.listing();
    let tag = etag(&listing.stamp);
    let body = || to_json(&stamped(json!({"projects": listing.projects()}))).into_bytes();
    send_conditional(request, &tag, body, "application/json", &[])
}

/// `GET …/<w>/search?q=` (`/api/search`): per project, the IDs of the working tasks (not the archive) in which every
/// word of the query appears, case-insensitively, as the dashboard's own search finds them; the project's name counts
/// as part of every task's text. `{}` for an empty query.
pub(super) fn get_search(workspace: &Workspace, query: &str) -> Response {
    ok_json(&json!(workspace.search(&parameter(query, "q"))), None)
}
