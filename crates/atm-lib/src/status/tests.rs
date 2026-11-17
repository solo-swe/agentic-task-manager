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

fn report_of(meta: Value, tasks: Value, claims: Value) -> Value {
    status_report(&project(meta, tasks, claims), &Filters::default())
        .unwrap()
        .report
}

#[test]
fn the_active_sprint_counts_its_unskipped_tasks_and_those_waiting_for_the_sprint_review() {
    let report = report_of(
        json!({"sprints": [
            {"id": "A", "state": "active", "due": "2026-12-01", "release": "", "start": "S"},
            {"id": "B"}
        ]}),
        json!([
            {"id": "T-1", "status": "done", "sprint": "A", "sprint_review": "pending"},
            {"id": "T-2", "status": "skipped", "sprint": "a"},
            {"id": "T-3", "status": "todo", "sprint": "A"},
            {"id": "T-4", "status": "todo", "sprint": "B"}
        ]),
        store(json!({}), json!({})),
    );
    // an empty field is left out
    assert_eq!(
        report["sprint"],
        json!({"id": "A", "due": "2026-12-01", "start": "S", "done": 1, "tasks": 2, "review_pending": ["T-1"]})
    );
    let none = report_of(
        json!({"sprints": [{"id": "B"}]}),
        json!([]),
        store(json!({}), json!({})),
    );
    assert_eq!(none["sprint"], Value::Null);
}

#[test]
fn work_in_progress_is_by_its_claims_agent_else_who_started_it() {
    let report = report_of(
        json!({}),
        json!([
            {"id": "T-1", "status": "in_progress", "started_by": "bob"},
            {"id": "T-2", "status": "in_progress", "agent": "old-name"},
            {"id": "T-3", "status": "in_progress"},
            {"id": "T-4", "status": "in_progress", "started_by": "bob"}
        ]),
        store(json!({"T-4": {"task": "T-4", "agent": ""}}), json!({})),
    );
    let by: Vec<Value> = report["in_progress"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["by"].clone())
        .collect();
    assert_eq!(
        by,
        [json!("bob"), json!("old-name"), Value::Null, json!("bob")]
    );
}

#[test]
fn work_in_review_elsewhere_is_listed_once_and_this_copys_records_are_not() {
    let report = report_of(
        json!({}),
        json!([
            {"id": "T-1", "name": "One", "status": "todo"},
            {"id": "T-2", "name": "Two", "status": "in_review", "review_requested_at": "R"},
            {"id": "T-3", "name": "Three", "status": "todo"}
        ]),
        store(
            json!({}),
            json!({
                "T-1": {"task": "T-1", "worktree": "/elsewhere", "branch": "b", "status": "in_review", "at": "A"},
                "T-2": {"task": "T-2", "worktree": "/elsewhere", "status": "in_review", "at": "A"},
                "T-3": {"task": "T-3", "status": "blocked"},
                "T-9": {"task": "T-9", "worktree": "/elsewhere", "status": "blocked", "questions": ["Q"]},
                "bad": "not a record"
            }),
        ),
    );
    assert_eq!(
        report["in_review"],
        json!([
            {"id": "T-2", "name": "Two", "since": "R"},
            {"id": "T-1", "name": "One", "branch": "b", "since": "A"}
        ])
    );
    // a task this copy doesn't have is listed without a name
    assert_eq!(
        report["blocked"],
        json!([{"id": "T-9", "name": "", "branch": null, "waiting_on": null, "questions": ["Q"]}])
    );
}

#[test]
fn the_counts_put_unknown_statuses_last_and_read_a_text_label_as_its_characters() {
    let report = report_of(
        json!({}),
        json!([
            {"id": "T-1", "status": "weird"},
            {"id": "T-2", "status": "done", "labels": "ab"},
            {"id": "T-3", "status": "todo", "labels": ["a", "a"]},
            {"id": "T-4", "labels": null}
        ]),
        store(json!({}), json!({})),
    );
    let order: Vec<&String> = report["by_status"].as_object().unwrap().keys().collect();
    assert_eq!(order, ["todo", "done", "weird", "None"]);
    assert_eq!(report["by_label"], json!({"a": 3, "b": 1}));
}

#[test]
fn the_largest_tasks_are_the_three_biggest_then_by_id() {
    let report = report_of(
        json!({}),
        json!([
            {"id": "T-1", "name": "x"},
            {"id": "T-2", "name": "xxxxxxxxxxxxxxxxxxxx"},
            {"id": "T-3", "name": "xxxxxxxxxx"},
            {"id": "T-4", "name": "x"},
            {"id": "T-5", "name": "xxxxxxxxxx"}
        ]),
        store(json!({}), json!({})),
    );
    let ids: Vec<Value> = report["largest_tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].clone())
        .collect();
    assert_eq!(ids, [json!("T-2"), json!("T-5"), json!("T-3")]);
    assert!(report["largest_tasks"][0]["bytes"].as_u64().unwrap() > 20);
}

#[test]
fn open_tasks_in_closed_sprints_are_reported_whatever_the_scope() {
    let p = project(
        json!({"sprints": [{"id": "Old", "state": "closed"}, {"id": "Now", "state": "active"}]}),
        json!([
            {"id": "T-1", "status": "todo", "sprint": "Old"},
            {"id": "T-2", "status": "todo", "sprint": "Now"}
        ]),
        store(json!({}), json!({})),
    );
    let scope = Filters {
        sprint: vec!["Now".into()],
        ..Filters::default()
    };
    let report = status_report(&p, &scope).unwrap().report;
    assert_eq!(report["tasks"], json!(1));
    assert!(
        report["open_in_closed_sprints"].to_string().contains("T-1"),
        "{}",
        report["open_in_closed_sprints"]
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
