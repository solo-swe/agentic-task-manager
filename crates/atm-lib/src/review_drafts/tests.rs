// A reviewer's drafts (ATM-612), on a project in git with a review, kept in a test's folder: what a draft takes and
// refuses, where it is kept and for whom, and sending them with a decision (each kind) or on their own.

use std::path::Path;

use serde_json::json;

use super::{DraftUpdate, Drafts, NewDraft, Place};
use crate::project::Project;
use crate::review_requests::{Decision, DecisionKind, submit_decision_with};
use crate::reviews::{
    self, Anchor, CommentId, NewReview, NewRound, Patch, ReviewRef, ReviewStore, Reviews,
    RoundKind, Side, Status,
};

struct Dir(std::path::PathBuf);
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn dir() -> Dir {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let count = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("atm-lib-drafts-{}-{count}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    // Without Windows' `\\?\` prefix, as atm and git name the folder
    Dir(atm_common::pathtext::canonical(&path).unwrap())
}

fn round() -> NewRound {
    NewRound {
        kind: RoundKind::Range {
            base: "a".into(),
            head: "b".into(),
        },
        submodule: None,
        // The file the drafts are on: a comment is on one of its round's files (ATM-658)
        files: vec![atm_review::model::FileChange {
            path: "src/a.rs".into(),
            status: reviews::FileStatus::Modified,
            binary: false,
        }],
        patch: Patch(String::new()),
        by: "claude:1".into(),
    }
}

/// A project in git (in `root/repo`) with T-1 in progress and CR-1 on it, `status`, by an agent, reviewed by Hakan;
/// opened as `actor`. The drafts are kept in `root/drafts`.
struct Setup {
    _dir: Dir,
    repo: std::path::PathBuf,
    drafts: Drafts,
}

fn setup(status: Status) -> Setup {
    let dir = dir();
    let repo = dir.0.join("repo");
    std::fs::create_dir_all(repo.join(".agentic-task-manager")).unwrap();
    assert!(atm_db::git::git(&["init", "-q"], &repo).is_some());
    let tasks = json!({
        "meta": { "project": "Drafts", "review": "blocker" },
        "tasks": [{ "id": "T-1", "name": "One", "type": "feature", "status": "in_progress", "started_by": "claude:1" }]
    });
    std::fs::write(
        repo.join(".agentic-task-manager/tasks.json"),
        tasks.to_string(),
    )
    .unwrap();
    let mut agent = open(&repo, "claude:1");
    reviews::create(
        &mut agent,
        NewReview {
            title: "One".into(),
            summary: String::new(),
            tasks: vec!["T-1".into()],
            author: Some("claude:1".into()),
            reviewers: vec!["Hakan".into()],
            status: if status == Status::Draft {
                Status::Draft
            } else {
                Status::Waiting
            },
            first: round(),
        },
    )
    .unwrap();
    let mut store = reviews::store(&agent).unwrap();
    let id = store.get_review(&cr()).unwrap().header.id;
    if !matches!(status, Status::Draft | Status::Waiting) {
        store.set_status(&id, status, &"Hakan".into()).unwrap();
    }
    let drafts = Drafts::at(&dir.0.join("drafts"));
    Setup {
        _dir: dir,
        repo,
        drafts,
    }
}

fn open(root: &Path, actor: &str) -> Project {
    Project::open(root).unwrap().with_actor(Some(actor.into()))
}
fn cr() -> ReviewRef {
    ReviewRef("CR-1".into())
}
fn on_lines(body: &str) -> NewDraft {
    NewDraft {
        path: Some("src/a.rs".into()),
        lines: Some((3, 4)),
        body: body.into(),
        ..NewDraft::default()
    }
}
fn reply(to: &str, body: &str) -> NewDraft {
    NewDraft {
        reply_to: Some(to.into()),
        body: body.into(),
        ..NewDraft::default()
    }
}
/// A comment made on CR-1 now, by `by`: its ID.
fn comment(s: &Setup, by: &str) -> CommentId {
    let project = open(&s.repo, by);
    let mut store = reviews::store(&project).unwrap();
    Reviews::new(&mut store)
        .comment(
            &cr(),
            Anchor {
                round: 1,
                ..Anchor::default()
            },
            "Why?",
            &by.into(),
        )
        .unwrap()
}
fn review(s: &Setup) -> reviews::Review {
    reviews::store(&open(&s.repo, "Hakan"))
        .unwrap()
        .get_review(&cr())
        .unwrap()
}
fn decide(s: &Setup, decision: DecisionKind, body: Option<&str>) -> atm_common::Result<Status> {
    submit_decision_with(
        &mut open(&s.repo, "Hakan"),
        &cr(),
        &Decision {
            decision,
            body: body.map(String::from),
            override_rules: None,
        },
        &s.drafts,
    )
    .map(|h| h.status)
}

#[test]
fn a_draft_is_kept_outside_the_review_numbered_and_listed_in_order() {
    let s = setup(Status::Waiting);
    let hakan = open(&s.repo, "Hakan");
    let c1 = comment(&s, "claude:1");
    let first = super::add_draft_in(&hakan, &cr(), on_lines("Rename this."), &s.drafts).unwrap();
    let second = super::add_draft_in(&hakan, &cr(), reply(&c1.0, "Because."), &s.drafts).unwrap();
    assert_eq!((first.id.as_str(), second.id.as_str()), ("d1", "d2"));
    // Anchored on the round it was written on, the latest when none is named
    assert_eq!(
        first.place,
        Place::Comment {
            path: Some("src/a.rs".into()),
            lines: Some((3, 4)),
            side: Side::New,
            quote: None,
            round: Some(1),
        }
    );
    assert_eq!(second.place, Place::Reply { to: c1.0.clone() });
    let listed = super::list_drafts_in(&hakan, &cr(), &s.drafts).unwrap();
    assert_eq!(listed, vec![first, second]);
    // Nothing in the review: no comment, no file in its folder
    assert_eq!(review(&s).comments().count(), 1);
    let folder = s.repo.join(".agentic-task-manager/reviews");
    let found = std::fs::read_dir(folder).unwrap().flatten().any(|review| {
        std::fs::read_dir(review.path())
            .unwrap()
            .flatten()
            .any(|f| f.file_name().to_string_lossy().contains("draft"))
    });
    assert!(!found);
}

#[test]
fn drafts_are_one_persons_and_never_an_agents() {
    let s = setup(Status::Waiting);
    super::add_draft_in(&open(&s.repo, "Hakan"), &cr(), on_lines("Mine."), &s.drafts).unwrap();
    // Another person on the same review sees none of them
    assert_eq!(
        super::list_drafts_in(&open(&s.repo, "Ana"), &cr(), &s.drafts)
            .unwrap()
            .len(),
        0
    );
    // An agent drafts nothing, and reads none
    let agent = open(&s.repo, "claude:1");
    for result in [
        super::add_draft_in(&agent, &cr(), on_lines("x"), &s.drafts).map(|_| ()),
        super::list_drafts_in(&agent, &cr(), &s.drafts).map(|_| ()),
    ] {
        assert_eq!(
            result.unwrap_err().to_string(),
            "a person drafts a review's comments: an agent comments at once (atm review comment)"
        );
    }
}

#[test]
fn a_draft_refuses_what_a_comment_would_and_a_reply_with_a_place_of_its_own() {
    let s = setup(Status::Waiting);
    let hakan = open(&s.repo, "Hakan");
    let add = |new: NewDraft| {
        super::add_draft_in(&hakan, &cr(), new, &s.drafts)
            .unwrap_err()
            .to_string()
    };
    assert_eq!(add(on_lines("  ")), "the draft is empty");
    assert_eq!(
        add(NewDraft {
            round: Some(2),
            ..on_lines("x")
        }),
        "CR-1 has no round 2"
    );
    assert_eq!(add(reply("c9", "x")), "CR-1 has no comment c9");
    assert_eq!(
        add(NewDraft {
            reply_to: Some("c1".into()),
            ..on_lines("x")
        }),
        "a reply is placed by the comment it answers: give no file, lines, quote or round"
    );
    assert!(
        super::add_draft_in(&hakan, &ReviewRef("CR-9".into()), on_lines("x"), &s.drafts).is_err()
    );
    // Nothing kept from any of them
    assert_eq!(
        super::list_drafts_in(&hakan, &cr(), &s.drafts)
            .unwrap()
            .len(),
        0
    );
    // An ended review takes none
    for ended in [Status::Approved, Status::Closed] {
        let s = setup(ended);
        let message = super::add_draft_in(&open(&s.repo, "Hakan"), &cr(), on_lines("x"), &s.drafts)
            .unwrap_err()
            .to_string();
        assert_eq!(
            message,
            format!("CR-1 is {}: it can't take drafts", ended.name())
        );
    }
}

#[test]
fn a_live_review_drafts_what_is_there_now_for_when_it_is_sent() {
    let s = setup(Status::Live);
    let draft =
        super::add_draft_in(&open(&s.repo, "Hakan"), &cr(), on_lines("x"), &s.drafts).unwrap();
    assert!(matches!(draft.place, Place::Comment { round: None, .. }));
    // An earlier round named stays it
    let earlier = NewDraft {
        round: Some(1),
        ..on_lines("y")
    };
    let draft = super::add_draft_in(&open(&s.repo, "Hakan"), &cr(), earlier, &s.drafts).unwrap();
    assert!(matches!(draft.place, Place::Comment { round: Some(1), .. }));
}

#[test]
fn a_draft_is_edited_and_removed_and_its_number_never_given_again() {
    let s = setup(Status::Waiting);
    let hakan = open(&s.repo, "Hakan");
    let d1 = super::add_draft_in(&hakan, &cr(), on_lines("One."), &s.drafts).unwrap();
    let edit = |id: &str, body: &str| {
        super::update_draft_in(
            &hakan,
            &cr(),
            id,
            &DraftUpdate { body: body.into() },
            &s.drafts,
        )
    };
    assert_eq!(edit(&d1.id, "One, better.").unwrap().body, "One, better.");
    assert_eq!(
        edit(&d1.id, " ").unwrap_err().to_string(),
        "the draft is empty"
    );
    assert_eq!(
        edit("d7", "x").unwrap_err().to_string(),
        "you have no draft d7 on CR-1"
    );
    super::remove_draft_in(&hakan, &cr(), &d1.id, &s.drafts).unwrap();
    assert_eq!(
        super::remove_draft_in(&hakan, &cr(), &d1.id, &s.drafts)
            .unwrap_err()
            .to_string(),
        "you have no draft d1 on CR-1"
    );
    // None left, the next is still d2
    let next = super::add_draft_in(&hakan, &cr(), on_lines("Two."), &s.drafts).unwrap();
    assert_eq!(next.id, "d2");
}

#[test]
fn drafts_are_sent_with_each_decision_in_order_and_are_gone_after() {
    for (decision, body, status) in [
        (
            DecisionKind::RequestChanges,
            Some("Two things."),
            Status::ChangesRequested,
        ),
        (
            DecisionKind::Comment,
            Some("Some thoughts."),
            Status::Waiting,
        ),
        // No words of its own: only the drafts sent (Publish comments)
        (DecisionKind::Comment, None, Status::Waiting),
    ] {
        let s = setup(Status::Waiting);
        let hakan = open(&s.repo, "Hakan");
        let c1 = comment(&s, "claude:1");
        super::add_draft_in(&hakan, &cr(), on_lines("Rename this."), &s.drafts).unwrap();
        super::add_draft_in(&hakan, &cr(), reply(&c1.0, "Because."), &s.drafts).unwrap();
        assert_eq!(decide(&s, decision, body).unwrap(), status, "{decision:?}");
        let after = review(&s);
        let bodies: Vec<&str> = after.comments().map(|c| c.body.as_str()).collect();
        let mut expected = vec!["Why?", "Because.", "Rename this."];
        expected.extend(body);
        let mut sorted = bodies.clone();
        sorted.sort_unstable();
        expected.sort_unstable();
        assert_eq!(sorted, expected, "{decision:?}");
        // Sent as the user, the drafted comment on its file's lines
        let drafted = after.comments().find(|c| c.body == "Rename this.").unwrap();
        assert_eq!(drafted.author.0, "Hakan");
        let chain = after
            .chains
            .iter()
            .find(|ch| ch.comments.iter().any(|c| c.body == "Rename this."))
            .unwrap();
        assert_eq!(
            (chain.anchor.path.as_deref(), chain.anchor.lines),
            (Some("src/a.rs"), Some((3, 4)))
        );
        assert_eq!(
            super::list_drafts_in(&hakan, &cr(), &s.drafts)
                .unwrap()
                .len(),
            0
        );
    }
}

#[test]
fn an_approval_with_drafts_that_would_be_open_is_refused_before_any_is_sent() {
    let s = setup(Status::Waiting);
    let hakan = open(&s.repo, "Hakan");
    super::add_draft_in(&hakan, &cr(), on_lines("Rename this."), &s.drafts).unwrap();
    assert_eq!(
        decide(&s, DecisionKind::Approve, None)
            .unwrap_err()
            .to_string(),
        "CR-1 can't be approved while comments are open: d1 (your draft)"
    );
    assert_eq!(review(&s).comments().count(), 0);
    assert_eq!(
        super::list_drafts_in(&hakan, &cr(), &s.drafts)
            .unwrap()
            .len(),
        1
    );
    // A reply drafted doesn't stand in the way: sent, then approved
    let s = setup(Status::Waiting);
    let c1 = comment(&s, "claude:1");
    let project = open(&s.repo, "Hakan");
    let mut store = reviews::store(&project).unwrap();
    Reviews::new(&mut store)
        .resolve(&cr(), &c1, true, &"Hakan".into())
        .unwrap();
    super::add_draft_in(&project, &cr(), reply(&c1.0, "Thanks."), &s.drafts).unwrap();
    assert_eq!(
        decide(&s, DecisionKind::Approve, None).unwrap(),
        Status::Approved
    );
    assert!(review(&s).comments().any(|c| c.body == "Thanks."));
}

#[test]
fn a_decision_refused_sends_no_draft_and_a_comment_needs_words_or_drafts() {
    // Sent back already: not sent back again, and the drafts stay
    let s = setup(Status::ChangesRequested);
    super::add_draft_in(&open(&s.repo, "Hakan"), &cr(), on_lines("x"), &s.drafts).unwrap();
    assert!(decide(&s, DecisionKind::RequestChanges, Some("Again.")).is_err());
    assert_eq!(review(&s).comments().count(), 0);
    // There, Publish comments sends them and leaves it sent back
    assert_eq!(
        decide(&s, DecisionKind::Comment, None).unwrap(),
        Status::ChangesRequested
    );
    assert_eq!(review(&s).comments().count(), 1);
    // Nothing to say and nothing drafted
    assert_eq!(
        decide(&s, DecisionKind::Comment, None)
            .unwrap_err()
            .to_string(),
        "a comment on the review needs its text, or drafts to send"
    );
}

#[test]
fn a_draft_that_cant_be_sent_stays_with_those_after_it() {
    let s = setup(Status::Waiting);
    let hakan = open(&s.repo, "Hakan");
    super::add_draft_in(&hakan, &cr(), on_lines("First."), &s.drafts).unwrap();
    super::add_draft_in(&hakan, &cr(), on_lines("Second."), &s.drafts).unwrap();
    // The review ends before they are sent (closed by its author)
    let agent = open(&s.repo, "claude:1");
    let mut store = reviews::store(&agent).unwrap();
    Reviews::new(&mut store)
        .close(&cr(), "Wrong turn.", &"claude:1".into())
        .unwrap();
    let message = super::send_drafts_in(&hakan, &cr(), &s.drafts)
        .unwrap_err()
        .to_string();
    assert!(
        message.starts_with("your draft d1 wasn't sent (0 sent before it): CR-1 is closed"),
        "{message}"
    );
    assert_eq!(
        super::list_drafts_in(&hakan, &cr(), &s.drafts)
            .unwrap()
            .len(),
        2
    );
}
