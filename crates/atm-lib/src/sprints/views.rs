//! The sprints as the dashboard shows them (ATM-720, docs/dashboard-derived-state.md group E): each sprint in the
//! project's order with its figures by one rule each, and the backlog as `atm sprint list` counts it. Served with the
//! tasks, so the scope bar, the overview card and the CLI say the same.

use std::collections::HashMap;

use atm_common::py::truthy;
use atm_common::time::{days_until, end_of_day};
use atm_common::{Kind, Result, Task, Vocabulary};
use serde_json::{Map, Value, json};

use super::{in_backlog, sprint_id};
use crate::project::Project;

/// The task's sprint history entry for `sprint` (the last one, if it was there more than once): its `from`.
fn entered(task: &Task, sprint: &str) -> Option<String> {
    history(task)
        .rev()
        .find(|(name, _)| name.eq_ignore_ascii_case(sprint))
        .map(|(_, from)| from)
}

/// The task's sprint history as (sprint, from), oldest first; entries without either left out.
fn history(task: &Task) -> impl DoubleEndedIterator<Item = (String, String)> + '_ {
    task.get("sprint_history")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|e| {
            let sprint = e.get("sprint").and_then(Value::as_str)?;
            let from = e.get("from").and_then(Value::as_str)?;
            Some((sprint.to_string(), from.to_string()))
        })
}

/// Pulled into the sprint after it started: its entry for the sprint begins after the sprint's start (SR-4.5).
fn added_mid_sprint(task: &Task, sprint: &Map<String, Value>) -> bool {
    let Some(start) = sprint.get("start").and_then(Value::as_str) else {
        return false;
    };
    entered(task, &sprint_id(sprint)).is_some_and(|from| from.as_str() > start)
}

/// How many sprints the task was carried through before `sprint`: the other sprints it was in before it entered this
/// one (SR-4.5); 0 when it started here.
fn carried_through(task: &Task, sprint: &str) -> usize {
    let Some(from) = entered(task, sprint) else {
        return 0;
    };
    let mut earlier: Vec<String> = history(task)
        .filter(|(name, at)| *at < from && !name.eq_ignore_ascii_case(sprint))
        .map(|(name, _)| name.to_lowercase())
        .collect();
    earlier.sort();
    earlier.dedup();
    earlier.len()
}

/// One sprint: its stored fields, and its figures:
/// - `done`, `total` and `percent` (the summary's rule: done of the tasks but the skipped, 100 with none);
/// - `added` (pulled in after it started), `carried` (in another sprint before) and `review_pending` (done batch tasks
///   waiting for the end-of-sprint review);
/// - `days_left` (to its due date, by the project's day; negative once overdue) and `due_end` (when the due date has
///   passed), while it isn't closed.
fn sprint_view(vocab: &Vocabulary, sprint: &Map<String, Value>, tasks: &[&Task]) -> Value {
    let id = sprint_id(sprint);
    let own: Vec<&&Task> = tasks
        .iter()
        .filter(|t| {
            vocab
                .find_sprint(t.get("sprint"))
                .is_some_and(|s| sprint_id(s) == id)
        })
        .collect();
    let kinds: Vec<Option<Kind>> = own.iter().map(|t| vocab.kind_of(t.get("status"))).collect();
    let numbers = crate::summary::sprint_numbers(sprint, &kinds);
    let mut view = sprint.clone();
    for key in ["done", "total", "percent"] {
        view.insert(key.into(), numbers[key].clone());
    }
    let count = |f: &dyn Fn(&Task) -> bool| own.iter().filter(|t| f(t)).count();
    view.insert(
        "added".into(),
        json!(count(&|t| added_mid_sprint(t, sprint))),
    );
    view.insert(
        "carried".into(),
        json!(count(&|t| carried_through(t, &id) > 0)),
    );
    view.insert(
        "review_pending".into(),
        json!(count(
            &|t| t.get("sprint_review").and_then(Value::as_str) == Some("pending")
        )),
    );
    due_fields(sprint, &mut view);
    Value::Object(view)
}

/// An open sprint's `days_left` (to its due date, by the project's day) and `due_end` (when the date has passed); none
/// for a closed one or one without a due date. The summary's sprints use it too.
pub(crate) fn due_fields(sprint: &Map<String, Value>, into: &mut Map<String, Value>) {
    let open = sprint.get("state").and_then(Value::as_str) != Some("closed");
    if let Some(due) = sprint.get("due").and_then(Value::as_str).filter(|_| open) {
        if let Some(days) = days_until(due) {
            into.insert("days_left".into(), json!(days));
        }
        if let Some(end) = end_of_day(due) {
            into.insert("due_end".into(), json!(end));
        }
    }
}

/// The sprints and tasks the views count, given to `f` as (vocabulary, working tasks, archived tasks): the project's
/// own; for a project not migrated to sprints yet (phases, no `meta.sprints`), those `atm db migrate` would make of
/// its phases (SR-8.1, the migration's rule, on a copy), so it shows the sprints it will have.
fn with_planned<T>(
    project: &Project,
    f: impl FnOnce(&Vocabulary, &[&Task], &[&Task], bool) -> T,
) -> Result<T> {
    let store = project.store();
    let vocab = store.vocabulary()?;
    let working = store.list_tasks()?;
    let archived = store.list_archived()?;
    let phased = vocab.sprints.is_empty()
        && (store
            .document()?
            .get("meta")
            .and_then(|m| m.get("phases"))
            .is_some_and(truthy)
            || working
                .iter()
                .chain(&archived)
                .any(|t| t.contains_key("phase")));
    if !phased {
        return Ok(f(vocab, &working, &archived, false));
    }
    let mut document = json!({
        "meta": store.document()?.get("meta").cloned().unwrap_or_else(|| json!({})),
        "tasks": working.iter().map(|t| Value::Object((*t).clone())).collect::<Vec<_>>(),
    });
    let mut archive: Vec<Value> = archived
        .iter()
        .map(|t| Value::Object((*t).clone()))
        .collect();
    atm_db::jsongit::migrate::phases_to_sprints(
        &mut document,
        &mut archive,
        vocab,
        &atm_common::time::now_iso(),
    );
    let (migrated, _, _) = Vocabulary::tolerant(document.get("meta"));
    let as_tasks = |list: &[Value]| -> Vec<Task> {
        list.iter().filter_map(|t| t.as_object().cloned()).collect()
    };
    let working = as_tasks(document["tasks"].as_array().map_or(&[][..], Vec::as_slice));
    let archived = as_tasks(&archive);
    let (working, archived): (Vec<&Task>, Vec<&Task>) =
        (working.iter().collect(), archived.iter().collect());
    Ok(f(&migrated, &working, &archived, true))
}

/// The project's vocabulary with the sprints the views count: its own, or for an unmigrated project those its phases
/// would make (as [`with_planned`]), so the stats (ATM-722) see the same sprints as the sprint views.
///
/// # Errors
/// A task file that can't be read.
pub(crate) fn planned_vocabulary(project: &Project) -> Result<Vocabulary> {
    with_planned(project, |vocab, _, _, _| vocab.clone())
}

/// The project's sprints for the dashboard, `{sprints, backlog, from_phases}`: each sprint in its order (closed ones
/// too) with its figures over the working tasks and the archive, how many unfinished tasks are in no sprint, and
/// whether they are an unmigrated project's phases (which can't be changed until `atm db migrate`).
///
/// # Errors
/// A task file that can't be read.
pub fn sprint_views(project: &Project) -> Result<Value> {
    with_planned(project, |vocab, working, archived, from_phases| {
        let all: Vec<&Task> = working.iter().chain(archived).copied().collect();
        let sprints: Vec<Value> = vocab
            .sprints
            .iter()
            .map(|s| sprint_view(vocab, s, &all))
            .collect();
        let backlog = working
            .iter()
            .filter(|t| in_backlog(vocab, t, false))
            .count();
        json!({"sprints": sprints, "backlog": backlog, "from_phases": from_phases})
    })
}

/// A task's place in the sprints, as the views count them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskSprint {
    /// Its sprint's name as the project writes it (matched in any case); for an unmigrated project, its phase's sprint.
    pub sprint: String,
    /// How many sprints it was carried through before this one.
    pub carried: usize,
    /// Its sprint is closed (ATM-719: an unfinished task left there, which the loop never takes).
    pub closed: bool,
}

/// Each task's sprint, by task ID (working and archived tasks). A task in no sprint, or one the project doesn't define,
/// isn't listed.
///
/// # Errors
/// A task file that can't be read.
pub fn task_sprints(project: &Project) -> Result<HashMap<String, TaskSprint>> {
    with_planned(project, |vocab, working, archived, _| {
        working
            .iter()
            .chain(archived)
            .filter_map(|t| {
                let id = t.get("id").and_then(Value::as_str)?;
                let found = vocab.find_sprint(t.get("sprint"))?;
                let closed = found.get("state").and_then(Value::as_str) == Some("closed");
                let sprint = sprint_id(found);
                let carried = carried_through(t, &sprint);
                Some((
                    id.to_string(),
                    TaskSprint {
                        sprint,
                        carried,
                        closed,
                    },
                ))
            })
            .collect()
    })
}

#[cfg(test)]
mod tests;
