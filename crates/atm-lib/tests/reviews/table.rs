//! The composed table (ATM-713, docs/task-review-state-machine.md §4, §7 step 2): a review and its task in each state
//! they can be in together, each built by the events that lead there, and every review event on each. A row says what
//! `lifecycle::check` answers, and the task's state once the command has run: the review's next status, or the
//! refusal's words. A scene and event without a row fails the test, so a new state or event can't go untested. The
//! three bugs the machine was built for are rows too (`BUGS`).

use atm_lib::lifecycle::{Event, check};
use atm_lib::reviews::{ReviewRef, ReviewStore, Status, Verdict};
use atm_lib::state::{Command, Options};

use crate::fixtures::{dir, keyed, lapse, open_review, project, renumber, reopen, round, task};

/// A review of the agent's task T-1, and where the two stand, each reached by what leads there
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scene {
    /// A draft: its task as it was, in progress
    Draft,
    /// Waiting on Hakan, its task in review, waiting
    Waiting,
    /// Sent back by Hakan, its task in review, changes requested
    SentBack,
    /// Sent back, and the agent took its task back to fix it
    TakenBack,
    /// Taken back, then the agent's claim lapsed: the task back in to do
    Lapsed,
    /// Approved by Hakan, its task in review, approved
    Approved,
    /// Closed by Hakan, its task as it was
    Closed,
}

const SCENES: [Scene; 7] = [
    Scene::Draft,
    Scene::Waiting,
    Scene::SentBack,
    Scene::TakenBack,
    Scene::Lapsed,
    Scene::Approved,
    Scene::Closed,
];

/// What happens to the review: its author's round, or Hakan's decision, comment or close
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Happens {
    Round,
    Approve,
    RequestChanges,
    Comment,
    Close,
    CloseRejecting,
}

const EVENTS: [Happens; 6] = [
    Happens::Round,
    Happens::Approve,
    Happens::RequestChanges,
    Happens::Comment,
    Happens::Close,
    Happens::CloseRejecting,
];

/// What a row expects: the review's next status (none: it stays) and the task's state after it, as `status` or
/// `status/review_state`; or the refusal, by words of it, with nothing written
#[derive(Debug, Clone, Copy)]
enum Expect {
    Moves(Option<Status>, &'static str),
    Refused(&'static str),
}

use Expect::{Moves, Refused};
use Happens::{Approve, Close, CloseRejecting, Comment, RequestChanges, Round};
use Scene::{Approved, Closed, Draft, Lapsed, SentBack, TakenBack, Waiting};

/// The table: one row per scene and event
const ROWS: &[(Scene, Happens, Expect)] = &[
    // A draft leaves its task as it was, whatever is done to it
    (Draft, Round, Moves(None, "in_progress")),
    (Draft, Approve, Refused("it can't be approved")),
    (Draft, RequestChanges, Refused("it can't be sent back")),
    (Draft, Comment, Moves(None, "in_progress")),
    (Draft, Close, Moves(Some(Status::Closed), "in_progress")),
    (
        Draft,
        CloseRejecting,
        Moves(Some(Status::Closed), "in_progress"),
    ),
    // Waiting: the decisions are its task's
    (Waiting, Round, Moves(None, "in_review/waiting")),
    (
        Waiting,
        Approve,
        Moves(Some(Status::Approved), "in_review/approved"),
    ),
    (
        Waiting,
        RequestChanges,
        Moves(
            Some(Status::ChangesRequested),
            "in_review/changes_requested",
        ),
    ),
    (Waiting, Comment, Moves(None, "in_review/waiting")),
    (
        Waiting,
        Close,
        Moves(Some(Status::Closed), "in_review/waiting"),
    ),
    (
        Waiting,
        CloseRejecting,
        Moves(Some(Status::Closed), "in_review/rejected"),
    ),
    // Sent back, its task not taken back yet: a round asks for review again
    (
        SentBack,
        Round,
        Moves(Some(Status::Waiting), "in_review/waiting"),
    ),
    (
        SentBack,
        Approve,
        Moves(Some(Status::Approved), "in_review/approved"),
    ),
    (SentBack, RequestChanges, Refused("it can't be sent back")),
    (
        SentBack,
        Comment,
        Moves(None, "in_review/changes_requested"),
    ),
    (
        SentBack,
        Close,
        Moves(Some(Status::Closed), "in_review/changes_requested"),
    ),
    (
        SentBack,
        CloseRejecting,
        Moves(Some(Status::Closed), "in_review/rejected"),
    ),
    // Taken back to fix: a round puts it in review again; a decision meanwhile waits for the round
    (
        TakenBack,
        Round,
        Moves(Some(Status::Waiting), "in_review/waiting"),
    ),
    (
        TakenBack,
        Approve,
        Moves(Some(Status::Approved), "in_progress"),
    ),
    (TakenBack, RequestChanges, Refused("it can't be sent back")),
    (TakenBack, Comment, Moves(None, "in_progress")),
    (TakenBack, Close, Moves(Some(Status::Closed), "in_progress")),
    (
        TakenBack,
        CloseRejecting,
        Moves(Some(Status::Closed), "in_progress"),
    ),
    // ATM-632's class (the design's hole 1): lapsed back to to do, a round takes it back before it goes in review
    (
        Lapsed,
        Round,
        Moves(Some(Status::Waiting), "in_review/waiting"),
    ),
    (Lapsed, Approve, Moves(Some(Status::Approved), "todo")),
    (Lapsed, RequestChanges, Refused("it can't be sent back")),
    (Lapsed, Comment, Moves(None, "todo")),
    (Lapsed, Close, Moves(Some(Status::Closed), "todo")),
    (Lapsed, CloseRejecting, Moves(Some(Status::Closed), "todo")),
    // Ended: nothing more
    (Approved, Round, Refused("it can't get another round")),
    (Approved, Approve, Refused("it can't be submitted on")),
    (
        Approved,
        RequestChanges,
        Refused("it can't be submitted on"),
    ),
    (Approved, Comment, Refused("it can't be submitted on")),
    (Approved, Close, Refused("it can't be closed")),
    (Approved, CloseRejecting, Refused("it can't be closed")),
    (Closed, Round, Refused("it can't get another round")),
    (Closed, Approve, Refused("it can't be submitted on")),
    (Closed, RequestChanges, Refused("it can't be submitted on")),
    (Closed, Comment, Refused("it can't be submitted on")),
    (Closed, Close, Refused("it can't be closed")),
    (Closed, CloseRejecting, Refused("it can't be closed")),
];

/// The scene, in a project of its own: the review's reference
fn build(scene: Scene, root: &std::path::Path) -> ReviewRef {
    let mut agent = project(root, "claude:1");
    if scene == Draft {
        let header = atm_lib::reviews::create(
            &mut agent,
            atm_lib::reviews::NewReview {
                title: "The change".into(),
                summary: String::new(),
                tasks: vec!["T-1".into()],
                author: Some("claude:1".into()),
                reviewers: vec!["Hakan".into()],
                status: Status::Draft,
                first: round(),
            },
        )
        .unwrap();
        return header.reference;
    }
    let reference = open_review(&mut agent, &["T-1"]).reference;
    let mut hakan = reopen(root, "Hakan");
    // Without words: a decision's words are an open comment, which a later approval would have to see resolved
    let decide = |hakan: &mut atm_lib::Project, verdict| {
        atm_lib::reviews::submit(hakan, &reference, verdict, None, None).unwrap();
    };
    match scene {
        Draft | Waiting => {}
        SentBack | TakenBack | Lapsed => decide(&mut hakan, Verdict::RequestChanges),
        Approved => decide(&mut hakan, Verdict::Approve),
        Closed => atm_lib::reviews::close(&mut hakan, &reference, "Not now.", false).unwrap(),
    }
    if matches!(scene, TakenBack | Lapsed) {
        let mut agent = reopen(root, "claude:1");
        atm_lib::changes::change_task_status(
            &mut agent,
            Command::Start,
            "T-1",
            &Options::default(),
        )
        .unwrap();
        if scene == Lapsed {
            lapse(&mut agent, "T-1");
        }
    }
    reference
}

/// Who makes it happen: the author's round, Hakan's decisions
fn by(happens: Happens) -> &'static str {
    if happens == Round {
        "claude:1"
    } else {
        "Hakan"
    }
}

/// The event as `lifecycle::check` takes it
fn event(happens: Happens) -> Event<'static> {
    let submit = |verdict| Event::Submit {
        verdict,
        overriding: None,
        also_open: &[],
    };
    match happens {
        Round => Event::Round,
        Approve => submit(Verdict::Approve),
        RequestChanges => submit(Verdict::RequestChanges),
        Comment => submit(Verdict::Comment),
        Close => Event::Close { reject: false },
        CloseRejecting => Event::Close { reject: true },
    }
}

/// The command that makes it happen, as the CLI and the page run it
fn apply(
    happens: Happens,
    root: &std::path::Path,
    reference: &ReviewRef,
) -> atm_common::Result<()> {
    let mut project = reopen(root, by(happens));
    let submit = |project: &mut atm_lib::Project, verdict| {
        atm_lib::reviews::submit(project, reference, verdict, Some("Words."), None)
    };
    match happens {
        Round => atm_lib::reviews::add_round(&mut project, reference, round()).map(|_| ()),
        Approve => submit(&mut project, Verdict::Approve),
        RequestChanges => submit(&mut project, Verdict::RequestChanges),
        Comment => submit(&mut project, Verdict::Comment),
        Close => atm_lib::reviews::close(&mut project, reference, "Done.", false),
        CloseRejecting => atm_lib::reviews::close(&mut project, reference, "Done.", true),
    }
}

/// Where T-1 stands: its status, and its review's state when it is in review
fn standing(root: &std::path::Path) -> String {
    let t = task(&reopen(root, "claude:1"), "T-1");
    let status = t["status"].as_str().unwrap_or_default();
    match t["review_state"].as_str() {
        Some(state) if status == "in_review" => format!("{status}/{state}"),
        _ => status.to_string(),
    }
}

fn status_of(root: &std::path::Path, reference: &ReviewRef) -> Status {
    let project = reopen(root, "Hakan");
    atm_lib::reviews::store(&project)
        .unwrap()
        .get_review(reference)
        .unwrap()
        .header
        .status
}

#[test]
fn every_scene_and_event_has_its_row_once() {
    for scene in SCENES {
        for happens in EVENTS {
            let rows = ROWS
                .iter()
                .filter(|(s, h, _)| *s == scene && *h == happens)
                .count();
            assert_eq!(rows, 1, "{scene:?} × {happens:?}: {rows} rows");
        }
    }
    assert_eq!(ROWS.len(), SCENES.len() * EVENTS.len());
}

/// What `row` gives, against what it expects: each difference, in words
fn differences(scene: Scene, happens: Happens, expect: Expect) -> Vec<String> {
    let dir = dir();
    let reference = build(scene, &dir.0);
    let before = (status_of(&dir.0, &reference), standing(&dir.0));
    let asked = reopen(&dir.0, by(happens));
    let review = atm_lib::reviews::store(&asked)
        .unwrap()
        .get_review(&reference)
        .unwrap();
    let checked = check(&asked, &review, &event(happens), &by(happens).into());
    let applied = apply(happens, &dir.0, &reference);
    let after = (status_of(&dir.0, &reference), standing(&dir.0));
    let mut found = Vec::new();
    match expect {
        Moves(next, task_after) => {
            match &checked {
                Ok(outcome) if outcome.review != next => {
                    found.push(format!("check says {:?}, not {next:?}", outcome.review));
                }
                Err(e) => found.push(format!("check refused: {e}")),
                Ok(_) => {}
            }
            if let Err(e) = &applied {
                found.push(format!("the command refused: {e}"));
            }
            let wanted = (next.unwrap_or(before.0), task_after.to_string());
            if after != wanted {
                found.push(format!("left {after:?}, not {wanted:?}"));
            }
        }
        Refused(words) => {
            match &checked {
                Ok(_) => found.push("check allowed it".into()),
                Err(e) if !e.to_string().contains(words) => {
                    found.push(format!("check refused: {e}"));
                }
                Err(_) => {}
            }
            match &applied {
                Ok(()) => found.push("the command ran".into()),
                Err(e) if !e.to_string().contains(words) => {
                    found.push(format!("the command refused: {e}"));
                }
                Err(_) => {}
            }
            if after != before {
                found.push(format!("written though refused: {before:?} → {after:?}"));
            }
        }
    }
    found
}

#[test]
fn each_row_is_what_check_answers_and_what_the_command_leaves() {
    let wrong: Vec<String> = ROWS
        .iter()
        .flat_map(|&(scene, happens, expect)| {
            differences(scene, happens, expect)
                .into_iter()
                .map(move |d| format!("{scene:?} × {happens:?}: {d}"))
        })
        .collect();
    assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
}

/// The two other bugs the machine was built for, each a row of its own (ATM-632's is `(Lapsed, Round)` above)
#[test]
fn the_bugs_are_rows_too() {
    // ATM-630: a review names its task by the key it was written with; a merge moved the task's number since (its old
    // key an alias): the approval still reaches it
    let dir = dir();
    let mut agent = keyed(&dir.0, "claude:1");
    let reference = open_review(&mut agent, &["ATM-11"]).reference;
    let final_task = "0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8011";
    renumber(&mut agent, final_task, 14);
    let mut hakan = reopen(&dir.0, "Hakan");
    atm_lib::reviews::submit(&mut hakan, &reference, Verdict::Approve, None, None).unwrap();
    assert_eq!(
        task(&hakan, final_task)["review_state"],
        "approved",
        "ATM-630"
    );
    assert_eq!(task(&hakan, final_task)["number"], 14, "ATM-630");

    // ATM-631: the author's task lapsed back to to do before its review was opened: taken back and in review with it,
    // never a review waiting beside a task in to do
    let dir = crate::fixtures::dir();
    let mut agent = project(&dir.0, "claude:1");
    lapse_now(&mut agent);
    open_review(&mut agent, &["T-1"]);
    assert_eq!(standing(&dir.0), "in_review/waiting", "ATM-631");
}

/// T-1, in progress with the agent's claim, lapses: back in to do
fn lapse_now(agent: &mut atm_lib::Project) {
    atm_lib::changes::change_task_status(
        agent,
        Command::Fail,
        "T-1",
        &Options {
            reason: Some("to start it with a claim".into()),
            ..Options::default()
        },
    )
    .unwrap();
    atm_lib::changes::change_task_status(agent, Command::Start, "T-1", &Options::default())
        .unwrap();
    lapse(agent, "T-1");
}
