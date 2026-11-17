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
    .with_actor(Some("claude:a".into()))
}

fn document(p: &Project) -> Value {
    p.store().document().unwrap().clone()
}

fn feature(extra: Value) -> Task {
    let mut t = task(json!({"name": "a", "type": "feature"}));
    t.extend(task(extra));
    t
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

fn edit_of(f: impl FnOnce(&mut TaskEdit)) -> TaskEdit {
    let mut e = TaskEdit::default();
    f(&mut e);
    e
}

#[test]
fn a_cycle_is_found_whatever_its_length_and_only_when_there_is_one() {
    let t = |id: &str, deps: Value| task(json!({"id": id, "dependencies": deps}));
    let cases: [(Vec<Task>, Option<Vec<&str>>); 5] = [
        (Vec::new(), None),
        (vec![t("A", json!(["A"]))], Some(vec!["A", "A"])),
        (
            vec![
                t("A", json!(["B"])),
                t("B", json!(["C"])),
                t("C", json!([])),
            ],
            None,
        ),
        // a dependency on a task that isn't there is no edge
        (vec![t("A", json!(["Z"]))], None),
        (
            vec![
                t("A", json!(["B"])),
                t("B", json!(["C"])),
                t("C", json!(["B"])),
            ],
            Some(vec!["B", "C", "B"]),
        ),
    ];
    for (tasks, expected) in cases {
        let refs: Vec<&Task> = tasks.iter().collect();
        assert_eq!(
            find_cycle(&refs),
            expected.map(|c| c.into_iter().map(String::from).collect())
        );
    }
    // a chain of thousands: no recursion to overflow
    let n = 20_000;
    let mut chain: Vec<Task> = (0..n)
        .map(|i| t(&format!("T-{i}"), json!([format!("T-{}", i + 1)])))
        .collect();
    let refs: Vec<&Task> = chain.iter().collect();
    assert_eq!(find_cycle(&refs), None);
    chain.push(t(&format!("T-{n}"), json!(["T-0"])));
    let refs: Vec<&Task> = chain.iter().collect();
    assert_eq!(find_cycle(&refs).map(|c| c.len()), Some(n + 2));
}

#[test]
fn a_label_is_lowercase_letters_digits_dashes_and_underscores_up_to_32() {
    for (label, ok) in [
        ("ui", true),
        ("a-b_c9", true),
        (&"x".repeat(32)[..], true),
        (&"x".repeat(33)[..], false),
        ("", false),
        ("Ui", false),
        ("a b", false),
        ("é", false),
    ] {
        assert_eq!(valid_label(label), ok, "{label:?}");
    }
}

#[test]
fn a_list_field_is_read_as_python_reads_it() {
    assert_eq!(py_list(None, "files").unwrap(), Vec::<Value>::new());
    assert_eq!(
        py_list(Some(&json!(null)), "files").unwrap(),
        Vec::<Value>::new()
    );
    assert_eq!(
        py_list(Some(&json!("")), "files").unwrap(),
        Vec::<Value>::new()
    );
    assert_eq!(
        py_list(Some(&json!("ab")), "files").unwrap(),
        [json!("a"), json!("b")]
    );
    assert_eq!(
        py_list(Some(&json!({"k": 1})), "files").unwrap(),
        [json!("k")]
    );
    assert_eq!(
        py_list(Some(&json!(5)), "files").unwrap_err().to_string(),
        "files 5: expected a list"
    );
}

#[test]
fn a_new_task_needs_a_name_and_a_type_that_arent_empty() {
    for entry in [
        json!({"type": "feature"}),
        json!({"name": "", "type": "feature"}),
        json!({"name": null, "type": "feature"}),
    ] {
        let mut p = project(json!([]), json!({}));
        assert_eq!(
            refused(&mut p, |p| add_new_tasks(p, &[task(entry.clone())])),
            "a new task needs a name",
            "{entry}"
        );
    }
    let mut p = project(json!([]), json!({}));
    assert_eq!(
        refused(&mut p, |p| add_new_tasks(
            p,
            &[task(json!({"name": "a", "type": ""}))]
        )),
        "a new task needs a type"
    );
}

#[test]
fn a_new_tasks_optional_fields_are_kept_only_when_given() {
    let mut p = project(json!([]), json!({"releases": [{"id": "R"}]}));
    let added = add_new_tasks(
        &mut p,
        &[
            feature(json!({"priority": null, "description": null, "labels": []})),
            feature(json!({
                "priority": "P0", "release": "R", "review": "batch", "with": "user", "labels": ["ui"]
            })),
        ],
    )
    .unwrap();
    let plain = &added.tasks[0];
    assert_eq!(plain["description"], json!(""));
    for absent in [
        "priority",
        "labels",
        "release",
        "review",
        "with",
        "components",
        "sprint",
    ] {
        assert!(plain.get(absent).is_none(), "{absent}");
    }
    let full = &added.tasks[1];
    assert_eq!(
        (
            full["priority"].clone(),
            full["release"].clone(),
            full["review"].clone(),
            full["with"].clone(),
            full["labels"].clone()
        ),
        (
            json!("P0"),
            json!("R"),
            json!("batch"),
            json!("user"),
            json!(["ui"])
        )
    );
    assert_eq!(
        refused(&mut p, |p| add_new_tasks(
            p,
            &[feature(json!({"release": "Q"}))]
        )),
        "no release 'Q'; the releases: R"
    );
}

#[test]
fn a_placeholder_is_used_once() {
    let mut p = project(json!([]), json!({}));
    assert_eq!(
        refused(&mut p, |p| add_new_tasks(
            p,
            &[feature(json!({"id": "x"})), feature(json!({"id": "x"}))]
        )),
        "placeholder ID 'x' is used twice"
    );
}

#[test]
fn a_new_task_may_depend_on_or_relate_to_an_archived_one_but_not_an_unknown_one() {
    let mut p = project_with_archive(
        json!([{"id": "T-002", "status": "todo"}]),
        json!([{"id": "T-001", "status": "done"}]),
        json!({}),
    );
    let added = add_new_tasks(
        &mut p,
        &[feature(
            json!({"dependencies": ["T-001"], "related": ["T-001", "T-002"]}),
        )],
    )
    .unwrap();
    assert_eq!(added.tasks[0]["id"], json!("T-003"));
    assert_eq!(
        refused(&mut p, |p| add_new_tasks(
            p,
            &[feature(json!({"related": ["T-9"]}))]
        )),
        "T-004 can't be related to T-9, which doesn't exist"
    );
    assert_eq!(
        refused(&mut p, |p| add_new_tasks(
            p,
            &[feature(json!({"id": "n", "related": ["n"]}))]
        )),
        "T-004 can't be related to itself"
    );
}

#[test]
fn the_next_number_ignores_a_next_id_that_isnt_a_whole_number() {
    for next_id in [json!("12"), json!(1.5), json!(null)] {
        let mut p = project(
            json!([{"id": "T-004", "status": "todo"}]),
            json!({"next_id": next_id}),
        );
        let added = add_new_tasks(&mut p, &[feature(json!({}))]).unwrap();
        assert_eq!(added.tasks[0]["id"], json!("T-005"));
    }
    // IDs of another prefix don't count, and a task without a number keeps the pattern
    let mut p = project(
        json!([{"id": "X-900", "status": "todo"}, {"id": "T-01", "status": "todo"}, {"id": "misc"}]),
        json!({}),
    );
    let added = add_new_tasks(&mut p, &[feature(json!({})), feature(json!({}))]).unwrap();
    assert_eq!(ids(&added), ["T-02", "T-03"]);
}

#[test]
fn an_edit_of_an_unknown_task_says_when_it_is_archived() {
    let mut p = project_with_archive(json!([]), json!([{"id": "T-0"}]), json!({}));
    assert_eq!(
        refused(&mut p, |p| edit_existing_task(
            p,
            "T-9",
            &TaskEdit::default()
        )),
        "no task T-9"
    );
    assert_eq!(
        refused(&mut p, |p| edit_existing_task(
            p,
            "T-0",
            &TaskEdit::default()
        )),
        "no task T-0 (it is archived: atm db unarchive brings it back)"
    );
}

#[test]
fn an_edit_moves_a_task_between_open_sprints_and_the_backlog() {
    let mut p = project(
        json!([{"id": "T-1", "status": "todo"}]),
        json!({"sprints": [{"id": "A"}, {"id": "Old", "state": "closed"}]}),
    );
    let edited =
        edit_existing_task(&mut p, "T-1", &edit_of(|e| e.sprint = Some("a".into()))).unwrap();
    assert_eq!(
        (edited.changed.clone(), edited.task["sprint"].clone()),
        (vec!["sprint".to_string()], json!("A"))
    );
    assert_eq!(
        refused(&mut p, |p| edit_existing_task(
            p,
            "T-1",
            &edit_of(|e| e.sprint = Some("Old".into()))
        )),
        "the sprint 'Old' is closed; a task can only go into a planned or active sprint"
    );
    assert_eq!(
        refused(&mut p, |p| edit_existing_task(
            p,
            "T-1",
            &edit_of(|e| e.sprint = Some("Z".into()))
        )),
        "no sprint 'Z'; the open sprints: A"
    );
    // backlog wins over a sprint named with it
    let edited = edit_existing_task(
        &mut p,
        "T-1",
        &edit_of(|e| {
            e.sprint = Some("A".into());
            e.backlog = true;
        }),
    )
    .unwrap();
    assert!(edited.task.get("sprint").is_none());
    let again = edit_existing_task(&mut p, "T-1", &edit_of(|e| e.backlog = true)).unwrap();
    assert!(again.changed.is_empty(), "already in the backlog");
}

#[test]
fn an_edit_checks_each_value_against_the_vocabularies() {
    let mut p = project(json!([{"id": "T-1", "status": "todo"}]), json!({}));
    for (edit, start) in [
        (
            edit_of(|e| e.type_ = Some("nope".into())),
            "unknown type 'nope'; use one of",
        ),
        (
            edit_of(|e| e.priority = Some("P9".into())),
            "unknown priority 'P9'; use one of",
        ),
        (
            edit_of(|e| e.review = Some("x".into())),
            "review 'x': use one of",
        ),
        (
            edit_of(|e| e.with_ = Some("bob".into())),
            "with 'bob': use user",
        ),
        (edit_of(|e| e.release = Some("R".into())), "no release 'R'"),
        (edit_of(|e| e.add_labels = vec!["No".into()]), "label 'No'"),
        (
            edit_of(|e| e.add_use_cases = vec![String::new()]),
            "an empty use case can't be added",
        ),
        (
            edit_of(|e| e.add_related = vec!["T-9".into()]),
            "T-1 can't be related to T-9, which doesn't exist",
        ),
    ] {
        let message = refused(&mut p, |p| edit_existing_task(p, "T-1", &edit));
        assert!(message.starts_with(start), "{message} / {start}");
    }
}

#[test]
fn an_edit_sets_and_removes_the_review_and_who_it_is_done_with() {
    let mut p = project(json!([{"id": "T-1", "status": "todo"}]), json!({}));
    let set = edit_existing_task(
        &mut p,
        "T-1",
        &edit_of(|e| {
            e.review = Some("blocker".into());
            e.with_ = Some("user".into());
        }),
    )
    .unwrap();
    assert_eq!(set.changed, ["review", "with"]);
    let removed = edit_existing_task(
        &mut p,
        "T-1",
        &edit_of(|e| {
            e.review = Some(String::new());
            e.with_ = Some(String::new());
            e.release = Some(String::new());
        }),
    )
    .unwrap();
    assert_eq!(removed.changed, ["review", "with"], "no release to remove");
    assert!(removed.task.get("review").is_none() && removed.task.get("with").is_none());
}

#[test]
fn an_edit_of_lists_adds_whats_new_removes_whats_named_and_drops_empty_lists() {
    let mut p = project_with_archive(
        json!([
            {"id": "T-1", "status": "todo", "files": ["a"], "related": ["T-2"], "updated_at": "U"},
            {"id": "T-2", "status": "todo"}
        ]),
        json!([{"id": "T-0"}]),
        json!({"components": [{"id": "ui", "paths": ["dashboard/**"]}]}),
    );
    let same = edit_existing_task(
        &mut p,
        "T-1",
        &edit_of(|e| {
            e.add_files = vec!["a".into()];
            e.remove_files = vec!["zzz".into()];
        }),
    )
    .unwrap();
    assert_eq!(same.changed.len(), 0);
    assert_eq!(
        document(&p)["tasks"][0]["updated_at"],
        json!("U"),
        "nothing written"
    );
    let edited = edit_existing_task(
        &mut p,
        "T-1",
        &edit_of(|e| {
            e.add_files = vec!["dashboard/x.ts".into()];
            e.components_auto = true;
            e.remove_related = vec!["T-2".into()];
            e.add_dependencies = vec!["T-0".into()];
            e.add_requirements = vec!["[R-1](docs/r.md)".into()];
        }),
    )
    .unwrap();
    assert_eq!(
        edited.changed,
        [
            "files",
            "requirements",
            "components",
            "related",
            "dependencies"
        ]
    );
    assert_eq!(edited.task["components"], json!(["ui"]));
    assert!(edited.task.get("related").is_none(), "an empty list goes");
    assert_eq!(
        edited.task["dependencies"],
        json!(["T-0"]),
        "an archived task is a dependency"
    );
}

#[test]
fn an_edit_may_name_another_projects_task() {
    let foreign = "other:0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8091";
    let mut p = project(json!([{"id": "T-1", "status": "todo"}]), json!({}));
    let edited = edit_existing_task(
        &mut p,
        "T-1",
        &edit_of(|e| {
            e.add_dependencies = vec![foreign.into()];
            e.add_related = vec![foreign.into()];
        }),
    )
    .unwrap();
    assert_eq!(edited.task["dependencies"], json!([foreign]));
}

#[test]
fn a_label_is_renamed_on_every_task_and_in_the_vocabulary() {
    let mut p = project_with_archive(
        json!([
            {"id": "T-1", "labels": ["old", "x"], "updated_at": "U"},
            {"id": "T-2", "labels": ["old", "new"]},
            {"id": "T-3", "labels": ["x"]},
            {"id": "T-4"}
        ]),
        json!([{"id": "T-0", "labels": ["old"]}]),
        json!({"labels": ["old", {"id": "old", "label": "Old"}, "x"]}),
    );
    assert_eq!(
        relabel(&mut p, "old", "new").unwrap(),
        RelabeledTasks {
            tasks: 2,
            archived: 1
        }
    );
    let doc = document(&p);
    assert_eq!(doc["tasks"][0]["labels"], json!(["new", "x"]));
    assert_eq!(doc["tasks"][0]["updated_at"], json!("U"));
    assert_eq!(doc["tasks"][1]["labels"], json!(["new"]), "one, not two");
    assert_eq!(
        doc["meta"]["labels"],
        json!(["new", {"id": "new", "label": "Old"}, "x"])
    );
    assert_eq!(
        p.store().list_archived().unwrap()[0]["labels"],
        json!(["new"])
    );
}

#[test]
fn a_label_is_renamed_only_to_a_valid_other_label_that_a_task_has() {
    let mut p = project(json!([{"id": "T-1", "labels": ["a"]}]), json!({}));
    assert_eq!(
        refused(&mut p, |p| relabel(p, "a", "B")),
        "label 'B': use lowercase letters, digits, - and _ (at most 32 characters)"
    );
    assert_eq!(
        refused(&mut p, |p| relabel(p, "a", "a")),
        "the label is already 'a'"
    );
    let before = document(&p);
    let error = relabel(&mut p, "zzz", "b").unwrap_err();
    assert_eq!(
        (error.to_string(), error.code()),
        ("no task has the label 'zzz'".to_string(), 2)
    );
    assert_eq!(document(&p), before);
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
fn in_a_keys_file_new_tasks_get_a_uuid_the_next_number_and_references_by_key() {
    let existing = "0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8091";
    let meta = json!({"format": 2, "key": "ATM", "key_history": ["T"], "next_number": 5});
    let mut p = project(
        json!([{"id": existing, "number": 4, "key_final": true, "status": "done"}]),
        meta.clone(),
    );
    let entries = [
        task(json!({"id": "new-1", "name": "a", "type": "feature", "dependencies": ["ATM-4"]})),
        task(
            json!({"id": "new-2", "name": "b", "type": "test", "dependencies": ["new-1", "T-4"], "related": ["atm-4"]}),
        ),
    ];
    let added = add_new_tasks(&mut p, &entries).unwrap();
    let uuids = ids(&added);
    assert!(uuids.iter().all(|u| atm_common::ids::valid_task_uuid(u)));
    assert_eq!(added.tasks[0]["number"], json!(5));
    assert_eq!(added.tasks[1]["number"], json!(6));
    assert_eq!(added.tasks[0]["key_final"], json!(false), "provisional");
    assert_eq!(added.tasks[0]["dependencies"], json!([existing]));
    assert_eq!(added.tasks[1]["dependencies"], json!([uuids[0], existing]));
    assert_eq!(added.tasks[1]["related"], json!([existing]));
    assert_eq!(
        p.store().document().unwrap()["meta"]["next_number"],
        json!(7)
    );

    let mut p = project(json!([]), meta.clone());
    let unknown = add_new_tasks(
        &mut p,
        &[task(
            json!({"name": "a", "type": "feature", "dependencies": ["ATM-99"]}),
        )],
    )
    .unwrap_err();
    assert_eq!(
        unknown.to_string(),
        "ATM-5 (a) depends on ATM-99, which doesn't exist"
    );
    let mut p = project(json!([]), meta);
    let itself = add_new_tasks(
        &mut p,
        &[task(
            json!({"id": "new-1", "name": "a", "type": "feature", "related": ["new-1"]}),
        )],
    )
    .unwrap_err();
    assert_eq!(itself.to_string(), "ATM-5 can't be related to itself");
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
