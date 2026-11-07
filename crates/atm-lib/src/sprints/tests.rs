use atm_db::MemoryStore;
use serde_json::{Value, json};

use super::*;

fn task(v: Value) -> Task {
    match v {
        Value::Object(m) => m,
        other => panic!("not a task: {other}"),
    }
}

fn project(tasks: Value, meta: Value) -> Project {
    let list: Vec<Task> = match tasks {
        Value::Array(items) => items.into_iter().map(task).collect(),
        other => panic!("not a list: {other}"),
    };
    Project::new(Box::new(
        MemoryStore::new(list, Vec::new(), &task(meta)).unwrap(),
    ))
    .with_actor(Some("alice".into()))
}

#[test]
fn a_sprint_is_added_started_and_listed_with_its_counts() {
    let mut p = project(json!([{"id": "T-1", "status": "todo"}]), json!({}));
    let added = add_sprint(
        &mut p,
        Some(" One "),
        Some("goal"),
        None,
        Some("2026-06-01"),
        None,
    )
    .unwrap();
    assert_eq!(
        added,
        json!({"id": "One", "state": "planned", "goal": "goal", "due": "2026-06-01"})
    );
    assert!(add_sprint(&mut p, Some("one"), None, None, None, None).is_err());
    assert!(add_sprint(&mut p, Some("Two"), None, None, Some("June"), None).is_err());
    move_tasks(&mut p, &["T-1".into()], "One").unwrap();
    let started = start_sprint(&mut p, "one").unwrap();
    assert_eq!(started["state"], json!("active"));
    assert!(start_sprint(&mut p, "One").is_err());
    let listed = list_sprints(&p, false).unwrap();
    assert_eq!(listed.sprints[0]["tasks"], json!(1));
    assert_eq!(listed.sprints[0]["by_kind"], json!({"todo": 1}));
    assert_eq!(listed.backlog, 0);
}

#[test]
fn closing_carries_the_unfinished_tasks_and_records_the_report() {
    let mut p = project(
        json!([
            {"id": "T-1", "status": "done", "sprint": "S"},
            {"id": "T-2", "status": "todo", "sprint": "S"}
        ]),
        json!({"sprints": [{"id": "S", "state": "active"}]}),
    );
    let err = close_sprint(&mut p, "S", None, None, false).unwrap_err();
    assert!(err.to_string().starts_with("T-2 isn't finished"), "{err}");
    let closed = close_sprint(&mut p, "S", Some("new"), Some("Next"), false).unwrap();
    assert_eq!(
        closed.sprint["report"],
        json!({"done": ["T-1"], "carried": ["T-2"], "added": [], "to": "Next"})
    );
    assert!(closed.was_active);
    assert_eq!(closed.created.unwrap()["id"], json!("Next"));
    let doc = p.store().document().unwrap();
    assert_eq!(doc["tasks"][1]["sprint"], json!("Next"));
    assert_eq!(doc["meta"]["sprints"][1]["id"], json!("Next"));
}

#[test]
fn a_sprint_with_a_history_is_deleted_only_with_force() {
    let mut p = project(
        json!([{"id": "T-1", "status": "done", "sprint": "S"}]),
        json!({"sprints": [{"id": "S"}, {"id": "Empty"}]}),
    );
    assert!(delete_sprint(&mut p, "S", None, false).is_err());
    let gone = delete_sprint(&mut p, "Empty", None, false).unwrap();
    assert_eq!(gone.moved.len(), 0);
    let forced = delete_sprint(&mut p, "S", None, true).unwrap();
    assert_eq!(forced.moved, ["T-1"]);
    assert_eq!(p.store().document().unwrap()["meta"]["sprints"], json!([]));
}

#[test]
fn moving_refuses_claimed_and_finished_tasks_and_knows_next_and_active() {
    let mut p = project(
        json!([
            {"id": "T-1", "status": "todo"},
            {"id": "T-2", "status": "in_progress"}
        ]),
        json!({"sprints": [{"id": "A", "state": "active"}, {"id": "B"}]}),
    );
    assert!(move_tasks(&mut p, &["T-2".into()], "A").is_err());
    let moved = move_tasks(&mut p, &["T-1".into()], "next").unwrap();
    assert_eq!(moved, [json!({"id": "T-1", "from": null, "to": "B"})]);
    let err = move_tasks(&mut p, &["T-1".into()], "next").unwrap_err();
    assert!(
        err.to_string()
            .starts_with("T-1: no planned sprint after B"),
        "{err}"
    );
    let moved = move_tasks(&mut p, &["T-1".into()], "active").unwrap();
    assert_eq!(moved[0]["to"], json!("A"));
    // T-2, in progress in no sprint, is the backlog's
    let (backlog, _) = backlog_tasks(&p, false).unwrap();
    assert_eq!(backlog[0]["id"], json!("T-2"));
}
