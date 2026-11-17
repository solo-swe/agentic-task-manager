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

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;

    use super::super::test_support::{a_state_with_a_project, answer, having, member};
    use super::*;

    #[test]
    fn a_project_with_no_agent_at_work_has_no_claims() {
        let home = a_state_with_a_project("claims");
        let member = member(&home);
        let first = answer(get_claims(&member, &HeaderMap::new()));
        assert_eq!(first.status, StatusCode::OK);
        assert_eq!(first.json(), json!({"claims": []}));
        let tag = first.header("etag").to_string();
        assert_eq!(
            answer(get_claims(&member, &having(&tag))).status,
            StatusCode::NOT_MODIFIED
        );
        std::fs::remove_dir_all(&home).expect("cleaned up");
    }

    #[test]
    fn the_worktrees_are_the_main_working_copy_first() {
        let home = a_state_with_a_project("worktrees");
        let member = member(&home);
        let first = answer(get_worktrees(&member, &HeaderMap::new()));
        assert_eq!(first.status, StatusCode::OK);
        let worktrees = first.json()["worktrees"].clone();
        assert_eq!(worktrees.as_array().map(Vec::len), Some(1), "{worktrees}");
        let tag = first.header("etag").to_string();
        assert_eq!(
            answer(get_worktrees(&member, &having(&tag))).status,
            StatusCode::NOT_MODIFIED
        );
        // The claims' and the worktrees' versions are tagged apart
        assert_ne!(
            tag,
            answer(get_claims(&member, &HeaderMap::new())).header("etag")
        );
        std::fs::remove_dir_all(&home).expect("cleaned up");
    }
}
