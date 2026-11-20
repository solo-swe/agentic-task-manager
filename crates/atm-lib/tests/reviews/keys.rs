//! A review names its tasks by the key each had (ATM-630).

use serde_json::json;

use crate::fixtures::{PROVISIONAL, dir, keyed, open_review, reopen, round, task};
use atm_lib::reviews::{ReviewStore, Status, Verdict};

#[test]
fn a_review_names_a_provisional_task_by_its_key_and_its_decision_reaches_it() {
    let dir = dir();
    let mut project = keyed(&dir.0, "claude:1");
    // Named as atm prints it, with the mark: the key itself
    let header = open_review(&mut project, &["~ATM-12", "ATM-11"]);
    assert_eq!(header.tasks, ["ATM-12", "ATM-11"]);
    let text = std::fs::read_to_string(
        std::fs::read_dir(dir.0.join(".agentic-task-manager/reviews"))
            .unwrap()
            .filter_map(Result::ok)
            .find(|e| e.path().is_dir())
            .unwrap()
            .path()
            .join("review.md"),
    )
    .unwrap();
    assert!(text.contains("tasks = [\"ATM-12\", \"ATM-11\"]"), "{text}");
    assert_eq!(task(&project, PROVISIONAL)["status"], "in_review");
    // The approval reaches both
    let mut hakan = reopen(&dir.0, "Hakan");
    atm_lib::reviews::submit(&mut hakan, &header.reference, Verdict::Approve, None, None).unwrap();
    assert_eq!(task(&hakan, PROVISIONAL)["review_state"], "approved");
    // And every command takes the key as people read it
    assert_eq!(hakan.resolve_task("~ATM-12").unwrap(), PROVISIONAL);
    assert_eq!(hakan.resolve_task("ATM-12").unwrap(), PROVISIONAL);
    assert!(hakan.resolve_task("~").is_err(), "a mark alone is no key");
}

#[test]
fn a_decision_that_cant_reach_a_named_task_isnt_recorded_and_says_why() {
    let dir = dir();
    let mut project = keyed(&dir.0, "claude:1");
    let header = open_review(&mut project, &["ATM-12"]);
    // The task is gone from the project (a hand edit, a branch that dropped it)
    let mut store = atm_lib::reviews::store(&project).unwrap();
    store.set_tasks(&header.id, vec!["ATM-99".into()]).unwrap();
    let mut hakan = reopen(&dir.0, "Hakan");
    let error =
        atm_lib::reviews::submit(&mut hakan, &header.reference, Verdict::Approve, None, None)
            .unwrap_err();
    assert!(
        error
            .to_string()
            .starts_with("CR-1's decision can't reach its tasks, so it isn't recorded: "),
        "{error}"
    );
    let reviews = atm_lib::reviews::store(&hakan).unwrap();
    assert_eq!(
        reviews.get_review(&header.reference).unwrap().header.status,
        Status::Waiting
    );
    // A round neither: refused before anything is written
    let mut agent = reopen(&dir.0, "claude:1");
    let error = atm_lib::reviews::add_round(&mut agent, &header.reference, round()).unwrap_err();
    assert!(
        error
            .to_string()
            .starts_with("CR-1's round can't reach its tasks, so it isn't added: "),
        "{error}"
    );
    let reviews = atm_lib::reviews::store(&agent).unwrap();
    assert_eq!(
        reviews.get_review(&header.reference).unwrap().rounds.len(),
        1
    );
}

/// ATM-630: a review names its task by the key it was written with; atm finds the task through it as every command
/// does: a task whose number moved (its old key an alias, ADR-009), an old review's `~KEY`; a current key over an
/// alias, as ADR-007 §5 has it
#[test]
fn a_review_names_its_task_by_the_key_it_had_an_alias_now_or_with_an_old_mark() {
    let dir = dir();
    let id = |n: u32| format!("0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f80{n:02}");
    let tasks = json!({
        "meta": { "project": "Moved", "format": 2, "key": "ATM", "next_number": 15 },
        "tasks": [
            // Moved off 9 at a merge, 9 free since
            { "id": id(14), "number": 14, "aliases": ["ATM-9"], "name": "Moved", "type": "feature", "status": "todo" },
            // 12 is this task's; another one moved off 12 to 13
            { "id": id(12), "number": 12, "name": "Holds 12", "type": "feature", "status": "todo" },
            { "id": id(13), "number": 13, "aliases": ["ATM-12"], "name": "Moved off 12", "type": "feature", "status": "todo" },
        ]
    });
    std::fs::create_dir_all(dir.0.join(".agentic-task-manager")).unwrap();
    std::fs::write(
        dir.0.join(".agentic-task-manager/tasks.json"),
        tasks.to_string(),
    )
    .unwrap();
    let project = reopen(&dir.0, "Hakan");
    let header = |keys: &[&str]| atm_lib::reviews::ReviewHeader {
        id: atm_lib::reviews::ReviewId("r".into()),
        reference: atm_lib::reviews::ReviewRef("CR-1".into()),
        number: 1,
        title: "T".into(),
        status: Status::Waiting,
        tasks: keys.iter().map(|k| (*k).to_string()).collect(),
        author: None,
        reviewers: Vec::new(),
        created: String::new(),
        updated: String::new(),
    };
    // Without its review's folder to read (review_keys' tests read one), a key resolves as every command's does
    let names = |keys: &[&str], wanted: &str| {
        atm_lib::reviews::names_task(&project, None, &header(keys), wanted)
    };
    // Its key then, an alias now: the task, by its key now; with an old review's mark too
    assert!(names(&["ATM-9"], "ATM-14"));
    assert!(names(&["~ATM-9"], "ATM-14"));
    assert!(names(&["atm-9", "ATM-12"], "ATM-14"));
    // A key a task holds now names that task, not the one that moved off it
    assert!(names(&["ATM-12"], "ATM-12"));
    assert!(!names(&["ATM-12"], "ATM-13"));
    // One the project doesn't have names no task of it
    assert!(!names(&["ATM-99"], "ATM-14"));
    assert_eq!(
        atm_lib::reviews::task_reference(&project, "ATM-99"),
        "ATM-99"
    );
    assert_eq!(
        atm_lib::reviews::task_reference(&project, "~ATM-9"),
        "ATM-14"
    );
}
