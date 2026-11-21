//! Live reviews (§3.5): the working copy, its checkpoints, and an approval that covers a task.

use serde_json::json;

use crate::fixtures::{dir, git, live, project, reopen, task};
use atm_lib::reviews::{NewReview, ReviewRef, ReviewStore, Status, Verdict};

/// ATM-607: a live review's page is told of the working copy's changes it shows, not of `.git/`, atm's task folder
/// or the files git ignores
#[test]
fn the_working_copy_is_watched_for_changes_a_live_review_shows() {
    use std::time::Duration;
    let dir = dir();
    git(&["init", "-q"], &dir.0);
    std::fs::write(dir.0.join(".gitignore"), "target/\n").unwrap();
    std::fs::create_dir_all(dir.0.join("target")).unwrap();
    let project = project(&dir.0, "claude:1");
    let (sent, received) = std::sync::mpsc::channel();
    let sent = std::sync::Mutex::new(sent);
    let subscription = atm_lib::reviews::subscribe_working_copy(
        &project,
        std::sync::Arc::new(move || {
            if let Ok(sent) = sent.lock() {
                let _ = sent.send(());
            }
        }),
    )
    .unwrap();
    // What was written just before the watch began (the .gitignore, git's own files) may still be told on Linux,
    // where the first events come late: let them pass, a few bursts at most (a watch waking itself, as reading
    // the ignore rules once did there, would never go quiet)
    std::thread::sleep(Duration::from_millis(300));
    let settled = (0..10).any(|_| received.recv_timeout(Duration::from_millis(600)).is_err());
    assert!(settled, "the watch never went quiet");
    let quiet = |what: &str| {
        assert!(
            received.recv_timeout(Duration::from_millis(1200)).is_err(),
            "{what}: nothing told"
        );
    };
    std::fs::write(dir.0.join("target/build.o"), "o").unwrap();
    quiet("an ignored file");
    std::fs::write(dir.0.join(".git/scratch"), "g").unwrap();
    quiet("a file in .git/");
    std::fs::write(dir.0.join(".agentic-task-manager/notes.txt"), "n").unwrap();
    quiet("atm's task folder");
    std::fs::write(dir.0.join("a.txt"), "a").unwrap();
    received
        .recv_timeout(Duration::from_secs(10))
        .expect("a file of the work: told");
    drop(subscription);
}

/// ATM-576: the page shows a live review's working copy as it is now, after its checkpoints, not stored; a
/// comment on it makes it a checkpoint first, and is anchored there; a named round is that round
#[test]
fn a_live_reviews_page_follows_the_working_copy_and_a_comment_makes_it_a_checkpoint() {
    let dir = dir();
    let (hakan, cr) = live(&dir.0);
    let page = || atm_lib::review_pages::review_page(&hakan, &cr, None, None).unwrap();
    // ATM-607: what its page now is made from: the same while nothing changes, another once a file is written; a
    // round named isn't the page now, and isn't tagged
    let stamp = || atm_lib::review_pages::live_page_stamp(&hakan, &cr, None, None).unwrap();
    let before = stamp().expect("a live review's page now is tagged");
    assert_eq!(stamp(), Some(before.clone()));
    assert_eq!(
        atm_lib::review_pages::live_page_stamp(&hakan, &cr, Some(1), None).unwrap(),
        None
    );
    // Nothing changed since its first round: that round, now
    let unchanged = page();
    // ATM-633 (§5.2): what the reader may do, the machine's answer: Hakan reviews it, its rounds are the agent's
    assert_eq!(
        unchanged["can"]["approve"],
        json!({"ok": true}),
        "{}",
        unchanged["can"]
    );
    assert_eq!(unchanged["can"]["comment"]["ok"], json!(true));
    assert_eq!(unchanged["can"]["round"]["ok"], json!(false));
    assert!(
        unchanged["can"]["round"]["why"]
            .as_str()
            .is_some_and(|why| why.contains("rounds")),
        "{}",
        unchanged["can"]
    );
    assert_eq!(
        (
            unchanged["view"]["round"].clone(),
            unchanged["view"]["now"].clone()
        ),
        (json!(1), json!(true))
    );
    // ATM-710: the round after the stored ones asked for, nothing changed: the last one, the working copy
    let unchanged_named = atm_lib::review_pages::review_page(&hakan, &cr, Some(2), None).unwrap();
    assert_eq!(
        (
            unchanged_named["view"]["round"].clone(),
            unchanged_named["view"]["now"].clone()
        ),
        (json!(1), json!(true))
    );
    // Changed: the working copy as the round after the stored ones, which `rounds` doesn't list
    std::fs::write(dir.0.join("a.txt"), "one\ntwo\nthree\n").unwrap();
    assert_ne!(stamp(), Some(before), "a file written: another tag");
    let changed = page();
    assert_eq!(changed["view"]["round"], 2);
    assert_eq!(changed["rounds"].as_array().unwrap().len(), 1);
    let patch = changed["view"]["files"][0]["patch"].as_str().unwrap();
    assert!(patch.contains("+three"), "{patch}");
    // ATM-710 (Hakan): the page shows that round by its number, and asks for it so when compared against an
    // earlier one; it is the working copy then too, not a round "the review hasn't"
    let named = atm_lib::review_pages::review_page(&hakan, &cr, Some(2), None).unwrap();
    assert_eq!(
        (named["view"]["round"].clone(), named["view"]["now"].clone()),
        (json!(2), json!(true))
    );
    assert_eq!(named["view"]["files"], changed["view"]["files"]);
    let against = atm_lib::review_pages::review_page(&hakan, &cr, Some(2), Some(1)).unwrap();
    assert_eq!(
        (
            against["view"]["round"].clone(),
            against["view"]["against"].clone(),
            against["view"]["now"].clone()
        ),
        (json!(2), json!(1), json!(true))
    );
    let between = against["view"]["files"][0]["patch"].as_str().unwrap();
    assert!(between.contains("+three"), "{between}");
    // A round past that one is none
    assert!(
        atm_lib::review_pages::review_page(&hakan, &cr, Some(3), None)
            .unwrap_err()
            .to_string()
            .contains("has no round 3")
    );
    // Read again, nothing is stored
    assert_eq!(
        atm_lib::reviews::store(&hakan)
            .unwrap()
            .get_review(&cr)
            .unwrap()
            .rounds
            .len(),
        1
    );
    // A comment on what is shown: the checkpoint, round 2, then the comment on it
    let comment = atm_lib::review_requests::NewComment {
        path: Some("a.txt".into()),
        lines: Some((3, 3)),
        side: atm_lib::reviews::Side::default(),
        quote: Some("three".into()),
        round: Some(2),
        body: "Why three?".into(),
    };
    atm_lib::review_requests::add_comment(&hakan, &cr, comment.clone()).unwrap();
    let review = atm_lib::reviews::store(&hakan)
        .unwrap()
        .get_review(&cr)
        .unwrap();
    assert_eq!(review.rounds.len(), 2);
    assert_eq!(review.chains[0].anchor.round, 2);
    assert_eq!(page()["view"]["round"], 2);
    // Another comment with nothing changed since: no other checkpoint
    atm_lib::review_requests::add_comment(&hakan, &cr, comment).unwrap();
    assert_eq!(
        atm_lib::reviews::store(&hakan)
            .unwrap()
            .get_review(&cr)
            .unwrap()
            .rounds
            .len(),
        2
    );
    // A round named: that round, not now
    let first = atm_lib::review_pages::review_page(&hakan, &cr, Some(1), None).unwrap();
    assert_eq!(
        (first["view"]["round"].clone(), first["view"]["now"].clone()),
        (json!(1), json!(false))
    );
}

/// ATM-576: approving a live review approves the working copy as it is then, a checkpoint, which its task's change
/// is compared with when it is done
#[test]
fn approving_a_live_review_approves_the_working_copy_as_it_is() {
    let dir = dir();
    let (mut hakan, cr) = live(&dir.0);
    std::fs::write(dir.0.join("a.txt"), "one\ntwo\nthree\n").unwrap();
    atm_lib::reviews::submit(&mut hakan, &cr, Verdict::Approve, None, None).unwrap();
    let review = atm_lib::reviews::store(&hakan)
        .unwrap()
        .get_review(&cr)
        .unwrap();
    assert_eq!(
        (review.header.status, review.rounds.len()),
        (Status::Approved, 2)
    );
    let mut agent = reopen(&dir.0, "claude:1");
    assert!(
        atm_lib::reviews::apply_live_approval(&mut agent, "T-1")
            .unwrap()
            .is_some()
    );
    assert_eq!(task(&agent, "T-1")["review_state"], "approved");
}

#[test]
fn an_approved_live_review_covers_a_task_until_its_change_moves() {
    let dir = dir();
    git(&["init", "-q"], &dir.0);
    std::fs::write(dir.0.join("a.txt"), "one\n").unwrap();
    let project = project(&dir.0, "claude:1");
    git(&["add", "-A"], &dir.0);
    git(&["commit", "-q", "-m", "base"], &dir.0);
    let vcs = project.vcs().unwrap();
    std::fs::write(dir.0.join("a.txt"), "one\ntwo\n").unwrap();
    let checkpoint = atm_lib::reviews::rounds::from_working(&vcs, "HEAD", &"Hakan".into()).unwrap();
    // Hakan opens a live review on the agent's work (§3.5): he reviews it, the change is the agent's who started
    // T-1, so he decides it
    let mut hakan = reopen(&dir.0, "Hakan");
    let header = atm_lib::reviews::create(
        &mut hakan,
        NewReview {
            title: "Live".into(),
            summary: String::new(),
            tasks: vec!["T-1".into()],
            author: Some("Hakan".into()),
            reviewers: Vec::new(),
            status: Status::Live,
            first: checkpoint,
        },
    )
    .unwrap();
    assert_eq!(header.author, Some("claude:1".into()));
    assert_eq!(
        header.reviewers,
        vec![atm_lib::reviews::Actor::from("Hakan")]
    );
    // Live: the task stays in progress
    assert_eq!(task(&project, "T-1")["status"], "in_progress");
    atm_lib::reviews::submit(
        &mut hakan,
        &ReviewRef(header.reference.0.clone()),
        Verdict::Approve,
        None,
        None,
    )
    .unwrap();

    // Changed since: not covered
    std::fs::write(dir.0.join("a.txt"), "one\ntwo\nthree\n").unwrap();
    let mut agent = reopen(&dir.0, "claude:1");
    assert_eq!(
        atm_lib::reviews::apply_live_approval(&mut agent, "T-1").unwrap(),
        None
    );
    // As approved: covered, the task approved naming the review
    std::fs::write(dir.0.join("a.txt"), "one\ntwo\n").unwrap();
    assert_eq!(
        atm_lib::reviews::apply_live_approval(&mut agent, "T-1")
            .unwrap()
            .unwrap()
            .reference
            .0,
        "CR-1"
    );
    let t1 = task(&agent, "T-1");
    assert_eq!(t1["review_state"], "approved");
    assert!(
        t1["notes"]
            .to_string()
            .contains("approved in the live review CR-1")
    );
    atm_lib::changes::change_task_status(
        &mut agent,
        atm_lib::state::Command::Done,
        "T-1",
        &atm_lib::state::Options::default(),
    )
    .unwrap();
}

// ATM-594: a provisional key (`~ATM-12` as people read it) in reviews
