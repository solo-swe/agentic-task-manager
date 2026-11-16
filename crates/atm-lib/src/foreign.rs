//! Other projects' tasks (ADR-007 §7): a dependency or related task in another project, stored as
//! `<project id>:<UUID>` and written by its key (`ATM-978`). Found among the projects the user registered
//! (`workspaces/<w>.json`), read only, through the same store and its checks: only their status, name and key are
//! used, and nothing in them is run or followed further (docs/security-architecture.md).

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use atm_common::keys::{key_final, key_of, keys_format, project_key, resolve};
use atm_common::{Error, Kind, Result, Task};
use serde_json::Value;

use crate::project::Project;
use crate::workspaces::{Workspace, Workspaces, folder};

/// Install the lookup the shared rules use for another project's task (once per process).
pub fn install() {
    atm_common::rules::set_foreign_status(status);
}

fn meta_id(project: &Project) -> Option<String> {
    project
        .store()
        .document()
        .ok()?
        .get("meta")?
        .get("id")?
        .as_str()
        .map(String::from)
}

fn same_folder(a: &Path, b: &Path) -> bool {
    let real = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    real(a) == real(b)
}

/// Every registered project once (a project in several workspaces is one), read tolerantly, but the one at `except`.
fn registered(except: Option<&Path>) -> Vec<Project> {
    let workspaces = Workspaces::open();
    let mut seen: Vec<std::path::PathBuf> = Vec::new();
    let mut out = Vec::new();
    for id in workspaces.ids() {
        let Some(workspace) = workspaces.get(&id) else {
            continue;
        };
        for entry in workspace.entries() {
            let path = folder(&entry);
            if except.is_some_and(|e| same_folder(e, &path))
                || seen.iter().any(|s| same_folder(s, &path))
            {
                continue;
            }
            seen.push(path);
            if let Ok(project) = Workspace::project_tolerant(&entry) {
                out.push(project);
            }
        }
    }
    out
}

/// The registered project whose `meta.id` is `id`.
fn project_by_id(id: &str) -> Option<Project> {
    registered(None)
        .into_iter()
        .find(|p| meta_id(p).as_deref() == Some(id))
}

/// The registered projects (but the one at `except`) whose key (`meta.key`) is `key`, in any case.
#[must_use]
pub fn projects_with_key(key: &str, except: Option<&Path>) -> Vec<Project> {
    registered(except)
        .into_iter()
        .filter(|p| {
            p.store()
                .document()
                .ok()
                .and_then(|d| project_key(d).map(|k| k.eq_ignore_ascii_case(key)))
                .unwrap_or(false)
        })
        .collect()
}

fn archive_of(project: &Project) -> Vec<Task> {
    project
        .store()
        .list_archived()
        .map(|a| a.into_iter().cloned().collect())
        .unwrap_or_default()
}

/// The task `<project id>:<UUID>` names, with its project: (the task, archived, the project).
fn find(reference: &str) -> Option<(Task, bool, Project)> {
    let (id, uuid) = reference.split_once(':')?;
    let project = project_by_id(id)?;
    let document = project.store().document().ok()?.clone();
    let archive = archive_of(&project);
    let found = resolve(&document, &archive, uuid).ok()?;
    let (task, archived) = (found.task.clone(), found.archived);
    Some((task, archived, project))
}

/// What [`status`] found, by reference, for the rest of the command.
type Seen = Mutex<HashMap<String, Option<(bool, Value)>>>;

/// What another project's task is, for the rules (`atm_common::rules::ForeignStatus`): done (its status's kind, or
/// archived), and its status; None when it can't be found. Remembered for the rest of the command.
#[must_use]
pub fn status(reference: &str) -> Option<(bool, Value)> {
    static SEEN: OnceLock<Seen> = OnceLock::new();
    let seen = SEEN.get_or_init(Default::default);
    if let Some(known) = seen.lock().ok().and_then(|s| s.get(reference).cloned()) {
        return known;
    }
    let answer = find(reference).map(|(task, archived, project)| {
        let status = task.get("status").cloned().unwrap_or(Value::Null);
        let done = archived
            || project
                .store()
                .vocabulary()
                .is_ok_and(|v| v.kind_of(task.get("status")) == Some(Kind::Done));
        (done, status)
    });
    if let Ok(mut s) = seen.lock() {
        s.insert(reference.to_string(), answer.clone());
    }
    answer
}

/// How another project's task reads: its key (`~` while provisional), or None when it can't be found.
#[must_use]
pub fn label(reference: &str) -> Option<String> {
    let (task, _, project) = find(reference)?;
    let document = project.store().document().ok()?;
    let key = key_of(document, &task)?;
    Some(if key_final(&task) {
        key
    } else {
        format!("~{key}")
    })
}

/// A key of another registered project's task (`ATM-978` from `LFB`), stored as `<project id>:<UUID>`. None when the
/// key's prefix is no other project's.
///
/// # Errors
/// Several projects with that key (rename one: ADR-007 §8); the task not in that project; a project without an ID.
pub fn resolve_in_other_project(reference: &str, here: Option<&Path>) -> Result<Option<String>> {
    let Some((prefix, _)) = reference.trim().rsplit_once('-') else {
        return Ok(None);
    };
    let projects = projects_with_key(prefix, here);
    let project = match projects.as_slice() {
        [] => return Ok(None),
        [one] => one,
        many => {
            return Err(Error::failed(format!(
                "{} projects use the key {prefix}: rename one with atm project key set, or give the task's UUID",
                many.len()
            )));
        }
    };
    let document = project.store().document()?;
    if !keys_format(document) {
        return Ok(None);
    }
    let archive = archive_of(project);
    let found = resolve(document, &archive, reference)?;
    let id = meta_id(project).ok_or_else(|| {
        Error::failed(format!(
            "the project with the key {prefix} has no meta.id (atm db init gives it one)"
        ))
    })?;
    let uuid = found
        .task
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    Ok(Some(format!("{id}:{uuid}")))
}
