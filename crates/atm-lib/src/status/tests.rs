use atm_db::MemoryStore;
use serde_json::{Value, json};

use super::*;

fn task(value: Value) -> Task {
    match value {
        Value::Object(map) => map,
        other => panic!("not a task: {other}"),
    }
}

fn project(meta: Value, tasks: Value, claims: Value) -> Project {
    let list: Vec<Task> = match tasks {
        Value::Array(items) => items.into_iter().map(task).collect(),
        other => panic!("not a list: {other}"),
    };
    let store = MemoryStore::new(list, Vec::new(), &task(meta))
        .unwrap()
        .with_claims(task(claims));
    Project::new(Box::new(store))
}

fn store(claims: Value, waiting: Value) -> Value {
    let mut store = task(json!({"completed": {}, "resolved": {}, "hints": {}}));
    store.insert("claims".into(), claims);
    store.insert("waiting".into(), waiting);
    Value::Object(store)
}

#[test]
fn the_report_counts_and_lists_what_waits() {
    let p = project(
        json!({"sprints": [{"id": "Alpha", "state": "active", "goal": "Ship"}]}),
        json!([
            {"id": "T-1", "name": "One", "type": "feature", "status": "done", "sprint": "Alpha", "labels": ["api"]},
            {"id": "T-2", "name": "Two", "type": "feature", "status": "in_progress", "sprint": "Alpha", "started_by": "bob"},
            {"id": "T-3", "name": "Three", "type": "bugfix", "status": "todo", "sprint": "Alpha", "dependencies": ["T-2"]},
            {"id": "T-4", "name": "Four", "status": "blocked", "waiting_on": "user", "questions": ["Why?"]},
            {"id": "T-5", "name": "Five", "status": "in_review", "review_requested_at": "2026-01-01T00:00:00Z"},
            {"id": "T-6", "name": "Six", "status": "todo", "sprint": "Alpha"}
        ]),
        store(
            json!({"T-2": {"task": "T-2", "agent": "claude:x"}}),
            json!({}),
        ),
    );
    let StatusReport {
        report,
        has_sprints,
    } = status_report(&p, &Filters::default()).unwrap();
    assert!(has_sprints);
    assert_eq!(
        report["sprint"],
        json!({"id": "Alpha", "goal": "Ship", "done": 1, "tasks": 4, "review_pending": []})
    );
    assert_eq!(report["tasks"], json!(6));
    assert_eq!(
        report["by_status"],
        json!({"todo": 2, "in_progress": 1, "in_review": 1, "blocked": 1, "done": 1})
    );
    assert_eq!(
        report["by_status"]
            .as_object()
            .unwrap()
            .keys()
            .next()
            .unwrap(),
        "todo",
        "in the statuses' order"
    );
    assert_eq!(report["by_sprint"], json!({"Alpha": 4, "backlog": 2}));
    assert_eq!(
        report["by_type"],
        json!({"feature": 2, "bugfix": 1, "None": 3})
    );
    assert_eq!(report["by_label"], json!({"api": 1}));
    assert_eq!(
        report["in_progress"],
        json!([{"id": "T-2", "name": "Two", "by": "claude:x"}])
    );
    assert_eq!(
        report["waiting_on_dependencies"],
        json!([{"id": "T-3", "name": "Three", "on": ["T-2"]}])
    );
    assert_eq!(
        report["blocked"],
        json!([{"id": "T-4", "name": "Four", "waiting_on": "user", "questions": ["Why?"]}])
    );
    assert_eq!(
        report["in_review"],
        json!([{"id": "T-5", "name": "Five", "since": "2026-01-01T00:00:00Z"}])
    );
    assert_eq!(report["next"], json!({"id": "T-6", "name": "Six"}));
    assert_eq!(report["next_reason"], Value::Null);
    assert_eq!(report["largest_tasks"].as_array().unwrap().len(), 3);
    assert!(report.get("suggest").is_none() && report.get("open_in_closed_sprints").is_none());
}

#[test]
fn work_waiting_in_another_worktree_is_listed_with_its_branch() {
    let p = project(
        json!({}),
        json!([{"id": "T-1", "name": "One", "status": "todo"}]),
        store(
            json!({}),
            json!({"T-1": {"task": "T-1", "worktree": "/elsewhere", "branch": "b", "status": "blocked", "waiting_on": "user"}}),
        ),
    );
    let report = status_report(&p, &Filters::default()).unwrap().report;
    assert_eq!(
        report["blocked"],
        json!([{"id": "T-1", "name": "One", "branch": "b", "waiting_on": "user", "questions": []}])
    );
    assert_eq!(
        report["next_reason"],
        json!(
            "1 todo task(s) waiting on dependencies; blocked or in review in another worktree: T-1"
        )
    );
}

#[test]
fn the_scope_narrows_the_counts_and_names_sprints_that_exist() {
    let p = project(
        json!({"sprints": [{"id": "Alpha"}]}),
        json!([{"id": "T-1", "status": "todo", "sprint": "Alpha"}, {"id": "T-2", "status": "todo"}]),
        store(json!({}), json!({})),
    );
    let backlog = Filters {
        backlog: true,
        ..Filters::default()
    };
    let report = status_report(&p, &backlog).unwrap().report;
    assert_eq!(report["tasks"], json!(1));
    assert_eq!(report["sprint"], Value::Null);
    let bad = Filters {
        sprint: vec!["Nope".into()],
        ..Filters::default()
    };
    assert_eq!(status_report(&p, &bad).unwrap_err().code(), 2);
}
