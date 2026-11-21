use atm_common::Task;
use atm_common::time::{civil, format_iso, local_now, parse_iso};
use atm_db::MemoryStore;
use serde_json::{Value, json};

use super::sprint_views;
use crate::project::Project;

fn tasks(v: Value) -> Vec<Task> {
    let Value::Array(items) = v else {
        panic!("a list of tasks")
    };
    items
        .into_iter()
        .map(|t| match t {
            Value::Object(task) => task,
            other => panic!("not a task: {other}"),
        })
        .collect()
}

fn views(working: Value, archive: Value, meta: &Value) -> Value {
    let store =
        MemoryStore::new(tasks(working), tasks(archive), meta.as_object().unwrap()).unwrap();
    sprint_views(&Project::new(Box::new(store))).unwrap()
}

/// The local day `by` days from today, as `YYYY-MM-DD`
fn day(by: i64) -> String {
    let (year, month, day, _, _) = civil(local_now() + by * 86_400);
    format!("{year:04}-{month:02}-{day:02}")
}

fn t(id: &str, status: &str, sprint: Option<&str>, history: Value) -> Value {
    let mut task = json!({"id": id, "name": id, "type": "feature", "status": status});
    task["sprint_history"] = history;
    if let Some(sprint) = sprint {
        task["sprint"] = json!(sprint);
    }
    task
}

#[test]
fn each_sprint_in_order_with_its_figures_by_one_rule() {
    let start = "2026-04-01T08:00:00Z";
    let meta = json!({"sprints": [
        {"id": "Old", "state": "closed", "start": "2026-03-01T08:00:00Z", "end": start, "due": day(-3),
         "report": {"done": ["T-9"], "carried": ["T-2"]}},
        {"id": "Now", "state": "active", "start": start, "due": day(0), "goal": "Ship"},
        {"id": "Next", "state": "planned", "due": day(-1)},
        {"id": "Empty", "state": "planned"}
    ]});
    let at = |s: &str, from: &str| json!({"sprint": s, "from": from});
    // Pulled in mid-sprint, waiting for the sprint review
    let mut reviewed = t(
        "T-4",
        "done",
        Some("Now"),
        json!([at("Now", "2026-04-02T00:00:00Z")]),
    );
    reviewed["sprint_review"] = json!("pending");
    let working = Value::Array(vec![
        // In Now from its start
        t("T-1", "done", Some("Now"), json!([at("Now", start)])),
        // Carried from Old into Now
        t(
            "T-2",
            "todo",
            Some("now"),
            json!([at("Old", "2026-03-02T00:00:00Z"), at("Now", start)]),
        ),
        // Pulled in mid-sprint, and skipped: not in the total
        t(
            "T-3",
            "skipped",
            Some("Now"),
            json!([at("Now", "2026-04-03T00:00:00Z")]),
        ),
        reviewed,
        // The backlog: one open, one finished (not counted)
        t("T-5", "todo", None, json!([])),
        t("T-6", "done", None, json!([])),
    ]);
    // An archived task still counts in its sprint
    let archive = json!([t(
        "T-9",
        "done",
        Some("Old"),
        json!([at("Old", "2026-03-01T08:00:00Z")])
    )]);
    let v = views(working, archive, &meta);
    let figures: Vec<Value> = v["sprints"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            json!([
                s["id"],
                s["done"],
                s["total"],
                s["percent"],
                s["added"],
                s["carried"],
                s["review_pending"],
                s.get("days_left")
            ])
        })
        .collect();
    assert_eq!(
        figures,
        [
            // A closed sprint has no days left, though its due date passed
            json!(["Old", 1, 1, 100, 0, 0, 0, null]),
            // T-1 and T-4 done of three (T-3 skipped); T-3 and T-4 added; T-2 carried; due today
            json!(["Now", 2, 3, 67, 2, 1, 1, 0]),
            // Due yesterday: overdue by a day
            json!(["Next", 0, 0, 100, 0, 0, 0, -1]),
            // An empty sprint: 100 %, as the summary says it; no due date, no days
            json!(["Empty", 0, 0, 100, 0, 0, 0, null])
        ]
    );
    // Its stored fields come along
    assert_eq!(v["sprints"][1]["goal"], json!("Ship"));
    assert_eq!(v["sprints"][0]["report"]["done"], json!(["T-9"]));
    // The backlog as `atm sprint list` counts it: unfinished only
    assert_eq!(v["backlog"], json!(1));
}

#[test]
fn an_open_sprints_due_date_ends_at_its_last_local_second() {
    let v = views(
        json!([]),
        json!([]),
        &json!({"sprints": [{"id": "A", "state": "active", "due": day(2)}]}),
    );
    let end = parse_iso(v["sprints"][0]["due_end"].as_str().unwrap()).unwrap();
    // A second later it is the next local day
    let next = format_iso(end + 1 + (local_now() - atm_common::time::now()));
    assert!(next.starts_with(&day(3)), "{next}");
    assert_eq!(v["sprints"][0]["days_left"], json!(2));
}

#[test]
fn an_unmigrated_projects_phases_are_the_sprints_the_migration_would_make() {
    let phased = |id: &str, status: &str, phase: i64| json!({"id": id, "name": id, "type": "feature", "status": status, "phase": phase});
    let working = json!([phased("T-1", "done", 0), phased("T-2", "todo", 1), {"id": "T-3", "name": "c", "status": "todo"}]);
    let meta = json!({"phases": [{"id": 1, "name": "Core"}]});
    let v = views(working.clone(), json!([]), &meta);
    let sprints: Vec<Value> = v["sprints"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| json!([s["id"], s["state"], s["done"], s["total"]]))
        .collect();
    // Phase 0's tasks all finished: closed; Core (phase 1, named) planned
    assert_eq!(
        sprints,
        [
            json!(["Phase 0", "closed", 1, 1]),
            json!(["Core", "planned", 0, 1])
        ]
    );
    assert_eq!(v["backlog"], json!(1));
    // Each task's sprint, as the views count them; the backlog's isn't listed
    let store = MemoryStore::new(tasks(working), Vec::new(), meta.as_object().unwrap()).unwrap();
    let by_task = super::task_sprints(&Project::new(Box::new(store))).unwrap();
    let sprint = |id: &str| by_task.get(id).map(|p| p.sprint.as_str());
    assert_eq!(sprint("T-1"), Some("Phase 0"));
    assert_eq!(sprint("T-2"), Some("Core"));
    assert_eq!(sprint("T-3"), None);
    assert_eq!(v["from_phases"], json!(true));
}

#[test]
fn a_tasks_sprint_is_its_sprints_name_as_the_project_writes_it_with_how_often_it_was_carried() {
    let at = |s: &str, from: &str| json!({"sprint": s, "from": from});
    let history = json!([
        at("A", "2026-01-01T00:00:00Z"),
        at("B", "2026-02-01T00:00:00Z"),
        at("A", "2026-02-15T00:00:00Z"),
        at("Now", "2026-03-01T00:00:00Z")
    ]);
    let working = json!([
        t("T-1", "todo", Some("now"), history),
        t("T-2", "todo", Some("Gone"), json!([]))
    ]);
    let meta = json!({"sprints": [{"id": "A"}, {"id": "B"}, {"id": "Now"}]});
    let store = MemoryStore::new(tasks(working), Vec::new(), meta.as_object().unwrap()).unwrap();
    let by_task = super::task_sprints(&Project::new(Box::new(store))).unwrap();
    // Carried through A and B (A twice counts once)
    assert_eq!(
        by_task.get("T-1"),
        Some(&super::TaskSprint {
            sprint: "Now".into(),
            carried: 2
        })
    );
    // A sprint the project doesn't define: in none
    assert_eq!(by_task.get("T-2"), None);
}

#[test]
fn a_project_without_sprints_has_none_and_everything_open_in_the_backlog() {
    let v = views(
        json!([t("T-1", "todo", None, json!([]))]),
        json!([]),
        &json!({}),
    );
    assert_eq!(
        v,
        json!({"sprints": [], "backlog": 1, "from_phases": false})
    );
}
