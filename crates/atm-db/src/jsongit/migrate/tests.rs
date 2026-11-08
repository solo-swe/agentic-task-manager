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
    assert_eq!(document["meta"]["migrations"], json!(["interactive-label"]));
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
