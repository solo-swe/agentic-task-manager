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
    .with_actor(Some("claude:a".into()))
}

fn ids(added: &Added) -> Vec<&str> {
    added
        .tasks
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect()
}

#[test]
fn ids_follow_the_pattern_and_never_go_behind_the_highest() {
    assert_eq!(split_id("T-001"), Some(("T-", "001")));
    assert_eq!(split_id("T-x"), None);
    let mut p = project(
        json!([{"id": "AB-07", "status": "todo"}]),
        json!({"next_id": 3}),
    );
    let added = add_new_tasks(&mut p, &[task(json!({"name": "a", "type": "feature"}))]).unwrap();
    assert_eq!(ids(&added), ["AB-08"]);
    let mut p = project(json!([]), json!({"next_id": 12}));
    let added = add_new_tasks(&mut p, &[task(json!({"name": "a", "type": "feature"}))]).unwrap();
    assert_eq!(ids(&added), ["T-012"]);
    assert_eq!(p.store().document().unwrap()["meta"]["next_id"], json!(13));
}

#[test]
fn placeholders_become_the_new_ids_in_dependencies() {
    let mut p = project(json!([{"id": "T-001", "status": "done"}]), json!({}));
    let entries = [
        task(json!({"id": "new-1", "name": "a", "type": "feature", "dependencies": ["T-001"]})),
        task(
            json!({"id": "new-2", "name": "b", "type": "test", "dependencies": ["new-1"], "related": ["new-1"]}),
        ),
    ];
    let added = add_new_tasks(&mut p, &entries).unwrap();
    assert_eq!(ids(&added), ["T-002", "T-003"]);
    assert_eq!(added.tasks[1]["dependencies"], json!(["T-002"]));
    assert_eq!(added.tasks[1]["related"], json!(["T-002"]));
    assert_eq!(added.tasks[0]["created_by"], json!("claude:a"));
    assert_eq!(added.tasks[0]["status"], json!("todo"));
    let keys: Vec<&str> = added.tasks[0].keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        [
            "id",
            "name",
            "type",
            "description",
            "requirements",
            "use_cases",
            "status",
            "dependencies",
            "files",
            "created_at",
            "updated_at",
            "created_by",
            "updated_by"
        ]
    );
}

#[test]
fn a_bad_entry_refuses_the_whole_file() {
    let cases = [
        (json!({"name": "a"}), "a new task needs a type"),
        (json!({"name": "a", "type": "nope"}), "unknown type 'nope'"),
        (
            json!({"name": "a", "type": "feature", "priority": "P9"}),
            "unknown priority 'P9'",
        ),
        (
            json!({"name": "a", "type": "feature", "labels": ["Bad"]}),
            "label 'Bad'",
        ),
        (
            json!({"name": "a", "type": "feature", "dependencies": ["T-9"]}),
            "depends on T-9, which doesn't exist",
        ),
        (
            json!({"name": "a", "type": "feature", "sprint": "S"}),
            "no sprint 'S'",
        ),
        (
            json!({"name": "a", "type": "feature", "review": "x"}),
            "review 'x'",
        ),
        (
            json!({"name": "a", "type": "feature", "with": "x"}),
            "with 'x'",
        ),
    ];
    for (entry, message) in cases {
        let mut p = project(json!([]), json!({}));
        let err = add_new_tasks(
            &mut p,
            &[task(json!({"name": "ok", "type": "feature"})), task(entry)],
        )
        .unwrap_err();
        assert!(err.to_string().contains(message), "{err} / {message}");
        assert_eq!(err.code(), 2);
        assert_eq!(p.store().document().unwrap()["tasks"], json!([]));
    }
}

#[test]
fn a_cycle_is_refused() {
    let mut p = project(
        json!([{"id": "T-001", "status": "todo", "dependencies": ["T-002"]}]),
        json!({}),
    );
    let err = add_new_tasks(
        &mut p,
        &[task(
            json!({"name": "a", "type": "feature", "dependencies": ["T-001"]}),
        )],
    )
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        "these dependencies would make a cycle: T-001 -> T-002 -> T-001"
    );
}

#[test]
fn a_sprint_starts_its_history_and_auto_components_come_from_the_files() {
    let meta = json!({
        "sprints": [{"id": "S1", "state": "planned"}],
        "components": [{"id": "ui", "paths": ["dashboard/**"]}]
    });
    let mut p = project(json!([]), meta);
    let added = add_new_tasks(
        &mut p,
        &[task(json!({
            "name": "a", "type": "feature", "sprint": "s1", "files": ["dashboard/x.ts"], "components": "auto"
        }))],
    )
    .unwrap();
    let t = &added.tasks[0];
    assert_eq!(t["sprint"], json!("S1"));
    assert_eq!(t["sprint_history"][0]["sprint"], json!("S1"));
    assert_eq!(t["components"], json!(["ui"]));
}

#[test]
fn a_note_goes_on_any_task_and_says_when_it_is_archived() {
    let mut p = project(json!([{"id": "T-1", "status": "done"}]), json!({}));
    let t = crate::changes::add_task_note(&mut p, "T-1", "hello").unwrap();
    assert_eq!(t["notes"][0]["text"], json!("hello"));
    assert_eq!(t["notes"][0]["by"], json!("claude:a"));
    let err = crate::changes::add_task_note(&mut p, "T-2", "x").unwrap_err();
    assert_eq!(err.to_string(), "no task T-2");
    let long = "x".repeat(501);
    assert!(crate::changes::add_task_note(&mut p, "T-1", &long).is_err());
}

#[test]
fn an_edit_says_what_changed_and_writes_nothing_when_it_fails() {
    let mut p = project(
        json!([
            {"id": "T-1", "name": "a", "status": "todo", "labels": ["x"], "release": "R"},
            {"id": "T-2", "name": "b", "status": "todo", "dependencies": ["T-1"]}
        ]),
        json!({"releases": [{"id": "R"}]}),
    );
    let edit = TaskEdit {
        name: Some("A".into()),
        add_labels: vec!["y".into()],
        remove_labels: vec!["x".into()],
        release: Some(String::new()),
        ..TaskEdit::default()
    };
    let edited = edit_existing_task(&mut p, "T-1", &edit).unwrap();
    assert_eq!(edited.changed, ["name", "labels", "release"]);
    assert_eq!(edited.task["labels"], json!(["y"]));
    assert!(edited.task.get("release").is_none());
    assert_eq!(edited.task["updated_by"], json!("claude:a"));

    let nothing = edit_existing_task(&mut p, "T-1", &TaskEdit::default()).unwrap();
    assert_eq!(nothing.changed.len(), 0);

    let cycle = TaskEdit {
        name: Some("changed".into()),
        add_dependencies: vec!["T-2".into()],
        ..TaskEdit::default()
    };
    let err = edit_existing_task(&mut p, "T-1", &cycle).unwrap_err();
    assert_eq!(
        err.to_string(),
        "these dependencies would make a cycle: T-1 -> T-2 -> T-1"
    );
    assert_eq!(
        p.store().document().unwrap()["tasks"][0]["name"],
        json!("A")
    );
    for (edit, message) in [
        (
            TaskEdit {
                add_dependencies: vec!["T-1".into()],
                ..TaskEdit::default()
            },
            "T-1 can't depend on itself",
        ),
        (
            TaskEdit {
                add_dependencies: vec!["T-9".into()],
                ..TaskEdit::default()
            },
            "T-1 can't depend on T-9, which doesn't exist",
        ),
        (
            TaskEdit {
                add_requirements: vec![" ".into()],
                ..TaskEdit::default()
            },
            "an empty requirement can't be added",
        ),
        (
            TaskEdit {
                add_related: vec!["T-1".into()],
                ..TaskEdit::default()
            },
            "T-1 can't be related to itself",
        ),
    ] {
        assert_eq!(
            edit_existing_task(&mut p, "T-1", &edit)
                .unwrap_err()
                .to_string(),
            message
        );
    }
}
