//! The status changes (the Python atm's `lib/status.py`): each runs as one change of the store, so the task's status,
//! its claim and the records shared by a clone's worktrees (completed, waiting, resolved) change together or not at
//! all. The transitions themselves are [`crate::state`]'s; the lease rules [`crate::leases`]'.

use atm_common::Shown;
use atm_common::archive::archive_hint;
use atm_common::py::{repr_str, truthy};
use atm_common::rules::unfinished_dependencies;
use atm_common::time::now_iso;
use atm_common::{Error, Kind, Result, Task, Vocabulary};
use atm_db::working::Working;
use serde_json::{Map, Value, json};

use crate::leases::{
    check_holder, claim_record, completion_record, drop_expired_claims, own_lease_expired,
    waiting_entry,
};
use crate::loop_rules::{Selection, next_task};
use crate::project::Project;
use crate::query::{Filters, task_views};
use crate::state::{Command, Options, change_status};
use crate::tasks::{TaskView, show_tasks, view_task};

/// The commands an agent runs on its own claimed task: if that claim just expired, they explain it instead of refusing.
const OWN_LEASE_COMMANDS: [Command; 5] = [
    Command::Done,
    Command::Release,
    Command::Fail,
    Command::Review,
    Command::Block,
];

/// The commands that leave a task waiting (blocked or in review) and record it for every worktree.
const WAITING_COMMANDS: [Command; 5] = [
    Command::Block,
    Command::Review,
    Command::Approve,
    Command::RequestChanges,
    Command::Reject,
];

/// The state `atm task state <id> <state>` names for a command an agent runs on its own claim (the hint's command).
fn command_state(command: Command) -> &'static str {
    match command {
        Command::Done => "done",
        Command::Review => "in_review",
        Command::Block => "blocked",
        _ => "todo",
    }
}

/// The kinds a task waiting in another worktree must have there for a decision made here to apply to it (T-204).
fn remote_kinds(command: Command) -> Option<&'static [Kind]> {
    match command {
        Command::Unblock => Some(&[Kind::Blocked]),
        Command::Skip => Some(&[Kind::Blocked, Kind::Review]),
        Command::Done | Command::Drop => Some(&[Kind::Review]),
        _ => None,
    }
}

/// What a status change did: the task after it, its status before, its claim now, the tasks now ready (after done),
/// the archive hint (at most once a day), a task reopened in a closed sprint, the claims that expired on the way, and
/// the task as `task details` shows it.
#[derive(Debug, Clone)]
pub struct StatusChange {
    /// The task after the change.
    pub task: Task,
    /// Its status before.
    pub previous: Value,
    /// Its claim now (null without one).
    pub claim: Value,
    /// The tasks now ready (after done).
    pub unblocked: Vec<String>,
    /// The archive hint (after done, at most once a day per clone).
    pub hint: Option<Value>,
    /// What to do about a task reopened in a closed sprint (T-330).
    pub stranded: Option<String>,
    /// The claims whose lease ran out on the way.
    pub expired: Vec<Value>,
    /// The task with what is derived for it.
    pub view: Option<TaskView>,
}

fn id_of(task: &Task) -> Option<&str> {
    task.get("id").and_then(Value::as_str)
}

fn section<'a>(store: &'a mut Map<String, Value>, key: &str) -> &'a mut Map<String, Value> {
    let entry = store.entry(key).or_insert_with(|| json!({}));
    if !entry.is_object() {
        *entry = json!({});
    }
    entry
        .as_object_mut()
        .expect("the section was made an object just above")
}

/// The tasks as a list to read, and the index of the one with this ID (the first, as `find_task` takes it).
fn snapshot(document: &Value, task_id: &str) -> (Vec<Task>, Option<usize>) {
    let tasks: Vec<Task> = document["tasks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
        .cloned()
        .collect();
    let at = document["tasks"].as_array().and_then(|list| {
        list.iter()
            .position(|t| t.get("id").and_then(Value::as_str) == Some(task_id))
    });
    (tasks, at)
}

/// The archived tasks a change can see (its own read of the archive).
fn archived_tasks(work: &mut Working) -> Result<Vec<Task>> {
    Ok(work.archive()?["tasks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
        .cloned()
        .collect())
}

/// Apply what was decided in another worktree about a task waiting in this one (`resolved`): the answer, the passed
/// or rejected review, the skip, as if given here. A task changed here in the meantime is left as it is.
fn apply_resolutions(
    work: &mut Working,
    vocab: &Vocabulary,
    archived: &dyn Fn(&str) -> Result<bool>,
) {
    let here = work.here().worktree.clone();
    let resolved = section(&mut work.claims_store, "resolved").clone();
    for (task_id, record) in resolved {
        if record.get("worktree").and_then(Value::as_str) != here.as_deref() || !record.is_object()
        {
            continue;
        }
        section(&mut work.claims_store, "resolved").shift_remove(&task_id);
        let (tasks, Some(at)) = snapshot(&work.document, &task_id) else {
            continue;
        };
        let Some(command) = record
            .get("command")
            .and_then(Value::as_str)
            .and_then(|c| Command::ALL.into_iter().find(|k| k.name() == c))
        else {
            continue;
        };
        let answers: Vec<String> = record
            .get("answers")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(atm_common::py::py_str)
            .collect();
        let options = Options {
            reason: record
                .get("reason")
                .and_then(Value::as_str)
                .map(String::from),
            answers,
            ..Options::default()
        };
        let mut task = tasks[at].clone();
        let others: Vec<&Task> = tasks.iter().collect();
        let by = record.get("by").and_then(Value::as_str);
        if change_status(
            &mut task,
            command,
            &options,
            vocab,
            &others,
            by,
            &now_iso(),
            archived,
        )
        .is_ok()
        {
            work.tasks()[at] = Value::Object(task);
        }
    }
}

/// When the task waits in another worktree (blocked or in review there) but not here, and `command` decides it:
/// give this copy the waiting state, so the command applies here as it would there. Returns the worktree it waits in.
fn adopt_waiting(
    task: &mut Task,
    vocab: &Vocabulary,
    store: &Map<String, Value>,
    here: Option<&str>,
    command: Command,
) -> Option<Value> {
    let kinds = remote_kinds(command)?;
    let record = store.get("waiting")?.get(id_of(task)?)?.as_object()?;
    if record.get("worktree").and_then(Value::as_str) == here {
        return None;
    }
    let kind_here = vocab.kind_of(task.get("status"));
    if kind_here.is_some_and(|k| kinds.contains(&k)) {
        return None; // it waits here too: decided here as usual
    }
    if !vocab
        .kind_of(record.get("status"))
        .is_some_and(|k| kinds.contains(&k))
    {
        return None;
    }
    task.insert("status".into(), record["status"].clone());
    for key in ["waiting_on", "questions", "review_state"] {
        if let Some(value) = record.get(key).filter(|v| truthy(v)) {
            task.insert(key.into(), value.clone());
        }
    }
    Some(record.get("worktree").cloned().unwrap_or(Value::Null))
}

/// The to-do tasks waiting on `task_id` whose dependencies are now all done.
fn newly_ready(
    vocab: &Vocabulary,
    tasks: &[&Task],
    task_id: &str,
    archived: &dyn Fn(&str) -> Result<bool>,
) -> Result<Vec<String>> {
    let mut ready = Vec::new();
    for t in tasks {
        let waits =
            atm_common::rules::contains(t.get("dependencies").filter(|d| d.is_array()), task_id);
        if waits
            && vocab.kind_of(t.get("status")) == Some(Kind::Todo)
            && unfinished_dependencies(vocab, tasks, t, archived)?.is_empty()
            && let Some(id) = id_of(t)
        {
            ready.push(id.to_string());
        }
    }
    Ok(ready)
}

/// One status command on a task, as the project's actor.
///
/// # Errors
/// What the state machine or the claims refuse; an unknown task (it says when the task is archived); a store error.
#[allow(clippy::too_many_lines)] // reason: one step per rule, in the order the Python change_task_status runs them
pub fn change_task_status(
    project: &mut Project,
    command: Command,
    task_id: &str,
    options: &Options,
) -> Result<StatusChange> {
    let agent = project.actor().map(String::from);
    let agent = agent.as_deref();
    let mut outcome: Option<StatusChange> = None;
    project.store_mut().change(&mut |work: &mut Working| {
        let vocab = Vocabulary::of_document(&work.document)?;
        let archive = archived_tasks(work)?;
        let in_archive = |id: &str| -> Result<bool> { Ok(archive.iter().any(|t| id_of(t) == Some(id) && !t.is_empty())) };
        let mut claims = section(&mut work.claims_store, "claims").clone();
        let expired = drop_expired_claims(&mut work.document, &mut claims, &vocab);
        *section(&mut work.claims_store, "claims") = claims;
        apply_resolutions(work, &vocab, &in_archive);
        let here = work.here().clone();
        let (tasks, at) = snapshot(&work.document, task_id);
        let Some(at) = at else {
            let hint = if in_archive(task_id)? { " (it is archived: atm db unarchive brings it back)" } else { "" };
            return Err(Error::failed(format!("no task {task_id}{hint}")));
        };
        if OWN_LEASE_COMMANDS.contains(&command) {
            let then = format!("atm task state {task_id} {}", command_state(command));
            if let Some(own) = own_lease_expired(&expired, &work.document, &vocab, task_id, agent, Some(&then)) {
                // The expiry is written first, so `atm task details` agrees with the error
                work.defer(own);
                return Ok(());
            }
        }
        check_holder(section(&mut work.claims_store, "claims"), task_id, agent, command.name())?;
        if let Some(dup) = options.duplicate_of.as_deref().filter(|d| !d.is_empty())
            && !tasks.iter().any(|t| id_of(t) == Some(dup))
            && !in_archive(dup)?
        {
            return Err(Error::Invalid(format!("{task_id} can't be a duplicate of {dup}, which doesn't exist")));
        }
        let mut task = tasks[at].clone();
        let waits_in = adopt_waiting(&mut task, &vocab, &work.claims_store, here.worktree.as_deref(), command);
        let others: Vec<&Task> = tasks.iter().collect();
        let previous = change_status(&mut task, command, options, &vocab, &others, agent, &now_iso(), &in_archive)?;
        work.tasks()[at] = Value::Object(task.clone());

        let document = work.document.clone();
        let claims = section(&mut work.claims_store, "claims");
        match command {
            Command::Start => {
                claims.insert(task_id.into(), claim_record(&here, &document, task_id, agent));
                section(&mut work.claims_store, "completed").shift_remove(task_id); // starting a task again on purpose
            }
            Command::Reopen => {
                claims.shift_remove(task_id);
                section(&mut work.claims_store, "completed").shift_remove(task_id); // not done any more, anywhere
            }
            _ => {
                claims.shift_remove(task_id);
            }
        }
        if command == Command::Done {
            section(&mut work.claims_store, "completed").insert(task_id.into(), completion_record(&here, task_id, agent));
        }
        // Blocked or in review, with its review's decision: recorded for every worktree (T-204); any other change
        // ends the wait
        if WAITING_COMMANDS.contains(&command) {
            section(&mut work.claims_store, "waiting").insert(task_id.into(), waiting_entry(&here, &task));
        } else {
            section(&mut work.claims_store, "waiting").shift_remove(task_id);
            let resolution = section(&mut work.claims_store, "resolved").shift_remove(task_id);
            if let Some(waits_in) = waits_in {
                let record = json!({
                    "task": task_id,
                    "command": command.name(),
                    "answers": options.answers,
                    "reason": options.reason,
                    "by": agent,
                    "at": now_iso(),
                    "worktree": waits_in,
                });
                section(&mut work.claims_store, "resolved").insert(task_id.into(), record);
            } else if let Some(resolution) = resolution.filter(|r| r.get("worktree").and_then(Value::as_str) != here.worktree.as_deref()) {
                section(&mut work.claims_store, "resolved").insert(task_id.into(), resolution); // still to be applied where it waits
            }
        }
        let (all, _) = snapshot(&work.document, task_id);
        let all_refs: Vec<&Task> = all.iter().collect();
        let unblocked = if command == Command::Done { newly_ready(&vocab, &all_refs, task_id, &in_archive)? } else { Vec::new() };
        // The archive hint after done, at most once a day per clone (CLI-5.2), so a loop doesn't repeat it
        let today: String = now_iso().chars().take(10).collect();
        let mut hint = if command == Command::Done { archive_hint(&work.document, &vocab, work.size()) } else { None };
        if hint.is_some() && section(&mut work.claims_store, "hints").get("archive").and_then(Value::as_str) == Some(today.as_str()) {
            hint = None;
        }
        if hint.is_some() {
            section(&mut work.claims_store, "hints").insert("archive".into(), json!(today));
        }
        // Reopened in a closed sprint, the task is open where the loop's default selection never looks (T-330)
        let stranded = if command == Command::Reopen {
            vocab.find_sprint(task.get("sprint")).filter(|s| s.get("state").and_then(Value::as_str) == Some("closed")).map(|s| {
                let name = s.get("id").and_then(Value::as_str).unwrap_or_default();
                format!(
                    "{task_id} is now open in the closed sprint {}, where the loop won't take it: atm sprint move {task_id} --to backlog (or --to active)",
                    repr_str(name)
                )
            })
        } else {
            None
        };
        let lookup = |id: &str| -> Result<Option<Task>> { Ok(archive.iter().rev().find(|t| id_of(t) == Some(id)).cloned()) };
        let view = view_task(&all[at], false, &all_refs, &lookup, &vocab)?;
        let claim = section(&mut work.claims_store, "claims").get(task_id).cloned().unwrap_or(Value::Null);
        outcome = Some(StatusChange { task, previous, claim, unblocked, hint, stranded, expired, view: Some(view) });
        Ok(())
    })?;
    outcome.ok_or_else(|| Error::failed(format!("no change to {task_id}")))
}

/// The status command `atm task state <id> todo` runs, by the kind the task is in: in progress, a failure (`failing`,
/// it says why) or a release; in review, drop (once rejected); blocked, unblock; done or skipped, reopen. A task to do
/// here but claimed (in progress in another worktree, T-202), or blocked or in review in another (T-204), is decided as
/// it is there.
///
/// # Errors
/// `Invalid` when the task is to do already; `NotFound` when there is no such task.
pub fn todo_command(project: &Project, task_id: &str, failing: bool) -> Result<Command> {
    let views = task_views(project, &Filters::default())?;
    let mut kind = if let Some(view) = views
        .iter()
        .find(|v| v["task"]["id"].as_str() == Some(task_id))
    {
        view["derived"]
            .get("kind")
            .and_then(Value::as_str)
            .and_then(Kind::parse)
    } else {
        show_tasks(project, &[atm_common::TaskId::new(task_id)])?; // NotFound when it doesn't exist
        Some(Kind::Done) // otherwise it is archived: finished
    };
    if kind == Some(Kind::Todo) {
        let store = project.store().read_claims();
        let in_claims = store
            .get("claims")
            .and_then(Value::as_object)
            .is_some_and(|c| c.contains_key(task_id));
        let record = store
            .get("waiting")
            .and_then(|w| w.get(task_id))
            .filter(|r| r.is_object());
        let here = project.store().working_copy().map(Shown::shown);
        if in_claims {
            kind = Some(Kind::Active); // the claim's holder check says whose it is
        } else if let Some(record) =
            record.filter(|r| r.get("worktree").and_then(Value::as_str) != here.as_deref())
        {
            // The project's settings, without its lists: the default statuses' kinds
            kind = Vocabulary::of_meta(&Map::new())?.kind_of(record.get("status"));
        }
    }
    match kind {
        Some(Kind::Active) => Ok(if failing {
            Command::Fail
        } else {
            Command::Release
        }),
        Some(Kind::Review) => Ok(Command::Drop),
        Some(Kind::Blocked) => Ok(Command::Unblock),
        Some(Kind::Done | Kind::Skipped) => Ok(Command::Reopen),
        other => Err(Error::Invalid(format!(
            "{task_id} is already {}",
            other.map_or("None", Kind::as_str)
        ))),
    }
}

/// What `atm loop next` found: the task (after its start, with `--claim`), its new claim, why there is none, and the
/// claims that expired on the way.
#[derive(Debug, Clone, Default)]
pub struct NextTask {
    /// The next task, if there is one.
    pub task: Option<Task>,
    /// Its claim, when it was claimed.
    pub claim: Option<Value>,
    /// Why there is none.
    pub reason: Option<String>,
    /// The claims whose lease ran out on the way.
    pub expired: Vec<Value>,
}

/// `atm loop next`: the task the loop takes next for the project's actor (the dashboard's Next up is the same rule);
/// with `claim`, started and claimed in the same change, so no other agent can take it.
///
/// # Errors
/// A selection that can't be (an unknown sprint), a start the state machine refuses, a store error.
pub fn next_task_for(
    project: &mut Project,
    sel: &Selection,
    with_user: bool,
    claim: bool,
    status: Option<&str>,
) -> Result<NextTask> {
    let agent = project.actor().map(String::from);
    let agent = agent.as_deref();
    let mut outcome = NextTask::default();
    project.store_mut().change(&mut |work: &mut Working| {
        let vocab = Vocabulary::of_document(&work.document)?;
        let archive = archived_tasks(work)?;
        let in_archive = |id: &str| -> Result<bool> {
            Ok(archive
                .iter()
                .any(|t| id_of(t) == Some(id) && !t.is_empty()))
        };
        let mut claims = section(&mut work.claims_store, "claims").clone();
        let expired = drop_expired_claims(&mut work.document, &mut claims, &vocab);
        *section(&mut work.claims_store, "claims") = claims;
        apply_resolutions(work, &vocab, &in_archive);
        let (tasks, _) = snapshot(&work.document, "");
        let refs: Vec<&Task> = tasks.iter().collect();
        let (found, reason) = next_task(
            &vocab,
            &work.document,
            &refs,
            &work.claims_store,
            sel,
            agent,
            with_user,
            &in_archive,
        )?;
        let mut task = found.cloned();
        let mut new = None;
        if claim && let Some(t) = task.as_mut() {
            let task_id = id_of(t).unwrap_or_default().to_string();
            let options = Options {
                status: status.map(String::from),
                ..Options::default()
            };
            change_status(
                t,
                Command::Start,
                &options,
                &vocab,
                &refs,
                agent,
                &now_iso(),
                &in_archive,
            )?;
            if let Some(at) = work.document["tasks"].as_array().and_then(|list| {
                list.iter()
                    .position(|v| v.get("id").and_then(Value::as_str) == Some(&task_id))
            }) {
                work.tasks()[at] = Value::Object(t.clone());
            }
            let here = work.here().clone();
            let record = claim_record(&here, &work.document, &task_id, agent);
            section(&mut work.claims_store, "claims").insert(task_id, record.clone());
            new = Some(record);
        }
        outcome = NextTask {
            task,
            claim: new,
            reason,
            expired,
        };
        Ok(())
    })?;
    Ok(outcome)
}

/// `atm task note`: a note on a task, whatever its status (DM-4), at most 500 characters, as the project's actor.
/// Returns the task.
///
/// # Errors
/// An unknown task (it says when the task is archived), a note too long, a store error.
pub fn add_task_note(project: &mut Project, task_id: &str, text: &str) -> Result<Task> {
    let actor = project.actor().map(String::from);
    let mut noted: Option<Task> = None;
    project.store_mut().change(&mut |work: &mut Working| {
        let (tasks, at) = snapshot(&work.document, task_id);
        let Some(at) = at else {
            let archived = archived_tasks(work)?
                .iter()
                .any(|t| id_of(t) == Some(task_id));
            let hint = if archived {
                " (it is archived: atm db unarchive brings it back)"
            } else {
                ""
            };
            return Err(Error::failed(format!("no task {task_id}{hint}")));
        };
        let mut task = tasks[at].clone();
        let stamp = now_iso();
        atm_common::edits::add_note(&mut task, "note", text, actor.as_deref(), &stamp, true)?;
        atm_common::edits::touch(&mut task, actor.as_deref(), &stamp);
        work.tasks()[at] = Value::Object(task.clone());
        noted = Some(task);
        Ok(())
    })?;
    noted.ok_or_else(|| Error::failed(format!("no note on {task_id}")))
}

#[cfg(test)]
mod tests;
