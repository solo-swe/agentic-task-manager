use atm_db::MemoryStore;
use serde_json::{Map, Value, json};

use super::*;

fn task(value: Value) -> Task {
    match value {
        Value::Object(map) => map,
        other => panic!("not a task: {other}"),
    }
}

fn project(tasks: Value, archive: Value) -> Project {
    let list = |v: Value| {
        v.as_array()
            .unwrap()
            .iter()
            .map(|t| task(t.clone()))
            .collect()
    };
    Project::new(Box::new(
        MemoryStore::new(list(tasks), list(archive), &Map::new()).unwrap(),
    ))
}

fn ids(list: &[&str]) -> Vec<TaskId> {
    list.iter().map(|id| TaskId::new(*id)).collect()
}

#[test]
fn tasks_are_shown_in_the_order_asked_for_archived_ones_too() {
    let p = project(
        json!([{"id": "T-2"}, {"id": "T-3"}]),
        json!([{"id": "T-1", "status": "done"}]),
    );
    let views = show_tasks(&p, &ids(&["T-3", "T-1", "T-2"])).unwrap();
    assert_eq!(
        views
            .iter()
            .map(|v| v.task["id"].clone())
            .collect::<Vec<_>>(),
        [json!("T-3"), json!("T-1"), json!("T-2")]
    );
    assert_eq!(
        views.iter().map(|v| v.archived).collect::<Vec<_>>(),
        [false, true, false]
    );
}

#[test]
fn an_unknown_task_is_not_found() {
    let p = project(json!([]), json!([]));
    assert_eq!(
        show_tasks(&p, &ids(&["T-9"])).unwrap_err(),
        Error::NotFound("no task T-9".into())
    );
}

#[test]
fn dependencies_have_their_status_where_they_are() {
    let p = project(
        json!([{"id": "T-3", "dependencies": ["T-2", "T-1", "T-9"]}, {"id": "T-2", "status": "todo"}]),
        json!([{"id": "T-1", "status": "done"}]),
    );
    let view = &show_tasks(&p, &ids(&["T-3"])).unwrap()[0];
    assert_eq!(
        view.dependencies,
        [
            Dependency {
                id: "T-2".into(),
                status: Some(json!("todo")),
                archived: false
            },
            Dependency {
                id: "T-1".into(),
                status: Some(json!("done")),
                archived: true
            },
            Dependency {
                id: "T-9".into(),
                status: None,
                archived: false
            },
        ]
    );
}

#[test]
fn related_tasks_on_both_sides_and_duplicates() {
    let p = project(
        json!([
            {"id": "T-1", "related": ["T-3"]},
            {"id": "T-2", "duplicate_of": "T-1"},
            {"id": "T-3", "related": ["T-4", "T-3"]},
            {"id": "T-4"}
        ]),
        json!([]),
    );
    let views = show_tasks(&p, &ids(&["T-1", "T-3"])).unwrap();
    assert_eq!(views[0].related, ["T-3"]);
    assert_eq!(views[0].duplicated_by, ["T-2"]);
    // Its own first, then those that name it; never itself twice
    assert_eq!(views[1].related, ["T-4", "T-3", "T-1"]);
}

#[test]
fn the_priority_and_who_started_it() {
    let p = project(
        json!([{"id": "T-1", "priority": "P0", "agent": "old"}, {"id": "T-2", "priority": "x"}]),
        json!([]),
    );
    let views = show_tasks(&p, &ids(&["T-1", "T-2"])).unwrap();
    assert_eq!(views[0].priority, "P0");
    assert_eq!(views[0].started_by, Some(json!("old")));
    assert_eq!(views[1].priority, "P2");
    assert_eq!(views[1].started_by, None);
}
