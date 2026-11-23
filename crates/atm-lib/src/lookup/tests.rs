use atm_db::MemoryStore;
use serde_json::{Map, Value, json};

use super::*;
use crate::project::Project;

fn task(v: &Value) -> atm_common::Task {
    v.as_object().unwrap().clone()
}

fn project(working: &[Value], archived: &[Value]) -> Project {
    let store = MemoryStore::new(
        working.iter().map(task).collect(),
        archived.iter().map(task).collect(),
        &Map::new(),
    )
    .unwrap();
    Project::new(Box::new(store))
}

#[test]
fn a_task_is_found_by_its_place_and_a_missing_one_says_whether_it_is_archived() {
    let mut p = project(
        &[json!({"id": "T-1"}), json!({"id": "T-2"})],
        &[json!({"id": "T-0"})],
    );
    let found = |p: &mut Project, id: &str| p.change(|work| task_at(work, id));
    assert_eq!(found(&mut p, "T-2").unwrap(), 1);
    assert_eq!(found(&mut p, "T-1").unwrap(), 0);
    for (id, message) in [
        (
            "T-0",
            "no task T-0 (it is archived: atm db unarchive brings it back)",
        ),
        ("T-9", "no task T-9"),
    ] {
        let err = found(&mut p, id).unwrap_err();
        assert!(matches!(err, Error::NotFound(_)), "{id}");
        assert_eq!(err.to_string(), message, "{id}");
    }
    // No working tasks at all: still not found
    let mut empty = project(&[], &[]);
    assert_eq!(
        found(&mut empty, "T-1").unwrap_err().to_string(),
        "no task T-1"
    );
}
