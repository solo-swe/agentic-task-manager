use super::{Reviews, Verdict, decides, is_author};
use crate::memory::MemoryStore;
use crate::model::{
    Actor, Anchor, Chain, CommentId, FileChange, FileStatus, Patch, ReviewHeader, ReviewRef,
    RoundKind, Status,
};
use crate::rules;
use crate::store::{NewReview, NewRound, ReviewFilter, ReviewStore};
use atm_common::Error;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
fn agent() -> Actor {
    Actor::from("claude:0f56a916")
}
fn hakan() -> Actor {
    Actor::from("Hakan Serce")
}
/// A round's file, changed.
fn changed(path: &str) -> FileChange {
    FileChange {
        path: path.into(),
        status: FileStatus::Modified,
        binary: false,
    }
}
fn round(by: &Actor) -> NewRound {
    NewRound {
        kind: RoundKind::Range {
            base: "a1".into(),
            head: "b2".into(),
        },
        submodule: None,
        files: ["a.rs", "x", "crates/atm-cli/src/process.rs"]
            .map(changed)
            .to_vec(),
        patch: Patch("diff --git a/x b/x\n".into()),
        by: by.clone(),
    }
}
fn new_review(status: Status) -> NewReview {
    NewReview {
        title: "Claim hold on Windows".into(),
        summary: "The watcher found its shell as pid 0.".into(),
        tasks: vec!["ATM-482".into()],
        author: Some(agent()),
        reviewers: vec![hakan()],
        status,
        first: round(&agent()),
    }
}
fn on(path: &str, round: u32) -> Anchor {
    Anchor {
        round,
        path: Some(path.into()),
        lines: Some((40, 52)),
        quote: Some("let shell = 0;".into()),
        ..Anchor::default()
    }
}
fn cr(n: u32) -> ReviewRef {
    ReviewRef::numbered(n)
}
fn status(store: &MemoryStore, n: u32) -> Status {
    store.get_review(&cr(n)).unwrap().header.status
}
fn someone() -> Actor {
    Actor::from("someone else")
}
/// Store a review of `status`, whatever the rules say about starting there: a review that came to it.
fn at(store: &mut MemoryStore, status: Status) -> ReviewRef {
    let header = store.create_review(new_review(Status::Waiting)).unwrap();
    store.set_status(&header.id, status, &hakan()).unwrap();
    header.reference
}
/// ATM-633 (docs/task-review-state-machine.md §5.4, §7 step 2): the review table, one row for every status and
/// event, each with the next status or the refusal's words; a pair without a row fails, so a new status or event
/// can't go untested. The author makes rounds and asks for review; the reviewer decides, closes and names tasks.
const TABLE_STATUSES: [Status; 6] = [
    Status::Draft,
    Status::Waiting,
    Status::Live,
    Status::ChangesRequested,
    Status::Approved,
    Status::Closed,
];
const TABLE_EVENTS: [&str; 9] = [
    "ask",
    "round",
    "checkpoint",
    "approve",
    "request",
    "comment",
    "close",
    "tasks",
    "summary",
];
/// A row of the review table: the status, the event, and the next status (`Ok(None)`: it stays) or words the
/// refusal says
type Row = (Status, &'static str, Result<Option<Status>, &'static str>);
/// The event a row names
fn table_event(name: &str) -> super::ReviewEvent<'static> {
    use super::ReviewEvent;
    match name {
        "ask" => ReviewEvent::AskForReview,
        "round" => ReviewEvent::Round,
        "checkpoint" => ReviewEvent::Checkpoint,
        "approve" | "request" | "comment" => ReviewEvent::Submit {
            verdict: match name {
                "approve" => Verdict::Approve,
                "request" => Verdict::RequestChanges,
                _ => Verdict::Comment,
            },
            overriding: None,
            also_open: &[],
        },
        "close" => ReviewEvent::Close,
        "tasks" => ReviewEvent::SetTasks,
        _ => ReviewEvent::EditSummary,
    }
}
/// Who does it: the author for its own events, the reviewer for the others
fn table_by(name: &str) -> Actor {
    match name {
        "ask" | "round" | "checkpoint" | "summary" => agent(),
        _ => hakan(),
    }
}
/// The review table's rows (docs/task-review-state-machine.md §3.4)
fn review_rows() -> Vec<Row> {
    use Status::{Approved, ChangesRequested, Closed, Draft, Live, Waiting};
    let mut rows: Vec<Row> = vec![
        (Draft, "ask", Ok(Some(Waiting))),
        (Waiting, "ask", Err("it can't be asked for again")),
        (Live, "ask", Err("it can't be asked for again")),
        (ChangesRequested, "ask", Err("it can't be asked for again")),
        (Approved, "ask", Err("it can't be asked for again")),
        (Closed, "ask", Err("it can't be asked for again")),
        (Draft, "round", Ok(None)),
        (Waiting, "round", Ok(None)),
        (Live, "round", Ok(Some(Waiting))),
        (ChangesRequested, "round", Ok(Some(Waiting))),
        (Draft, "checkpoint", Err("it can't take a checkpoint")),
        (Waiting, "checkpoint", Err("it can't take a checkpoint")),
        (Live, "checkpoint", Ok(None)),
        (
            ChangesRequested,
            "checkpoint",
            Err("it can't take a checkpoint"),
        ),
        (Approved, "checkpoint", Err("it can't take a checkpoint")),
        (Closed, "checkpoint", Err("it can't take a checkpoint")),
        (Draft, "approve", Err("it can't be approved")),
        (Waiting, "approve", Ok(Some(Approved))),
        (Live, "approve", Ok(Some(Approved))),
        (ChangesRequested, "approve", Ok(Some(Approved))),
        (Draft, "request", Err("it can't be sent back")),
        (Waiting, "request", Ok(Some(ChangesRequested))),
        (Live, "request", Err("it can't be sent back")),
        (ChangesRequested, "request", Err("it can't be sent back")),
    ];
    for status in [Draft, Waiting, Live, ChangesRequested] {
        rows.push((status, "comment", Ok(None)));
        rows.push((status, "close", Ok(Some(Closed))));
        rows.push((status, "tasks", Ok(None)));
        rows.push((status, "summary", Ok(None)));
    }
    let ended_words = [
        ("round", "it can't get another round"),
        ("approve", "it can't be submitted on"),
        ("request", "it can't be submitted on"),
        ("comment", "it can't be submitted on"),
        ("close", "it can't be closed"),
        ("tasks", "it can't change its tasks"),
        ("summary", "it can't change its summary"),
    ];
    for (what, words) in ended_words {
        rows.push((Approved, what, Err(words)));
        rows.push((Closed, what, Err(words)));
    }
    rows
}
fn is_refused(result: &Result<impl std::fmt::Debug, Error>) -> bool {
    matches!(result, Err(Error::Refused(_)))
}
fn is_not_now(result: &Result<impl std::fmt::Debug, Error>) -> bool {
    matches!(result, Err(Error::Failed { .. }))
}

mod comments;
mod deciding;
mod rounds;
mod table;
