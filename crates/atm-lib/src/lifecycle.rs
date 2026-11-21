//! The one machine for tasks and their reviews (ATM-633, docs/task-review-state-machine.md §5): a review event is the
//! review table's (`atm_review::rules::review_next`) composed with the task table's (`state::change_status`), checked
//! whole before anything is written. [`check`] says what an event does, or the first refusal, and writes nothing;
//! the review commands (`reviews::{submit, close, add_round, …}`) call it first, then write in §5.3's order, and
//! [`can`] asks it of every action a reader could take, for the review page's buttons (§5.2).

use std::collections::BTreeMap;

use atm_common::{Error, Kind, Result};
use atm_review::rules::{ReviewEvent, review_next};

use crate::project::Project;
use crate::reviews::{self, Actor, Review, ReviewRef, ReviewStore, Status, Verdict};
use crate::state::{Command, Options};

/// What a review event asks for, as the commands ask it (ATM-633). The review's own events, and with them what they
/// do to its tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event<'a> {
    /// A reviewer's decision, or a comment on the whole; `overriding` a person's override of who decides, with why;
    /// `also_open` comments open by then besides the review's own (a reviewer's drafts sent with it).
    Submit {
        verdict: Verdict,
        overriding: Option<&'a str>,
        also_open: &'a [String],
    },
    /// The author's next round: its tasks in progress go back in review.
    Round,
    /// Ended without approval; with `reject`, its tasks in review are rejected.
    Close { reject: bool },
}

/// What an event does, decided before anything is written: the review's next status (none: it stays) and each task
/// it moves, by the command the task table takes it through.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Outcome {
    pub review: Option<Status>,
    pub tasks: Vec<(String, Command)>,
    /// Every task the review names, by ID, as it meant each (ATM-630): what a round puts back in review.
    pub named: Vec<String>,
}

/// What a decision's note and reason say of the review: the round it was made on, and its open comments.
#[must_use]
pub fn decision_options(review: &Review) -> Options {
    let round = review.rounds.last().map_or(1, |r| r.number);
    let open = review.open_comments().count();
    Options {
        note: Some(format!("in {} (round {round})", review.header.reference)),
        reason: Some(format!(
            "{} round {round}: {open} open comment{}",
            review.header.reference,
            if open == 1 { "" } else { "s" }
        )),
        ..Options::default()
    }
}

/// What `event` by `by` does to `review` and its tasks (§5.4's `check`): the review table's answer, then every task it
/// names that the event moves, checked through the task table on a copy. The first refusal, with its words and exit
/// code, as the commands have always said them; nothing is written.
///
/// # Errors
/// What the review's rules refuse, a task the review names that can't be found ("can't reach its tasks"), or a task
/// the task table refuses ("the decision can't reach …").
pub fn check(project: &Project, review: &Review, event: &Event, by: &Actor) -> Result<Outcome> {
    let reference = &review.header.reference;
    let review_event = match *event {
        Event::Submit {
            verdict,
            overriding,
            also_open,
        } => ReviewEvent::Submit {
            verdict,
            overriding,
            also_open,
        },
        Event::Round => ReviewEvent::Round,
        Event::Close { .. } => ReviewEvent::Close,
    };
    // The tasks found first (ATM-594): an event that moves them is never recorded without reaching them. Closing
    // without rejecting moves none, so it needs none found
    let ids = if let Event::Close { reject: false } = event {
        Vec::new()
    } else {
        let (what, kept) = match event {
            Event::Round => ("round", "added"),
            Event::Close { .. } => ("closing", "recorded"),
            Event::Submit { .. } => ("decision", "recorded"),
        };
        let store = reviews::store(project)?;
        reviews::named(project, &store, &review.header).map_err(|e| {
            Error::failed(format!(
                "{reference}'s {what} can't reach its tasks, so it isn't {kept}: {e}"
            ))
        })?
    };
    let next = review_next(review, &review_event, by)?;
    let command = match *event {
        Event::Submit { verdict, .. } => match verdict {
            Verdict::Approve => Some(Command::Approve),
            Verdict::RequestChanges => Some(Command::RequestChanges),
            Verdict::Comment => None,
        },
        Event::Close { reject: true } => Some(Command::Reject),
        Event::Close { reject: false } | Event::Round => None,
    };
    let mut tasks = Vec::new();
    if let Some(command) = command {
        let options = match event {
            Event::Close { .. } => Options {
                reason: Some(format!("{reference} closed")),
                ..Options::default()
            },
            _ => decision_options(review),
        };
        // Every task the decision changes checked before the review records it (ATM-670): all or none
        reviews::check_decision(project, &ids, command, &options)?;
        for id in &ids {
            if reviews::kind_of(project, id)?.is_some_and(|(kind, _)| kind == Kind::Review) {
                tasks.push((id.clone(), command));
            }
        }
    }
    Ok(Outcome {
        review: next.status,
        tasks,
        named: ids,
    })
}

/// The actions the review page offers (§5.2): each one's [`check`] for `by`, `Ok` when it may, else the refusal whose
/// words the page shows beside it. Writes nothing.
///
/// # Errors
/// The review can't be read.
pub fn can(
    project: &Project,
    reference: &ReviewRef,
    by: &Actor,
    drafts: &[String],
) -> Result<BTreeMap<&'static str, Result<()>>> {
    let review = reviews::store(project)?.get_review(reference)?;
    let submit = |verdict| Event::Submit {
        verdict,
        overriding: None,
        also_open: drafts,
    };
    let actions = [
        ("approve", submit(Verdict::Approve)),
        ("request_changes", submit(Verdict::RequestChanges)),
        ("comment", submit(Verdict::Comment)),
        ("round", Event::Round),
        ("close", Event::Close { reject: false }),
    ];
    Ok(actions
        .into_iter()
        .map(|(name, event)| (name, check(project, &review, &event, by).map(|_| ())))
        .collect())
}
