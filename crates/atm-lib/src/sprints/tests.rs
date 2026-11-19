use atm_db::MemoryStore;
use serde_json::{Value, json};

use super::*;

fn task(v: Value) -> Task {
    match v {
        Value::Object(m) => m,
        other => panic!("not a task: {other}"),
    }
}

fn list(tasks: Value) -> Vec<Task> {
    match tasks {
        Value::Array(items) => items.into_iter().map(task).collect(),
        other => panic!("not a list: {other}"),
    }
}

fn project(tasks: Value, meta: Value) -> Project {
    project_with_archive(tasks, json!([]), meta)
}

fn project_with_archive(tasks: Value, archive: Value, meta: Value) -> Project {
    Project::new(Box::new(
        MemoryStore::new(list(tasks), list(archive), &task(meta)).unwrap(),
    ))
    .with_actor(Some("alice".into()))
}

fn document(p: &Project) -> Value {
    p.store().document().unwrap().clone()
}

fn sprint_ids(p: &Project) -> Vec<String> {
    document(p)["meta"]["sprints"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["id"].as_str().unwrap().to_string())
        .collect()
}

/// The refusal's message, with the store left as it was.
fn refused<T: std::fmt::Debug>(
    p: &mut Project,
    change: impl FnOnce(&mut Project) -> Result<T>,
) -> String {
    let before = document(p);
    let error = change(p).unwrap_err();
    assert_eq!(document(p), before, "a refusal changes nothing");
    error.to_string()
}

#[test]
fn a_new_sprint_without_a_name_gets_one_no_sprint_has() {
    let mut p = project(json!([]), json!({"sprints": [{"id": "A"}]}));
    let added = add_sprint(&mut p, Some("  "), None, None, None, None).unwrap();
    let name = added["id"].as_str().unwrap();
    assert!(!name.is_empty() && name != "A", "{name}");
}

#[test]
fn a_new_sprints_name_is_unique_and_at_most_the_limit() {
    let mut p = project(json!([]), json!({"sprints": [{"id": "A"}]}));
    assert_eq!(
        refused(&mut p, |p| add_sprint(p, Some("a"), None, None, None, None)),
        "there is already a sprint 'A'"
    );
    let long = "x".repeat(MAX_SPRINT_NAME + 1);
    assert_eq!(
        refused(&mut p, |p| add_sprint(
            p,
            Some(&long),
            None,
            None,
            None,
            None
        )),
        "a sprint name is at most 48 characters"
    );
    add_sprint(
        &mut p,
        Some(&"x".repeat(MAX_SPRINT_NAME)),
        None,
        None,
        None,
        None,
    )
    .unwrap();
}

#[test]
fn a_new_sprint_goes_after_the_one_named_else_last() {
    let mut p = project(json!([]), json!({"sprints": [{"id": "A"}, {"id": "B"}]}));
    add_sprint(&mut p, Some("C"), None, None, None, None).unwrap();
    add_sprint(&mut p, Some("A2"), None, None, None, Some("a")).unwrap();
    assert_eq!(sprint_ids(&p), ["A", "A2", "B", "C"]);
    assert_eq!(
        refused(&mut p, |p| add_sprint(
            p,
            Some("D"),
            None,
            None,
            None,
            Some("Z")
        )),
        "no sprint 'Z' to add it after"
    );
}

#[test]
fn a_sprints_release_must_be_one_the_project_has() {
    let mut p = project(json!([]), json!({"releases": [{"id": "1.0"}]}));
    assert_eq!(
        refused(&mut p, |p| add_sprint(
            p,
            Some("A"),
            None,
            Some("2.0"),
            None,
            None
        )),
        "no release '2.0'; the releases: 1.0"
    );
    let added = add_sprint(&mut p, Some("A"), None, Some("1.0"), None, None).unwrap();
    assert_eq!(added["release"], json!("1.0"));
}

#[test]
fn editing_a_sprint_changes_removes_or_leaves_each_field() {
    let mut p = project(
        json!([]),
        json!({"sprints": [{"id": "A", "goal": "g", "due": "2026-01-01"}]}),
    );
    let (sprint, changed) = edit_sprint(&mut p, "a", Some("new"), None, Some("")).unwrap();
    assert_eq!(changed, ["goal", "due"]);
    assert_eq!(sprint, json!({"id": "A", "goal": "new"}));
    assert_eq!(document(&p)["meta"]["sprints"][0], sprint);
    let (_, changed) = edit_sprint(&mut p, "A", Some("new"), None, Some("")).unwrap();
    assert!(changed.is_empty(), "the same values change nothing");
    assert_eq!(
        refused(&mut p, |p| edit_sprint(
            p,
            "A",
            None,
            None,
            Some("2026-1-1")
        )),
        "--due '2026-1-1': use a date, YYYY-MM-DD"
    );
}

#[test]
fn an_unknown_sprint_is_refused_with_the_sprints_named() {
    let mut empty = project(json!([]), json!({}));
    assert_eq!(
        refused(&mut empty, |p| edit_sprint(p, "X", None, None, None)),
        "no sprint 'X'; the sprints: none yet: atm sprint add"
    );
    let mut p = project(json!([]), json!({"sprints": [{"id": "A"}, {"id": "B"}]}));
    for message in [
        refused(&mut p, |p| start_sprint(p, "X")),
        refused(&mut p, |p| close_sprint(p, "X", None, None, false)),
        refused(&mut p, |p| delete_sprint(p, "X", None, false)),
        refused(&mut p, |p| rename_sprint(p, "X", "Y")),
    ] {
        assert_eq!(message, "no sprint 'X'; the sprints: A, B");
    }
}

#[test]
fn renaming_a_sprint_renames_it_everywhere_it_is_named() {
    let mut p = project_with_archive(
        json!([
            {"id": "T-1", "status": "todo", "sprint": "Old", "updated_at": "U"},
            {"id": "T-2", "status": "todo", "sprint": "B", "sprint_history": [{"sprint": "old", "from": "x"}]},
            {"id": "T-3", "status": "todo"}
        ]),
        json!([{"id": "T-0", "status": "done", "sprint": "Old"}]),
        json!({"sprints": [
            {"id": "Prev", "state": "closed", "report": {"to": "Old"}},
            {"id": "Old"}, {"id": "B"}
        ]}),
    );
    let renamed = rename_sprint(&mut p, "old", " New ").unwrap();
    assert_eq!(
        renamed,
        Renamed {
            from: "Old".into(),
            to: "New".into(),
            tasks: 2,
            archived: 1,
            reports: 1
        }
    );
    let doc = document(&p);
    assert_eq!(doc["tasks"][0]["sprint"], json!("New"));
    assert_eq!(
        doc["tasks"][0]["updated_at"],
        json!("U"),
        "the task isn't touched"
    );
    assert_eq!(doc["tasks"][1]["sprint_history"][0]["sprint"], json!("New"));
    assert_eq!(doc["meta"]["sprints"][0]["report"]["to"], json!("New"));
    assert_eq!(
        p.store().list_archived().unwrap()[0]["sprint"],
        json!("New")
    );
}

#[test]
fn a_rename_needs_a_new_free_name_that_fits() {
    let mut p = project(
        json!([{"id": "T-1", "status": "todo", "sprint_history": [{"sprint": "Gone"}]}]),
        json!({"sprints": [{"id": "A"}, {"id": "B"}]}),
    );
    for (new, message) in [
        ("  ", "a sprint needs a name".to_string()),
        (
            &"x".repeat(MAX_SPRINT_NAME + 1)[..],
            "a sprint name is at most 48 characters".to_string(),
        ),
        ("A", "the sprint is already called 'A'".to_string()),
        (
            "b",
            "there is already a sprint 'B' (or tasks' history names one)".to_string(),
        ),
        (
            "gone",
            "there is already a sprint 'Gone' (or tasks' history names one)".to_string(),
        ),
    ] {
        assert_eq!(refused(&mut p, |p| rename_sprint(p, "A", new)), message);
    }
    // its own name in another case is its own
    assert_eq!(rename_sprint(&mut p, "A", "a").unwrap().to, "a");
}

#[test]
fn only_a_planned_sprint_starts_and_one_at_a_time() {
    let mut p = project(
        json!([]),
        json!({"sprints": [{"id": "Done", "state": "closed"}, {"id": "Now", "state": "active"}, {"id": "Next"}]}),
    );
    assert_eq!(
        refused(&mut p, |p| start_sprint(p, "Done")),
        "the sprint 'Done' is closed; closed sprints aren't started again"
    );
    assert_eq!(
        refused(&mut p, |p| start_sprint(p, "Now")),
        "the sprint 'Now' is already active"
    );
    assert_eq!(
        refused(&mut p, |p| start_sprint(p, "Next")),
        "the sprint 'Now' is active; close it first (one active sprint at a time)"
    );
    close_sprint(&mut p, "Now", None, None, false).unwrap();
    let started = start_sprint(&mut p, "Next").unwrap();
    assert!(started["start"].as_str().is_some_and(|s| !s.is_empty()));
}

#[test]
fn a_sprint_with_work_in_progress_is_neither_closed_nor_deleted() {
    let mut p = project(
        json!([
            {"id": "T-1", "status": "in_progress", "sprint": "S"},
            {"id": "T-2", "status": "in_progress", "sprint": "S"}
        ]),
        json!({"sprints": [{"id": "S", "state": "active"}]}),
    );
    assert_eq!(
        refused(&mut p, |p| close_sprint(
            p,
            "S",
            Some("backlog"),
            None,
            false
        )),
        "T-1, T-2 are in progress: finish or release them before closing the sprint"
    );
    assert_eq!(
        refused(&mut p, |p| delete_sprint(p, "S", None, true)),
        "T-1, T-2 are in progress: finish or release them before deleting the sprint"
    );
}

#[test]
fn batch_tasks_wait_for_the_end_of_sprint_review_before_the_close() {
    let mut p = project(
        json!([{"id": "T-1", "status": "done", "sprint": "S", "sprint_review": "pending"}]),
        json!({"sprints": [{"id": "S", "state": "active"}]}),
    );
    assert_eq!(
        refused(&mut p, |p| close_sprint(p, "S", None, None, false)),
        "T-1 waits for the end-of-sprint review: review it with the user (/atm-close-sprint), then close with --reviewed"
    );
    close_sprint(&mut p, "S", None, None, true).unwrap();
    let doc = document(&p);
    assert_eq!(doc["tasks"][0]["sprint_review"], json!("reviewed"));
    assert_eq!(doc["tasks"][0]["updated_by"], json!("alice"));
}

#[test]
fn a_closed_sprint_isnt_closed_again() {
    let mut p = project(
        json!([]),
        json!({"sprints": [{"id": "S", "state": "closed"}]}),
    );
    assert_eq!(
        refused(&mut p, |p| close_sprint(p, "S", None, None, false)),
        "the sprint 'S' is already closed"
    );
}

#[test]
fn closing_carries_to_the_backlog_or_an_open_sprint_but_not_to_itself_or_a_closed_one() {
    let meta = json!({"sprints": [
        {"id": "S", "state": "active"}, {"id": "Next"}, {"id": "Old", "state": "closed"}
    ]});
    let tasks = json!([
        {"id": "T-1", "status": "todo", "sprint": "S"},
        {"id": "T-2", "status": "blocked", "sprint": "S"},
        {"id": "T-3", "status": "skipped", "sprint": "S"}
    ]);
    let mut p = project(tasks.clone(), meta.clone());
    assert_eq!(
        refused(&mut p, |p| close_sprint(p, "S", None, None, false)),
        "T-1, T-2 aren't finished: say where they go with --carry-to <sprint>, new or backlog"
    );
    assert_eq!(
        refused(&mut p, |p| close_sprint(p, "S", Some("s"), None, false)),
        "the tasks can't be carried to the sprint that is closing ('S')"
    );
    assert_eq!(
        refused(&mut p, |p| close_sprint(p, "S", Some("Old"), None, false)),
        "the sprint 'Old' is closed; a task can only go into a planned or active sprint"
    );
    assert_eq!(
        refused(&mut p, |p| close_sprint(
            p,
            "S",
            Some("Next"),
            Some("N"),
            false
        )),
        "--name names a new sprint: use it with --carry-to new"
    );
    let closed = close_sprint(&mut p, "S", Some("next"), None, false).unwrap();
    assert_eq!(closed.target.as_deref(), Some("Next"));
    assert_eq!(
        closed.sprint["report"],
        json!({"done": [], "carried": ["T-1", "T-2"], "added": [], "to": "Next"})
    );
    let doc = document(&p);
    assert_eq!(
        (
            doc["tasks"][0]["sprint"].clone(),
            doc["tasks"][1]["status"].clone()
        ),
        (json!("Next"), json!("blocked")),
        "carried tasks keep their status"
    );
    assert_eq!(doc["tasks"][2]["sprint"], json!("S"), "finished work stays");
    let mut p = project(tasks, meta);
    let closed = close_sprint(&mut p, "S", Some("backlog"), None, false).unwrap();
    assert_eq!(closed.target, None);
    assert_eq!(closed.sprint["report"]["to"], json!("backlog"));
    assert!(document(&p)["tasks"][0].get("sprint").is_none());
}

#[test]
fn a_finished_sprints_report_counts_the_archive_and_the_tasks_added_mid_sprint() {
    let mut p = project_with_archive(
        json!([
            {"id": "T-1", "status": "done", "sprint": "S",
             "sprint_history": [{"sprint": "S", "from": "2026-01-02T00:00:00Z"}]},
            {"id": "T-2", "status": "done", "sprint": "S",
             "sprint_history": [{"sprint": "S", "from": "2025-12-31T00:00:00Z"}]}
        ]),
        json!([{"id": "T-0", "status": "done", "sprint": "S"}]),
        json!({"sprints": [{"id": "S", "state": "active", "start": "2026-01-01T00:00:00Z"}]}),
    );
    let closed = close_sprint(&mut p, "S", Some("backlog"), None, false).unwrap();
    // nothing carried: no "to"
    assert_eq!(
        closed.sprint["report"],
        json!({"done": ["T-1", "T-2", "T-0"], "carried": [], "added": ["T-1"]})
    );
    assert_eq!(closed.sprint["state"], json!("closed"));
    assert!(closed.sprint["end"].is_string());
    assert!(closed.was_active);
}

#[test]
fn a_closed_sprint_with_tasks_stays_and_an_open_one_needs_force() {
    let mut p = project_with_archive(
        json!([{"id": "T-2", "status": "todo", "sprint": "B", "sprint_history": [{"sprint": "A"}]}]),
        json!([{"id": "T-1", "status": "done", "sprint": "C"}]),
        json!({"sprints": [{"id": "A"}, {"id": "B"}, {"id": "C", "state": "closed"}, {"id": "E", "state": "closed"}]}),
    );
    assert_eq!(
        refused(&mut p, |p| delete_sprint(p, "C", None, true)),
        "the sprint 'C' is closed and had tasks (T-1): its record and report stay; only an empty sprint can be deleted"
    );
    assert_eq!(
        refused(&mut p, |p| delete_sprint(p, "A", None, false)),
        "tasks finished in the sprint 'A' or moved on from it (T-2): delete it with --force to drop that record (its tasks go to --move-to, the backlog by default)"
    );
    let gone = delete_sprint(&mut p, "E", None, false).unwrap();
    assert_eq!(gone.state, "closed", "an empty closed sprint goes");
    delete_sprint(&mut p, "A", None, true).unwrap();
    assert_eq!(sprint_ids(&p), ["B", "C"]);
}

#[test]
fn a_deleted_sprints_unfinished_tasks_move_where_asked() {
    let meta = json!({"sprints": [{"id": "S"}, {"id": "T"}, {"id": "Old", "state": "closed"}]});
    let tasks = json!([
        {"id": "T-1", "status": "todo", "sprint": "S"},
        {"id": "T-2", "status": "in_review", "sprint": "S"},
        {"id": "T-3", "status": "todo", "sprint": "T"}
    ]);
    let mut p = project(tasks.clone(), meta.clone());
    assert_eq!(
        refused(&mut p, |p| delete_sprint(p, "S", Some("S"), false)),
        "the tasks can't move to the sprint being deleted ('S')"
    );
    assert_eq!(
        refused(&mut p, |p| delete_sprint(p, "S", Some("Old"), false)),
        "the sprint 'Old' is closed; a task can only go into a planned or active sprint"
    );
    let deleted = delete_sprint(&mut p, "S", Some("t"), false).unwrap();
    assert_eq!(
        (deleted.moved.clone(), deleted.target.clone()),
        (
            vec!["T-1".to_string(), "T-2".to_string()],
            Some("T".to_string())
        )
    );
    let doc = document(&p);
    assert_eq!(doc["tasks"][0]["sprint"], json!("T"));
    assert_eq!(doc["tasks"][0]["updated_by"], json!("alice"));
    let mut p = project(tasks, meta);
    let deleted = delete_sprint(&mut p, "S", None, false).unwrap();
    assert_eq!(deleted.target, None);
    assert!(document(&p)["tasks"][1].get("sprint").is_none());
}

#[test]
fn moving_is_all_or_nothing_and_names_what_it_cant_move() {
    let mut p = project_with_archive(
        json!([
            {"id": "T-1", "status": "todo"},
            {"id": "T-2", "status": "done"},
            {"id": "T-3", "status": "blocked"},
            {"id": "T-4", "status": "in_review"}
        ]),
        json!([{"id": "T-0", "status": "done"}]),
        json!({"sprints": [{"id": "A"}]}),
    );
    for (ids, message) in [
        (vec!["T-1", "T-9"], "no task T-9"),
        (
            vec!["T-1", "T-0"],
            "no task T-0 (it is archived: atm db unarchive brings it back)",
        ),
        (
            vec!["T-1", "T-2"],
            "T-2 is done: finished work stays in the sprint it was done in",
        ),
    ] {
        let ids: Vec<String> = ids.into_iter().map(String::from).collect();
        assert_eq!(refused(&mut p, |p| move_tasks(p, &ids, "A")), message);
    }
    assert_eq!(
        refused(&mut p, |p| move_tasks(p, &["T-1".into()], "active")),
        "no sprint is active (atm sprint start <name>)"
    );
    let moved = move_tasks(&mut p, &["T-3".into(), "T-4".into()], "A").unwrap();
    assert_eq!(moved.len(), 2, "blocked and in-review tasks move");
}

#[test]
fn a_move_to_where_the_task_is_changes_nothing_on_it() {
    let mut p = project(
        json!([{"id": "T-1", "status": "todo", "sprint": "A", "updated_at": "U"}]),
        json!({"sprints": [{"id": "A"}]}),
    );
    let moved = move_tasks(&mut p, &["T-1".into()], "A").unwrap();
    assert_eq!(moved, [json!({"id": "T-1", "from": "A", "to": "A"})]);
    assert_eq!(document(&p)["tasks"][0]["updated_at"], json!("U"));
}

#[test]
fn next_is_the_first_planned_sprint_after_the_tasks_own() {
    let mut p = project(
        json!([
            {"id": "T-1", "status": "todo"},
            {"id": "T-2", "status": "todo", "sprint": "Old"},
            {"id": "T-3", "status": "todo", "sprint": "Now"},
            {"id": "T-4", "status": "todo", "sprint": "P1"}
        ]),
        json!({"sprints": [
            {"id": "Old", "state": "closed"}, {"id": "Now", "state": "active"},
            {"id": "P1"}, {"id": "P2"}
        ]}),
    );
    let ids: Vec<String> = ["T-1", "T-2", "T-3", "T-4"].map(String::from).to_vec();
    let moved = move_tasks(&mut p, &ids, "next").unwrap();
    let to: Vec<&Value> = moved.iter().map(|m| &m["to"]).collect();
    // from the backlog, a closed sprint or the active one: the first planned after it; from P1: P2
    assert_eq!(to, [&json!("P1"), &json!("P1"), &json!("P1"), &json!("P2")]);
    let mut none = project(json!([{"id": "T-1", "status": "todo"}]), json!({}));
    assert_eq!(
        refused(&mut none, |p| move_tasks(p, &["T-1".into()], "next")),
        "T-1: no planned sprint after the active one; add one with atm sprint add [\"<name>\"]"
    );
}

#[test]
fn the_sprint_list_counts_tasks_by_kind_with_the_archive_and_hides_closed_sprints_unless_asked() {
    let p = project_with_archive(
        json!([
            {"id": "T-1", "status": "todo", "sprint": "A"},
            {"id": "T-2", "status": "done", "sprint": "a", "sprint_review": "pending"},
            {"id": "T-3", "status": "todo"},
            {"id": "T-4", "status": "done"},
            {"id": "T-5", "status": "todo", "sprint": "C"}
        ]),
        json!([
            {"id": "T-0", "status": "done", "sprint": "A"},
            {"id": "T-9", "status": "todo"}
        ]),
        json!({"sprints": [{"id": "A", "state": "active"}, {"id": "C", "state": "closed"}]}),
    );
    let listed = list_sprints(&p, false).unwrap();
    assert_eq!(listed.sprints.len(), 1);
    let a = &listed.sprints[0];
    assert_eq!(
        (
            a["tasks"].clone(),
            a["by_kind"].clone(),
            a["review_pending"].clone()
        ),
        (json!(3), json!({"todo": 1, "done": 2}), json!(1))
    );
    // the backlog: unfinished tasks in tasks.json in no sprint
    assert_eq!(listed.backlog, 1);
    assert!(listed.has_sprints);
    assert_eq!(list_sprints(&p, true).unwrap().sprints.len(), 2);
    let plain = project(json!([]), json!({}));
    let listed = list_sprints(&plain, true).unwrap();
    assert!(!listed.has_sprints && listed.sprints.is_empty());
}

#[test]
fn the_backlog_lists_its_finished_tasks_only_when_asked() {
    let p = project(
        json!([
            {"id": "T-1", "status": "todo"},
            {"id": "T-2", "status": "done"},
            {"id": "T-3", "status": "skipped"},
            {"id": "T-4", "status": "todo", "sprint": "A"}
        ]),
        json!({"sprints": [{"id": "A"}]}),
    );
    let ids = |finished: bool| -> Vec<Value> {
        backlog_tasks(&p, finished)
            .unwrap()
            .0
            .iter()
            .map(|t| t["id"].clone())
            .collect()
    };
    assert_eq!(ids(false), [json!("T-1")]);
    assert_eq!(ids(true), [json!("T-1"), json!("T-2"), json!("T-3")]);
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
