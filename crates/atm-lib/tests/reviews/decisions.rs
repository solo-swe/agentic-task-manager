//! A reviewer's decision: every named task's, recorded as the reviewer.

use serde_json::json;

use crate::fixtures::{dir, open_review, project, reopen, round, task};
use atm_lib::reviews::{NewReview, ReviewStore, Status, Verdict};

#[test]
fn a_review_of_two_tasks_puts_both_in_review_and_its_decisions_are_theirs() {
    let dir = dir();
    let mut project = project(&dir.0, "claude:1");
    let header = atm_lib::reviews::create(
        &mut project,
        NewReview {
            title: "Both".into(),
            summary: String::new(),
            tasks: vec!["T-1".into(), "T-2".into()],
            author: Some("claude:1".into()),
            reviewers: vec!["Hakan".into()],
            status: Status::Waiting,
            first: round(),
        },
    )
    .unwrap();
    let cr = header.reference.clone();
    for id in ["T-1", "T-2"] {
        let t = task(&project, id);
        assert_eq!(t["status"], "in_review", "{id}");
        assert_eq!(
            t["code_reviews"],
            json!([{ "id": header.id.0, "ref": "CR-1" }])
        );
        assert!(t["notes"].to_string().contains("CR-1: Both"));
    }

    // The reviewer comments and sends it back: both tasks have changes asked for, naming the review
    let mut project = reopen(&dir.0, "Hakan");
    let mut reviews = atm_lib::reviews::store(&project).unwrap();
    atm_lib::reviews::Reviews::new(&mut reviews)
        .comment(
            &cr,
            atm_lib::reviews::Anchor {
                round: 1,
                ..atm_lib::reviews::Anchor::default()
            },
            "Fix it.",
            &"Hakan".into(),
        )
        .unwrap();
    atm_lib::reviews::submit(&mut project, &cr, Verdict::RequestChanges, None, None).unwrap();
    let t1 = task(&project, "T-1");
    assert_eq!(t1["review_state"], "changes_requested");
    assert!(
        t1["notes"]
            .to_string()
            .contains("CR-1 round 1: 1 open comment"),
        "{}",
        t1["notes"]
    );

    // The agent takes it back, fixes it, adds a round: in review again
    let mut agent = reopen(&dir.0, "claude:1");
    atm_lib::changes::change_task_status(
        &mut agent,
        atm_lib::state::Command::Start,
        "T-1",
        &atm_lib::state::Options::default(),
    )
    .unwrap();
    atm_lib::reviews::add_round(&mut agent, &cr, round()).unwrap();
    assert_eq!(task(&agent, "T-1")["review_state"], "waiting");

    // Approved once the comment is resolved: approved for the task in review
    let mut project = reopen(&dir.0, "Hakan");
    let mut reviews = atm_lib::reviews::store(&project).unwrap();
    let open = reviews
        .get_review(&cr)
        .unwrap()
        .open_comments()
        .next()
        .unwrap()
        .id
        .clone();
    atm_lib::reviews::Reviews::new(&mut reviews)
        .resolve(&cr, &open, true, &"Hakan".into())
        .unwrap();
    atm_lib::reviews::submit(&mut project, &cr, Verdict::Approve, None, None).unwrap();
    assert_eq!(task(&project, "T-1")["review_state"], "approved");
    // T-2 stayed in review with changes asked for: approved now too
    assert_eq!(task(&project, "T-2")["review_state"], "approved");
}

/// ATM-609: the reviewer approves a review they sent back, with no round since: the review and its task approved
#[test]
fn a_review_sent_back_is_approved_without_a_round_since_and_so_is_its_task() {
    let dir = dir();
    let mut project = project(&dir.0, "claude:1");
    let cr = atm_lib::reviews::create(
        &mut project,
        NewReview {
            title: "One".into(),
            summary: String::new(),
            tasks: vec!["T-1".into()],
            author: Some("claude:1".into()),
            reviewers: vec!["Hakan".into()],
            status: Status::Waiting,
            first: round(),
        },
    )
    .unwrap()
    .reference;
    let mut project = reopen(&dir.0, "Hakan");
    let mut reviews = atm_lib::reviews::store(&project).unwrap();
    let anchor = atm_lib::reviews::Anchor {
        round: 1,
        ..atm_lib::reviews::Anchor::default()
    };
    atm_lib::reviews::Reviews::new(&mut reviews)
        .comment(&cr, anchor, "Fix it.", &"Hakan".into())
        .unwrap();
    atm_lib::reviews::submit(&mut project, &cr, Verdict::RequestChanges, None, None).unwrap();
    assert_eq!(task(&project, "T-1")["review_state"], "changes_requested");
    // With its comment open, still refused
    assert!(atm_lib::reviews::submit(&mut project, &cr, Verdict::Approve, None, None).is_err());
    let open = reviews
        .get_review(&cr)
        .unwrap()
        .open_comments()
        .next()
        .unwrap()
        .id
        .clone();
    atm_lib::reviews::Reviews::new(&mut reviews)
        .resolve(&cr, &open, true, &"Hakan".into())
        .unwrap();
    atm_lib::reviews::submit(&mut project, &cr, Verdict::Approve, None, None).unwrap();
    let reviews = atm_lib::reviews::store(&project).unwrap();
    assert_eq!(
        reviews.get_review(&cr).unwrap().header.status,
        Status::Approved
    );
    assert_eq!(task(&project, "T-1")["review_state"], "approved");
}

#[test]
fn a_comment_only_submission_leaves_the_tasks_waiting() {
    let dir = dir();
    let mut project = project(&dir.0, "claude:1");
    let header = open_review(&mut project, &["T-1"]);
    let mut hakan = reopen(&dir.0, "Hakan");
    atm_lib::reviews::submit(
        &mut hakan,
        &header.reference,
        Verdict::Comment,
        Some("hm"),
        None,
    )
    .unwrap();
    assert_eq!(task(&hakan, "T-1")["review_state"], "waiting");
}

#[test]
fn closing_a_review_rejects_its_tasks_only_when_asked() {
    for (reject, state) in [(false, "waiting"), (true, "rejected")] {
        let dir = dir();
        let mut project = project(&dir.0, "claude:1");
        let header = open_review(&mut project, &["T-1"]);
        let mut hakan = reopen(&dir.0, "Hakan");
        atm_lib::reviews::close(&mut hakan, &header.reference, "not needed", reject).unwrap();
        let t1 = task(&hakan, "T-1");
        assert_eq!(t1["review_state"], state, "reject {reject}");
        if reject {
            assert!(t1["notes"].to_string().contains("CR-1 closed: not needed"));
        }
    }
}
