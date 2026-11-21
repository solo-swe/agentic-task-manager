//! The one machine (ATM-633, docs/task-review-state-machine.md §5): `lifecycle::check` and `can` through the real
//! store.

use atm_lib::lifecycle::{Event, Outcome, can, check};
use atm_lib::reviews::{ReviewStore, Status, Verdict};
use atm_lib::state::Command;

use crate::fixtures::{dir, open_review, project, project_of, reopen, round, task};

/// The task file with `id`'s status set to `status`, as a lapse or another branch's merge leaves it
fn set_status(root: &std::path::Path, id: &str, status: &str) {
    let file = root.join(".agentic-task-manager/tasks.json");
    let mut tasks: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    for t in tasks["tasks"].as_array_mut().unwrap() {
        if t["id"] == id {
            t["status"] = status.into();
            t.as_object_mut().unwrap().remove("review_state");
        }
    }
    std::fs::write(&file, tasks.to_string()).unwrap();
}

/// ATM-712, the design's hole 1 (ATM-632's class): a round on a review whose task its lapse sent back to to do took
/// the round and left the task there; the author's own task is taken back first, then goes in review with it
#[test]
fn a_round_takes_the_authors_lapsed_task_back_before_it_goes_in_review() {
    let dir = dir();
    let mut agent = project(&dir.0, "claude:1");
    let header = open_review(&mut agent, &["T-1"]);
    let mut hakan = reopen(&dir.0, "Hakan");
    atm_lib::reviews::submit(
        &mut hakan,
        &header.reference,
        Verdict::RequestChanges,
        Some("Fix it."),
        None,
    )
    .unwrap();
    // The agent took it back, then its claim lapsed: the task is to do, started by the agent
    set_status(&dir.0, "T-1", "todo");
    let agent = reopen(&dir.0, "claude:1");
    let review = review_of(&agent, &header.reference);
    let outcome = check(&agent, &review, &Event::Round, &"claude:1".into()).unwrap();
    assert_eq!(outcome.take_back, ["T-1"]);
    // Checking wrote nothing
    assert_eq!(task(&agent, "T-1")["status"], "todo");
    let mut agent = reopen(&dir.0, "claude:1");
    atm_lib::reviews::add_round(&mut agent, &header.reference, round()).unwrap();
    let t = task(&reopen(&dir.0, "claude:1"), "T-1");
    assert_eq!(
        (t["status"].as_str(), t["review_state"].as_str()),
        (Some("in_review"), Some("waiting"))
    );
    assert!(
        t["notes"]
            .to_string()
            .contains("round 2 (its claim had lapsed: taken back for this round)"),
        "{}",
        t["notes"]
    );
    assert_eq!(
        review_of(&agent, &header.reference).header.status,
        Status::Waiting
    );
}

/// ATM-712: a person's to-do task in the review isn't the author's to take back; the round leaves it as it is
#[test]
fn a_round_takes_back_only_the_authors_own_task() {
    let dir = dir();
    let mut agent = project(&dir.0, "claude:1");
    let header = open_review(&mut agent, &["T-1"]);
    let mut hakan = reopen(&dir.0, "Hakan");
    atm_lib::reviews::submit(
        &mut hakan,
        &header.reference,
        Verdict::RequestChanges,
        Some("Fix."),
        None,
    )
    .unwrap();
    set_status(&dir.0, "T-1", "todo");
    // Another agent's round (a review whose author isn't known yet would take it; this one has its author)
    let other = reopen(&dir.0, "claude:2");
    let review = review_of(&other, &header.reference);
    let mut by_other = round();
    by_other.by = "claude:2".into();
    assert!(check(&other, &review, &Event::Round, &by_other.by).is_err());
    assert_eq!(task(&other, "T-1")["status"], "todo");
}

/// ATM-712: tasks added to an agent's review waiting on its reviewers are checked as a new review checks its tasks
/// (ATM-631): one that can't go in review refuses the change, and nothing is written
#[test]
fn tasks_added_to_a_waiting_review_are_checked_first_and_a_refusal_writes_nothing() {
    let dir = dir();
    let tasks = serde_json::json!([
        { "id": "T-1", "name": "One", "type": "feature", "status": "in_progress", "started_by": "claude:1" },
        { "id": "T-2", "name": "Two", "type": "feature", "status": "in_progress", "started_by": "claude:1" },
        { "id": "T-3", "name": "Three", "type": "feature", "status": "blocked" },
    ]);
    let mut agent = project_of(&dir.0, "claude:1", &tasks, &serde_json::json!({}));
    let header = open_review(&mut agent, &["T-1"]);
    let refused = atm_lib::reviews::set_tasks(
        &mut agent,
        &header.reference,
        &["T-2".into(), "T-3".into()],
        &[],
    )
    .unwrap_err();
    assert!(refused.to_string().contains("it is blocked"), "{refused}");
    let agent = reopen(&dir.0, "claude:1");
    assert_eq!(review_of(&agent, &header.reference).header.tasks, ["T-1"]);
    assert_eq!(task(&agent, "T-2")["status"], "in_progress");
    assert!(task(&agent, "T-2").get("code_reviews").is_none());
    // T-2 alone: added, in review
    let mut agent = reopen(&dir.0, "claude:1");
    atm_lib::reviews::set_tasks(&mut agent, &header.reference, &["T-2".into()], &[]).unwrap();
    let t = task(&reopen(&dir.0, "claude:1"), "T-2");
    assert_eq!(t["status"], "in_review");
}

/// ATM-712: a live review's approval reaches its task in two moves, into review then approved, both tried first
#[test]
fn a_live_approval_that_cant_reach_its_task_writes_nothing() {
    let dir = dir();
    let tasks = serde_json::json!([
        { "id": "T-1", "name": "One", "type": "feature", "status": "blocked" },
    ]);
    let project = project_of(&dir.0, "claude:1", &tasks, &serde_json::json!({}));
    let options = atm_lib::state::Options {
        note: Some("approved in the live review CR-1".into()),
        ..atm_lib::state::Options::default()
    };
    assert!(atm_lib::lifecycle::check_live_approval(&project, "T-1", &options).is_err());
    assert_eq!(task(&project, "T-1")["status"], "blocked");
}

fn review_of(
    project: &atm_lib::Project,
    reference: &atm_lib::reviews::ReviewRef,
) -> atm_lib::reviews::Review {
    atm_lib::reviews::store(project)
        .unwrap()
        .get_review(reference)
        .unwrap()
}

#[test]
fn check_says_what_a_decision_does_to_the_review_and_its_tasks_and_writes_nothing() {
    let dir = dir();
    let mut project = project(&dir.0, "claude:1");
    let header = open_review(&mut project, &["T-1"]);
    let hakan = reopen(&dir.0, "Hakan");
    let review = review_of(&hakan, &header.reference);
    let approve = Event::Submit {
        verdict: Verdict::Approve,
        overriding: None,
        also_open: &[],
    };
    let outcome = check(&hakan, &review, &approve, &"Hakan".into()).unwrap();
    assert_eq!(
        outcome,
        Outcome {
            review: Some(Status::Approved),
            tasks: vec![("T-1".into(), Command::Approve)],
            named: vec!["T-1".into()],
            take_back: vec![],
        }
    );
    // Nothing written: the review waits, the task is still in review, waiting
    assert_eq!(
        review_of(&hakan, &header.reference).header.status,
        Status::Waiting
    );
    assert_eq!(task(&hakan, "T-1")["review_state"], "waiting");
    // The author may not decide: the review table's refusal, as `atm review submit` says it
    let refused = check(&project, &review, &approve, &"claude:1".into()).unwrap_err();
    assert!(
        refused.to_string().contains("is decided by its reviewers"),
        "{refused}"
    );
}

#[test]
fn can_answers_each_action_the_page_offers_with_why_not() {
    let dir = dir();
    let mut project = project(&dir.0, "claude:1");
    let header = open_review(&mut project, &["T-1"]);
    let hakan = reopen(&dir.0, "Hakan");
    let answers = can(&hakan, &header.reference, &"Hakan".into(), &[]).unwrap();
    for action in ["approve", "request_changes", "comment", "close"] {
        assert!(answers[action].is_ok(), "{action}: {:?}", answers[action]);
    }
    // A round is its author's
    assert!(
        answers["round"]
            .as_ref()
            .unwrap_err()
            .to_string()
            .contains("author adds its rounds")
    );
    // A draft of the reader's counts as open; so does a comment
    let drafts = ["d1".to_string()];
    let with_draft = can(&hakan, &header.reference, &"Hakan".into(), &drafts).unwrap();
    assert!(
        with_draft["approve"]
            .as_ref()
            .unwrap_err()
            .to_string()
            .contains("comments are open: d1")
    );
    atm_lib::review_requests::add_comment(
        &hakan,
        &header.reference,
        atm_lib::review_requests::NewComment {
            path: None,
            lines: None,
            side: atm_lib::reviews::Side::default(),
            quote: None,
            round: None,
            body: "Why?".into(),
        },
    )
    .unwrap();
    let with_comment = can(&hakan, &header.reference, &"Hakan".into(), &[]).unwrap();
    assert!(
        with_comment["approve"]
            .as_ref()
            .unwrap_err()
            .to_string()
            .contains("comments are open: c1")
    );
    assert!(with_comment["request_changes"].is_ok());
}

#[test]
fn closing_moves_no_task_unless_it_rejects_them_and_then_reaches_each_first() {
    let dir = dir();
    let mut project = project(&dir.0, "claude:1");
    let header = open_review(&mut project, &["T-1"]);
    // T-1 gone from the task file (another branch's merge, a hand edit): the review names a task atm can't find
    let file = dir.0.join(".agentic-task-manager/tasks.json");
    let mut tasks: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    tasks["tasks"]
        .as_array_mut()
        .unwrap()
        .retain(|t| t["id"] != "T-1");
    std::fs::write(&file, tasks.to_string()).unwrap();
    let hakan = reopen(&dir.0, "Hakan");
    let review = review_of(&hakan, &header.reference);
    // Rejecting its tasks: refused before the review closes
    let refused = check(
        &hakan,
        &review,
        &Event::Close { reject: true },
        &"Hakan".into(),
    )
    .unwrap_err();
    assert!(refused.to_string().contains("T-1"), "{refused}");
    let mut closing = reopen(&dir.0, "Hakan");
    assert!(atm_lib::reviews::close(&mut closing, &header.reference, "no", true).is_err());
    assert_eq!(
        review_of(&hakan, &header.reference).header.status,
        Status::Waiting
    );
    // Closing alone moves none, so it needs none found
    let outcome = check(
        &hakan,
        &review,
        &Event::Close { reject: false },
        &"Hakan".into(),
    )
    .unwrap();
    assert_eq!(
        (outcome.review, outcome.tasks.len()),
        (Some(Status::Closed), 0)
    );
    atm_lib::reviews::close(&mut closing, &header.reference, "", false).unwrap();
    assert_eq!(
        review_of(&hakan, &header.reference).header.status,
        Status::Closed
    );
}
