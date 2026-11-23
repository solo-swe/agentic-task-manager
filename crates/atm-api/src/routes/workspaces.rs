//! The workspaces, a workspace's projects with their summaries, and search across them (docs/multi-project-api.md
//! §2.4, §2.8, §3).

use atm_common::py::to_json;
use axum::http::HeaderMap;
use axum::response::Response;
use serde_json::json;

use super::query::parameter;
use super::source::{ProjectList, WorkspaceSource};
use super::{Projects, etag, ok_json, stamped};
use crate::send_conditional;

/// `GET /api/v2/workspaces`: the workspaces, with how many projects each lists, and the one opened last.
pub(super) fn get_workspaces(projects: &Projects) -> Response {
    ok_json(&stamped(projects.source.summary()), None)
}

/// `GET …/<w>/projects` (`/api/projects` for `default`): a workspace's projects, with their summaries. The `ETag`
/// changes with its file, and with each project's task data, claims, worktrees, logo and the day; the list is made
/// only when it changed.
pub(super) fn get_projects(workspace: &dyn WorkspaceSource, request: &HeaderMap) -> Response {
    let ProjectList { stamp, projects } = workspace.projects();
    let body = move || to_json(&stamped(json!({"projects": projects()}))).into_bytes();
    send_conditional(request, &etag(&stamp), body, "application/json", &[])
}

/// `GET …/<w>/search?q=` (`/api/search`): per project, the IDs of the working tasks (not the archive) in which every
/// word of the query appears, case-insensitively, as the dashboard's own search finds them; the project's name counts
/// as part of every task's text. `{}` for an empty query.
pub(super) fn get_search(workspace: &dyn WorkspaceSource, query: &str) -> Response {
    // Stamped as every JSON answer is (ATM-672, audit H13); a reader takes the project entries, lists, beside it
    ok_json(
        &stamped(json!(workspace.search(&parameter(query, "q")))),
        None,
    )
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::{Map, Value};

    use super::super::test_support::{
        Call, Fake, FakeProject, FakeWorkspace, answer, having, plain,
    };
    use super::*;

    fn fake(stamp: Value, projects: Vec<Value>, search: Map<String, Value>) -> Fake {
        let mut fake = Fake::with(FakeProject::default());
        fake.workspaces[1].1 = FakeWorkspace {
            stamp,
            projects,
            search,
            ..FakeWorkspace::default()
        };
        fake
    }

    #[test]
    fn the_workspaces_are_atm_libs_summary_stamped_and_never_cached() {
        let mut fake = Fake::with(FakeProject::default());
        fake.summary = json!({"workspaces": [{"id": "default", "projects": 3}, {"id": "work", "projects": 0}],
                              "recent": "work"});
        let (projects, calls) = fake.projects();
        let answer = answer(get_workspaces(&projects));
        assert_eq!(answer.status, StatusCode::OK);
        assert_eq!(
            answer.unstamped(),
            json!({"workspaces": [{"id": "default", "projects": 3}, {"id": "work", "projects": 0}],
                   "recent": "work"})
        );
        assert_eq!(
            answer
                .json()
                .as_object()
                .and_then(|o| o.keys().next().cloned())
                .as_deref(),
            Some("generated_at")
        );
        assert_eq!(answer.tag(), "");
        assert_eq!(answer.header("cache-control"), "no-store");
        assert_eq!(calls.taken(), []);
    }

    #[test]
    fn a_workspaces_projects_none_or_several_are_listed_as_atm_lib_lists_them() {
        for list in [
            Vec::new(),
            vec![json!({"id": "a"})],
            vec![json!({"id": "b"}), json!({"id": "a", "missing": true})],
        ] {
            let (projects, calls) = fake(json!(["s"]), list.clone(), Map::new()).projects();
            let work = projects.source.workspace("work").expect("work");
            let answer = answer(get_projects(work.as_ref(), &plain()));
            assert_eq!(answer.status, StatusCode::OK);
            assert_eq!(answer.unstamped(), json!({"projects": list}));
            assert_ne!(answer.tag(), "");
            assert_eq!(calls.taken(), [Call::ListedProjects("work".into())]);
        }
    }

    #[test]
    fn a_workspaces_projects_are_a_304_without_being_listed_until_its_stamp_changes() {
        let (projects, calls) =
            fake(json!(["s", 1]), vec![json!({"id": "a"})], Map::new()).projects();
        let work = projects.source.workspace("work").expect("work");
        let tag = answer(get_projects(work.as_ref(), &plain())).tag();
        calls.taken();
        let again = answer(get_projects(work.as_ref(), &having(&tag)));
        assert_eq!(again.status, StatusCode::NOT_MODIFIED);
        assert_eq!(again.body, b"");
        assert_eq!(calls.taken(), [], "the list isn't made for a 304");
        let (changed, _) = fake(json!(["s", 2]), vec![json!({"id": "a"})], Map::new()).projects();
        let work = changed.source.workspace("work").expect("work");
        assert_eq!(
            answer(get_projects(work.as_ref(), &having(&tag))).status,
            StatusCode::OK
        );
    }

    #[test]
    fn search_asks_atm_lib_for_the_q_parameter_decoded_and_answers_what_it_finds() {
        let mut found = Map::new();
        found.insert("p1".into(), json!(["T-1", "T-7"]));
        found.insert("p2".into(), json!(["T-3"]));
        for (query, asked) in [
            ("q=needle", "needle"),
            ("q=two+words", "two words"),
            ("q=%C3%BCber%21", "über!"),
            ("other=x&q=late", "late"),
            ("q=first&q=second", "first"),
            ("q=", ""),
            ("", ""),
        ] {
            let (projects, calls) = fake(Value::Null, Vec::new(), found.clone()).projects();
            let work = projects.source.workspace("work").expect("work");
            let answer = answer(get_search(work.as_ref(), query));
            assert_eq!(answer.status, StatusCode::OK, "{query}");
            // Stamped as every JSON answer is (ATM-672)
            assert!(answer.json()["generated_at"].is_string(), "{query}");
            assert_eq!(answer.unstamped(), json!(found), "{query}");
            assert_eq!(answer.tag(), "", "{query}: a search isn't cached");
            assert_eq!(
                calls.taken(),
                [Call::Searched("work".into(), asked.into())],
                "{query}"
            );
        }
    }

    #[test]
    fn a_search_that_finds_nothing_is_an_empty_object() {
        let (projects, _) = fake(Value::Null, Vec::new(), Map::new()).projects();
        let work = projects.source.workspace("work").expect("work");
        assert_eq!(
            answer(get_search(work.as_ref(), "q=x")).unstamped(),
            json!({})
        );
    }
}
