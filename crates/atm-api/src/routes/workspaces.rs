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

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;

    use super::super::test_support::{PROJECT, a_state_with_a_project, answer, having};
    use super::*;

    #[test]
    fn the_workspaces_are_listed_with_how_many_projects_each_has() {
        let home = a_state_with_a_project("workspaces");
        let answer = answer(get_workspaces(&Projects::new(&home)));
        assert_eq!(answer.status, StatusCode::OK);
        assert_eq!(
            answer.json(),
            json!({"workspaces": [{"id": "default", "projects": 1}, {"id": "work", "projects": 1}],
                   "recent": "default"})
        );
        std::fs::remove_dir_all(&home).expect("cleaned up");
    }

    #[test]
    fn a_workspaces_projects_are_listed_and_a_304_while_unchanged() {
        let home = a_state_with_a_project("projects");
        let projects = Projects::new(&home);
        let work = projects.workspaces.get("work").expect("work");
        let first = answer(get_projects(&work, &HeaderMap::new()));
        assert_eq!(first.status, StatusCode::OK);
        let listed = first.json();
        assert_eq!(listed["projects"][0]["id"], json!(PROJECT));
        assert_eq!(listed["projects"][0]["name"], json!("Routes"));
        let tag = first.header("etag").to_string();
        let again = answer(get_projects(&work, &having(&tag)));
        assert_eq!(again.status, StatusCode::NOT_MODIFIED);
        assert_eq!(again.body, b"");
        std::fs::remove_dir_all(&home).expect("cleaned up");
    }

    #[test]
    fn search_finds_every_word_and_nothing_for_an_empty_query() {
        let home = a_state_with_a_project("search");
        let default = Projects::new(&home).workspaces.default_workspace();
        let found = |query: &str| answer(get_search(&default, query)).json();
        assert_eq!(found("q=NEEDLE+find"), json!({PROJECT: ["T-1"]}));
        // The project's name counts as part of every task's text
        assert_eq!(found("q=routes"), json!({PROJECT: ["T-1"]}));
        assert_eq!(found("q=needle+haystack"), json!({}));
        assert_eq!(found(""), json!({}));
        std::fs::remove_dir_all(&home).expect("cleaned up");
    }
}
