//! The reviews list and a project's summary of them.

use serde_json::json;

use crate::fixtures::{dir, git, project, reopen, round, task, waiting};
use atm_lib::Project;
use atm_lib::reviews::{NewReview, ReviewRef, ReviewStore, RoundKind, Status, Verdict};

/// ATM-606: a project's summary counts the reviews waiting on the user at the machine (their git name), and a task
/// in review isn't counted again through them; the summary's stamp moves with a comment
#[test]
fn a_projects_summary_counts_the_reviews_waiting_on_the_user_once() {
    let dir = dir();
    git(&["init", "-q"], &dir.0);
    git(&["config", "user.name", "Hakan"], &dir.0);
    let mut project = project(&dir.0, "claude:1");
    let summary = |p: &Project| atm_lib::summary::project_summary(p).unwrap();
    assert_eq!(summary(&project)["needs_you"]["code_reviews"], 0);
    let stamp = atm_lib::review_pages::reviews_stamp(&project);
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
    assert_ne!(atm_lib::review_pages::reviews_stamp(&project), stamp);
    // Made by the agent, read as anyone: the user at the machine is the one it waits on
    let s = summary(&reopen(&dir.0, "claude:1"));
    assert_eq!(s["needs_you"]["code_reviews"], 1);
    // T-1 is in review, through CR-1: not counted again
    assert_eq!(task(&project, "T-1")["status"], "in_review");
    assert_eq!(s["needs_you"]["review"], 0);
    // With its ID, as T-1's code_reviews link it: the dashboard drops T-1 from its own list by it (ATM-640)
    let link = task(&project, "T-1")["code_reviews"][0]["id"].clone();
    assert_eq!(
        s["needs_you_reviews"],
        json!([{ "id": link, "reference": "CR-1", "title": "One", "why": "review" }])
    );
    // Sent back by the user: off their list until the agent answers
    let mut hakan = reopen(&dir.0, "Hakan");
    let mut reviews = atm_lib::reviews::store(&hakan).unwrap();
    let anchor = atm_lib::reviews::Anchor {
        round: 1,
        ..atm_lib::reviews::Anchor::default()
    };
    atm_lib::reviews::Reviews::new(&mut reviews)
        .comment(&cr, anchor, "Fix it.", &"Hakan".into())
        .unwrap();
    atm_lib::reviews::submit(&mut hakan, &cr, Verdict::RequestChanges, None, None).unwrap();
    assert_eq!(summary(&hakan)["needs_you"]["code_reviews"], 0);
    let open = reviews
        .get_review(&cr)
        .unwrap()
        .open_comments()
        .next()
        .unwrap()
        .id
        .clone();
    atm_lib::reviews::Reviews::new(&mut reviews)
        .reply(&cr, &open, "Fixing.", &"claude:1".into())
        .unwrap();
    assert_eq!(summary(&hakan)["needs_you_reviews"][0]["why"], "answered");
}

/// ATM-714: the list says whether each review is decided (approved or closed), atm's rule the dashboard groups by
#[test]
fn the_list_says_whether_each_review_has_ended() {
    let dir = dir();
    let mut project = project(&dir.0, "claude:1");
    waiting(&mut project, &["T-1"]).unwrap();
    let ended = |project: &atm_lib::Project| {
        atm_lib::review_pages::list_reviews(project, &atm_lib::review_pages::ReviewQuery::default())
            .unwrap()["reviews"][0]["ended"]
            .clone()
    };
    assert_eq!(ended(&project), json!(false));
    let mut hakan = reopen(&dir.0, "Hakan");
    atm_lib::reviews::close(&mut hakan, &ReviewRef("CR-1".into()), "Not this.", false).unwrap();
    assert_eq!(ended(&hakan), json!(true));
}

/// ATM-610: the list says since when each review waits on its reviewers, its latest round's time
#[test]
fn the_list_says_since_when_each_review_waits() {
    let dir = dir();
    let mut project = project(&dir.0, "claude:1");
    waiting(&mut project, &["T-1"]).unwrap();
    let cr = ReviewRef("CR-1".into());
    let listed = atm_lib::review_pages::list_reviews(
        &project,
        &atm_lib::review_pages::ReviewQuery::default(),
    )
    .unwrap();
    let first = atm_lib::reviews::store(&project)
        .unwrap()
        .get_review(&cr)
        .unwrap();
    assert_eq!(
        listed["reviews"][0]["waiting_since"],
        json!(first.rounds[0].at)
    );
    let mut reviews = atm_lib::reviews::store(&project).unwrap();
    let mut later = round();
    later.kind = RoundKind::Range {
        base: "a".repeat(40),
        head: "c".repeat(40),
    };
    atm_lib::reviews::Reviews::new(&mut reviews)
        .add_round(&cr, later)
        .unwrap();
    let again = atm_lib::review_pages::list_reviews(
        &project,
        &atm_lib::review_pages::ReviewQuery::default(),
    )
    .unwrap();
    let second = atm_lib::reviews::store(&project)
        .unwrap()
        .get_review(&cr)
        .unwrap();
    assert_eq!(
        again["reviews"][0]["waiting_since"],
        json!(second.rounds[1].at),
        "its latest round"
    );
}
