//! Who works where: a project's claims and its worktrees (docs/multi-project-api.md §2.1, §2.2).

use atm_common::py::to_json;
use axum::http::HeaderMap;
use axum::response::Response;
use serde_json::json;

use super::source::ProjectSource;
use super::{etag, stamped};
use crate::send_conditional;

/// `GET …/projects/<p>/claims` (`/p/<p>/claims.json`): the claims of all its worktrees, from the shared claims store.
/// The `ETag` changes with the claims store.
pub(super) fn get_claims(project: &dyn ProjectSource, request: &HeaderMap) -> Response {
    let tag = etag(&json!(["claims", project.stamp()["claims"]]));
    let body = || to_json(&stamped(json!({"claims": project.claims()}))).into_bytes();
    send_conditional(request, &tag, body, "application/json", &[])
}

/// `GET …/projects/<p>/worktrees` (`/p/<p>/worktrees.json`): the working copies, main first, and the tasks that
/// differ in the others; the claims decide which copy of a task is stale, so the `ETag` changes with both.
pub(super) fn get_worktrees(project: &dyn ProjectSource, request: &HeaderMap) -> Response {
    let stamp = project.stamp();
    let tag = etag(&json!(["worktrees", stamp["copies"], stamp["claims"]]));
    let body = || to_json(&stamped(json!({"worktrees": project.worktrees()}))).into_bytes();
    send_conditional(request, &tag, body, "application/json", &[])
}

/// The claiming rules (who may claim a task, from which worktree, for how long) are atm-lib's, and its tests' (ATM-580):
/// here, what the routes make of the claims and worktrees atm-lib gives them.
#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::Value;

    use super::super::test_support::{Call, FakeProject, answer, bound, having, plain};
    use super::*;

    fn claim(task: &str, agent: &str, worktree: &str) -> Value {
        json!({"task": task, "agent": agent, "worktree": worktree,
               "claimed_at": "2026-11-17T10:00:00Z", "lease_expires_at": "2026-11-17T12:00:00Z"})
    }

    /// A project as the fake gives it: its claims and worktrees, and the store's stamp of each
    #[allow(clippy::needless_pass_by_value)] // reason: the tests write the stamps inline, with json!
    fn fake(claims: Vec<Value>, claims_stamp: Value, copies_stamp: Value) -> FakeProject {
        FakeProject {
            stamp: json!({"tasks": [1, 1], "archive": null, "claims": claims_stamp, "copies": copies_stamp}),
            claims,
            worktrees: vec![
                json!({"path": "/main", "main": true}),
                json!({"path": "/wt", "main": false}),
            ],
            ..FakeProject::default()
        }
    }

    #[test]
    fn the_claims_are_atm_libs_as_it_gives_them_none_one_or_several_from_any_worktree() {
        let several = vec![
            claim("T-1", "claude:a", "/main"),
            claim("T-2", "claude:b", "/wt"),
            claim("T-3", "codex:c", "/wt"),
        ];
        for claims in [Vec::new(), vec![claim("T-1", "claude:a", "/main")], several] {
            let (project, calls) = bound(fake(claims.clone(), json!([1, 1]), json!([1])));
            let answer = answer(get_claims(project.as_ref(), &plain()));
            assert_eq!(answer.status, StatusCode::OK);
            assert_eq!(answer.header("content-type"), "application/json");
            assert!(answer.json()["generated_at"].is_string());
            // Every claim, every field, in atm-lib's order: none filtered, none rewritten
            assert_eq!(answer.unstamped(), json!({"claims": claims}));
            assert_eq!(calls.taken(), [Call::Claims]);
        }
    }

    #[test]
    fn the_claims_are_a_304_until_the_claims_store_changes() {
        let (project, calls) = bound(fake(Vec::new(), json!([1, 1]), json!([1])));
        let tag = answer(get_claims(project.as_ref(), &plain())).tag();
        calls.taken();
        let again = answer(get_claims(project.as_ref(), &having(&tag)));
        assert_eq!(again.status, StatusCode::NOT_MODIFIED);
        assert_eq!(again.tag(), tag);
        assert_eq!(again.body, b"");
        assert_eq!(calls.taken(), [], "a 304 doesn't read the claims");
        // Another version the client has: the claims again
        assert_eq!(
            answer(get_claims(project.as_ref(), &having("\"other\""))).status,
            StatusCode::OK
        );

        // A claim made or released anywhere changes the claims store's stamp, and so the tag
        let (claimed, _) = bound(fake(
            vec![claim("T-1", "claude:a", "/wt")],
            json!([2, 2]),
            json!([1]),
        ));
        let after = answer(get_claims(claimed.as_ref(), &having(&tag)));
        assert_eq!(after.status, StatusCode::OK);
        assert_ne!(after.tag(), tag);
        // A change to the worktrees alone doesn't
        let (moved, _) = bound(fake(Vec::new(), json!([1, 1]), json!([9])));
        assert_eq!(
            answer(get_claims(moved.as_ref(), &having(&tag))).status,
            StatusCode::NOT_MODIFIED
        );
    }

    #[test]
    fn the_worktrees_are_atm_libs_and_a_304_until_the_copies_or_the_claims_change() {
        let (project, calls) = bound(fake(Vec::new(), json!([1, 1]), json!([1])));
        let first = answer(get_worktrees(project.as_ref(), &plain()));
        assert_eq!(first.status, StatusCode::OK);
        assert_eq!(
            first.unstamped(),
            json!({"worktrees": [{"path": "/main", "main": true}, {"path": "/wt", "main": false}]})
        );
        assert_eq!(calls.taken(), [Call::Worktrees]);
        let tag = first.tag();
        assert_eq!(
            answer(get_worktrees(project.as_ref(), &having(&tag))).status,
            StatusCode::NOT_MODIFIED
        );
        assert_eq!(calls.taken(), [], "a 304 doesn't read the worktrees");
        // The claims decide which copy of a task is stale: either changing is a new version; the tasks alone aren't
        for (claims, copies, changed) in [
            (json!([2, 2]), json!([1]), true),
            (json!([1, 1]), json!([2]), true),
            (json!([1, 1]), json!([1]), false),
        ] {
            let (other, _) = bound(FakeProject {
                stamp: json!({"tasks": [5, 5], "archive": null, "claims": claims, "copies": copies}),
                ..FakeProject::default()
            });
            let status = answer(get_worktrees(other.as_ref(), &having(&tag))).status;
            assert_eq!(status == StatusCode::OK, changed, "{claims} {copies}");
        }
    }

    #[test]
    fn the_claims_and_the_worktrees_are_tagged_apart() {
        let (project, _) = bound(fake(Vec::new(), json!([1, 1]), json!([1])));
        let claims = answer(get_claims(project.as_ref(), &plain())).tag();
        let worktrees = answer(get_worktrees(project.as_ref(), &plain())).tag();
        assert_ne!(claims, worktrees);
        assert_eq!(
            answer(get_worktrees(project.as_ref(), &having(&claims))).status,
            StatusCode::OK
        );
    }

    #[test]
    fn a_store_with_no_claims_yet_still_answers() {
        let (project, _) = bound(fake(Vec::new(), Value::Null, Value::Null));
        assert_eq!(
            answer(get_claims(project.as_ref(), &plain())).status,
            StatusCode::OK
        );
        assert_eq!(
            answer(get_worktrees(project.as_ref(), &plain())).status,
            StatusCode::OK
        );
    }
}
