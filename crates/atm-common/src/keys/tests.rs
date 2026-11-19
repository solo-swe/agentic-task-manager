use super::*;
use serde_json::json;

const A: &str = "0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8091";
const B: &str = "0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8092";
const C: &str = "0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8093";
const OLD: &str = "0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8094";

fn task(v: Value) -> Task {
    match v {
        Value::Object(m) => m,
        other => panic!("not a task: {other}"),
    }
}

/// ATM (formerly T): ATM-527, ATM-538 holding the number another task moved off (its alias), ATM-541 that moved;
/// ATM-12 archived
fn project() -> (Value, Vec<Task>) {
    let document = json!({
        "meta": {"format": 2, "key": "ATM", "key_history": ["T"], "next_number": 542},
        "tasks": [
            {"id": A, "number": 527, "aliases": ["T-P1-03"]},
            {"id": B, "number": 538},
            {"id": C, "number": 541, "aliases": ["ATM-538"]}
        ]
    });
    let archive = vec![task(json!({"id": OLD, "number": 12}))];
    (document, archive)
}

fn found(reference: &str) -> (String, bool, Via) {
    let (document, archive) = project();
    let f = resolve(&document, &archive, reference).unwrap();
    (
        f.task["id"].as_str().unwrap().to_string(),
        f.archived,
        f.via,
    )
}

fn not_found(reference: &str) -> String {
    let (document, archive) = project();
    let e = resolve(&document, &archive, reference).unwrap_err();
    assert!(matches!(e, Error::NotFound(_)), "{e:?}");
    e.to_string()
}

#[test]
fn a_key_any_case_an_earlier_prefix_an_alias_or_the_uuid_finds_its_task() {
    assert_eq!(found("ATM-527"), (A.into(), false, Via::Key));
    assert_eq!(found("atm-527"), (A.into(), false, Via::Key));
    assert_eq!(found(" ATM-527 "), (A.into(), false, Via::Key));
    assert_eq!(found("T-527"), (A.into(), false, Via::EarlierPrefix));
    assert_eq!(found("T-P1-03"), (A.into(), false, Via::Alias));
    assert_eq!(found(A), (A.into(), false, Via::Uuid));
}

/// ADR-009: keys read plain, whatever a file still says of `key_final`; a `~` written before (old notes, commits,
/// reviews) still finds its task; a `~` alone is no key
#[test]
fn keys_read_plain_and_an_old_draft_mark_still_finds_its_task() {
    for task in [
        json!({"id": A, "key": "ATM-527"}),
        json!({"id": A, "key": "ATM-527", "key_final": false}),
        json!({"id": A, "key": "ATM-527", "key_final": true}),
    ] {
        assert_eq!(label_of(&task), "ATM-527", "{task}");
    }
    assert_eq!(label_of(&json!({"id": "T-1"})), "T-1");
    assert_eq!(found("~ATM-527"), (A.into(), false, Via::Key));
    assert_eq!(found(" ~atm-527 "), (A.into(), false, Via::Key));
    assert_eq!(found("~T-527"), (A.into(), false, Via::EarlierPrefix));
    assert_eq!(without_draft_mark("~"), "~");
    assert!(not_found("~").starts_with("no task '~'"));
}

#[test]
fn the_archive_too() {
    assert_eq!(found("ATM-12"), (OLD.into(), true, Via::Key));
    assert_eq!(found("T-12"), (OLD.into(), true, Via::EarlierPrefix));
    assert_eq!(found(OLD), (OLD.into(), true, Via::Uuid));
}

#[test]
fn a_current_key_wins_over_an_alias_that_moved_off_it() {
    // ATM-538 is B's number now; C moved off it (its alias)
    assert_eq!(found("ATM-538"), (B.into(), false, Via::Key));
    assert_eq!(found("ATM-541"), (C.into(), false, Via::Key));
    // Once no task holds 538, the alias finds C
    let (mut document, archive) = project();
    document["tasks"].as_array_mut().unwrap().remove(1);
    let f = resolve(&document, &archive, "ATM-538").unwrap();
    assert_eq!((f.task["id"].as_str().unwrap(), f.via), (C, Via::Alias));
}

#[test]
fn what_finds_nothing_says_why() {
    assert_eq!(not_found("ATM-999"), "no task 'ATM-999' in this project");
    assert_eq!(
        not_found("LFB-5"),
        "no task 'LFB-5': LFB isn't this project's key (ATM) nor one it had"
    );
    assert_eq!(
        not_found("nonsense"),
        "no task 'nonsense': give its key (ATM-527), or its UUID"
    );
    assert_eq!(
        not_found("0193a5b2-7c1d-7e2f-8a3b-000000000000"),
        "no task with the UUID 0193a5b2-7c1d-7e2f-8a3b-000000000000"
    );
}

#[test]
fn two_tasks_with_one_number_are_named_not_guessed() {
    let (mut document, archive) = project();
    document["tasks"][1]["number"] = json!(527);
    let e = resolve(&document, &archive, "ATM-527").unwrap_err();
    assert!(!matches!(e, Error::NotFound(_)));
    assert_eq!(
        e.to_string(),
        format!("'ATM-527' names 2 tasks ({A}, {B}): use one's UUID, and run atm doctor")
    );
}

#[test]
fn a_legacy_file_matches_ids_exactly() {
    let document = json!({"tasks": [{"id": "T-1"}, {"id": "T-10"}]});
    let archive = vec![task(json!({"id": "T-0"}))];
    let f = resolve(&document, &archive, "T-10").unwrap();
    assert_eq!((f.task["id"].as_str().unwrap(), f.via), ("T-10", Via::Id));
    assert!(resolve(&document, &archive, "T-0").unwrap().archived);
    assert!(
        resolve(&document, &archive, "t-10").is_err(),
        "exactly, as before"
    );
    assert_eq!(
        resolve(&document, &archive, "T-2").unwrap_err().to_string(),
        "no task 'T-2'"
    );
}

#[test]
fn keys_numbers_and_order() {
    let (document, archive) = project();
    let tasks: Vec<&Task> = tasks_of(&document).collect();
    assert_eq!(key_of(&document, tasks[0]).as_deref(), Some("ATM-527"));
    assert_eq!(
        key_of(&json!({"tasks": []}), tasks[0]),
        None,
        "legacy: no keys"
    );
    assert_eq!(next_task_number(&document, &archive), 542);
    // Never at or below a number given, whatever meta says
    let mut behind = document.clone();
    behind["meta"]["next_number"] = json!(3);
    assert_eq!(next_task_number(&behind, &archive), 542);
    assert_eq!(next_task_number(&json!({"meta": {}, "tasks": []}), &[]), 1);
    let two = task(json!({"id": "x", "number": 2}));
    let ten = task(json!({"id": "a", "number": 10}));
    assert_eq!(
        task_order(&two, &ten),
        Ordering::Less,
        "by number, not by UUID"
    );
    let t2 = task(json!({"id": "T-2"}));
    let t10 = task(json!({"id": "T-10"}));
    assert_eq!(task_order(&t2, &t10), Ordering::Less, "legacy: as before");
}
