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
    project_with_meta(tasks, archived, json!({}))
}

fn project_with_meta(tasks: Value, archived: Value, meta: Value) -> Project {
    let list = |v: Value| -> Vec<Task> {
        match v {
            Value::Array(items) => items.into_iter().map(task).collect(),
            other => panic!("not a list: {other}"),
        }
    };
    Project::new(Box::new(
        MemoryStore::new(list(tasks), list(archived), &task(meta)).unwrap(),
    ))
}

const OLD: &str = "2020-01-01T00:00:00Z";

fn ids_of(list: &[&Task]) -> Vec<Value> {
    list.iter().map(|t| t["id"].clone()).collect()
}

#[test]
fn sprint_narrows_the_archive_to_the_sprints_chosen() {
    let mut p = project_with_meta(
        json!([
            {"id": "T-1", "status": "done", "completed_at": OLD, "sprint": "A"},
            {"id": "T-2", "status": "done", "completed_at": OLD, "sprint": "B"},
            {"id": "T-3", "status": "done", "completed_at": OLD}
        ]),
        json!([]),
        json!({"sprints": [{"id": "A", "state": "closed"}, {"id": "B"}]}),
    );
    let request = ArchiveRequest {
        sprints: vec!["a".into()],
        ..ArchiveRequest::default()
    };
    let result = archive_finished(&mut p, &request).unwrap();
    assert_eq!(result.moved, [json!("T-1")]);
    assert_eq!(
        result.kept,
        [
            json!({"id": "T-2", "reason": "not in the sprints chosen"}),
            json!({"id": "T-3", "reason": "not in the sprints chosen"})
        ]
    );
    let unknown = ArchiveRequest {
        sprints: vec!["Z".into()],
        ..ArchiveRequest::default()
    };
    let error = archive_finished(&mut p, &unknown).unwrap_err();
    assert_eq!(
        (error.to_string(), error.code()),
        ("no sprint 'Z'".into(), 2)
    );
}

#[test]
fn a_task_in_both_files_keeps_its_working_copy() {
    let mut p = project(
        json!([{"id": "T-1", "status": "todo", "name": "working"}]),
        json!([{"id": "T-1", "status": "done", "name": "archived"}, {"id": "T-0", "status": "done"}]),
    );
    let dry = archive_finished(
        &mut p,
        &ArchiveRequest {
            dry_run: true,
            ..ArchiveRequest::default()
        },
    )
    .unwrap();
    assert_eq!(dry.duplicates_dropped, [json!("T-1")]);
    assert_eq!(
        p.store().list_archived().unwrap().len(),
        2,
        "a dry run drops nothing"
    );
    let result = archive_finished(&mut p, &ArchiveRequest::default()).unwrap();
    assert_eq!(
        (result.moved.clone(), result.duplicates_dropped.clone()),
        (Vec::new(), vec![json!("T-1")])
    );
    assert_eq!(ids_of(&p.store().list_archived().unwrap()), [json!("T-0")]);
    assert_eq!(
        p.store().get_task("T-1").unwrap().unwrap()["name"],
        json!("working")
    );
    assert_eq!(p.store().document().unwrap()["meta"]["archived"], json!(1));
}

#[test]
fn nothing_to_archive_changes_nothing() {
    let mut p = project(json!([{"id": "T-1", "status": "todo"}]), json!([]));
    let before = p.store().document().unwrap().clone();
    let result = archive_finished(&mut p, &ArchiveRequest::default()).unwrap();
    assert!(result.moved.is_empty() && result.kept.is_empty());
    assert_eq!(p.store().document().unwrap(), &before);
}

#[test]
fn skipped_tasks_are_finished_too() {
    let mut p = project(
        json!([{"id": "T-1", "status": "skipped", "skipped_at": OLD}]),
        json!([]),
    );
    let result = archive_finished(&mut p, &ArchiveRequest::default()).unwrap();
    assert_eq!(result.moved, [json!("T-1")]);
}

#[test]
fn in_a_keys_file_the_next_number_never_comes_back_to_an_archived_one() {
    let uuid = "0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8091";
    let mut p = project_with_meta(
        json!([{"id": uuid, "number": 9, "key_final": true, "status": "done", "completed_at": OLD}]),
        json!([]),
        json!({"format": 2, "key": "ATM", "next_number": 3}),
    );
    archive_finished(&mut p, &ArchiveRequest::default()).unwrap();
    let meta = &p.store().document().unwrap()["meta"];
    assert_eq!(meta["next_number"], json!(10));
    assert!(meta.get("next_id").is_none());
}

#[test]
fn the_days_kept_are_written_as_python_writes_them() {
    let none = json!({"meta": {}});
    assert_eq!(days_text(&none, Some(3.0)), "3.0");
    assert_eq!(days_text(&none, Some(2.5)), "2.5");
    assert_eq!(
        days_text(&json!({"meta": {"archive": {"keep_days": 7}}}), None),
        "7"
    );
    // a negative or missing setting: the default
    assert_eq!(
        days_text(&json!({"meta": {"archive": {"keep_days": -1}}}), None),
        days_text(&none, None)
    );
}

#[test]
fn before_is_a_date_or_a_full_time() {
    assert_eq!(parse_before(None).unwrap(), None);
    assert_eq!(parse_before(Some("")).unwrap(), None);
    assert_eq!(
        parse_before(Some("2026-04-01")).unwrap(),
        parse_before(Some("2026-04-01T00:00:00Z")).unwrap()
    );
    assert!(
        parse_before(Some("2026-04-01T12:00:00Z"))
            .unwrap()
            .is_some()
    );
    for bad in ["2026-13-01", "2026-04-01Tnoon", "yesterday"] {
        assert!(
            matches!(parse_before(Some(bad)), Err(Error::Invalid(_))),
            "{bad}"
        );
    }
}

#[test]
fn a_finished_task_without_a_finish_time_isnt_before_anything() {
    let mut p = project(json!([{"id": "T-1", "status": "done"}]), json!([]));
    let request = ArchiveRequest {
        before: Some("2026-04-01".into()),
        keep_days: Some(0.0),
        dry_run: true,
        ..ArchiveRequest::default()
    };
    let result = archive_finished(&mut p, &request).unwrap();
    assert_eq!(result.moved.len(), 0);
    assert_eq!(
        result.kept,
        [json!({"id": "T-1", "reason": "finished on or after 2026-04-01"})]
    );
}

#[test]
fn unarchive_of_a_task_in_both_files_says_it_is_already_back() {
    let mut p = project(
        json!([{"id": "T-1", "status": "todo"}]),
        json!([{"id": "T-1", "status": "done"}]),
    );
    let error = unarchive_tasks(&mut p, &["T-1".into()]).unwrap_err();
    assert_eq!(
        error.to_string(),
        "not in the archive: T-1 (already in tasks.json)"
    );
}

#[test]
fn a_task_back_from_the_archive_that_isnt_old_stays() {
    let mut p = project(
        json!([]),
        json!([{"id": "T-1", "status": "done", "completed_at": "2999-01-01T00:00:00Z"}]),
    );
    let back = unarchive_tasks(&mut p, &["T-1".into()]).unwrap();
    assert_eq!(back.restored, ["T-1"]);
    assert_eq!(back.moves_back.len(), 0);
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
