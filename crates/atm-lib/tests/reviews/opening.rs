//! Opening a review: what it does to each of its tasks (ATM-631), and the tasks named later.

use serde_json::{Value, json};

use crate::fixtures::{
    LAPSED, LATER, claim, dir, no_review, project, project_of, round, task, task_of, waiting,
};
use atm_lib::reviews::{NewReview, ReviewRef, Status};

/// One case of opening a review: its name, the task, its claims, opened or the refusal's words, its status after
type Case = (
    &'static str,
    Value,
    Value,
    Result<(), &'static str>,
    &'static str,
);

/// ATM-631: opening a review waiting on its reviewers puts each task in review, or refuses and writes nothing
#[test]
fn opening_a_review_moves_each_task_into_review_or_writes_nothing() {
    let task = |status: &str, started_by: Option<&str>| {
        let mut t = json!({ "id": "T-1", "name": "One", "type": "feature", "status": status });
        if let Some(agent) = started_by {
            t["started_by"] = json!(agent);
        }
        t
    };
    let cases: [Case; 9] = [
        (
            "in progress, claimed by the author",
            task("in_progress", Some("claude:1")),
            json!({ "T-1": claim("T-1", "claude:1", LATER) }),
            Ok(()),
            "in_review",
        ),
        (
            "in progress, no claim",
            task("in_progress", None),
            json!({}),
            Ok(()),
            "in_review",
        ),
        (
            "in progress, the author's claim run out",
            task("in_progress", Some("claude:1")),
            json!({ "T-1": claim("T-1", "claude:1", LAPSED) }),
            Ok(()),
            "in_review",
        ),
        (
            "to do after the author's claim lapsed",
            task("todo", Some("claude:1")),
            json!({}),
            Ok(()),
            "in_review",
        ),
        (
            "claimed by another agent",
            task("in_progress", Some("claude:2")),
            json!({ "T-1": claim("T-1", "claude:2", LATER) }),
            Err("claimed by claude:2"),
            "in_progress",
        ),
        (
            "to do, never started by the author",
            task("todo", Some("claude:2")),
            json!({}),
            Err("you didn't start it"),
            "todo",
        ),
        (
            "blocked",
            task("blocked", Some("claude:1")),
            json!({}),
            Err("it is blocked"),
            "blocked",
        ),
        (
            "done",
            task("done", Some("claude:1")),
            json!({}),
            Err("it is done"),
            "done",
        ),
        (
            "skipped",
            task("skipped", Some("claude:1")),
            json!({}),
            Err("it is skipped"),
            "skipped",
        ),
    ];
    for (case, t, claims, expected, after) in cases {
        let dir = dir();
        let mut project = project_of(&dir.0, "claude:1", &json!([t]), &claims);
        let opened = waiting(&mut project, &["T-1"]);
        match expected {
            Ok(()) => {
                opened.unwrap_or_else(|e| panic!("{case}: {e}"));
                let t = task_of(&project, "T-1");
                assert_eq!(t["code_reviews"][0]["ref"], "CR-1", "{case}");
            }
            Err(words) => {
                let message = opened.expect_err(case).to_string();
                assert!(message.contains(words), "{case}: {message}");
                assert!(no_review(&project), "{case}: a review was kept");
            }
        }
        assert_eq!(task_of(&project, "T-1")["status"], after, "{case}");
    }
}

#[test]
fn a_task_taken_back_after_its_claim_lapsed_says_so_and_is_claimed_again() {
    let dir = dir();
    let tasks = json!([{ "id": "T-1", "name": "One", "type": "feature", "status": "todo", "started_by": "claude:1" }]);
    let mut project = project_of(&dir.0, "claude:1", &tasks, &json!({}));
    waiting(&mut project, &["T-1"]).unwrap();
    let t = task_of(&project, "T-1");
    assert_eq!(t["status"], "in_review");
    assert!(
        t["notes"].to_string().contains("its claim had lapsed"),
        "{}",
        t["notes"]
    );
}

#[test]
fn a_task_in_review_is_linked_unless_another_review_waits_on_it() {
    let dir = dir();
    let mut project = project(&dir.0, "claude:1");
    waiting(&mut project, &["T-1"]).unwrap();
    // CR-1 waits on T-1: a second review is refused, and points to a round of CR-1
    let refused = waiting(&mut project, &["T-1"]).unwrap_err().to_string();
    assert!(
        refused.contains("CR-1 already waits on it") && refused.contains("atm review round CR-1"),
        "{refused}"
    );
    // In review with no review waiting on it (CR-1 closed): linked, still in review
    let mut reviews = atm_lib::reviews::store(&project).unwrap();
    atm_lib::reviews::Reviews::new(&mut reviews)
        .close(&ReviewRef("CR-1".into()), "Replaced.", &"claude:1".into())
        .unwrap();
    let second = waiting(&mut project, &["T-1"]).unwrap();
    let t = task_of(&project, "T-1");
    assert_eq!(t["status"], "in_review");
    assert!(t["code_reviews"].to_string().contains(&second.reference.0));
}

#[test]
fn a_review_refused_for_one_task_moves_none_of_the_others() {
    let dir = dir();
    let tasks = json!([
        { "id": "T-1", "name": "One", "type": "feature", "status": "todo", "started_by": "claude:1" },
        { "id": "T-2", "name": "Two", "type": "feature", "status": "done" },
    ]);
    let mut project = project_of(&dir.0, "claude:1", &tasks, &json!({}));
    assert!(waiting(&mut project, &["T-1", "T-2"]).is_err());
    assert!(no_review(&project));
    assert_eq!(task_of(&project, "T-1")["status"], "todo");
}

/// A person's review of files for a task an agent hasn't started (the sandbox's red-team review): linked, as it is
#[test]
fn a_persons_review_of_a_task_not_started_links_it_as_it_is() {
    let dir = dir();
    let tasks = json!([{ "id": "T-1", "name": "One", "type": "feature", "status": "todo" }]);
    let mut project = project_of(&dir.0, "Hakan", &tasks, &json!({}));
    waiting(&mut project, &["T-1"]).unwrap();
    let t = task_of(&project, "T-1");
    assert_eq!(t["status"], "todo");
    assert_eq!(t["code_reviews"][0]["ref"], "CR-1");
}

#[test]
fn a_draft_review_leaves_its_tasks_as_they_are_whatever_their_state() {
    let dir = dir();
    let tasks = json!([{ "id": "T-1", "name": "One", "type": "feature", "status": "done" }]);
    let mut project = project_of(&dir.0, "claude:1", &tasks, &json!({}));
    let draft = NewReview {
        title: "Draft".into(),
        summary: String::new(),
        tasks: vec!["T-1".into()],
        author: Some("claude:1".into()),
        reviewers: Vec::new(),
        status: Status::Draft,
        first: round(),
    };
    atm_lib::reviews::create(&mut project, draft).unwrap();
    assert_eq!(task_of(&project, "T-1")["status"], "done");
}

#[test]
fn tasks_named_later_are_linked_and_removed_ones_unlinked_with_a_note() {
    let dir = dir();
    let mut project = project(&dir.0, "claude:1");
    let header = atm_lib::reviews::create(
        &mut project,
        NewReview {
            title: "Draft".into(),
            summary: String::new(),
            tasks: vec!["T-1".into()],
            author: Some("claude:1".into()),
            reviewers: Vec::new(),
            status: Status::Draft,
            first: round(),
        },
    )
    .unwrap();
    // A draft puts nothing in review
    assert_eq!(task(&project, "T-1")["status"], "in_progress");
    let header2 = atm_lib::reviews::set_tasks(
        &mut project,
        &header.reference,
        &["T-2".into()],
        &["T-1".into()],
    )
    .unwrap();
    assert_eq!(header2.tasks, ["T-2"]);
    assert!(task(&project, "T-1").get("code_reviews").is_none());
    assert!(
        task(&project, "T-1")["notes"]
            .to_string()
            .contains("No longer reviewed in CR-1")
    );
    assert_eq!(task(&project, "T-2")["code_reviews"][0]["ref"], "CR-1");
}
