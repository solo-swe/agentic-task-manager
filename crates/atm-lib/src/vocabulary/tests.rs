use super::served;
use atm_common::vocabulary::Vocabulary;
use serde_json::{Map, Value, json};

fn serve(meta: Value) -> Value {
    let Value::Object(meta) = meta else {
        panic!("a meta object")
    };
    let meta: Map<String, Value> = meta;
    served(&Vocabulary::of_meta(&meta).unwrap(), &meta)
}

#[test]
fn without_a_vocabulary_the_defaults_are_served() {
    let v = serve(json!({}));
    let kinds: Vec<&str> = v["statuses"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["kind"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds,
        ["todo", "active", "review", "blocked", "done", "skipped"]
    );
    assert_eq!(
        v["statuses"][0],
        json!({"id": "todo", "label": "To Do", "kind": "todo"})
    );
    assert_eq!(v["priorities"][0]["id"], json!("P0"));
    assert_eq!(v["default_priority"], json!("P2"));
    assert_eq!(v["types"][0]["id"], json!("feature"));
    assert_eq!(v["labels"], json!([]));
    assert_eq!(v["components"], json!([]));
    assert_eq!(v["workflow"], Value::Null);
    assert!(v["review"].is_string());
    assert_eq!(v["phases"], json!([]));
}

#[test]
fn named_phases_are_served_others_left_out() {
    let v = serve(json!({"phases": [
        {"id": 1, "name": "Foundations"}, {"id": 2}, {"id": "x", "name": "not a number"}, {"id": 3, "name": ""}
    ]}));
    assert_eq!(v["phases"], json!([{"id": 1, "name": "Foundations"}]));
}

#[test]
fn a_projects_own_lists_are_served_as_atm_resolves_them() {
    let v = serve(json!({
        "statuses": [
            {"id": "open", "label": "Open", "kind": "todo"},
            {"id": "doing", "kind": "active"},
            {"id": "shipped", "label": "Shipped", "kind": "done"}
        ],
        "priorities": [{"id": "must"}, {"id": "should"}, {"id": "could"}],
        "default_priority": "could"
    }));
    assert_eq!(v["statuses"][1]["id"], json!("doing"));
    assert_eq!(v["statuses"][1]["kind"], json!("active"));
    assert_eq!(
        v["statuses"][2],
        json!({"id": "shipped", "label": "Shipped", "kind": "done"})
    );
    let priorities: Vec<&str> = v["priorities"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_str().unwrap())
        .collect();
    assert_eq!(priorities, ["must", "should", "could"]);
    assert_eq!(v["default_priority"], json!("could"));
    // Without a default: the middle one (no P2 here)
    let middle = serve(json!({"priorities": [{"id": "a"}, {"id": "b"}, {"id": "c"}]}));
    assert_eq!(middle["default_priority"], json!("b"));
}

#[test]
fn labels_are_ids_or_entries_with_their_text_in_meta_order() {
    let v = serve(json!({"labels": [
        "ux",
        {"id": "api", "label": "API"},
        {"id": "docs"},
        {"label": "no id"},
        ""
    ]}));
    assert_eq!(
        v["labels"],
        json!([
            {"id": "ux", "label": "ux"},
            {"id": "api", "label": "API"},
            {"id": "docs", "label": "docs"}
        ])
    );
}

#[test]
fn components_keep_their_order_paths_and_review_with_a_label() {
    let v = serve(json!({"components": [
        {"id": "dash", "label": "Dashboard", "paths": ["dashboard/**"], "review": "async"},
        {"id": "cli", "paths": ["crates/**"]}
    ]}));
    assert_eq!(
        v["components"],
        json!([
            {"id": "dash", "label": "Dashboard", "paths": ["dashboard/**"], "review": "async"},
            {"id": "cli", "label": "cli", "paths": ["crates/**"]}
        ])
    );
}

#[test]
fn the_workflow_has_its_defaults_filled_in() {
    let v = serve(json!({"workflow": {"type": "pull-request", "base": "main"}}));
    assert_eq!(v["workflow"]["type"], json!("pull-request"));
    assert_eq!(v["workflow"]["base"], json!("main"));
    assert_eq!(v["workflow"]["max_rounds"], json!(3));
    assert_eq!(v["workflow"]["verify"], json!(true));
}

#[test]
fn a_broken_list_is_served_as_its_default_by_a_tolerant_read() {
    // The tolerant read leaves out what doesn't work (and reports it); what is served is the rest
    let meta = json!({"statuses": [{"id": "x", "kind": "nonsense"}], "types": [{"id": "chore"}]});
    let (vocab, problems, usable) = Vocabulary::tolerant(Some(&meta));
    assert_eq!(problems.len(), 1, "{problems:?}");
    let v = served(&vocab, &usable);
    assert_eq!(v["statuses"][0]["id"], json!("todo"));
    assert_eq!(v["types"], json!([{"id": "chore", "label": "Chore"}]));
}
