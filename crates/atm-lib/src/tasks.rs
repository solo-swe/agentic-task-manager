//! Reading tasks (docs/layered-architecture.md §5.1): the Python atm's `lib/tasks.py`.

use atm_common::py::truthy;
use atm_common::{Error, Result, Task, TaskId, Vocabulary};
use serde_json::Value;

use crate::project::Project;

/// One of a task's dependencies, with its status where it is.
#[derive(Debug, Clone, PartialEq)]
pub struct Dependency {
    /// The dependency's ID.
    pub id: String,
    /// Its status; None when there is no such task (or it has no status).
    pub status: Option<Value>,
    /// Whether it was found in the archive.
    pub archived: bool,
}

/// A task with what is derived from the rest of the project.
#[derive(Debug, Clone, PartialEq)]
pub struct TaskView {
    /// The task as stored.
    pub task: Task,
    /// Whether it came from the archive.
    pub archived: bool,
    /// Its effective priority.
    pub priority: String,
    /// Who started it (`started_by`, or `agent` in older files).
    pub started_by: Option<Value>,
    /// Its dependencies' statuses.
    pub dependencies: Vec<Dependency>,
    /// The tasks related to it, either way (`related` is stored on one side, SR-2.8): its own first.
    pub related: Vec<String>,
    /// The tasks that duplicate it.
    pub duplicated_by: Vec<String>,
}

fn id_of(task: &Task) -> Option<&str> {
    task.get("id").and_then(Value::as_str)
}

/// Whether `id` is in a task's `related`, as Python's `in` reads it: an element of a list, part of a text.
fn relates_to(related: Option<&Value>, id: &str) -> bool {
    match related {
        Some(Value::Array(items)) => items.iter().any(|r| r == id),
        Some(Value::String(text)) => text.contains(id),
        _ => false,
    }
}

/// A task with its effective priority, its dependencies' statuses, the tasks related to it either way and those
/// that duplicate it. `tasks` are the working tasks, `archived` looks a task up in the archive.
///
/// # Errors
/// When the archive has to be read for a dependency and can't be.
pub fn view_task(
    task: &Task,
    is_archived: bool,
    tasks: &[&Task],
    archived: &dyn Fn(&str) -> Result<Option<Task>>,
    vocab: &Vocabulary,
) -> Result<TaskView> {
    let mut dependencies = Vec::new();
    for d in task
        .get("dependencies")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        // The last task with the ID, as a dict of the tasks by ID has it
        let dependency = if let Some(live) = tasks.iter().rev().find(|t| id_of(t) == Some(d)) {
            Dependency {
                id: d.into(),
                status: live.get("status").cloned(),
                archived: false,
            }
        } else if let Some(old) = archived(d)?.filter(|t| !t.is_empty()) {
            Dependency {
                id: d.into(),
                status: old.get("status").cloned(),
                archived: true,
            }
        } else {
            Dependency {
                id: d.into(),
                status: None,
                archived: false,
            }
        };
        dependencies.push(dependency);
    }
    let own: Vec<String> = task
        .get("related")
        .filter(|r| truthy(r))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(String::from)
        .collect();
    let task_id = id_of(task).unwrap_or_default();
    let others = tasks
        .iter()
        .filter(|t| !std::ptr::eq(**t, task) && relates_to(t.get("related"), task_id))
        .filter_map(|t| id_of(t))
        .filter(|id| !own.iter().any(|o| o == id))
        .map(String::from)
        .collect::<Vec<_>>();
    let duplicated_by = tasks
        .iter()
        .filter(|t| t.get("duplicate_of").and_then(Value::as_str) == Some(task_id))
        .filter_map(|t| id_of(t))
        .map(String::from)
        .collect();
    Ok(TaskView {
        task: task.clone(),
        archived: is_archived,
        priority: vocab.priority_of(task),
        started_by: atm_common::started_by(task).cloned(),
        dependencies,
        related: own.into_iter().chain(others).collect(),
        duplicated_by,
    })
}

/// `atm task details`: the tasks in full, archived ones too, in the order asked for.
///
/// # Errors
/// `NotFound` for an ID that is neither a working nor an archived task; any error reading the task data.
pub fn show_tasks(project: &Project, ids: &[TaskId]) -> Result<Vec<TaskView>> {
    let store = project.store();
    let tasks = store.list_tasks()?;
    let vocab = store.vocabulary()?;
    let archived = |id: &str| store.get_archived_task(id).map(Option::<&Task>::cloned);
    let mut views = Vec::new();
    for id in ids {
        // The store's own task, not a copy: a task is never related to itself
        let (task, is_archived) = match store.get_task(id.as_str())? {
            Some(task) => (task, false),
            None => match store.get_archived_task(id.as_str())? {
                Some(task) => (task, true),
                None => return Err(Error::NotFound(format!("no task {id}"))),
            },
        };
        views.push(view_task(task, is_archived, &tasks, &archived, vocab)?);
    }
    Ok(views)
}

#[cfg(test)]
mod tests;
