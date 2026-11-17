//! Who works where: a project's claims and its worktrees (docs/multi-project-api.md §2.1, §2.2).

use atm_common::py::to_json;
use atm_lib::workspaces::Member;
use axum::http::HeaderMap;
use axum::response::Response;
use serde_json::json;

use super::{etag, stamped};
use crate::send_conditional;

/// `GET …/projects/<p>/claims` (`/p/<p>/claims.json`): the claims of all its worktrees, from the shared claims store.
pub(super) fn get_claims(member: &Member, request: &HeaderMap) -> Response {
    let tag = etag(&json!(["claims", member.stamp["claims"]]));
    let body = || to_json(&stamped(json!({"claims": member.claims()}))).into_bytes();
    send_conditional(request, &tag, body, "application/json", &[])
}

/// `GET …/projects/<p>/worktrees` (`/p/<p>/worktrees.json`): the working copies, main first, and the tasks that
/// differ in the others; the claims decide which copy of a task is stale.
pub(super) fn get_worktrees(member: &Member, request: &HeaderMap) -> Response {
    let stamp = &member.stamp;
    let tag = etag(&json!(["worktrees", stamp["copies"], stamp["claims"]]));
    let body = || to_json(&stamped(json!({"worktrees": member.worktrees()}))).into_bytes();
    send_conditional(request, &tag, body, "application/json", &[])
}
