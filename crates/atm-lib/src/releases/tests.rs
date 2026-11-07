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
}

#[test]
fn a_release_takes_the_tasks_of_its_pull_requests() {
    let mut p = project(
        json!([
            {"id": "T-1", "status": "done", "type": "feature", "pr": {"number": 7, "url": "https://x/7"}},
            {"id": "T-2", "status": "todo", "type": "bugfix"}
        ]),
        json!({}),
    );
    assert!(add_release(&mut p, "1 0", None, None, &[], None).is_err());
    assert!(add_release(&mut p, "1.0", None, None, &[9], None).is_err());
    let added = add_release(&mut p, "1.0", Some("One"), None, &[7], None).unwrap();
    assert_eq!(added.tasks, ["T-1"]);
    assert_eq!(
        added.release["pr"],
        json!({"number": 7, "url": "https://x/7"})
    );
    assert!(add_release(&mut p, "1.0", None, None, &[], None).is_err());
    let (listed, any) = list_releases(&p, false).unwrap();
    assert!(any);
    assert_eq!(listed[0]["by_stage"], json!({"in_pr": 1}));
    let (edited, changed) = edit_release(&mut p, "1.0", Some(""), Some("2026-12-01")).unwrap();
    assert_eq!(changed, ["name", "target"]);
    assert!(edited.get("name").is_none());
    let notes = release_notes(&p, "1.0").unwrap();
    assert_eq!(notes["groups"][0]["type"], json!("feature"));
    archive_release(&mut p, "1.0").unwrap();
    assert!(archive_release(&mut p, "1.0").is_err());
    assert_eq!(list_releases(&p, false).unwrap().0.len(), 0);
    assert_eq!(
        release_notes(&p, "2.0").unwrap_err().to_string(),
        "no release '2.0'; the releases: 1.0"
    );
}
