use super::*;

fn meta(v: Value) -> Map<String, Value> {
    match v {
        Value::Object(m) => m,
        other => panic!("not an object: {other}"),
    }
}

#[test]
fn ids_are_meta_id_unless_taken_or_invalid_then_from_the_path() {
    let path = "/w/a";
    let hash = &sha1_hex(path.as_bytes())[..7];
    assert_eq!(project_id(&meta(json!({})), path), format!("p{hash}"));
    let registry = json!({"projects": [{"id": "x", "path": "/w/b"}]});
    assert_eq!(unique_id(&registry, path, "y"), "y");
    assert_eq!(unique_id(&registry, path, "x"), format!("x-{hash}"));
    assert_eq!(unique_id(&registry, path, "a b"), format!("p-{hash}"));
}

#[test]
fn an_entry_is_added_kept_and_removed_when_the_project_opts_out() {
    let mut registry = empty_registry();
    let path = Path::new("/w/proj");
    let m = meta(json!({"id": "proj", "project": "Proj"}));
    assert_eq!(update_entry(&mut registry, path, &m), "added");
    assert_eq!(update_entry(&mut registry, path, &m), "unchanged");
    assert_eq!(registry["projects"][0]["name"], json!("Proj"));
    let renamed = meta(json!({"id": "proj", "project": "New"}));
    assert_eq!(update_entry(&mut registry, path, &renamed), "updated");
    let out = meta(json!({"id": "proj", "dashboard": {"listed": false}}));
    assert_eq!(update_entry(&mut registry, path, &out), "removed");
    assert_eq!(update_entry(&mut registry, path, &out), "unlisted");
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("atm-db-registry-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn a_project(dir: &Path, id: &str) -> String {
    let folder = dir.join(id);
    std::fs::create_dir_all(folder.join(TASK_DIR)).unwrap();
    std::fs::write(
        folder.join(TASK_DIR).join(TASKS_FILE),
        to_json(&json!({"meta": {"id": id, "project": id}, "tasks": []})),
    )
    .unwrap();
    folder.shown()
}

fn listed_paths(home: &Path) -> Vec<String> {
    let registry = read_json(&registry_file(home)).unwrap_or(Value::Null);
    entries(&registry)
        .iter()
        .filter_map(|e| e.get("path").and_then(Value::as_str).map(String::from))
        .collect()
}

/// T-501: an older atm keeps listing projects in `~/.agentic-task-manager` after its content moved; they join
/// `default`, only added, only when that list changed, and a folder that's gone stays out
#[test]
fn projects_an_older_atm_listed_after_the_move_join_default() {
    let dir = scratch("legacy");
    let (legacy, home) = (dir.join("legacy"), dir.join("state"));
    std::fs::create_dir_all(&legacy).unwrap();
    let (kept, theirs, other) = (
        a_project(&dir, "kept"),
        a_project(&dir, "theirs"),
        a_project(&dir, "other"),
    );
    // The state folder already lists `kept`; the older atm lists `kept`, `theirs`, and a folder that's gone
    write_registry(
        &registry_file(&home),
        &json!({"version": 1, "projects": [{"id": "kept", "name": "kept", "path": kept}]}),
    )
    .unwrap();
    let gone = dir.join("gone").shown();
    let older = |paths: &[&str]| {
        let projects: Vec<Value> = paths
            .iter()
            .map(|p| json!({"id": "x", "path": p}))
            .collect();
        std::fs::write(
            legacy.join(LEGACY_REGISTRY_FILE),
            to_json(&json!({"version": 1, "projects": projects})),
        )
        .unwrap();
    };
    older(&[&kept, &theirs, &gone]);
    assert_eq!(
        adopt_legacy_projects(&legacy, &home).unwrap(),
        vec![theirs.clone()]
    );
    assert_eq!(listed_paths(&home), vec![kept.clone(), theirs.clone()]);
    // Unchanged since: nothing is taken in again, so a project removed from `default` stays removed
    write_registry(
        &registry_file(&home),
        &json!({"version": 1, "projects": [{"id": "kept", "name": "kept", "path": kept}]}),
    )
    .unwrap();
    assert_eq!(
        adopt_legacy_projects(&legacy, &home).unwrap(),
        Vec::<String>::new()
    );
    assert_eq!(listed_paths(&home), vec![kept.clone()]);
    // The older atm lists another project: that one joins (and `theirs`, which it still lists)
    older(&[&theirs, &other, "/a/folder/that/is/gone/too"]);
    assert_eq!(
        adopt_legacy_projects(&legacy, &home).unwrap(),
        vec![theirs.clone(), other.clone()]
    );
    // No legacy registry at all: nothing happens
    std::fs::remove_file(legacy.join(LEGACY_REGISTRY_FILE)).unwrap();
    assert_eq!(
        adopt_legacy_projects(&legacy, &home).unwrap(),
        Vec::<String>::new()
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_project_id_is_one_to_64_url_safe_characters_and_a_workspace_id_does_not_start_with_a_dot() {
    // (the ID, a valid project ID, a valid workspace ID)
    for (id, project, workspace) in [
        ("work", true, true),
        ("a.b_c-1", true, true),
        (".hidden", true, false),
        ("x".repeat(64).as_str(), true, true),
        ("x".repeat(65).as_str(), false, false),
        ("", false, false),
        ("a b", false, false),
        ("a/b", false, false),
        ("é", false, false),
    ] {
        assert_eq!(valid_project_id(id), project, "{id:?}");
        assert_eq!(valid_workspace_id(id), workspace, "{id:?}");
    }
}

#[test]
fn an_id_a_project_already_has_stays_its_own_and_a_long_taken_one_is_made_from_the_path() {
    let path = "/w/a";
    let hash = &sha1_hex(path.as_bytes())[..7];
    let registry =
        json!({"projects": [{"id": "x", "path": path}, {"id": &"y".repeat(57), "path": "/w/b"}]});
    assert_eq!(unique_id(&registry, path, "x"), "x");
    assert_eq!(
        unique_id(&registry, path, &"y".repeat(57)),
        format!("p-{hash}")
    );
    assert_eq!(
        project_id(&meta(json!({"id": ""})), path),
        format!("p{hash}")
    );
}

#[test]
fn a_projects_name_is_meta_project_else_its_folders_name() {
    let path = Path::new("/w/folder");
    for (m, name) in [
        (json!({"project": "Proj"}), "Proj"),
        (json!({"project": ""}), "folder"),
        (json!({}), "folder"),
    ] {
        assert_eq!(project_name(&meta(m.clone()), path), name, "{m}");
    }
}

#[test]
fn a_project_is_listed_unless_meta_dashboard_listed_is_false() {
    for (m, shown) in [
        (json!({}), true),
        (json!({"dashboard": {}}), true),
        (json!({"dashboard": {"listed": true}}), true),
        (json!({"dashboard": {"listed": "no"}}), true),
        (json!({"dashboard": {"listed": false}}), false),
    ] {
        assert_eq!(listed(&meta(m.clone())), shown, "{m}");
    }
}

#[test]
fn an_unchanged_entry_last_seen_a_day_ago_or_at_an_unreadable_time_is_refreshed() {
    let path = Path::new("/w/proj");
    let m = meta(json!({"id": "proj", "project": "Proj"}));
    for seen in [
        json!("2000-01-01T00:00:00Z"),
        json!("yesterday"),
        json!(null),
    ] {
        let mut registry = json!({"version": 1, "projects": [
            {"id": "proj", "name": "Proj", "path": "/w/proj", "added_at": "2000-01-01T00:00:00Z", "seen_at": seen}
        ]});
        assert_eq!(update_entry(&mut registry, path, &m), "updated", "{seen}");
        let entry = &registry["projects"][0];
        assert_ne!(entry["seen_at"], seen);
        assert_eq!(
            entry["added_at"],
            json!("2000-01-01T00:00:00Z"),
            "added_at stays"
        );
    }
}

#[test]
fn a_missing_registry_reads_as_empty_and_an_unreadable_one_is_moved_aside() {
    let dir = crate::test_dir::TestDir::new();
    let file = dir.path().join("default.json");
    assert_eq!(read_registry(&file).unwrap(), (empty_registry(), None));
    for text in [
        "{not json",
        "[]",
        r#"{"projects": {}}"#,
        r#"{"projects": [{"id": "x"}]}"#,
        r#"{"projects": [{"path": 1}]}"#,
    ] {
        std::fs::write(&file, text).unwrap();
        let (registry, damaged) = read_registry(&file).unwrap();
        assert_eq!(registry, empty_registry(), "{text}");
        let damaged = damaged.expect("said where it went");
        assert!(damaged.contains("default.json.damaged-"), "{damaged}");
        assert_eq!(std::fs::read_to_string(&damaged).unwrap(), text);
        assert!(!file.exists());
        std::fs::remove_file(&damaged).unwrap();
    }
    std::fs::write(&file, r#"{"version": 1, "projects": [{"path": "/w/a"}]}"#).unwrap();
    assert_eq!(
        read_registry(&file).unwrap(),
        (json!({"version": 1, "projects": [{"path": "/w/a"}]}), None)
    );
}

#[test]
fn the_legacy_projects_json_is_renamed_into_place_unless_the_workspace_file_is_there() {
    let dir = crate::test_dir::TestDir::new();
    let home = dir.path();
    std::fs::write(home.join(LEGACY_REGISTRY_FILE), "legacy").unwrap();
    let file = registry_file(home);
    assert_eq!(file, home.join(REGISTRY_FILE));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "legacy");
    assert!(!home.join(LEGACY_REGISTRY_FILE).exists());
    // A legacy file found later, beside the workspace file, is left alone
    std::fs::write(home.join(LEGACY_REGISTRY_FILE), "older").unwrap();
    let _ = registry_file(home);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "legacy");
    assert!(home.join(LEGACY_REGISTRY_FILE).exists());
}

#[test]
fn a_workspaces_file_is_its_id_in_the_workspaces_folder_and_only_default_always_exists() {
    let dir = crate::test_dir::TestDir::new();
    let home = dir.path();
    assert_eq!(
        workspace_file(home, "default"),
        home.join("workspaces/default.json")
    );
    assert_eq!(
        workspace_file(home, "work"),
        home.join("workspaces/work.json")
    );
    assert!(FileRegistry::at(home).exists());
    let work = FileRegistry::workspace(home, "work");
    assert_eq!(work.id(), "work");
    assert!(!work.exists());
    work.create().unwrap();
    assert!(work.exists());
}

#[test]
fn entries_are_those_with_a_path_and_none_without_a_file() {
    let dir = crate::test_dir::TestDir::new();
    let registry = FileRegistry::workspace(dir.path(), "work");
    assert_eq!(registry.entries(), Vec::<Value>::new());
    write_registry(
        &registry.file(),
        &json!({"projects": [{"id": "a", "path": "/w/a"}, {"id": "b"}, {"id": "c", "path": 3}]}),
    )
    .unwrap();
    assert_eq!(registry.entries(), [json!({"id": "a", "path": "/w/a"})]);
}

#[test]
fn the_stamp_is_null_without_a_file_and_changes_with_the_list() {
    let dir = crate::test_dir::TestDir::new();
    let registry = FileRegistry::workspace(dir.path(), "work");
    assert_eq!(registry.stamp(), Value::Null);
    registry.create().unwrap();
    let before = registry.stamp();
    assert!(before.is_array());
    write_registry(
        &registry.file(),
        &json!({"version": 1, "projects": [{"path": "/w/a"}]}),
    )
    .unwrap();
    assert_ne!(registry.stamp(), before);
}

#[test]
fn listing_marks_each_project_missing_or_not_and_replaces_a_damaged_file() {
    let dir = crate::test_dir::TestDir::new();
    let home = dir.path().join("state");
    let registry = FileRegistry::workspace(&home, "work");
    let here = a_project(dir.path(), "here");
    let gone = dir.path().join("gone").shown();
    write_registry(
        &registry.file(),
        &json!({"projects": [{"id": "here", "path": here}, {"id": "gone", "path": gone}]}),
    )
    .unwrap();
    let listed = registry.list_projects().unwrap();
    assert_eq!(listed["damaged"], Value::Null);
    let missing: Vec<&Value> = listed["projects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| &p["missing"])
        .collect();
    assert_eq!(missing, [&json!(false), &json!(true)]);
    std::fs::write(registry.file(), "{not json").unwrap();
    let listed = registry.list_projects().unwrap();
    assert!(listed["damaged"].is_string());
    assert_eq!(listed["projects"], json!([]));
    assert_eq!(read_json(&registry.file()), Some(empty_registry()));
}

#[test]
fn adding_a_project_lists_it_once_by_its_folder_relative_to_here() {
    let dir = crate::test_dir::TestDir::new();
    let registry = FileRegistry::workspace(&dir.path().join("state"), "work");
    let path = a_project(dir.path(), "one");
    let added = registry.add_project(Some("one"), dir.path()).unwrap();
    assert_eq!(added["result"], json!("added"));
    assert_eq!(added["project"]["id"], json!("one"));
    assert_eq!(added["project"]["path"], json!(path));
    assert_eq!(added["damaged"], Value::Null);
    // Again, from inside it with no target: the same entry
    let again = registry
        .add_project(None, &dir.path().join("one").join(TASK_DIR))
        .unwrap();
    assert_eq!(again["result"], json!("unchanged"));
    assert_eq!(registry.entries().len(), 1);
}

#[test]
fn adding_refuses_what_is_not_a_listed_project() {
    let dir = crate::test_dir::TestDir::new();
    let registry = FileRegistry::workspace(&dir.path().join("state"), "work");
    std::fs::create_dir_all(dir.path().join("empty")).unwrap();
    let out = dir.path().join("out");
    std::fs::create_dir_all(out.join(TASK_DIR)).unwrap();
    std::fs::write(
        out.join(TASK_DIR).join(TASKS_FILE),
        to_json(&json!({"meta": {"dashboard": {"listed": false}}, "tasks": []})),
    )
    .unwrap();
    // (the target, the start of the message, the exit code)
    for (target, message, code) in [
        ("nowhere", "nowhere: not a directory", 2),
        ("empty", "no tasks.json in ", 1),
        ("out", "", 1),
    ] {
        let error = registry.add_project(Some(target), dir.path()).unwrap_err();
        assert!(error.to_string().starts_with(message), "{target}: {error}");
        assert_eq!(error.code(), code, "{target}");
    }
    let error = registry.add_project(Some("out"), dir.path()).unwrap_err();
    assert!(
        error.to_string().contains("opts out of the dashboard"),
        "{error}"
    );
    assert!(!registry.file().exists(), "nothing was written");
}

#[test]
fn a_project_is_removed_by_its_id_or_its_path_and_an_unknown_one_is_an_error() {
    let dir = crate::test_dir::TestDir::new();
    let registry = FileRegistry::workspace(&dir.path().join("state"), "work");
    for name in ["one", "two"] {
        a_project(dir.path(), name);
        registry.add_project(Some(name), dir.path()).unwrap();
    }
    let removed = registry.remove_project("one", Path::new("/")).unwrap();
    assert_eq!(removed["removed"]["id"], json!("one"));
    let removed = registry.remove_project("two", dir.path()).unwrap();
    assert_eq!(removed["removed"]["id"], json!("two"));
    assert_eq!(registry.entries(), Vec::<Value>::new());
    let error = registry.remove_project("one", dir.path()).unwrap_err();
    assert_eq!(
        error.to_string(),
        "no project one in the workspace work (see `atm workspace projects`)"
    );
    assert_eq!(
        registry.remove_project("", dir.path()).unwrap_err().code(),
        2
    );
}

#[test]
fn removing_by_path_finds_the_project_by_a_folder_inside_it() {
    let dir = crate::test_dir::TestDir::new();
    let registry = FileRegistry::workspace(&dir.path().join("state"), "work");
    let path = a_project(dir.path(), "one");
    // Listed under another ID, so only its path finds it
    write_registry(
        &registry.file(),
        &json!({"projects": [{"id": "x", "path": path}]}),
    )
    .unwrap();
    let removed = registry
        .remove_project(TASK_DIR, &dir.path().join("one"))
        .unwrap();
    assert_eq!(removed["removed"]["id"], json!("x"));
}

#[test]
fn pruning_removes_the_projects_whose_task_file_is_gone_and_keeps_the_rest() {
    let dir = crate::test_dir::TestDir::new();
    let registry = FileRegistry::workspace(&dir.path().join("state"), "work");
    let here = a_project(dir.path(), "here");
    let gone = dir.path().join("gone").shown();
    let entry = |id: &str, path: &str| json!({"id": id, "path": path});
    write_registry(
        &registry.file(),
        &json!({"projects": [entry("gone", &gone), entry("here", &here)]}),
    )
    .unwrap();
    let pruned = registry.prune().unwrap();
    assert_eq!(pruned["removed"], json!([entry("gone", &gone)]));
    assert_eq!(registry.entries(), [entry("here", &here)]);
    // Nothing more to prune: nothing removed
    assert_eq!(registry.prune().unwrap()["removed"], json!([]));
}

#[test]
fn a_workspace_is_made_once_and_only_with_a_valid_id() {
    let dir = crate::test_dir::TestDir::new();
    let work = FileRegistry::workspace(dir.path(), "work");
    assert!(work.create().unwrap());
    assert!(!work.create().unwrap(), "already there");
    assert_eq!(read_json(&work.file()), Some(empty_registry()));
    for id in [".hidden", "a b", ""] {
        let error = FileRegistry::workspace(dir.path(), id)
            .create()
            .unwrap_err();
        assert!(matches!(error, Error::Invalid(_)), "{id:?}: {error:?}");
    }
}

#[test]
fn a_workspace_is_deleted_unless_it_is_default_or_unknown() {
    let dir = crate::test_dir::TestDir::new();
    let work = FileRegistry::workspace(dir.path(), "work");
    work.create().unwrap();
    work.delete().unwrap();
    assert!(!work.exists());
    assert!(matches!(work.delete(), Err(Error::NotFound(_))));
    assert!(matches!(
        FileRegistry::workspace(dir.path(), "../x").delete(),
        Err(Error::NotFound(_))
    ));
    assert!(matches!(
        FileRegistry::at(dir.path()).delete(),
        Err(Error::Refused(_))
    ));
}

#[test]
fn a_workspace_is_renamed_to_a_free_valid_id_and_default_never_takes_part() {
    let dir = crate::test_dir::TestDir::new();
    let home = dir.path();
    let work = FileRegistry::workspace(home, "work");
    work.create().unwrap();
    FileRegistry::workspace(home, "taken").create().unwrap();
    assert!(matches!(work.rename("default"), Err(Error::Refused(_))));
    assert!(matches!(
        FileRegistry::at(home).rename("other"),
        Err(Error::Refused(_))
    ));
    assert!(matches!(work.rename(".x"), Err(Error::Invalid(_))));
    assert!(matches!(work.rename("taken"), Err(Error::Invalid(_))));
    assert!(matches!(
        FileRegistry::workspace(home, "unknown").rename("other"),
        Err(Error::NotFound(_))
    ));
    work.rename("other").unwrap();
    assert!(!work.exists());
    assert!(FileRegistry::workspace(home, "other").exists());
}

#[test]
fn the_workspace_ids_are_default_first_then_the_valid_ones_sorted() {
    let dir = crate::test_dir::TestDir::new();
    let home = dir.path();
    assert_eq!(workspace_ids(home), ["default"]);
    let folder = home.join(WORKSPACES_DIR);
    std::fs::create_dir_all(folder.join("folder.json")).unwrap();
    for name in [
        "zeta.json",
        "alpha.json",
        "default.json",
        ".hidden.json",
        "notes.txt",
        "a b.json",
    ] {
        std::fs::write(folder.join(name), "{}").unwrap();
    }
    assert_eq!(workspace_ids(home), ["default", "alpha", "zeta"]);
}

#[test]
fn what_was_opened_last_is_written_and_read_back_and_is_empty_without_a_readable_file() {
    let dir = crate::test_dir::TestDir::new();
    let home = dir.path().join("state");
    assert_eq!(read_recent(&home), Map::new());
    let recent = meta(json!({"workspace": "work", "projects": {"work": "one"}}));
    write_recent(&home, &recent).unwrap();
    assert_eq!(read_recent(&home), recent);
    for text in ["[1]", "{not json"] {
        std::fs::write(home.join(RECENT_FILE), text).unwrap();
        assert_eq!(read_recent(&home), Map::new(), "{text}");
    }
}

/// T-555: a workspace's subscription tells when its list changes, and when a workspace is made; nothing once dropped
#[test]
fn a_registry_subscription_tells_when_the_list_or_the_workspaces_change() {
    use crate::subscription::{quiet, told};
    use std::time::Duration;
    let dir = scratch("subscribe");
    let home = dir.join("state");
    let registry = FileRegistry::at(&home);
    write_registry(&registry.file(), &empty_registry()).unwrap();
    let (on_change, received) = told();
    let subscription = registry.subscribe(on_change).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    registry
        .add_project(Some(&a_project(&dir, "one")), &dir)
        .unwrap();
    received
        .recv_timeout(Duration::from_secs(10))
        .expect("told of the list's change");
    let _ = quiet(&received);
    assert!(FileRegistry::workspace(&home, "work").create().unwrap());
    received
        .recv_timeout(Duration::from_secs(10))
        .expect("told of a workspace made");
    drop(subscription);
    std::thread::sleep(Duration::from_millis(100));
    registry.remove_project("one", &dir).unwrap();
    assert!(quiet(&received), "nothing once dropped");
    std::fs::remove_dir_all(&dir).unwrap();
}
