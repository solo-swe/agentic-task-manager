use atm_db::MemoryStore;
use serde_json::{Value, json};

use super::*;

fn project(tasks: &Value, meta: &Value) -> Project {
    let list: Vec<Task> = tasks
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t.as_object().unwrap().clone())
        .collect();
    Project::new(Box::new(
        MemoryStore::new(list, Vec::new(), meta.as_object().unwrap()).unwrap(),
    ))
}

#[test]
fn counts_are_by_kind_and_skipped_tasks_count_as_neither() {
    let p = project(
        &json!([
            {"id": "T-1", "status": "done", "completed_at": "2026-01-01T00:00:00Z"},
            {"id": "T-2", "status": "skipped"},
            {"id": "T-3", "status": "todo", "name": "next one"},
            {"id": "T-4", "status": "blocked", "waiting_on": "user", "questions": ["q"]}
        ]),
        &json!({}),
    );
    let s = project_summary(&p).unwrap();
    assert_eq!(
        (s["total"].clone(), s["done"].clone(), s["skipped"].clone()),
        (json!(4), json!(1), json!(1))
    );
    assert_eq!(s["percent"], json!(33));
    assert_eq!(s["next"], json!([{"id": "T-3", "name": "next one"}]));
    assert_eq!(s["needs_you"]["blocked_on_user"], json!(1));
    assert_eq!(s["board"]["blocked"]["cards"][0]["questions"], json!(1));
    assert_eq!(s["recent_sprints"], Value::Null);
    assert_eq!(s["stats"]["tasks"], json!(4));
}

#[test]
fn names_and_descriptions_are_capped() {
    assert_eq!(capped(Some(&json!("x".repeat(100)))).chars().count(), 80);
    assert_eq!(capped(Some(&json!(3))), "");
    assert_eq!(
        description(Some(&json!({"description": "  hi  "}))),
        json!("hi")
    );
    assert_eq!(description(Some(&json!({}))), Value::Null);
    assert!(
        web_address("HTTPS://x/pull/1")
            && !web_address("javascript:x")
            && !web_address("https://a b")
    );
    assert_eq!(percent(1, 8), 12, "12.5 rounds to even");
}
