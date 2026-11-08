//! The task state machine (docs/task-states.md; the Python atm's `lib/state.py`): the status commands as transitions
//! between the six kinds, each with its guards, and what each records on the task. It refuses and records exactly as
//! the Python atm does: the same messages, exit codes, notes and key order.
//!
//! The order of checks: the resolution (skip), then whether the command applies to the task's kind, then its guards,
//! then its arguments. Only then does anything change, in one place ([`apply`]).

use atm_common::edits::{
    add_interval, add_note, back_to_todo, check_note, is_agent, status_for, statuses_of, touch,
};
use atm_common::py::truthy;
use atm_common::rules::unfinished_dependencies;
use atm_common::{Error, Kind, Result, Task, Vocabulary};
use serde_json::{Value, json};

/// A status command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// To do (or back from review) -> in progress.
    Start,
    /// In progress -> in review.
    Review,
    /// In progress, or approved in review -> done.
    Done,
    /// In progress -> to do, with a failure note.
    Fail,
    /// In progress -> to do, with a handoff.
    Release,
    /// A review's decision: approved (it stays in review).
    Approve,
    /// A review's decision: changes asked for (it stays in review).
    RequestChanges,
    /// A review's decision: rejected (it stays in review).
    Reject,
    /// A rejected review -> to do, its work dropped.
    Drop,
    /// To do or in progress -> blocked.
    Block,
    /// Blocked -> to do, with the answers.
    Unblock,
    /// Done or skipped -> to do.
    Reopen,
    /// Any open state (review once rejected) -> skipped.
    Skip,
}

impl Command {
    /// Every command.
    pub const ALL: [Command; 13] = [
        Command::Start,
        Command::Review,
        Command::Done,
        Command::Fail,
        Command::Release,
        Command::Approve,
        Command::RequestChanges,
        Command::Reject,
        Command::Drop,
        Command::Block,
        Command::Unblock,
        Command::Reopen,
        Command::Skip,
    ];

    /// The command's name, as messages say it (`request-changes`).
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Command::Start => "start",
            Command::Review => "review",
            Command::Done => "done",
            Command::Fail => "fail",
            Command::Release => "release",
            Command::Approve => "approve",
            Command::RequestChanges => "request-changes",
            Command::Reject => "reject",
            Command::Drop => "drop",
            Command::Block => "block",
            Command::Unblock => "unblock",
            Command::Reopen => "reopen",
            Command::Skip => "skip",
        }
    }

    /// The kinds a task may be in for the command, in the kinds' order.
    #[must_use]
    pub fn sources(self) -> &'static [Kind] {
        match self {
            Command::Start => &[Kind::Todo, Kind::Review],
            Command::Review | Command::Fail | Command::Release => &[Kind::Active],
            Command::Done => &[Kind::Active, Kind::Review],
            Command::Approve | Command::RequestChanges | Command::Reject | Command::Drop => {
                &[Kind::Review]
            }
            Command::Block => &[Kind::Todo, Kind::Active],
            Command::Unblock => &[Kind::Blocked],
            Command::Reopen => &[Kind::Done, Kind::Skipped],
            Command::Skip => &[Kind::Todo, Kind::Active, Kind::Review, Kind::Blocked],
        }
    }

    /// The kind the command leaves the task in.
    #[must_use]
    pub fn destination(self) -> Kind {
        match self {
            Command::Start => Kind::Active,
            Command::Review | Command::Approve | Command::RequestChanges | Command::Reject => {
                Kind::Review
            }
            Command::Done => Kind::Done,
            Command::Fail
            | Command::Release
            | Command::Drop
            | Command::Unblock
            | Command::Reopen => Kind::Todo,
            Command::Block => Kind::Blocked,
            Command::Skip => Kind::Skipped,
        }
    }

    /// The review state a review's decision records.
    fn decision(self) -> Option<&'static str> {
        match self {
            Command::Approve => Some("approved"),
            Command::RequestChanges => Some("changes_requested"),
            Command::Reject => Some("rejected"),
            _ => None,
        }
    }
}

/// How a skipped task ended, when it isn't the default (SR-2.8).
const SKIP_RESOLUTIONS: [&str; 3] = ["wont_do", "obsolete", "duplicate"];
const WAITING_ON: [&str; 2] = ["user", "agent"];
/// Review states that make a task in review work for an agent again (SR-3.9).
const REWORK_STATES: [&str; 2] = ["changes_requested", "approved"];
const DECISIONS: [&str; 3] = ["approved", "changes_requested", "rejected"];

/// A status command's options, as `atm task state` and `atm task review` take them.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// The status to take when the target kind has several (`--status`).
    pub status: Option<String>,
    /// A note (`--note`).
    pub note: Option<String>,
    /// A reason (`--reason`).
    pub reason: Option<String>,
    /// The task's commit (`--commit`).
    pub commit: Option<String>,
    /// Who a blocked task waits on (`--on`).
    pub waiting_on: Option<String>,
    /// A blocked task's open questions (`--question`).
    pub questions: Vec<String>,
    /// The answers to a blocked task's questions (`--answer`).
    pub answers: Vec<String>,
    /// How a skipped task ended (`--resolution`).
    pub resolution: Option<String>,
    /// The task a duplicate duplicates (`--duplicate-of`).
    pub duplicate_of: Option<String>,
}

fn fail(message: impl Into<String>) -> Error {
    Error::failed(message)
}

fn usage(message: impl Into<String>) -> Error {
    Error::Failed {
        message: message.into(),
        code: 2,
    }
}

fn text(task: &Task, key: &str) -> Option<String> {
    task.get(key).and_then(Value::as_str).map(String::from)
}

/// `skip`'s resolution, checked (a done task is done, T-438), with the task it may duplicate.
fn check_resolution(task_id: &str, options: &Options) -> Result<String> {
    let resolution = options
        .resolution
        .clone()
        .filter(|r| !r.is_empty())
        .unwrap_or_else(|| "wont_do".into());
    if !SKIP_RESOLUTIONS.contains(&resolution.as_str()) {
        return Err(usage(format!(
            "--resolution must be one of {}",
            SKIP_RESOLUTIONS.join(", ")
        )));
    }
    if options.duplicate_of.is_some() && resolution != "duplicate" {
        return Err(usage("--duplicate-of goes with --resolution duplicate"));
    }
    if options.duplicate_of.as_deref() == Some(task_id) {
        return Err(usage(format!("{task_id} can't be a duplicate of itself")));
    }
    Ok(resolution)
}

/// Every command: notes, reasons, questions and answers fit in a note; a block says what it waits for.
fn check_arguments(command: Command, options: &Options) -> Result<()> {
    if let Some(note) = &options.note {
        check_note(note)?;
    }
    if let Some(reason) = &options.reason {
        check_note(reason)?;
    }
    for question in &options.questions {
        check_note(question)?;
    }
    let has_reason = options.reason.as_deref().is_some_and(|r| !r.is_empty());
    if matches!(command, Command::Reject | Command::RequestChanges) && !has_reason {
        return Err(usage("a review that isn't approved says why: --reason"));
    }
    if command == Command::Block {
        if !options
            .waiting_on
            .as_deref()
            .is_some_and(|w| WAITING_ON.contains(&w))
        {
            return Err(usage(format!("--on must be {}", WAITING_ON.join(" or "))));
        }
        if options.questions.is_empty() && !has_reason {
            return Err(usage(
                "say what it waits for: --question (repeatable) or --reason",
            ));
        }
    }
    for answer in &options.answers {
        check_note(answer)?;
    }
    Ok(())
}

/// The guards of a command on a task in `kind`, in the order the Python machine runs them.
fn check_guards(
    command: Command,
    kind: Kind,
    task: &Task,
    vocab: &Vocabulary,
    tasks: &[&Task],
    agent: Option<&str>,
    archived: &dyn Fn(&str) -> Result<bool>,
) -> Result<()> {
    let id = text(task, "id").unwrap_or_default();
    let review_state = text(task, "review_state");
    let state_is = |s: &str| review_state.as_deref() == Some(s);
    match command {
        Command::Start => {
            if kind == Kind::Review
                && !review_state
                    .as_deref()
                    .is_some_and(|s| REWORK_STATES.contains(&s))
            {
                return Err(fail(format!(
                    "can't start {id}: it waits for its review (atm task review approve or request-changes {id} decides it)"
                )));
            }
            let blocking = unfinished_dependencies(vocab, tasks, task, archived)?;
            if !blocking.is_empty() {
                let each: Vec<String> = blocking
                    .iter()
                    .map(|(d, s)| format!("{d} ({})", atm_common::py::py_str_opt(s.as_ref())))
                    .collect();
                return Err(fail(format!(
                    "can't start {id}: waiting on {}",
                    each.join(", ")
                )));
            }
        }
        Command::Done => {
            // An agent finishing a `blocker` or `async` task asks for the review instead (SR-3.8)
            let mode = vocab.effective_review(task);
            if (mode == "blocker" || mode == "async") && is_agent(agent) && kind == Kind::Active {
                return Err(usage(format!(
                    "{id} is reviewed ({mode}): ask for it with atm task state {id} in_review --note '<what to check>'"
                )));
            }
            if kind == Kind::Review && !state_is("approved") {
                return Err(fail(format!(
                    "can't finish {id}: its review isn't approved (atm task review approve {id} decides it)"
                )));
            }
        }
        Command::Drop | Command::Skip if kind == Kind::Review && !state_is("rejected") => {
            let way = if command == Command::Drop {
                "send back to to do"
            } else {
                "skip"
            };
            return Err(fail(format!(
                "can't {way} {id}: its review isn't rejected (atm task review reject {id} --reason … decides it)"
            )));
        }
        _ => {}
    }
    Ok(())
}

/// Apply `command` to `task`; returns its status before the command. `tasks` are the project's working tasks (for
/// dependencies), `archived(id)` whether the archive has a task, `agent` who runs it, `stamp` the time.
///
/// # Errors
/// The command doesn't apply to the task's kind, a guard refuses it, or its options don't fit; the message says why.
#[allow(clippy::too_many_arguments)] // reason: the task, its project and the command, as the Python change_status takes them
pub fn change_status(
    task: &mut Task,
    command: Command,
    options: &Options,
    vocab: &Vocabulary,
    tasks: &[&Task],
    agent: Option<&str>,
    stamp: &str,
    archived: &dyn Fn(&str) -> Result<bool>,
) -> Result<Value> {
    let id = text(task, "id").unwrap_or_default();
    let current = task.get("status").cloned().unwrap_or(Value::Null);
    let kind = vocab.kind_of(task.get("status")).unwrap_or(Kind::Todo);
    // Each question was checked when the task was blocked, each answer when it is given: together they may be longer
    // than one note, and must still be recordable, or the task could never be unblocked
    let asked: Vec<String> = task
        .get("questions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(atm_common::py::py_str)
        .collect();
    let answered = (command == Command::Unblock
        && (!asked.is_empty() || !options.answers.is_empty()))
    .then(|| atm_common::edits::answer_note(&asked, &options.answers));
    let resolution = if command == Command::Skip {
        Some(check_resolution(&id, options)?)
    } else {
        None
    };
    if !command.sources().contains(&kind) {
        let expected: Vec<&str> = command
            .sources()
            .iter()
            .flat_map(|k| statuses_of(vocab, *k))
            .collect();
        return Err(fail(format!(
            "can't {} {id}: it is {}, not {}",
            command.name(),
            atm_common::py::py_str(&current),
            expected.join(" or ")
        )));
    }
    check_guards(command, kind, task, vocab, tasks, agent, archived)?;
    check_arguments(command, options)?;
    apply(
        task,
        command,
        kind,
        options,
        vocab,
        agent,
        stamp,
        &current,
        answered.as_deref(),
        resolution.as_deref(),
    )?;
    Ok(current)
}

/// Record the command on the task, in the order the Python atm does (it shapes the key order in `tasks.json`).
#[allow(clippy::too_many_arguments, clippy::too_many_lines)] // reason: one branch per command, as the Python _apply has them
fn apply(
    task: &mut Task,
    command: Command,
    old: Kind,
    options: &Options,
    vocab: &Vocabulary,
    agent: Option<&str>,
    stamp: &str,
    current: &Value,
    answered: Option<&str>,
    resolution: Option<&str>,
) -> Result<()> {
    let new = command.destination();
    let note = options.note.as_deref().filter(|n| !n.is_empty());
    let reason = options.reason.as_deref().filter(|r| !r.is_empty());
    let now = json!(stamp);
    // A block's or a review's start goes when the task moves on: keep each interval, so waiting time can be measured
    if old == Kind::Blocked && task.get("blocked_at").is_some_and(truthy) {
        let (from, on) = (task["blocked_at"].clone(), task.get("waiting_on").cloned());
        add_interval(task, "blocks", &from, &now, on.as_ref(), None);
    } else if old == Kind::Review
        && new != Kind::Review
        && task.get("review_requested_at").is_some_and(truthy)
    {
        // The round ends when the reviewer decided, with its outcome (SR-3.9)
        let decided = text(task, "review_state").filter(|s| DECISIONS.contains(&s.as_str()));
        let from = task["review_requested_at"].clone();
        let to = task
            .get("reviewed_at")
            .filter(|v| truthy(v))
            .cloned()
            .unwrap_or_else(|| now.clone());
        add_interval(task, "reviews", &from, &to, None, decided.as_deref());
    }
    if new != Kind::Review {
        task.shift_remove("review_state");
        task.shift_remove("reviewed_at");
    }

    if new == Kind::Todo {
        // Reopening, dropping a rejected task or failing one undoes its work; releasing or unblocking it doesn't
        let keep_commit = !matches!(command, Command::Reopen | Command::Drop | Command::Fail);
        back_to_todo(task, vocab, agent, stamp, keep_commit);
        if let Some(answered) = answered.filter(|a| !a.is_empty()) {
            add_note(task, "answer", answered, agent, stamp, false)?;
        }
        match command {
            Command::Fail => add_note(
                task,
                "failure",
                reason.unwrap_or("verification failed"),
                agent,
                stamp,
                true,
            )?,
            Command::Reopen => {
                let given = options.reason.as_deref().unwrap_or("None");
                add_note(
                    task,
                    "note",
                    &format!("Reopened: {given}"),
                    agent,
                    stamp,
                    true,
                )?;
            }
            _ => {
                if let Some(note) = note {
                    add_note(task, "handoff", note, agent, stamp, true)?;
                }
            }
        }
        return Ok(());
    }

    let status = if command.decision().is_some() && options.status.is_none() {
        current.clone() // it stays in its review status (a project may have several)
    } else {
        json!(status_for(vocab, new, options.status.as_deref())?)
    };
    task.insert("status".into(), status);
    touch(task, agent, stamp);

    match command {
        Command::Block => {
            // Who started it stays visible while it waits; unblock returns it to to-do and clears the attempt
            task.insert("waiting_on".into(), json!(options.waiting_on));
            task.insert("questions".into(), json!(options.questions));
            task.insert("blocked_at".into(), now.clone());
            if let Some(reason) = reason {
                add_note(task, "note", reason, agent, stamp, true)?;
            }
        }
        Command::Review => {
            // The work ends here: the review isn't agent work (DM-4.1); done keeps this timestamp
            task.insert("review_requested_at".into(), now.clone());
            task.insert("review_state".into(), json!("waiting"));
            if let Some(note) = note {
                add_note(task, "review", note, agent, stamp, true)?;
            }
        }
        Command::Approve | Command::RequestChanges | Command::Reject => {
            task.insert("review_state".into(), json!(command.decision()));
            task.insert("reviewed_at".into(), now.clone());
            if command == Command::Approve {
                let text =
                    note.map_or_else(|| "Approved".to_string(), |n| format!("Approved: {n}"));
                add_note(task, "note", &text, agent, stamp, true)?;
            } else {
                add_note(
                    task,
                    "rejection",
                    reason.unwrap_or("None"),
                    agent,
                    stamp,
                    true,
                )?;
            }
        }
        Command::Skip => {
            for field in ["waiting_on", "questions", "blocked_at"] {
                task.shift_remove(field);
            }
            task.insert("skipped_at".into(), now.clone());
            if let Some(resolution) = resolution.filter(|r| *r != "wont_do") {
                task.insert("resolution".into(), json!(resolution)); // none means the default, won't do
            }
            if let Some(duplicate_of) = options.duplicate_of.as_deref().filter(|d| !d.is_empty()) {
                task.insert("duplicate_of".into(), json!(duplicate_of));
            }
            add_note(
                task,
                "note",
                &format!("Skipped: {}", options.reason.as_deref().unwrap_or("None")),
                agent,
                stamp,
                true,
            )?;
        }
        Command::Start => {
            task.insert("started_at".into(), now.clone());
            task.shift_remove("review_requested_at"); // taken back from review: a new round of work
            task.shift_remove("completed_at");
            task.shift_remove("completed_by");
            task.shift_remove("agent"); // the old name of started_by
            if let Some(agent) = agent.filter(|a| !a.is_empty()) {
                task.insert("started_by".into(), json!(agent));
            }
        }
        Command::Done => {
            task.insert("completed_at".into(), now.clone());
            if let Some(agent) = agent.filter(|a| !a.is_empty()) {
                task.insert("completed_by".into(), json!(agent));
            }
            if let Some(commit) = options.commit.as_deref().filter(|c| !c.is_empty()) {
                task.insert("commit".into(), json!(commit));
            }
            // A batch task an agent finishes waits for the end-of-sprint review (SR-4.4): its note is what to check
            let batch =
                old == Kind::Active && is_agent(agent) && vocab.effective_review(task) == "batch";
            if batch {
                task.insert("sprint_review".into(), json!("pending"));
            }
            if let Some(note) = note {
                add_note(
                    task,
                    if batch { "review" } else { "outcome" },
                    note,
                    agent,
                    stamp,
                    true,
                )?;
            }
        }
        Command::Fail | Command::Release | Command::Drop | Command::Unblock | Command::Reopen => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests;
