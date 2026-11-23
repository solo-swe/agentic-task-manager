use super::*;
use crate::jsongit::paths::find_paths;
use crate::test_dir::TestDir;
use atm_common::TASK_DIR;

fn project(tasks: &str) -> (TestDir, Paths) {
    let dir = TestDir::new();
    std::fs::create_dir(dir.path().join(TASK_DIR)).unwrap();
    std::fs::write(dir.path().join(TASK_DIR).join("tasks.json"), tasks).unwrap();
    let paths = find_paths(dir.path()).unwrap();
    (dir, paths)
}

#[test]
fn a_legacy_task_file_is_moved_and_the_folder_set_up() {
    let dir = TestDir::new();
    std::fs::create_dir(dir.path().join("docs")).unwrap();
    std::fs::write(dir.path().join("docs/tasks.json"), r#"{"tasks": []}"#).unwrap();
    std::fs::write(dir.path().join("docs/tasks-dashboard.html"), "old").unwrap();
    let paths = find_paths(dir.path()).unwrap();
    let result = migrate(&paths).unwrap();
    assert_eq!(
        result["moved"],
        json!({"from": "docs/tasks.json", "to": ".agentic-task-manager/tasks.json", "git": false})
    );
    assert_eq!(
        result["removed_dashboard"],
        json!("docs/tasks-dashboard.html")
    );
    assert!(paths.tasks.is_file() && !dir.path().join("docs/tasks.json").exists());
    let ignore = std::fs::read_to_string(paths.dir.join(".gitignore")).unwrap();
    assert!(ignore.starts_with("# atm runtime files (managed by atm)\ntasks-dashboard.html\n"));
    assert_eq!(
        describe_migration(&result, &paths)[0],
        "Moved docs/tasks.json to .agentic-task-manager/tasks.json; removed docs/tasks-dashboard.html (the \
         dashboard now comes with atm: /atm-serve)."
    );
    // A second run has nothing to do
    let again = migrate(&paths).unwrap();
    assert_eq!(describe_migration(&again, &paths), ["Nothing to migrate."]);
}

#[test]
fn finish_times_and_with_user_are_filled_in_once() {
    let (_dir, paths) = project(
        r#"{"tasks": [
            {"id": "T-1", "status": "done", "updated_at": "2026-01-01T00:00:00Z"},
            {"id": "T-2", "status": "todo", "labels": ["interactive"]}
        ]}"#,
    );
    let result = migrate(&paths).unwrap();
    assert_eq!(
        (
            result["finish_times_filled"].clone(),
            result["with_user_from_label"].clone()
        ),
        (json!(1), json!(1))
    );
    let document = read_json(&paths.tasks).unwrap();
    assert_eq!(
        document["tasks"][0]["completed_at"],
        json!("2026-01-01T00:00:00Z")
    );
    assert_eq!(document["tasks"][1]["with"], json!("user"));
    // The label moved to `with: user` (ATM-721): gone from the task
    assert!(document["tasks"][1].get("labels").is_none());
    assert_eq!(
        document["meta"]["migrations"],
        json!(["interactive-label", "interactive-label-moved"])
    );
}

/// ATM-721 (decision 1): `with: user` alone says a task was done with the user, so the label `interactive` moves to it
/// on every task, finished and archived ones too, once; a `with` a task has stays
#[test]
fn the_interactive_label_moves_to_with_user_everywhere_once() {
    let (_dir, paths) = project(
        r#"{"meta": {"migrations": ["interactive-label"]}, "tasks": [
            {"id": "T-1", "status": "done", "completed_at": "2026-01-01T00:00:00Z", "labels": ["ux", "interactive"]},
            {"id": "T-2", "status": "todo", "with": "agent", "labels": ["interactive"]},
            {"id": "T-3", "status": "todo", "labels": ["ux"]}
        ]}"#,
    );
    std::fs::write(
        paths.archive(),
        r#"{"tasks": [{"id": "T-0", "status": "done", "completed_at": "2026-01-01T00:00:00Z", "labels": ["interactive"]}]}"#,
    )
    .unwrap();
    let result = migrate(&paths).unwrap();
    assert_eq!(result["interactive_label_moved"], json!(3));
    assert!(describe_migration(&result, &paths).contains(
        &"Moved the label interactive to `with: user` on 3 tasks, finished ones too: `with: user` alone says the work was done with you."
            .to_string()
    ));
    let document = read_json(&paths.tasks).unwrap();
    assert_eq!(document["tasks"][0]["labels"], json!(["ux"]));
    assert_eq!(document["tasks"][0]["with"], json!("user"));
    assert_eq!(document["tasks"][1]["with"], json!("agent"));
    assert!(document["tasks"][1].get("labels").is_none());
    assert_eq!(document["tasks"][2]["labels"], json!(["ux"]));
    let archived = read_json(&paths.archive()).unwrap();
    assert_eq!(archived["tasks"][0]["with"], json!("user"));
    // Once: a label put back later stays
    let mut again = document.clone();
    again["tasks"][2]["labels"] = json!(["interactive"]);
    std::fs::write(&paths.tasks, serde_json::to_string(&again).unwrap()).unwrap();
    assert_eq!(
        migrate(&paths).unwrap()["interactive_label_moved"],
        json!(0)
    );
}

/// ADR-009 (ATM-635): `key_final`, true or false, leaves the task file and the archive once, and says so; after it
/// the files are left alone
#[test]
fn key_final_is_dropped_from_the_tasks_and_the_archive_once() {
    let (_dir, paths) = project(
        r#"{"meta": {"format": 2, "key": "ATM"}, "tasks": [
            {"id": "0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8001", "number": 1, "key_final": true, "status": "todo"},
            {"id": "0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8002", "number": 2, "key_final": false, "status": "todo"},
            {"id": "0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8003", "number": 3, "status": "todo"}
        ]}"#,
    );
    std::fs::write(
        paths.archive(),
        r#"{"tasks": [{"id": "0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8000", "number": 9, "key_final": true, "status": "done"}]}"#,
    )
    .unwrap();
    let result = migrate(&paths).unwrap();
    assert_eq!(result["key_final_dropped"], json!(3));
    assert!(
        describe_migration(&result, &paths)
            .contains(&"Task keys are plain now (ADR-009): dropped key_final from 3 tasks (staged, not committed).".to_string())
    );
    for file in [paths.tasks.clone(), paths.archive()] {
        let document = read_json(&file).unwrap();
        assert!(
            document["tasks"]
                .as_array()
                .unwrap()
                .iter()
                .all(|t| t.get("key_final").is_none()),
            "{}",
            file.display()
        );
    }
    let document = read_json(&paths.tasks).unwrap();
    assert_eq!(
        document["tasks"][1]["number"],
        json!(2),
        "nothing else changed"
    );
    let again = migrate(&paths).unwrap();
    assert_eq!(again["key_final_dropped"], json!(0));
    assert_eq!(describe_migration(&again, &paths), ["Nothing to migrate."]);
}

/// ADR-009 (ATM-636): what a merge renumbered is said once by the next migrate, then forgotten
#[test]
fn what_a_merge_renumbered_is_said_once() {
    let (_dir, paths) = project(
        r#"{"meta": {"format": 2, "key": "ATM", "renumbered": [{"from": "ATM-5", "to": "ATM-9"}]}, "tasks": [
            {"id": "0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8009", "number": 9, "aliases": ["ATM-5"], "status": "todo"}
        ]}"#,
    );
    let result = migrate(&paths).unwrap();
    assert_eq!(
        result["renumbered"],
        json!([{"from": "ATM-5", "to": "ATM-9"}])
    );
    assert!(describe_migration(&result, &paths).contains(
        &"ATM-5 is now ATM-9 (both branches made ATM-5); ATM-5 still finds it.".to_string()
    ));
    assert_eq!(
        read_json(&paths.tasks).unwrap()["meta"].get("renumbered"),
        None
    );
    let again = migrate(&paths).unwrap();
    assert_eq!(again["renumbered"], json!([]));
    assert_eq!(describe_migration(&again, &paths), ["Nothing to migrate."]);
}

#[test]
fn phases_become_sprints_named_after_them() {
    let mut document = json!({
        "meta": {"phases": [{"id": 1, "name": " Setup "}, {"id": 2, "name": "Setup"}]},
        "tasks": [
            {"id": "T-1", "phase": 1, "status": "done", "completed_at": "2026-01-02T00:00:00Z", "created_at": "c"},
            {"id": "T-2", "phase": 2, "status": "todo"},
            {"id": "T-3", "phase": 3, "status": "todo", "sprint": "Mine"}
        ]
    });
    let vocab = Vocabulary::of_document(&json!({})).unwrap();
    let added = phases_to_sprints(&mut document, &mut [], &vocab, "now");
    let ids: Vec<&Value> = added.iter().map(|s| &s["id"]).collect();
    assert_eq!(
        ids,
        [
            &json!("Setup"),
            &json!("Setup (phase 2)"),
            &json!("Phase 3")
        ]
    );
    assert_eq!(added[0]["state"], json!("closed"));
    assert_eq!(added[0]["report"], json!({"done": ["T-1"], "carried": []}));
    assert_eq!(document["tasks"][0]["sprint"], json!("Setup"));
    assert_eq!(
        document["tasks"][0]["sprint_history"],
        json!([{"sprint": "Setup", "from": "c"}])
    );
    assert_eq!(document["tasks"][2]["sprint"], json!("Mine"));
    assert!(
        document["tasks"][2].get("phase").is_none() && document["meta"].get("phases").is_none()
    );
    assert_eq!(
        describe_sprints_from_phases(&added),
        [
            "Phases are now sprints: turned 3 phases into sprints (staged, not committed).",
            "Closed (all their tasks finished): Setup.",
            "Planned: Setup (phase 2), Phase 3.",
            "None is active yet: plan one with /atm-plan-sprint."
        ]
    );
}
