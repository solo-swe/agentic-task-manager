use serde_json::{Value, json};

use super::*;

fn meta(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        other => panic!("not a meta object: {other}"),
    }
}

fn error(value: Value) -> String {
    Vocabulary::of_meta(&meta(value))
        .expect_err("refused")
        .to_string()
}

#[test]
fn an_empty_meta_has_the_defaults() {
    let vocab = Vocabulary::of_meta(&Map::new()).unwrap();
    assert_eq!(
        vocab
            .statuses
            .iter()
            .map(|s| s.id.as_str())
            .collect::<Vec<_>>(),
        [
            "todo",
            "in_progress",
            "in_review",
            "blocked",
            "done",
            "skipped"
        ]
    );
    assert_eq!(vocab.default_priority, "P2");
    assert_eq!(vocab.types.len(), 7);
    assert_eq!(vocab.review, "none");
    assert_eq!(vocab.sprint_names, "people");
    assert_eq!(vocab.sprints.len(), 0);
}

#[test]
fn a_document_without_a_meta_object_has_the_defaults() {
    assert_eq!(
        Vocabulary::of_document(&json!({"meta": 3})).unwrap(),
        Vocabulary::of_meta(&Map::new()).unwrap()
    );
}

#[test]
fn own_statuses_need_the_kinds_todo_active_and_done() {
    assert_eq!(
        error(json!({"statuses": [{"id": "a", "kind": "todo"}, {"id": "b", "kind": "done"}]})),
        "meta.statuses needs at least one status of the kind 'active'"
    );
    let vocab = Vocabulary::of_meta(&meta(json!({"statuses": [
        {"id": "new_one", "kind": "todo"}, {"id": "doing", "label": "Doing", "kind": "active"}, {"id": "end", "kind": "done"}
    ]})))
    .unwrap();
    assert_eq!(vocab.statuses[0].label, json!("New one"));
    assert_eq!(vocab.kind_of(Some(&json!("doing"))), Some(Kind::Active));
    // A default status the project doesn't list keeps its kind; anything else has none
    assert_eq!(vocab.kind_of(Some(&json!("blocked"))), Some(Kind::Blocked));
    assert_eq!(vocab.kind_of(Some(&json!("nope"))), None);
    assert_eq!(vocab.kind_of(None), None);
}

#[test]
fn terms_are_checked_entry_by_entry() {
    assert_eq!(
        error(json!({"types": []})),
        "meta.types must be a non-empty list"
    );
    assert_eq!(
        error(json!({"types": [{"label": "x"}]})),
        "meta.types: every entry needs an \"id\" (a non-empty string)"
    );
    assert_eq!(
        error(json!({"types": [{"id": "a"}, {"id": "a"}]})),
        "meta.types: \"a\" is listed twice"
    );
    assert_eq!(
        error(json!({"statuses": [{"id": "a", "kind": "x"}, {"label": "no id"}]})),
        "meta.statuses: \"a\" has the kind 'x'; use one of todo, active, review, blocked, done, skipped"
    );
    assert_eq!(
        error(json!({"statuses": [{"id": "a"}]})),
        "meta.statuses: \"a\" has the kind None; use one of todo, active, review, blocked, done, skipped"
    );
}

#[test]
fn the_default_priority_is_p2_or_the_middle_one() {
    let vocab = Vocabulary::of_meta(&meta(
        json!({"priorities": [{"id": "a"}, {"id": "b"}, {"id": "c"}, {"id": "d"}]}),
    ));
    assert_eq!(vocab.unwrap().default_priority, "b");
    assert_eq!(
        error(json!({"default_priority": "P9"})),
        "meta.default_priority 'P9' is not one of meta.priorities"
    );
    assert_eq!(
        error(json!({"default_priority": null})),
        "meta.default_priority None is not one of meta.priorities"
    );
}

#[test]
fn a_tasks_priority_is_its_own_or_the_default() {
    let vocab = Vocabulary::of_meta(&Map::new()).unwrap();
    let task = |v: Value| v.as_object().unwrap().clone();
    assert_eq!(vocab.priority_of(&task(json!({"priority": "P0"}))), "P0");
    assert_eq!(vocab.priority_of(&task(json!({"priority": "P9"}))), "P2");
    assert_eq!(vocab.priority_of(&task(json!({"priority": 1}))), "P2");
    assert_eq!(vocab.priority_of(&task(json!({}))), "P2");
}

#[test]
fn the_workflow_is_checked_when_set() {
    assert!(Vocabulary::of_meta(&meta(json!({"workflow": null}))).is_ok());
    assert_eq!(
        error(json!({"workflow": {"type": "x"}})),
        "meta.workflow.type must be one of direct, pull-request"
    );
    assert_eq!(
        error(json!({"workflow": {"type": "direct", "base": "a b"}})),
        "meta.workflow.base must be a branch name (letters, digits, `.`, `_`, `/`, `-`; not starting with `-`)"
    );
    assert_eq!(
        error(json!({"workflow": {"type": "direct", "max_rounds": 0}})),
        "meta.workflow.max_rounds must be a positive whole number"
    );
    assert_eq!(
        error(json!({"workflow": {"type": "direct", "max_rounds": 2.0}})),
        "meta.workflow.max_rounds must be a positive whole number"
    );
    assert_eq!(
        error(json!({"workflow": {"type": "direct", "trailers": {"require": [1]}}})),
        "meta.workflow.trailers must be \"forbid\" or {\"require\": [\"Co-Authored-By: ...\"]}"
    );
    assert!(
        Vocabulary::of_meta(&meta(json!({"workflow": {
            "type": "pull-request", "base": "main", "verify": false, "max_rounds": 3,
            "commit_template": "x", "trailers": "forbid"
        }})))
        .is_ok()
    );
}

#[test]
fn sprints_are_checked_and_planned_by_default() {
    let vocab = Vocabulary::of_meta(&meta(
        json!({"sprints": [{"id": "A"}, {"id": "B", "state": "active"}]}),
    ))
    .unwrap();
    assert_eq!(vocab.sprints[0]["state"], json!("planned"));
    assert_eq!(vocab.sprints[1]["state"], json!("active"));
    assert_eq!(
        error(json!({"sprints": {}})),
        "meta.sprints must be a list of sprints"
    );
    assert_eq!(
        error(json!({"sprints": [{"id": " "}]})),
        "meta.sprints: every sprint needs an \"id\", its name (a non-empty string)"
    );
    assert_eq!(
        error(json!({"sprints": [{"id": "A "}]})),
        "meta.sprints: 'A ' is not a sprint name (at most 48 characters, no space at either end)"
    );
    assert_eq!(
        error(json!({"sprints": [{"id": "A"}, {"id": "a"}]})),
        "meta.sprints: 'a' is listed twice"
    );
    assert_eq!(
        error(json!({"sprints": [{"id": "A", "state": "x"}]})),
        "meta.sprints: 'A' has the state 'x'; use one of planned, active, closed"
    );
    assert_eq!(
        error(json!({"sprints": [{"id": "A", "due": "2026-1-1"}]})),
        "meta.sprints: 'A' has the due date '2026-1-1'; use YYYY-MM-DD"
    );
    assert_eq!(
        error(json!({"sprints": [{"id": "A", "goal": 1}]})),
        "meta.sprints: 'A''s goal must be a string"
    );
    assert_eq!(
        error(json!({"sprints": [{"id": "A", "state": "active"}, {"id": "B", "state": "active"}]})),
        "meta.sprints: only one sprint can be active, but A and B are"
    );
}

#[test]
fn releases_and_components_are_checked() {
    assert_eq!(
        error(json!({"releases": [{"id": "1 0"}]})),
        "meta.releases: every release needs an \"id\" of letters, digits, \".\", \"-\" and \"_\" (at most 64)"
    );
    assert_eq!(
        error(json!({"releases": [{"id": "1"}, {"id": "1"}]})),
        "meta.releases: '1' is listed twice"
    );
    assert_eq!(
        error(json!({"releases": [{"id": "1", "target": "soon"}]})),
        "meta.releases: '1' has the target 'soon'; use YYYY-MM-DD"
    );
    assert_eq!(
        error(json!({"components": [{"id": "api", "paths": [""]}]})),
        "meta.components: 'api''s paths must be a list of globs (strings)"
    );
    assert_eq!(
        error(json!({"components": [{"id": "api", "review": "later"}]})),
        "meta.components: 'api' has the review 'later'; use one of blocker, async, batch, none"
    );
    let vocab = Vocabulary::of_meta(&meta(json!({"components": [{"id": "api"}]}))).unwrap();
    assert_eq!(vocab.components[0]["paths"], json!([]));
}

#[test]
fn settings_must_be_one_of_their_choices() {
    assert_eq!(
        error(json!({"review": "x"})),
        "meta.review must be one of blocker, async, batch, none"
    );
    assert_eq!(
        error(json!({"sprint_names": null})),
        "meta.sprint_names must be one of people, time"
    );
}
