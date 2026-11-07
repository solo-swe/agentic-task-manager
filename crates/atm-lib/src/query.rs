//! The read commands (docs/layered-architecture.md §5.1): the Python atm's `lib/query.py`: `task list`
//! (`list_matching`, `task_views`) and `task deps`; `project status` comes with its slice. Reads never write.

use std::collections::HashSet;

use atm_common::py::{repr_str, truthy};
use atm_common::rules::{contains, unfinished_dependencies};
use atm_common::{Error, Kind, Result, Task, Vocabulary};
use serde_json::{Map, Value, json};

use crate::project::Project;

/// What a read keeps (`atm task list`'s options); what isn't given keeps everything.
#[derive(Debug, Clone, Default)]
#[allow(clippy::struct_excessive_bools)] // reason: each is one of `atm task list`'s flags, as the Python atm has them
pub struct Filters {
    /// Only these sprints' tasks (names, in any case).
    pub sprint: Vec<String>,
    /// Only tasks in no sprint.
    pub backlog: bool,
    /// Only tasks in this release (their own, or their sprint's).
    pub release: Option<String>,
    /// Only tasks in this component.
    pub component: Option<String>,
    /// Only these statuses.
    pub status: Vec<String>,
    /// Only this type.
    pub type_: Option<String>,
    /// Only this (effective) priority.
    pub priority: Option<String>,
    /// Only tasks with all of these labels.
    pub label: Vec<String>,
    /// Only tasks started by this agent.
    pub agent: Option<String>,
    /// Only to-do tasks whose dependencies are done.
    pub ready: bool,
    /// Only blocked tasks and to-do tasks still waiting.
    pub blocked: bool,
    /// Only tasks in a PR that isn't recorded as merged.
    pub in_pr: bool,
    /// Only tasks done together with the user.
    pub with_user: bool,
    /// The archive's tasks instead of the working ones.
    pub archived: bool,
}

/// In PR: a recorded PR, not merged yet (DM-4.3).
#[must_use]
pub fn in_open_pr(task: &Task) -> bool {
    task.get("pr").is_some_and(Value::is_object) && !task.get("merged_at").is_some_and(truthy)
}

fn id_of(task: &Task) -> Option<&str> {
    task.get("id").and_then(Value::as_str)
}

/// The IDs of the to-do tasks whose dependencies are all done (a dependency only in the archive is done).
///
/// # Errors
/// When the archive has to be read and can't be.
pub fn ready_ids(project: &Project) -> Result<HashSet<String>> {
    let store = project.store();
    let vocab = store.vocabulary()?;
    let tasks = store.list_tasks()?;
    let archived = |id: &str| {
        store
            .get_archived_task(id)
            .map(|t| t.is_some_and(|t| !t.is_empty()))
    };
    let mut ready = HashSet::new();
    for t in &tasks {
        if vocab.kind_of(t.get("status")) == Some(Kind::Todo)
            && unfinished_dependencies(vocab, &tasks, t, &archived)?.is_empty()
            && let Some(id) = id_of(t)
        {
            ready.insert(id.to_string());
        }
    }
    Ok(ready)
}

/// The sprints `--sprint` names, as the project spells them.
fn sprint_names(vocab: &Vocabulary, given: &[String]) -> Result<Vec<String>> {
    given
        .iter()
        .map(|name| {
            vocab
                .find_sprint(Some(&Value::String(name.clone())))
                .and_then(|s| s.get("id").and_then(Value::as_str))
                .map(String::from)
                .ok_or_else(|| {
                    let ids: Vec<&str> = vocab
                        .sprints
                        .iter()
                        .filter_map(|s| s.get("id").and_then(Value::as_str))
                        .collect();
                    let known = if ids.is_empty() {
                        "none".to_string()
                    } else {
                        ids.join(", ")
                    };
                    Error::Failed {
                        message: format!("no sprint {}; the sprints: {known}", repr_str(name)),
                        code: 2,
                    }
                })
        })
        .collect()
}

/// The tasks `--sprint` (repeatable), `--backlog`, `--release` and `--component` keep (SR-3.2): `task list`'s and
/// `project status`'s scope. A task's release is its effective one (its own, else its sprint's).
pub(crate) struct Scope {
    names: Vec<String>,
    backlog: bool,
    release: Option<String>,
    component: Option<String>,
}

impl Scope {
    /// The scope of the filters, after checking that the sprints and the release they name exist.
    pub(crate) fn new(vocab: &Vocabulary, filters: &Filters) -> Result<Self> {
        let names = sprint_names(vocab, &filters.sprint)?;
        // An option given as an empty text keeps everything, as in Python
        let release = filters.release.clone().filter(|r| !r.is_empty());
        if let Some(release) = &release {
            vocab.known_release(release)?;
        }
        let component = filters.component.clone().filter(|c| !c.is_empty());
        Ok(Self {
            names,
            backlog: filters.backlog,
            release,
            component,
        })
    }

    pub(crate) fn keeps(&self, vocab: &Vocabulary, t: &Task) -> bool {
        let sprint = vocab
            .find_sprint(t.get("sprint"))
            .and_then(|s| s.get("id"))
            .and_then(Value::as_str);
        if !self.names.is_empty() && !sprint.is_some_and(|s| self.names.iter().any(|n| n == s)) {
            return false;
        }
        if self.backlog && sprint.is_some_and(|s| !s.is_empty()) {
            return false;
        }
        if let Some(release) = &self.release
            && vocab.effective_release(t).as_ref().and_then(Value::as_str) != Some(release.as_str())
        {
            return false;
        }
        self.component
            .as_deref()
            .is_none_or(|c| contains(t.get("components").filter(|v| truthy(v)), c))
    }
}

/// A task's labels as a set, as Python's `set(labels or [])` makes one (a text's characters, if it is one).
fn label_set(labels: Option<&Value>) -> HashSet<String> {
    match labels {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .map(String::from)
            .collect(),
        Some(Value::String(text)) => text.chars().map(String::from).collect(),
        _ => HashSet::new(),
    }
}

/// `atm task list`: the tasks the filters keep, in the project's order (the archive's with `archived`).
///
/// # Errors
/// An unknown sprint or release (exit code 2); task data that can't be read.
pub fn list_matching<'p>(project: &'p Project, filters: &Filters) -> Result<Vec<&'p Task>> {
    let store = project.store();
    let vocab = store.vocabulary()?;
    let ready = ready_ids(project)?;
    let scope = Scope::new(vocab, filters)?;
    // An option given as an empty text keeps everything, as in Python
    let given = |o: &Option<String>| o.clone().filter(|v| !v.is_empty());
    let (type_, priority, agent) = (
        given(&filters.type_),
        given(&filters.priority),
        given(&filters.agent),
    );
    let source = if filters.archived {
        store.list_archived()?
    } else {
        store.list_tasks()?
    };
    let mut kept = Vec::new();
    for t in source {
        if !scope.keeps(vocab, t) {
            continue;
        }
        let kind = vocab.kind_of(t.get("status"));
        let id = id_of(t);
        let in_ready = id.is_some_and(|id| ready.contains(id));
        let keep = (filters.status.is_empty()
            || t.get("status")
                .and_then(Value::as_str)
                .is_some_and(|s| filters.status.iter().any(|f| f == s)))
            && type_
                .as_ref()
                .is_none_or(|ty| t.get("type").and_then(Value::as_str) == Some(ty.as_str()))
            && priority.as_ref().is_none_or(|p| vocab.priority_of(t) == *p)
            && (filters.label.is_empty() || {
                let have = label_set(t.get("labels").filter(|l| truthy(l)));
                filters.label.iter().all(|l| have.contains(l))
            })
            && agent.as_ref().is_none_or(|a| {
                atm_common::started_by(t).and_then(Value::as_str) == Some(a.as_str())
            })
            && (!filters.ready || in_ready)
            && (!filters.blocked
                || kind == Some(Kind::Blocked)
                || (kind == Some(Kind::Todo) && !in_ready))
            && (!filters.in_pr || in_open_pr(t))
            && (!filters.with_user || t.get("with").and_then(Value::as_str) == Some("user"));
        if keep {
            kept.push(t);
        }
    }
    Ok(kept)
}

/// A done task's stage (DM-4.3): `done`, `in_pr` (a recorded PR, not merged) or `merged`; None for other kinds.
#[must_use]
pub fn task_stage(vocab: &Vocabulary, task: &Task) -> Option<&'static str> {
    if vocab.kind_of(task.get("status")) != Some(Kind::Done) {
        return None;
    }
    if task.get("merged_at").is_some_and(truthy) {
        return Some("merged");
    }
    Some(if task.get("pr").is_some_and(Value::is_object) {
        "in_pr"
    } else {
        "done"
    })
}

/// The tasks the filters keep, each as `{task, derived}`: what atm derives for it (the REST API's tasks,
/// docs/layered-architecture.md §6.2): its kind, whether it is ready, the dependencies it waits for, who a blocked
/// task waits on, its effective priority, review mode and release, its stage, and whether it is archived. What
/// doesn't apply is left out, not null.
///
/// # Errors
/// As [`list_matching`].
pub fn task_views(project: &Project, filters: &Filters) -> Result<Vec<Value>> {
    let store = project.store();
    let vocab = store.vocabulary()?;
    let tasks = store.list_tasks()?;
    let ready = ready_ids(project)?;
    let archived = |id: &str| {
        store
            .get_archived_task(id)
            .map(|t| t.is_some_and(|t| !t.is_empty()))
    };
    // A task a tolerant read found wrong (T-377) says so; what its broken fields would decide isn't decided
    let troubled = if filters.archived {
        Map::new()
    } else {
        store.problems()?.tasks
    };
    let mut views = Vec::new();
    for t in list_matching(project, filters)? {
        let kind = vocab.kind_of(t.get("status"));
        let problems = id_of(t).and_then(|id| troubled.get(id));
        let waiting_for: Vec<String> = if kind == Some(Kind::Todo) {
            unfinished_dependencies(vocab, &tasks, t, &archived)?
                .into_iter()
                .map(|(d, _)| d)
                .collect()
        } else {
            Vec::new()
        };
        let mut derived = Map::new();
        if let Some(kind) = kind {
            derived.insert("kind".into(), json!(kind.as_str()));
        }
        derived.insert(
            "ready".into(),
            json!(id_of(t).is_some_and(|id| ready.contains(id)) && problems.is_none()),
        );
        derived.insert("waiting_for".into(), json!(waiting_for));
        if let Some(problems) = problems {
            derived.insert("problems".into(), problems.clone());
        }
        if kind == Some(Kind::Blocked)
            && let Some(on) = t.get("waiting_on").filter(|v| !v.is_null())
        {
            derived.insert("waiting_on".into(), on.clone());
        }
        derived.insert("priority".into(), json!(vocab.priority_of(t)));
        derived.insert("effective_review".into(), json!(vocab.effective_review(t)));
        if let Some(release) = vocab.effective_release(t) {
            derived.insert("effective_release".into(), release);
        }
        if let Some(stage) = task_stage(vocab, t) {
            derived.insert("stage".into(), json!(stage));
        }
        derived.insert("archived".into(), json!(filters.archived));
        views.push(json!({"task": t, "derived": derived}));
    }
    Ok(views)
}

/// What a tolerant read found wrong with the project's task data as a whole (T-377): entries left out, the
/// vocabularies that fell back to their defaults. Empty for a strict read, which refuses such data instead.
///
/// # Errors
/// When the task data can't be read.
pub fn read_problems(project: &Project) -> Result<Vec<String>> {
    Ok(project.store().problems()?.file)
}

/// What a task is found by: its ID, name, description, type, labels and notes (T-258), in lowercase.
fn search_text(task: &Task) -> String {
    let mut strings: Vec<&str> = ["id", "name", "description", "type"]
        .iter()
        .filter_map(|f| task.get(*f).and_then(Value::as_str))
        .collect();
    if let Some(labels) = task.get("labels").and_then(Value::as_array) {
        strings.extend(labels.iter().filter_map(Value::as_str));
    }
    if let Some(notes) = task
        .get("notes")
        .filter(|n| truthy(n))
        .and_then(Value::as_array)
    {
        strings.extend(
            notes
                .iter()
                .filter_map(|n| n.get("text").and_then(Value::as_str)),
        );
    }
    strings.join(" ").to_lowercase()
}

/// The IDs of the working tasks (not the archive) in which every word of `query` appears, case-insensitively, in the
/// task's text or in `also` (the project's name, so it finds all of its tasks), in the project's order.
///
/// # Errors
/// Task data that can't be read.
pub fn search_tasks(project: &Project, query: &str, also: &str) -> Result<Vec<String>> {
    let query = query.to_lowercase();
    let terms: Vec<&str> = query.split_whitespace().collect();
    if terms.is_empty() {
        return Ok(Vec::new());
    }
    let extra = also.to_lowercase();
    Ok(project
        .store()
        .list_tasks()?
        .into_iter()
        .filter_map(|t| {
            let id = id_of(t)?;
            let text = format!("{extra} {}", search_text(t));
            terms
                .iter()
                .all(|term| text.contains(term))
                .then(|| id.to_string())
        })
        .collect())
}

/// The project's claims, from every working copy, by task.
#[must_use]
pub fn list_claims(project: &Project) -> Vec<Value> {
    let store = project.store().read_claims();
    let mut listed: Vec<Value> = store
        .get("claims")
        .and_then(Value::as_object)
        .map(|c| {
            c.values()
                .filter(|c| c.get("task").is_some_and(Value::is_string))
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    listed.sort_by(|a, b| {
        a["task"]
            .as_str()
            .unwrap_or_default()
            .cmp(b["task"].as_str().unwrap_or_default())
    });
    listed
}

/// `atm task deps`: what a task waits for, and what waits for it, archived tasks included (T-358):
/// `{id, waits_for: [{id, status, name}], waited_on_by: [...]}`. An archived task's status says so (`done, archived`);
/// an unknown dependency is `missing`; a task in both files is the `tasks.json` one.
///
/// # Errors
/// `NotFound` when the task is neither a working nor an archived one; task data that can't be read.
pub fn task_dependencies(project: &Project, task_id: &str) -> Result<Value> {
    let store = project.store();
    let task = match store.get_task(task_id)?.filter(|t| !t.is_empty()) {
        Some(task) => task,
        None => store
            .get_archived_task(task_id)?
            .ok_or_else(|| Error::NotFound(format!("no task {task_id}")))?,
    };
    let tasks = store.list_tasks()?;
    // The last task with an ID, as a dict of them by ID has it
    let live = |id: &str| tasks.iter().rev().find(|t| id_of(t) == Some(id)).copied();
    let name = |t: &Task| t.get("name").cloned().unwrap_or_else(|| json!(""));
    let archived_status = |t: &Task| {
        json!(format!(
            "{}, archived",
            atm_common::py::py_str_opt(t.get("status"))
        ))
    };
    let mut waits_for = Vec::new();
    for d in task
        .get("dependencies")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        let entry = match live(d).filter(|t| !t.is_empty()) {
            Some(t) => {
                json!({"id": d, "status": t.get("status").cloned().unwrap_or(Value::Null), "name": name(t)})
            }
            None => match store.get_archived_task(d)?.filter(|t| !t.is_empty()) {
                Some(t) => json!({"id": d, "status": archived_status(t), "name": name(t)}),
                None => json!({"id": d, "status": "missing", "name": ""}),
            },
        };
        waits_for.push(entry);
    }
    let depends_on_it =
        |t: &Task| contains(t.get("dependencies").filter(|d| d.is_array()), task_id);
    let mut waited_on_by: Vec<Value> = tasks
        .iter()
        .filter(|t| depends_on_it(t))
        .map(|t| json!({"id": t.get("id"), "status": t.get("status"), "name": name(t)}))
        .collect();
    for t in store.list_archived()? {
        if id_of(t).is_none_or(|id| live(id).is_none()) && depends_on_it(t) {
            waited_on_by
                .push(json!({"id": t.get("id"), "status": archived_status(t), "name": name(t)}));
        }
    }
    Ok(json!({"id": task_id, "waits_for": waits_for, "waited_on_by": waited_on_by}))
}

#[cfg(test)]
mod tests;
