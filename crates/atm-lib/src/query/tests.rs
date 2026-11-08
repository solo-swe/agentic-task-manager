use atm_db::MemoryStore;
use serde_json::{Value, json};

use super::*;

fn task(value: Value) -> Task {
    match value {
        Value::Object(map) => map,
        other => panic!("not a task: {other}"),
    }
}

fn project_with(meta: Value, tasks: Value, archive: Value) -> Project {
    let list = |v: Value| match v {
        Value::Array(items) => items.into_iter().map(task).collect(),
        other => panic!("not a list: {other}"),
    };
    Project::new(Box::new(
        MemoryStore::new(list(tasks), list(archive), &task(meta)).unwrap(),
    ))
}

fn project(tasks: Value) -> Project {
    project_with(json!({}), tasks, json!([]))
}

fn ids(p: &Project, filters: &Filters) -> Vec<String> {
    list_matching(p, filters)
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn ready_and_blocked_follow_the_dependencies() {
    let p = project(json!([
        {"id": "T-1", "status": "done"},
        {"id": "T-2", "status": "todo", "dependencies": ["T-1"]},
        {"id": "T-3", "status": "todo", "dependencies": ["T-2"]},
        {"id": "T-4", "status": "blocked"}
    ]));
    assert_eq!(
        ids(
            &p,
            &Filters {
                ready: true,
                ..Filters::default()
            }
        ),
        ["T-2"]
    );
    assert_eq!(
        ids(
            &p,
            &Filters {
                blocked: true,
                ..Filters::default()
            }
        ),
        ["T-3", "T-4"]
    );
}

#[test]
fn filters_combine_and_labels_need_all() {
    let p = project(json!([
        {"id": "T-1", "type": "test", "status": "done", "labels": ["api"]},
        {"id": "T-2", "type": "test", "status": "todo", "priority": "P0", "labels": ["api", "ux"]},
        {"id": "T-3", "type": "test", "status": "todo", "labels": ["api", "ux"]},
        {"id": "T-4", "type": "docs", "status": "todo", "labels": ["api", "ux"]}
    ]));
    let filters = Filters {
        label: vec!["api".into(), "ux".into()],
        status: vec!["todo".into()],
        priority: Some("P2".into()),
        type_: Some("test".into()),
        ..Filters::default()
    };
    assert_eq!(ids(&p, &filters), ["T-3"]);
    // An empty text keeps everything, as in Python
    assert_eq!(
        ids(
            &p,
            &Filters {
                type_: Some(String::new()),
                ..Filters::default()
            }
        )
        .len(),
        4
    );
}

#[test]
fn sprints_backlog_release_and_component() {
    let p = project_with(
        json!({"sprints": [{"id": "Alpha", "release": "1.0"}], "releases": [{"id": "1.0"}, {"id": "2.0"}]}),
        json!([
            {"id": "T-1", "sprint": "alpha", "components": ["api"]},
            {"id": "T-2", "release": "2.0"},
            {"id": "T-3", "sprint": "Gone"}
        ]),
        json!([]),
    );
    assert_eq!(
        ids(
            &p,
            &Filters {
                sprint: vec!["ALPHA".into()],
                ..Filters::default()
            }
        ),
        ["T-1"]
    );
    // A sprint the project doesn't have counts as the backlog
    assert_eq!(
        ids(
            &p,
            &Filters {
                backlog: true,
                ..Filters::default()
            }
        ),
        ["T-2", "T-3"]
    );
    assert_eq!(
        ids(
            &p,
            &Filters {
                release: Some("1.0".into()),
                ..Filters::default()
            }
        ),
        ["T-1"]
    );
    assert_eq!(
        ids(
            &p,
            &Filters {
                component: Some("api".into()),
                ..Filters::default()
            }
        ),
        ["T-1"]
    );
    let unknown = list_matching(
        &p,
        &Filters {
            sprint: vec!["Nope".into()],
            ..Filters::default()
        },
    )
    .unwrap_err();
    assert_eq!(
        (unknown.to_string(), unknown.code()),
        ("no sprint 'Nope'; the sprints: Alpha".into(), 2)
    );
    assert!(
        list_matching(
            &p,
            &Filters {
                release: Some("9".into()),
                ..Filters::default()
            }
        )
        .is_err()
    );
}

#[test]
fn agents_pull_requests_and_work_together() {
    let p = project(json!([
        {"id": "T-1", "status": "done", "started_by": "alice", "pr": {"number": 1}},
        {"id": "T-2", "status": "done", "agent": "alice", "pr": {"number": 1}, "merged_at": "x"},
        {"id": "T-3", "status": "todo", "with": "user"}
    ]));
    assert_eq!(
        ids(
            &p,
            &Filters {
                agent: Some("alice".into()),
                ..Filters::default()
            }
        ),
        ["T-1", "T-2"]
    );
    assert_eq!(
        ids(
            &p,
            &Filters {
                in_pr: true,
                ..Filters::default()
            }
        ),
        ["T-1"]
    );
    assert_eq!(
        ids(
            &p,
            &Filters {
                with_user: true,
                ..Filters::default()
            }
        ),
        ["T-3"]
    );
}

#[test]
fn the_archive_lists_instead_with_archived() {
    let p = project_with(
        json!({}),
        json!([{"id": "T-2"}]),
        json!([{"id": "T-1", "status": "done"}]),
    );
    assert_eq!(
        ids(
            &p,
            &Filters {
                archived: true,
                ..Filters::default()
            }
        ),
        ["T-1"]
    );
}

#[test]
fn views_derive_what_applies_and_leave_out_the_rest() {
    let p = project_with(
        json!({}),
        json!([
            {"id": "T-2", "status": "todo", "dependencies": ["T-1"]},
            {"id": "T-3", "status": "todo", "dependencies": ["T-2"], "priority": "P0"},
            {"id": "T-4", "status": "blocked", "waiting_on": "user"},
            {"id": "T-5", "status": "done", "pr": {"number": 2}}
        ]),
        json!([{"id": "T-1", "status": "done"}]),
    );
    let views = task_views(&p, &Filters::default()).unwrap();
    assert_eq!(
        views[0]["derived"],
        json!({
            "kind": "todo", "ready": true, "waiting_for": [], "priority": "P2", "effective_review": "none", "archived": false
        })
    );
    assert_eq!(views[1]["derived"]["waiting_for"], json!(["T-2"]));
    assert_eq!(views[2]["derived"]["waiting_on"], json!("user"));
    assert_eq!(views[3]["derived"]["stage"], json!("in_pr"));
    assert_eq!(
        views[0]["derived"]
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>(),
        [
            "kind",
            "ready",
            "waiting_for",
            "priority",
            "effective_review",
            "archived"
        ]
    );
}

#[test]
fn deps_say_what_a_task_waits_for_and_what_waits_for_it_archived_ones_too() {
    let p = project_with(
        json!({}),
        json!([
            {"id": "T-2", "name": "Two", "status": "todo", "dependencies": ["T-1", "T-9"]},
            {"id": "T-5", "name": "Live", "status": "todo", "dependencies": ["T-2"]}
        ]),
        json!([
            {"id": "T-1", "name": "Old", "status": "done"},
            {"id": "T-3", "name": "Older", "status": "done", "dependencies": ["T-2"]}
        ]),
    );
    assert_eq!(
        task_dependencies(&p, "T-2").unwrap(),
        json!({
            "id": "T-2",
            "waits_for": [
                {"id": "T-1", "status": "done, archived", "name": "Old"},
                {"id": "T-9", "status": "missing", "name": ""}
            ],
            "waited_on_by": [
                {"id": "T-5", "status": "todo", "name": "Live"},
                {"id": "T-3", "status": "done, archived", "name": "Older"}
            ]
        })
    );
    assert_eq!(
        task_dependencies(&p, "T-1").unwrap()["waits_for"],
        json!([])
    );
    assert_eq!(
        task_dependencies(&p, "T-7").unwrap_err(),
        Error::NotFound("no task T-7".into())
    );
}
