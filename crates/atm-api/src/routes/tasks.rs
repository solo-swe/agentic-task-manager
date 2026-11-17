//! A project's tasks, with what atm derives for them (docs/multi-project-api.md §2.5).

use atm_lib::query::Filters;
use atm_lib::workspaces::{Member, too_large};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use serde_json::json;

use super::query::parse_query;
use super::{error_response, etag, not_modified, ok_json, stamped, unchanged};
use crate::send_error;

/// atm-lib's filters from the route's query string (docs/layered-architecture.md §6.2): the list filters repeatable,
/// the flags `true` or `1`, other parameters ignored.
fn task_filters(query: &str) -> Filters {
    let given = parse_query(query);
    let all = |name: &str| -> Vec<String> {
        given
            .iter()
            .filter(|(n, _)| n == name)
            .map(|(_, v)| v.clone())
            .collect()
    };
    let first = |name: &str| all(name).into_iter().next();
    let flag =
        |name: &str| first(name).is_some_and(|v| matches!(v.to_lowercase().as_str(), "true" | "1"));
    Filters {
        sprint: all("sprint"),
        backlog: flag("backlog"),
        release: first("release"),
        component: first("component"),
        status: all("status"),
        type_: first("type"),
        priority: first("priority"),
        label: all("label"),
        agent: first("agent"),
        ready: flag("ready"),
        blocked: flag("blocked"),
        in_pr: flag("in_pr"),
        with_user: flag("with_user"),
        archived: flag("archived"),
    }
}

/// `GET …/projects/<p>/tasks`: the tasks the filters keep, each with its derived values, and in the same snapshot its
/// meta and how big its task data is. Read tolerantly: the dashboard shows a malformed task file, with what is wrong
/// with it (T-377). The `ETag` changes with the task data, the archive, the query, and the task data of the other
/// projects its tasks name (docs/cross-project-freshness-design.md §5.3 D): a poll sees another project's change, and
/// still gets a 304 for anything else.
pub(super) fn get_tasks(member: &Member, query: &str, request: &HeaderMap) -> Response {
    if too_large(&member.stamp) {
        // Each request reads the whole task data into memory: a runaway file is refused, not served
        return error_response(&member.tasks(&Filters::default()).unwrap_err());
    }
    let tag = etag(&json!([
        member.stamp["tasks"],
        member.stamp["archive"],
        query,
        member.others_version()
    ]));
    if unchanged(request, &tag) {
        return not_modified(&tag, &[]);
    }
    match member.tasks(&task_filters(query)) {
        Ok(snapshot) => ok_json(&stamped(snapshot), Some(&tag)),
        // A filter naming no such sprint or release (400), an unreadable task file (500)
        Err(e) => {
            let status = if e.code() == 2 {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            send_error(status, &e.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{a_state_with_a_project, answer, having, member};
    use super::*;

    #[test]
    fn the_filters_are_read_from_the_query() {
        let filters = task_filters("sprint=S1&ready=TRUE&archived=0&type=bugfix");
        assert_eq!(filters.sprint, ["S1"]);
        assert!(filters.ready && !filters.archived);
        assert_eq!(filters.type_.as_deref(), Some("bugfix"));
        let filters =
            task_filters("status=todo&status=blocked&label=a&label=b&backlog=1&with_user=no");
        assert_eq!(filters.status, ["todo", "blocked"]);
        assert_eq!(filters.label, ["a", "b"]);
        assert!(filters.backlog && !filters.with_user);
    }

    #[test]
    fn the_tasks_the_filters_keep_and_a_304_while_unchanged() {
        let home = a_state_with_a_project("tasks");
        let member = member(&home);
        let ids = |query: &str| -> Vec<serde_json::Value> {
            answer(get_tasks(&member, query, &HeaderMap::new())).json()["tasks"]
                .as_array()
                .expect("the tasks")
                .iter()
                .map(|view| view["task"]["id"].clone())
                .collect()
        };
        assert_eq!(ids(""), [json!("T-1")]);
        assert_eq!(ids("status=todo"), [json!("T-1")]);
        assert_eq!(ids("status=done"), [] as [serde_json::Value; 0]);
        let first = answer(get_tasks(&member, "", &HeaderMap::new()));
        assert_eq!(first.json()["meta"]["project"], json!("Routes"));
        let tag = first.header("etag").to_string();
        assert_eq!(
            answer(get_tasks(&member, "", &having(&tag))).status,
            StatusCode::NOT_MODIFIED
        );
        // Another query is another answer
        assert_eq!(
            answer(get_tasks(&member, "status=todo", &having(&tag))).status,
            StatusCode::OK
        );
        std::fs::remove_dir_all(&home).expect("cleaned up");
    }

    #[test]
    fn a_filter_naming_no_such_sprint_is_a_bad_request() {
        let home = a_state_with_a_project("tasks-refused");
        let answer = answer(get_tasks(&member(&home), "sprint=Nope", &HeaderMap::new()));
        assert_eq!(answer.status, StatusCode::BAD_REQUEST, "{}", answer.json());
        assert!(
            answer.json()["error"]
                .as_str()
                .is_some_and(|e| e.contains("Nope"))
        );
        std::fs::remove_dir_all(&home).expect("cleaned up");
    }
}
