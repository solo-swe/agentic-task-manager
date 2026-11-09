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
