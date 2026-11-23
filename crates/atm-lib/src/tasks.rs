//! Reading tasks (docs/layered-architecture.md §5.1): the Python atm's `lib/tasks.py`.

use std::collections::HashMap;

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

use atm_common::task_id as id_of;

/// The links that point at a task, among `tasks` (SR-2.8, ATM-719): the tasks that depend on it, relate to it and
/// duplicate it. Worked out once for every task, as the tasks route serves each one's (`derived.required_by`,
/// `related_by`, `duplicated_by`), in the tasks' own order.
pub struct Backlinks<'a> {
    required: HashMap<&'a str, Vec<(usize, &'a str)>>,
    related: HashMap<&'a str, Vec<(usize, &'a str)>>,
    /// A `related` written as a text, as Python's `in` reads it: any id inside it
    related_text: Vec<(usize, &'a str, &'a str)>,
    duplicated: HashMap<&'a str, Vec<(usize, &'a str)>>,
}

impl<'a> Backlinks<'a> {
    /// The links of `tasks`.
    #[must_use]
    pub fn of(tasks: &[&'a Task]) -> Self {
        let mut links = Backlinks {
            required: HashMap::new(),
            related: HashMap::new(),
            related_text: Vec::new(),
            duplicated: HashMap::new(),
        };
        for (at, t) in tasks.iter().enumerate() {
            let Some(id) = id_of(t) else { continue };
            let strings = |key: &str| {
                t.get(key)
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
            };
            for dependency in strings("dependencies") {
                links.required.entry(dependency).or_default().push((at, id));
            }
            match t.get("related") {
                Some(Value::String(text)) => links.related_text.push((at, id, text)),
                _ => {
                    for related in strings("related") {
                        links.related.entry(related).or_default().push((at, id));
                    }
                }
            }
            if let Some(original) = t.get("duplicate_of").and_then(Value::as_str) {
                links.duplicated.entry(original).or_default().push((at, id));
            }
        }
        links
    }

    fn named(map: &HashMap<&'a str, Vec<(usize, &'a str)>>, id: &str) -> Vec<String> {
        map.get(id)
            .into_iter()
            .flatten()
            .filter(|(_, from)| *from != id)
            .map(|(_, from)| (*from).to_string())
            .collect()
    }

    /// The tasks whose dependencies name `task`.
    #[must_use]
    pub fn required_by(&self, task: &Task) -> Vec<String> {
        id_of(task).map_or_else(Vec::new, |id| Self::named(&self.required, id))
    }

    /// The tasks that relate to `task`, but not those it relates to itself (its own `related`, shown first).
    #[must_use]
    pub fn related_by(&self, task: &Task) -> Vec<String> {
        let Some(id) = id_of(task) else {
            return Vec::new();
        };
        let own: Vec<&str> = task
            .get("related")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        let mut found: Vec<(usize, &str)> = self.related.get(id).cloned().unwrap_or_default();
        found.extend(
            self.related_text
                .iter()
                .filter(|(_, _, text)| text.contains(id))
                .map(|(at, from, _)| (*at, *from)),
        );
        found.sort_by_key(|(at, _)| *at);
        found
            .into_iter()
            .filter(|(_, from)| *from != id && !own.contains(from))
            .map(|(_, from)| from.to_string())
            .collect()
    }

    /// The tasks that duplicate `task`.
    #[must_use]
    pub fn duplicated_by(&self, task: &Task) -> Vec<String> {
        id_of(task).map_or_else(Vec::new, |id| Self::named(&self.duplicated, id))
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
        } else if let Some((_, status)) = atm_common::rules::is_foreign_reference(d)
            .then(|| crate::foreign::status(d))
            .flatten()
        {
            // Another project's task, its status there (ADR-007 §7)
            Dependency {
                id: d.into(),
                status: Some(status),
                archived: false,
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
    // The links pointing at it: the one rule the tasks route serves too (ATM-719)
    let links = Backlinks::of(tasks);
    let others = links.related_by(task);
    let duplicated_by = links.duplicated_by(task);
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
        let mut view = view_task(task, is_archived, &tasks, &archived, vocab)?;
        // With task keys (ADR-007 §1): its key beside its UUID, and the tasks it names by their labels
        let document = store.document()?;
        if atm_common::keys::keys_format(document) {
            let archive: Vec<Task> = store.list_archived()?.into_iter().cloned().collect();
            let label = |id: &str| {
                atm_common::keys::resolve(document, &archive, id).map_or_else(
                    // Another project's task by its key there (ADR-007 §7)
                    |_| crate::foreign::label(id).unwrap_or_else(|| id.to_string()),
                    |f| {
                        atm_common::keys::label_of(&Value::Object(atm_common::keys::with_key(
                            document, f.task,
                        )))
                    },
                )
            };
            for d in &mut view.dependencies {
                d.id = label(&d.id);
            }
            view.related = view.related.iter().map(|r| label(r)).collect();
            view.duplicated_by = view.duplicated_by.iter().map(|r| label(r)).collect();
            view.task = atm_common::keys::with_key(document, &view.task);
        }
        views.push(view);
    }
    Ok(views)
}

#[cfg(test)]
mod tests;
