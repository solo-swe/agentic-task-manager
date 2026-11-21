//! The one machine for tasks and their reviews (ATM-633, docs/task-review-state-machine.md §5): a review event is the
//! review table's (`atm_review::rules::review_next`) composed with the task table's (`state::change_status`), checked
//! whole before anything is written. [`check`] says what an event does, or the first refusal, and writes nothing;
//! the review commands (`reviews::{submit, close, add_round, …}`) call it first, then write in §5.3's order, and
//! [`can`] asks it of every action a reader could take, for the review page's buttons (§5.2).

use std::collections::BTreeMap;

use atm_common::edits::is_agent;
use atm_common::{Error, Kind, Result};
use atm_review::rules::{ReviewEvent, review_next};
use serde_json::Value;

use crate::project::Project;
use crate::reviews::{self, Actor, Opening, Review, ReviewRef, ReviewStore, Status, Verdict};
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
    /// The author's own tasks whose claim lapsed (ATM-632, the design's hole 1): started again before a round puts
    /// them back in review, so the round is never kept beside a task left in to do.
    pub take_back: Vec<String>,
}

/// Whether each task's `moves`, in order, are allowed by the task table, tried on a copy (ATM-670, ATM-712): the
/// first refusal, worded by `refused` with the task's key; nothing is written.
fn check_moves(
    project: &Project,
    moves: &[(String, Vec<Command>)],
    options: &Options,
    refused: &dyn Fn(&str, &Error) -> Error,
) -> Result<()> {
    if moves.is_empty() {
        return Ok(());
    }
    let vocab = project.store().vocabulary()?;
    let tasks = project.store().list_tasks()?;
    let stamp = atm_common::time::now_iso();
    for (id, commands) in moves {
        let Some(task) = project.store().get_task(id)? else {
            return Err(Error::NotFound(format!("no task {id}")));
        };
        let mut copy = task.clone();
        for &command in commands {
            crate::state::change_status(
                &mut copy,
                command,
                options,
                vocab,
                &tasks,
                project.actor(),
                &stamp,
                &|id| crate::query::archived_task(project.store(), id),
            )
            .map_err(|e| refused(&project.task_key(task), &e))?;
        }
    }
    Ok(())
}

/// Whether `task`, in to do, is `agent`'s own work its lapse sent back (§3.3): its latest note is the lapse's ("lease
/// expired (agent)"), which [`lapse`] writes; a lapse clears `started_by` with the rest of the attempt, so that alone
/// can't tell (ATM-712). A task `agent` started and still names is its own too.
pub(crate) fn lapsed_for(task: &atm_common::Task, agent: &str) -> bool {
    let last = task
        .get("notes")
        .and_then(Value::as_array)
        .and_then(|notes| notes.last());
    let lapse = format!("lease expired ({agent})");
    last.is_some_and(|n| {
        n.get("kind").and_then(Value::as_str) == Some("failure")
            && n.get("text").and_then(Value::as_str) == Some(lapse.as_str())
    }) || task.get("started_by").and_then(Value::as_str) == Some(agent)
}

/// The moves that put a task in review from where it is: in progress, into review; the author's own task whose claim
/// lapsed (`take_back`), started again first (from to do; one still in progress, its claim run out but not dropped
/// yet, goes straight in).
fn into_review(project: &Project, id: &str, take_back: bool) -> Result<Vec<Command>> {
    let active = reviews::kind_of(project, id)?.is_some_and(|(kind, _)| kind == Kind::Active);
    Ok(if take_back && !active {
        vec![Command::Start, Command::Review]
    } else {
        vec![Command::Review]
    })
}

/// What opening a review in `status` with the tasks `ids` does to them (§4.2's create, and tasks added to a review):
/// an agent's review waiting on its reviewers has every task checked first (ATM-631's openings), and each one's move
/// into review tried through the task table; a person's, or a draft or a live one, links them as they are. The tasks
/// to take back first (the author's own whose claim lapsed). Writes nothing.
///
/// # Errors
/// A task the openings refuse (claimed by another agent, never started by the author, blocked, finished, in another
/// open review), or one the task table won't put in review.
pub fn check_open(project: &Project, status: Status, ids: &[String]) -> Result<Vec<String>> {
    if status != Status::Waiting || !is_agent(project.actor()) {
        return Ok(Vec::new());
    }
    let planned = reviews::openings(project, ids)?;
    let mut take_back = Vec::new();
    let mut moves = Vec::new();
    for (id, opening) in ids.iter().zip(planned) {
        match opening {
            Opening::TakeBack => {
                take_back.push(id.clone());
                moves.push((id.clone(), into_review(project, id, true)?));
            }
            Opening::Review => moves.push((id.clone(), vec![Command::Review])),
            Opening::Link => {}
        }
    }
    check_moves(project, &moves, &Options::default(), &|key, e| {
        Error::failed(format!(
            "{key} can't go in review, so the review isn't opened: {e}"
        ))
    })?;
    Ok(take_back)
}

/// What naming more tasks in `review` does (§4.2's tasks added): the review's rules first (its author or a decider,
/// not an ended review), then the added tasks it doesn't name yet as [`check_open`] checks a new review's. The tasks
/// to take back first. Writes nothing.
///
/// # Errors
/// What the review's rules refuse, or what [`check_open`] refuses for an added task.
pub fn check_set_tasks(
    project: &Project,
    review: &Review,
    added: &[String],
    by: &Actor,
) -> Result<Vec<String>> {
    review_next(review, &ReviewEvent::SetTasks, by)?;
    let store = reviews::store(project)?;
    let named = reviews::named(project, &store, &review.header).unwrap_or_default();
    let fresh: Vec<String> = added
        .iter()
        .filter(|id| !named.contains(id))
        .cloned()
        .collect();
    check_open(project, review.header.status, &fresh)
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
    // A round puts its tasks in progress back in review, and first takes back the author's own task its lapse sent to
    // to do (the design's hole 1): each move tried before the round is kept, so it never waits on a task in to do
    let mut take_back = Vec::new();
    if let Event::Round = event {
        let mut moves = Vec::new();
        for id in &ids {
            let Some(task) = project.store().get_task(id)? else {
                continue;
            };
            let own = is_agent(Some(&by.0)) && lapsed_for(task, &by.0);
            match reviews::kind_of(project, id)?.map(|(kind, _)| kind) {
                Some(Kind::Active) => moves.push((id.clone(), vec![Command::Review])),
                Some(Kind::Todo) if own => {
                    take_back.push(id.clone());
                    moves.push((id.clone(), into_review(project, id, true)?));
                }
                _ => {}
            }
        }
        check_moves(project, &moves, &Options::default(), &|key, e| {
            Error::failed(format!(
                "{reference}'s round can't reach {key}, so it isn't added: {e}"
            ))
        })?;
    }
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
        take_back,
    })
}

/// Whether a live review's approval can reach task `id` (§3.5): into review, then approved, both tried before either
/// is written, so the task is never left in review without the approval (ATM-712). Writes nothing.
///
/// # Errors
/// The task table refuses one of the two moves.
pub fn check_live_approval(project: &Project, id: &str, options: &Options) -> Result<()> {
    check_moves(
        project,
        &[(id.to_string(), vec![Command::Review, Command::Approve])],
        options,
        &|key, e| Error::failed(format!("the live review's approval can't reach {key}: {e}")),
    )
}

/// What a lapse does to a task in progress (§3.3): back to to do, with a failure note naming the agent whose claim
/// ran out, its commit kept. An event, never refused; `drop_expired_claims` applies it as it drops the claim.
pub fn lapse(
    task: &mut serde_json::Map<String, Value>,
    vocab: &atm_common::Vocabulary,
    claim: &Value,
) {
    let stamp = atm_common::time::now_iso();
    atm_common::edits::back_to_todo(task, vocab, None, &stamp, true);
    let text = format!(
        "lease expired ({})",
        atm_common::py::py_str_opt(claim.get("agent"))
    );
    // A note built by atm, short by construction
    let _ = atm_common::edits::add_note(task, "failure", &text, None, &stamp, false);
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
