use atm_db::MemoryStore;
use serde_json::{Value, json};

use super::*;

fn task(v: Value) -> Task {
    match v {
        Value::Object(m) => m,
        other => panic!("not a task: {other}"),
    }
}

fn project(tasks: Value, archived: Value) -> Project {
    let list = |v: Value| -> Vec<Task> {
        match v {
            Value::Array(items) => items.into_iter().map(task).collect(),
            other => panic!("not a list: {other}"),
        }
    };
    Project::new(Box::new(
        MemoryStore::new(list(tasks), list(archived), &serde_json::Map::new()).unwrap(),
    ))
}

#[test]
fn finished_tasks_move_and_the_rest_stay_saying_why() {
    let mut p = project(
        json!([
            {"id": "T-001", "status": "done", "completed_at": "2020-01-01T00:00:00Z"},
            {"id": "T-002", "status": "done", "completed_at": "2020-01-01T00:00:00Z", "pr": {"number": 7}},
            {"id": "T-003", "status": "done", "completed_at": "2999-01-01T00:00:00Z"},
            {"id": "T-004", "status": "todo"}
        ]),
        json!([]),
    );
    let dry = archive_finished(
        &mut p,
        &ArchiveRequest {
            dry_run: true,
            ..ArchiveRequest::default()
        },
    )
    .unwrap();
    assert_eq!(dry.moved, [json!("T-001")]);
    assert_eq!(
        p.store().list_tasks().unwrap().len(),
        4,
        "a dry run moves nothing"
    );
    let done = archive_finished(&mut p, &ArchiveRequest::default()).unwrap();
    assert_eq!(done.moved, [json!("T-001")]);
    assert_eq!(
        done.kept,
        [
            json!({"id": "T-002", "reason": "in open PR #7"}),
            json!({"id": "T-003", "reason": "finished in the last 14 days"})
        ]
    );
    let document = p.store().document().unwrap();
    assert_eq!(document["meta"]["next_id"], json!(5));
    assert_eq!(document["meta"]["archived"], json!(1));
    assert!(p.store().get_archived_task("T-001").unwrap().is_some());
}

#[test]
fn before_and_keep_days_narrow_it_and_a_bad_date_is_refused() {
    let mut p = project(
        json!([
            {"id": "T-1", "status": "done", "completed_at": "2026-03-01T00:00:00Z"},
            {"id": "T-2", "status": "done", "completed_at": "2026-05-01T00:00:00Z"}
        ]),
        json!([]),
    );
    let request = ArchiveRequest {
        before: Some("2026-04-01".into()),
        keep_days: Some(0.0),
        dry_run: true,
        ..ArchiveRequest::default()
    };
    let result = archive_finished(&mut p, &request).unwrap();
    assert_eq!(result.moved, [json!("T-1")]);
    assert_eq!(
        result.kept,
        [json!({"id": "T-2", "reason": "finished on or after 2026-04-01"})]
    );
    let bad = ArchiveRequest {
        before: Some("April".into()),
        ..ArchiveRequest::default()
    };
    assert_eq!(
        archive_finished(&mut p, &bad).unwrap_err(),
        Error::Invalid("--before April: use a date like 2026-05-01".into())
    );
}

#[test]
fn unarchive_brings_tasks_back_and_tells_mistakes_apart() {
    let mut p = project(
        json!([{"id": "T-3", "status": "todo"}]),
        json!([{"id": "T-1", "status": "done", "completed_at": "2020-01-01T00:00:00Z"}, {"id": "T-2", "status": "todo"}]),
    );
    let error = unarchive_tasks(&mut p, &["T-3".into(), "T-9".into()]).unwrap_err();
    assert_eq!(
        error.to_string(),
        "not in the archive: T-3 (already in tasks.json); T-9 (no such task)"
    );
    let back = unarchive_tasks(&mut p, &["T-1".into(), "T-2".into()]).unwrap();
    assert_eq!(back.moves_back, [json!("T-1")]);
    assert_eq!(p.store().list_tasks().unwrap().len(), 3);
    assert_eq!(p.store().list_archived().unwrap().len(), 0);
    assert_eq!(p.store().document().unwrap()["meta"]["archived"], json!(0));
}
