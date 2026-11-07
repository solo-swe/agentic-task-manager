use atm_db::MemoryStore;
use serde_json::{Value, json};

use super::*;
use atm_common::Task;

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
}

#[test]
fn a_label_becomes_a_component_and_globs_assign_it() {
    let mut p = project(
        json!([
            {"id": "T-1", "status": "todo", "labels": ["ui", "x"]},
            {"id": "T-2", "status": "todo", "files": ["dashboard/a.ts"]},
            {"id": "T-3", "status": "todo", "files": ["other.rs"]}
        ]),
        json!({"labels": [{"id": "ui", "label": "Dashboard"}]}),
    );
    assert!(add_component(&mut p, "ui", None, &[], None, None, true).is_err());
    assert!(add_component(&mut p, "ui", None, &[], None, Some("typo"), false).is_err());
    let (component, moved) = add_component(
        &mut p,
        "ui",
        None,
        &["dashboard/**".into()],
        Some("async"),
        Some("ui"),
        true,
    )
    .unwrap();
    assert_eq!(
        component,
        json!({"id": "ui", "label": "Dashboard", "paths": ["dashboard/**"], "review": "async"})
    );
    assert_eq!(moved.from_label, ["T-1"]);
    assert_eq!(moved.by_files, ["T-2"]);
    let doc = p.store().document().unwrap();
    assert_eq!(doc["tasks"][0]["labels"], json!(["x"]));
    assert!(doc["meta"].get("labels").is_none());
    assert!(add_component(&mut p, "ui", None, &[], None, None, false).is_err());

    let edited =
        edit_component(&mut p, "ui", Some(""), &["*.rs".into()], None, None, true).unwrap();
    assert_eq!(edited.changed, ["label", "paths"]);
    assert_eq!(edited.moved.by_files, ["T-3"]);
    let listed = list_components(&p).unwrap();
    assert_eq!(listed[0]["tasks"], json!(3));
    assert_eq!(
        edit_component(&mut p, "nope", None, &[], None, None, false)
            .unwrap_err()
            .to_string(),
        "no component 'nope'; the components: ui"
    );
}
