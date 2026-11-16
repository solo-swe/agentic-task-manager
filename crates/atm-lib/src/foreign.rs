//! Other projects' tasks (ADR-007 §7): a dependency or related task in another project, stored as
//! `<project id>:<UUID>` and written by its key (`ATM-978`). Found among the projects the user registered
//! (`workspaces/<w>.json`), read only, through the same store and its checks: only their status, name and key are
//! used, and nothing in them is run or followed further (docs/security-architecture.md).

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use atm_common::keys::{key_final, key_of, keys_format, project_key, resolve};
use atm_common::{Error, Kind, Result, Task};
use serde_json::{Value, json};

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

/// How another project's task reads: its key (`~` while provisional); its UUID's first 8 characters when another
/// registered project uses the same key too, as the key would be ambiguous (ADR-007 §8); None when it can't be found.
#[must_use]
pub fn label(reference: &str) -> Option<String> {
    let (task, _, project) = find(reference)?;
    let document = project.store().document().ok()?;
    let key = key_of(document, &task)?;
    let prefix = project_key(document)?;
    if projects_with_key(prefix, None).len() > 1 {
        let uuid = task.get("id").and_then(Value::as_str).unwrap_or_default();
        return Some(uuid.chars().take(8).collect());
    }
    Some(if key_final(&task) {
        key
    } else {
        format!("~{key}")
    })
}

/// Where a project is registered: `{workspace, project}`, its workspace's ID and its ID there, for each workspace that
/// lists it (a dashboard address names a project by these).
fn places(project: &Project) -> Vec<Value> {
    let Some(root) = project.root() else {
        return Vec::new();
    };
    let workspaces = Workspaces::open();
    let mut out = Vec::new();
    for id in workspaces.ids() {
        let Some(workspace) = workspaces.get(&id) else {
            continue;
        };
        for entry in workspace.entries() {
            if same_folder(&folder(&entry), root) {
                out.push(json!({"workspace": id, "project": entry.get("id")}));
            }
        }
    }
    out
}

/// Other projects' tasks as a task's view shows them (ADR-007 §7), looked up once each for one read: the registered
/// projects are read once, and each reference once.
#[derive(Default)]
pub struct Lookup {
    projects: Option<Vec<Project>>,
    seen: HashMap<String, Value>,
}

impl Lookup {
    /// What the dashboard shows of the task `<project id>:<UUID>` names: `{project, found}`; found, the project's name
    /// and where it is registered (`places`), and the task's `key` and `key_final` (in a project with task keys),
    /// `ambiguous` (another registered project has the same key, ADR-007 §8), `name`, `status`, `kind` and `archived`;
    /// not found, `reason`: `unregistered` (no registered project has that ID, or its folder is gone) or `missing`
    /// (the project has no such task), with the project's name and places when it is there. Only those fields are
    /// read from the other project.
    pub fn view(&mut self, reference: &str) -> Value {
        if let Some(known) = self.seen.get(reference) {
            return known.clone();
        }
        let projects = self.projects.get_or_insert_with(|| registered(None));
        let answer = look_up(projects, reference);
        self.seen.insert(reference.to_string(), answer.clone());
        answer
    }
}

/// [`Lookup::view`], among the registered `projects`.
fn look_up(projects: &[Project], reference: &str) -> Value {
    let Some((id, uuid)) = reference.split_once(':') else {
        return json!({"project": reference, "found": false, "reason": "unregistered"});
    };
    let Some(project) = projects.iter().find(|p| meta_id(p).as_deref() == Some(id)) else {
        return json!({"project": id, "found": false, "reason": "unregistered"});
    };
    let Ok(document) = project.store().document() else {
        return json!({"project": id, "found": false, "reason": "unregistered"});
    };
    let name = document
        .get("meta")
        .and_then(|m| m.get("project"))
        .cloned()
        .unwrap_or(Value::Null);
    let places = places(project);
    let archive = archive_of(project);
    let Ok(found) = resolve(document, &archive, uuid) else {
        return json!({
            "project": id, "project_name": name, "places": places, "found": false, "reason": "missing",
        });
    };
    let task = found.task;
    let kind = if found.archived {
        Some(Kind::Done)
    } else {
        project
            .store()
            .vocabulary()
            .ok()
            .and_then(|v| v.kind_of(task.get("status")))
    };
    let mut view = json!({
        "project": id,
        "project_name": name,
        "places": places,
        "found": true,
        "name": task.get("name").cloned().unwrap_or_else(|| json!("")),
        "status": task.get("status").cloned().unwrap_or(Value::Null),
        "archived": found.archived,
    });
    if let Some(kind) = kind {
        view["kind"] = json!(kind.as_str());
    }
    if let Some(key) = key_of(document, task) {
        view["key"] = json!(key);
        view["key_final"] = json!(key_final(task));
        let prefix = project_key(document).unwrap_or_default().to_string();
        let sharing = projects
            .iter()
            .filter(|p| {
                p.store()
                    .document()
                    .ok()
                    .and_then(|d| project_key(d).map(|k| k.eq_ignore_ascii_case(&prefix)))
                    .unwrap_or(false)
            })
            .count();
        view["ambiguous"] = json!(sharing > 1);
    }
    view
}

/// Another registered project's task by its UUID alone (not the one at `here`): `<project id>:<UUID>`.
#[must_use]
pub fn find_uuid(uuid: &str, here: Option<&Path>) -> Option<String> {
    registered(here).into_iter().find_map(|project| {
        let document = project.store().document().ok()?.clone();
        let archive = archive_of(&project);
        resolve(&document, &archive, uuid).ok()?;
        Some(format!("{}:{uuid}", meta_id(&project)?))
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
