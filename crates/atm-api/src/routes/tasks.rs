//! A project's tasks, with what atm derives for them (docs/multi-project-api.md §2.5).

use atm_lib::query::Filters;
use atm_lib::workspaces::too_large;
use axum::http::HeaderMap;
use axum::response::Response;
use serde_json::json;

use super::query::parse_query;
use super::source::ProjectSource;
use super::{error_response, etag, not_modified, ok_json, stamped, unchanged};

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
pub(super) fn get_tasks(project: &dyn ProjectSource, query: &str, request: &HeaderMap) -> Response {
    let stamp = project.stamp();
    // Each request reads the whole task data into memory: a runaway file is refused before anything is worked out
    if too_large(stamp)
        && let Err(e) = project.tasks(&Filters::default())
    {
        return error_response(&e);
    }
    let tag = etag(&json!([
        stamp["tasks"],
        stamp["archive"],
        query,
        project.others_version()
    ]));
    if unchanged(request, &tag) {
        return not_modified(&tag, &[]);
    }
    match project.tasks(&task_filters(query)) {
        Ok(snapshot) => ok_json(&stamped(snapshot), Some(&tag)),
        // As every route maps them (ATM-664): a filter naming no such sprint or release 400, an unreadable task file 500
        Err(e) => error_response(&e),
    }
}

#[cfg(test)]
mod tests {
    use atm_common::Error;
    use atm_lib::workspaces::MAX_SERVED_BYTES;
    use axum::http::StatusCode;
    use serde_json::Value;

    use super::super::test_support::{Call, FakeProject, answer, bound, having, plain};
    use super::*;

    #[test]
    fn no_query_keeps_every_task() {
        for query in ["", "unknown=x", "&&", "=x"] {
            assert_eq!(
                format!("{:?}", task_filters(query)),
                format!("{:?}", Filters::default()),
                "{query}"
            );
        }
    }

    #[test]
    fn the_list_filters_repeat_and_the_others_take_the_first() {
        let filters = task_filters(
            "sprint=S1&sprint=S+2&status=todo&status=blocked&label=a&label=b\
             &release=1.0&release=2.0&component=api&type=bugfix&priority=P1&agent=claude%3Aa",
        );
        assert_eq!(filters.sprint, ["S1", "S 2"]);
        assert_eq!(filters.status, ["todo", "blocked"]);
        assert_eq!(filters.label, ["a", "b"]);
        assert_eq!(filters.release.as_deref(), Some("1.0"));
        assert_eq!(filters.component.as_deref(), Some("api"));
        assert_eq!(filters.type_.as_deref(), Some("bugfix"));
        assert_eq!(filters.priority.as_deref(), Some("P1"));
        assert_eq!(filters.agent.as_deref(), Some("claude:a"));
        // A blank value is no value
        assert_eq!(task_filters("sprint=&status=").sprint, Vec::<String>::new());
    }

    #[test]
    fn a_flag_is_on_only_for_true_or_1() {
        type Flag = fn(&Filters) -> bool;
        let flags: [(&str, Flag); 6] = [
            ("backlog", |f| f.backlog),
            ("ready", |f| f.ready),
            ("blocked", |f| f.blocked),
            ("in_pr", |f| f.in_pr),
            ("with_user", |f| f.with_user),
            ("archived", |f| f.archived),
        ];
        for (name, on) in flags {
            for (value, expected) in [
                ("true", true),
                ("TRUE", true),
                ("True", true),
                ("1", true),
                ("0", false),
                ("false", false),
                ("yes", false),
                ("", false),
            ] {
                let filters = task_filters(&format!("{name}={value}"));
                assert_eq!(on(&filters), expected, "{name}={value}");
                // Only its own flag
                let others_on = flags
                    .iter()
                    .filter(|(n, f)| *n != name && f(&filters))
                    .count();
                assert_eq!(others_on, 0, "{name}={value}");
            }
        }
        // The first value counts
        assert!(task_filters("ready=1&ready=0").ready);
        assert!(!task_filters("ready=0&ready=1").ready);
    }

    fn fake(stamp: Value, tasks: atm_common::Result<Value>, others: Value) -> FakeProject {
        FakeProject {
            stamp,
            tasks,
            others,
            ..FakeProject::default()
        }
    }

    #[allow(clippy::needless_pass_by_value)] // reason: the tests write the archive's stamp inline, with json!
    fn stamp(tasks: u64, archive: Value, claims: u64) -> Value {
        json!({"tasks": [tasks, "1"], "archive": archive, "claims": [claims, "1"], "copies": [1]})
    }

    #[test]
    fn the_tasks_are_atm_libs_snapshot_for_the_querys_filters() {
        let snapshot = json!({"tasks": [{"task": {"id": "T-1"}, "derived": {}}], "meta": {"project": "P"}, "size": 1});
        let (project, calls) = bound(fake(
            stamp(1, Value::Null, 1),
            Ok(snapshot.clone()),
            json!({}),
        ));
        let answer = answer(get_tasks(project.as_ref(), "status=todo&ready=1", &plain()));
        assert_eq!(answer.status, StatusCode::OK);
        assert_eq!(answer.unstamped(), snapshot);
        assert!(answer.json()["generated_at"].is_string());
        assert_eq!(
            calls.taken(),
            [
                Call::OthersVersion,
                Call::Tasks(task_filters("status=todo&ready=1"))
            ]
        );
    }

    #[test]
    fn the_tasks_are_a_304_without_being_read_until_what_they_depend_on_changes() {
        let base = || {
            fake(
                stamp(1, Value::Null, 1),
                Ok(json!({"tasks": []})),
                json!({"OTHER": 1}),
            )
        };
        let (project_now, calls) = bound(base());
        let tag = answer(get_tasks(project_now.as_ref(), "status=todo", &plain())).tag();
        calls.taken();
        let again = answer(get_tasks(
            project_now.as_ref(),
            "status=todo",
            &having(&tag),
        ));
        assert_eq!(again.status, StatusCode::NOT_MODIFIED);
        assert_eq!(again.body, b"");
        assert_eq!(calls.taken(), [Call::OthersVersion], "not read");

        // The task data, the archive, the query and the other projects its tasks name each change it; claims don't
        let cases = [
            (
                fake(stamp(2, Value::Null, 1), Ok(json!({})), json!({"OTHER": 1})),
                "status=todo",
                true,
            ),
            (
                fake(
                    stamp(1, json!([5, "1"]), 1),
                    Ok(json!({})),
                    json!({"OTHER": 1}),
                ),
                "status=todo",
                true,
            ),
            (base(), "status=done", true),
            (
                fake(stamp(1, Value::Null, 1), Ok(json!({})), json!({"OTHER": 2})),
                "status=todo",
                true,
            ),
            (
                fake(stamp(1, Value::Null, 9), Ok(json!({})), json!({"OTHER": 1})),
                "status=todo",
                false,
            ),
        ];
        for (fake, query, changed) in cases {
            let (other, _) = bound(fake);
            let status = answer(get_tasks(other.as_ref(), query, &having(&tag))).status;
            assert_eq!(status == StatusCode::OK, changed, "{query} {changed}");
        }
    }

    /// atm-lib's refusals, mapped as every route maps them (ATM-664): a filter naming no such sprint or release is
    /// the request's fault (400), anything else by its kind, an unreadable task file atm's (500)
    #[test]
    fn a_refused_read_is_400_for_a_bad_filter_and_by_its_kind_otherwise() {
        for (error, status) in [
            (
                Error::Invalid("no sprint Nope".into()),
                StatusCode::BAD_REQUEST,
            ),
            (Error::NotFound("no such".into()), StatusCode::NOT_FOUND),
            (Error::Conflict("claimed".into()), StatusCode::CONFLICT),
            (
                Error::failed("unreadable"),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
        ] {
            let (project, _) = bound(fake(
                stamp(1, Value::Null, 1),
                Err(error.clone()),
                json!({}),
            ));
            let answer = answer(get_tasks(project.as_ref(), "sprint=Nope", &plain()));
            assert_eq!(answer.status, status, "{error}");
            assert_eq!(answer.json(), json!({"error": error.to_string()}));
            assert_eq!(answer.tag(), "", "a refusal isn't cached");
        }
    }

    /// Task data over the limit is refused before anything is worked out for it, with atm-lib's word for it
    #[test]
    fn task_data_too_large_is_413_before_anything_is_worked_out() {
        for size in [MAX_SERVED_BYTES + 1, u64::MAX] {
            let too_large = Error::TooLarge("larger than 100 MB".into());
            let (project, calls) =
                bound(fake(stamp(size, Value::Null, 1), Err(too_large), json!({})));
            let answer = answer(get_tasks(
                project.as_ref(),
                "status=todo",
                &having("\"any\""),
            ));
            assert_eq!(answer.status, StatusCode::PAYLOAD_TOO_LARGE);
            assert_eq!(answer.json(), json!({"error": "larger than 100 MB"}));
            assert_eq!(calls.taken(), [Call::Tasks(Filters::default())]);
        }
        // At the limit it is read
        let (at_limit, calls) = bound(fake(
            stamp(MAX_SERVED_BYTES, Value::Null, 1),
            Ok(json!({})),
            json!({}),
        ));
        assert_eq!(
            answer(get_tasks(at_limit.as_ref(), "", &plain())).status,
            StatusCode::OK
        );
        assert_eq!(
            calls.taken(),
            [Call::OthersVersion, Call::Tasks(Filters::default())]
        );
    }
}
