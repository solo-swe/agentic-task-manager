//! The one machine (ATM-633, docs/task-review-state-machine.md §5): `lifecycle::check` and `can` through the real
//! store.

use atm_lib::lifecycle::{Event, Outcome, can, check};
use atm_lib::reviews::{ReviewStore, Status, Verdict};
use atm_lib::state::Command;

use crate::fixtures::{dir, open_review, project, reopen, task};

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
