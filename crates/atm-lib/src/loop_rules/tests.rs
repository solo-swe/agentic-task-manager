use serde_json::{Value, json};

use super::*;

fn task(value: Value) -> Task {
    match value {
        Value::Object(map) => map,
        other => panic!("not a task: {other}"),
    }
}

struct Fixture {
    document: Value,
    tasks: Vec<Task>,
}

impl Fixture {
    fn new(meta: Value, tasks: Value) -> Self {
        let list: Vec<Task> = match tasks.clone() {
            Value::Array(items) => items.into_iter().map(task).collect(),
            other => panic!("not a list: {other}"),
        };
        let mut document = Map::new();
        document.insert("meta".into(), meta);
        document.insert("tasks".into(), tasks);
        Self {
            document: Value::Object(document),
            tasks: list,
        }
    }

    fn next(
        &self,
        sel: &Selection,
        store: Value,
        with_user: bool,
    ) -> (Option<String>, Option<String>) {
        let vocab = Vocabulary::of_document(&self.document).unwrap();
        let tasks: Vec<&Task> = self.tasks.iter().collect();
        let store = match store {
            Value::Object(map) => map,
            other => panic!("not a store: {other}"),
        };
        let archived = |_: &str| Ok(false);
        let (t, reason) = next_task(
            &vocab,
            &self.document,
            &tasks,
            &store,
            sel,
            None,
            with_user,
            &archived,
        )
        .unwrap();
        (t.and_then(|t| t["id"].as_str().map(String::from)), reason)
    }
}

fn empty_store() -> Value {
    json!({"claims": {}, "completed": {}, "waiting": {}, "resolved": {}, "hints": {}})
}

#[test]
fn the_next_task_is_by_priority_then_natural_id() {
    let f = Fixture::new(
        json!({}),
        json!([
            {"id": "T-10", "status": "todo"},
            {"id": "T-2", "status": "todo"},
            {"id": "T-3", "status": "todo", "priority": "P0"}
        ]),
    );
    assert_eq!(
        f.next(&Selection::default(), empty_store(), false)
            .0
            .as_deref(),
        Some("T-3")
    );
    let f = Fixture::new(
        json!({}),
        json!([{"id": "T-10", "status": "todo"}, {"id": "T-2", "status": "todo"}]),
    );
    assert_eq!(
        f.next(&Selection::default(), empty_store(), false)
            .0
            .as_deref(),
        Some("T-2")
    );
}

#[test]
fn work_back_from_review_comes_first_and_claimed_work_is_skipped() {
    let f = Fixture::new(
        json!({}),
        json!([
            {"id": "T-1", "status": "todo", "priority": "P0"},
            {"id": "T-2", "status": "in_review", "review_state": "changes_requested"}
        ]),
    );
    assert_eq!(
        f.next(&Selection::default(), empty_store(), false)
            .0
            .as_deref(),
        Some("T-2")
    );
    let store = json!({"claims": {"T-2": {}}, "completed": {}, "waiting": {}});
    assert_eq!(
        f.next(&Selection::default(), store, false).0.as_deref(),
        Some("T-1")
    );
}

#[test]
fn the_active_sprint_is_the_default_and_reasons_say_why_nothing_is_ready() {
    let meta = json!({"sprints": [{"id": "Alpha", "state": "active"}, {"id": "Beta"}]});
    let f = Fixture::new(
        meta.clone(),
        json!([
            {"id": "T-1", "status": "done", "sprint": "Alpha"},
            {"id": "T-2", "status": "todo", "sprint": "Beta"}
        ]),
    );
    assert_eq!(
        f.next(&Selection::default(), empty_store(), false),
        (None, Some("the sprint “Alpha” is finished: 1 done".into()))
    );
    let sel = Selection {
        sprints: vec!["beta".into()],
        ..Selection::default()
    };
    assert_eq!(f.next(&sel, empty_store(), false).0.as_deref(), Some("T-2"));
    let all = Selection {
        all: true,
        ..Selection::default()
    };
    assert_eq!(f.next(&all, empty_store(), false).0.as_deref(), Some("T-2"));

    let waiting = Fixture::new(
        meta,
        json!([{"id": "T-1", "status": "in_progress", "sprint": "Alpha"}]),
    );
    assert_eq!(
        waiting
            .next(&Selection::default(), empty_store(), false)
            .1
            .as_deref(),
        Some("nothing ready in the sprint “Alpha”: the rest is waiting (1 in progress)")
    );
    let none_active = Fixture::new(json!({"sprints": [{"id": "Beta"}]}), json!([]));
    assert_eq!(
        none_active
            .next(&Selection::default(), empty_store(), false)
            .1
            .as_deref(),
        Some("no active sprint: plan one with /atm-plan-sprint, or run with --all")
    );
}

#[test]
fn work_together_waits_for_the_user_and_dependencies_are_named() {
    let f = Fixture::new(
        json!({}),
        json!([{"id": "T-1", "status": "todo", "with": "user"}]),
    );
    assert_eq!(
        f.next(&Selection::default(), empty_store(), false)
            .1
            .as_deref(),
        Some(
            "only work done together with the user is left in the tasks: T-1 (--with-user takes it)"
        )
    );
    assert_eq!(
        f.next(&Selection::default(), empty_store(), true)
            .0
            .as_deref(),
        Some("T-1")
    );
    let deps = Fixture::new(
        json!({}),
        json!([{"id": "T-1", "status": "todo", "dependencies": ["T-9"]}]),
    );
    assert_eq!(
        deps.next(&Selection::default(), empty_store(), false)
            .1
            .as_deref(),
        Some("1 todo task(s) waiting on dependencies")
    );
    let empty = Fixture::new(json!({}), json!([]));
    assert_eq!(
        empty
            .next(&Selection::default(), empty_store(), false)
            .1
            .as_deref(),
        Some("nothing left to do")
    );
}

#[test]
fn listed_tasks_go_in_their_order_and_unknown_ones_are_refused() {
    let f = Fixture::new(
        json!({}),
        json!([{"id": "T-1", "status": "todo"}, {"id": "T-2", "status": "todo"}]),
    );
    let sel = Selection {
        tasks: vec!["T-2".into(), "T-1".into()],
        ..Selection::default()
    };
    assert_eq!(f.next(&sel, empty_store(), false).0.as_deref(), Some("T-2"));
    let vocab = Vocabulary::of_document(&f.document).unwrap();
    let tasks: Vec<&Task> = f.tasks.iter().collect();
    let bad = Selection {
        tasks: vec!["T-9".into()],
        ..Selection::default()
    };
    let store = Map::new();
    let error = next_task(
        &vocab,
        &f.document,
        &tasks,
        &store,
        &bad,
        None,
        false,
        &|_| Ok(false),
    )
    .unwrap_err();
    assert_eq!(
        (error.to_string(), error.code()),
        ("--tasks: no task T-9".into(), 2)
    );
}

#[test]
fn the_lease_is_the_projects_when_it_is_a_positive_number() {
    assert!((lease_minutes(&json!({})) - 120.0).abs() < f64::EPSILON);
    assert!((lease_minutes(&json!({"meta": {"lease_minutes": 30}})) - 30.0).abs() < f64::EPSILON);
    assert!((lease_minutes(&json!({"meta": {"lease_minutes": "15"}})) - 15.0).abs() < f64::EPSILON);
    assert!((lease_minutes(&json!({"meta": {"lease_minutes": -1}})) - 120.0).abs() < f64::EPSILON);
}
