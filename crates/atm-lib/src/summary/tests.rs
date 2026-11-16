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
fn a_keys_format_project_gives_its_tasks_keys_and_a_legacy_one_does_not() {
    const SETUP: &str = "0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8001";
    const LOGIN: &str = "0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8002";
    const DRAFT: &str = "0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8003";
    let keyed = project(
        &json!([
            {"id": SETUP, "number": 2, "key_final": true, "status": "done", "completed_at": "2026-01-01T00:00:00Z"},
            {"id": LOGIN, "number": 10, "key_final": true, "status": "todo"},
            {"id": DRAFT, "number": 11, "key_final": false, "status": "todo", "dependencies": [LOGIN]},
            {"id": "0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8004", "number": 12, "status": "in_progress"},
            {"id": "0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8005", "number": 13, "key_final": true, "status": "in_review"}
        ]),
        &json!({"format": 2, "key": "ATM", "next_number": 14}),
    );
    let s = project_summary(&keyed).unwrap();
    let key_of = |v: &Value| (v["key"].clone(), v["key_final"].clone());
    assert_eq!(key_of(&s["last_done"]), (json!("ATM-2"), json!(true)));
    assert_eq!(key_of(&s["next"][0]), (json!("ATM-10"), json!(true)));
    assert_eq!(
        key_of(&s["needs_you_tasks"][0]),
        (json!("ATM-13"), json!(true))
    );
    let todo = &s["board"]["todo"]["cards"];
    let draft = todo
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == DRAFT)
        .unwrap();
    assert_eq!(
        key_of(draft),
        (json!("ATM-11"), json!(false)),
        "provisional"
    );
    assert_eq!(draft["waiting_for"], json!([LOGIN]));
    assert_eq!(draft["waiting_for_labels"], json!(["ATM-10"]));
    assert_eq!(
        key_of(&s["board"]["active"]["cards"][0]),
        (json!("ATM-12"), json!(false)),
        "no key_final: provisional"
    );

    let legacy = project(
        &json!([
            {"id": "T-1", "status": "done", "completed_at": "2026-01-01T00:00:00Z"},
            {"id": "T-2", "status": "todo", "dependencies": ["T-3"]},
            {"id": "T-3", "status": "todo"}
        ]),
        &json!({}),
    );
    let s = project_summary(&legacy).unwrap();
    for entry in [
        &s["last_done"],
        &s["next"][0],
        &s["board"]["todo"]["cards"][1],
    ] {
        assert!(
            entry.get("key").is_none() && entry.get("key_final").is_none(),
            "{entry}"
        );
    }
    assert!(
        s["board"]["todo"]["cards"][1]
            .get("waiting_for_labels")
            .is_none()
    );
    assert_eq!(
        s["board"]["todo"]["cards"][1]["waiting_for"],
        json!(["T-3"])
    );
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
