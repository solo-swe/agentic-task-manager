//! Other projects' tasks (ADR-007 §7): a dependency or related task in another project, stored as
//! `<project id>:<UUID>` and written by its key (`ATM-978`). Found among the projects the user registered
//! (`workspaces/<w>.json`), read only, through the same store and its checks: only their status, name and key are
//! used, and nothing in them is run or followed further (docs/security-architecture.md).
//!
//! **One cache for the rules and the display** (docs/cross-project-freshness-design.md §5.1, option B), shared by the
//! threads of a long-lived process (`atm serve`, the app):
//! - **the registry** (which projects are registered, where, in which workspaces), per state folder, kept while the
//!   workspaces' files (their IDs, and each file's size and modification time) are unchanged;
//! - **each registered project** (its document, archive and vocabulary, as read), by its registered folder, kept while
//!   its store's `stamp()` for its task data (`tasks`, `archive`) is unchanged. The project is opened (its folder's
//!   checks, no task data read) and its stamp asked for on every use, so a change on disk is seen on the next use.
//!
//! Both also expire after [`FRESH_FOR`], against coarse file timestamps (a write within their resolution that keeps
//! the size). A project whose folder is gone can't be opened, so it is unregistered on the next use; one removed from
//! the workspaces changes their files, so the registry is read again. The cache's lock is never held while reading
//! from disk.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use atm_common::keys::{key_final, key_of, keys_format, project_key, resolve};
use atm_common::{Error, Kind, Result, Task, Vocabulary};
use serde_json::{Value, json};

use crate::project::Project;
use crate::workspaces::{Workspaces, folder};

/// How long a cached project or registry stands without being read again, even when its stamp is unchanged.
pub const FRESH_FOR: Duration = Duration::from_secs(5);

/// Install the lookup the shared rules use for another project's task (once per process).
pub fn install() {
    atm_common::rules::set_foreign_status(status);
}

/// A registered project: its folder as registered, the folder it really is, and where it is registered
/// (`{workspace, project}` for each workspace that lists it; a dashboard address names a project by these).
struct Registered {
    folder: PathBuf,
    real: PathBuf,
    places: Vec<Value>,
}

/// The registered projects of a state folder, and what they were read for.
struct Registry {
    stamp: Value,
    at: Instant,
    projects: Arc<Vec<Registered>>,
}

/// A registered project's task data, as read.
struct Snapshot {
    meta_id: Option<String>,
    document: Value,
    archive: Vec<Task>,
    vocabulary: Option<Vocabulary>,
    /// Changes each time the data read differs from what was cached ([`version`]).
    generation: u64,
}

impl Snapshot {
    fn key(&self) -> Option<&str> {
        project_key(&self.document)
    }

    fn same_data(&self, other: &Self) -> bool {
        self.document == other.document
            && self.archive == other.archive
            && self.vocabulary == other.vocabulary
    }
}

/// A cached project: its stamp when read, when it was read, what was read (none when its task data can't be read).
struct Entry {
    stamp: Value,
    at: Instant,
    snapshot: Option<Arc<Snapshot>>,
    reads: u64,
}

#[derive(Default)]
struct Cache {
    /// By state folder.
    registries: HashMap<PathBuf, Registry>,
    /// By registered folder.
    projects: HashMap<PathBuf, Entry>,
    /// Where a project ID was found last, to look there first.
    ids: HashMap<String, PathBuf>,
}

fn cache() -> MutexGuard<'static, Cache> {
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    CACHE
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
thread_local! {
    /// A test's state folder, for the lookups made on its thread.
    static TEST_HOME: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
    /// A test's expiry, for the lookups made on its thread.
    static TEST_FRESH_FOR: std::cell::Cell<Option<Duration>> = const { std::cell::Cell::new(None) };
}

fn home() -> PathBuf {
    #[cfg(test)]
    if let Some(home) = TEST_HOME.with(|h| h.borrow().clone()) {
        return home;
    }
    atm_common::home::state_dir()
}

fn fresh_for() -> Duration {
    #[cfg(test)]
    if let Some(fresh) = TEST_FRESH_FOR.with(std::cell::Cell::get) {
        return fresh;
    }
    FRESH_FOR
}

fn real(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// The registered projects, every one once (a project in several workspaces is one), as the workspaces' files say
/// now.
fn registry() -> Arc<Vec<Registered>> {
    let home = home();
    let workspaces = Workspaces::at(&home);
    let ids = workspaces.ids();
    let listed: Vec<_> = ids.iter().filter_map(|id| workspaces.get(id)).collect();
    let stamp = json!(
        listed
            .iter()
            .map(|w| json!([w.id(), w.stamp()]))
            .collect::<Vec<_>>()
    );
    if let Some(known) = cache().registries.get(&home)
        && known.stamp == stamp
        && known.at.elapsed() < fresh_for()
    {
        return Arc::clone(&known.projects);
    }
    let mut projects: Vec<Registered> = Vec::new();
    for workspace in &listed {
        for entry in workspace.entries() {
            let path = folder(&entry);
            let real = real(&path);
            let place = json!({"workspace": workspace.id(), "project": entry.get("id")});
            match projects.iter_mut().find(|p| p.real == real) {
                Some(known) => known.places.push(place),
                None => projects.push(Registered {
                    folder: path,
                    real,
                    places: vec![place],
                }),
            }
        }
    }
    let projects = Arc::new(projects);
    let mut cache = cache();
    let Cache {
        registries,
        projects: entries,
        ..
    } = &mut *cache;
    registries.insert(
        home,
        Registry {
            stamp,
            at: Instant::now(),
            projects: Arc::clone(&projects),
        },
    );
    // A project no registry lists any more is forgotten
    let listed: HashSet<&PathBuf> = registries
        .values()
        .flat_map(|r| r.projects.iter().map(|p| &p.folder))
        .collect();
    entries.retain(|folder, _| listed.contains(folder));
    projects
}

/// What a project's task data is now: its store's stamp for it.
fn data_stamp(project: &Project) -> Value {
    let stamp = project.store().stamp();
    json!([stamp["tasks"], stamp["archive"]])
}

fn read(project: &Project) -> Option<Snapshot> {
    static GENERATION: AtomicU64 = AtomicU64::new(0);
    let store = project.store();
    let document = store.document().ok()?.clone();
    let meta_id = document
        .get("meta")
        .and_then(|m| m.get("id"))
        .and_then(Value::as_str)
        .map(String::from);
    Some(Snapshot {
        meta_id,
        archive: store
            .list_archived()
            .map(|a| a.into_iter().cloned().collect())
            .unwrap_or_default(),
        vocabulary: store.vocabulary().ok().cloned(),
        document,
        generation: GENERATION.fetch_add(1, Ordering::Relaxed),
    })
}

/// The task data of the project registered at `folder`: as cached while its stamp is unchanged and the entry fresh,
/// else read again. None when the project can't be opened (its folder gone, refused) or its task data read.
fn snapshot(folder: &Path) -> Option<Arc<Snapshot>> {
    let Ok(project) = Project::open_tolerant(folder) else {
        cache().projects.remove(folder);
        return None;
    };
    let stamp = data_stamp(&project);
    let (previous, reads) = match cache().projects.get(folder) {
        Some(e) if e.stamp == stamp && e.at.elapsed() < fresh_for() => return e.snapshot.clone(),
        Some(e) => (e.snapshot.clone(), e.reads),
        None => (None, 0),
    };
    // Read without the lock; the stamp was taken first, so a change meanwhile is read again on the next use
    let snapshot = match (previous, read(&project)) {
        (Some(old), Some(new)) if old.same_data(&new) => Some(old),
        (_, new) => new.map(Arc::new),
    };
    cache().projects.insert(
        folder.to_path_buf(),
        Entry {
            stamp,
            at: Instant::now(),
            snapshot: snapshot.clone(),
            reads: reads + 1,
        },
    );
    snapshot
}

/// A registered project, read.
struct Other {
    folder: PathBuf,
    places: Vec<Value>,
    snapshot: Arc<Snapshot>,
}

/// Every registered project that can be read, but the one at `except`.
fn registered(except: Option<&Path>) -> Vec<Other> {
    let except = except.map(real);
    registry()
        .iter()
        .filter(|p| except.as_ref() != Some(&p.real))
        .filter_map(|p| {
            Some(Other {
                folder: p.folder.clone(),
                places: p.places.clone(),
                snapshot: snapshot(&p.folder)?,
            })
        })
        .collect()
}

/// The registered project whose `meta.id` is `id`: where it was found last first, else each in turn.
fn project_by_id(id: &str) -> Option<Other> {
    let projects = registry();
    let found = |p: &Registered| {
        let snapshot = snapshot(&p.folder)?;
        (snapshot.meta_id.as_deref() == Some(id)).then(|| Other {
            folder: p.folder.clone(),
            places: p.places.clone(),
            snapshot,
        })
    };
    let last = cache().ids.get(id).cloned();
    if let Some(last) = last
        && let Some(other) = projects.iter().find(|p| p.folder == last).and_then(found)
    {
        return Some(other);
    }
    let other = projects.iter().find_map(found)?;
    cache().ids.insert(id.to_string(), other.folder.clone());
    Some(other)
}

/// The registered projects (but the one at `except`) whose key (`meta.key`) is `key`, in any case.
#[must_use]
pub fn projects_with_key(key: &str, except: Option<&Path>) -> Vec<Project> {
    with_key(key, except)
        .into_iter()
        .filter_map(|o| Project::open_tolerant(&o.folder).ok())
        .collect()
}

fn with_key(key: &str, except: Option<&Path>) -> Vec<Other> {
    registered(except)
        .into_iter()
        .filter(|o| {
            o.snapshot
                .key()
                .is_some_and(|k| k.eq_ignore_ascii_case(key))
        })
        .collect()
}

/// What another project's task is, for the rules (`atm_common::rules::ForeignStatus`): done (its status's kind, or
/// archived), and its status; None when it can't be found. As its project is now (see the module's cache).
#[must_use]
pub fn status(reference: &str) -> Option<(bool, Value)> {
    let (id, uuid) = reference.split_once(':')?;
    let other = project_by_id(id)?;
    let snapshot = &other.snapshot;
    let found = resolve(&snapshot.document, &snapshot.archive, uuid).ok()?;
    let status = found.task.get("status").cloned().unwrap_or(Value::Null);
    let done = found.archived
        || snapshot
            .vocabulary
            .as_ref()
            .is_some_and(|v| v.kind_of(found.task.get("status")) == Some(Kind::Done));
    Some((done, status))
}

/// How another project's task reads: its key (`~` while provisional); its UUID's first 8 characters when another
/// registered project uses the same key too, as the key would be ambiguous (ADR-007 §8); None when it can't be found.
#[must_use]
pub fn label(reference: &str) -> Option<String> {
    let (id, uuid) = reference.split_once(':')?;
    let other = project_by_id(id)?;
    let snapshot = &other.snapshot;
    let task = resolve(&snapshot.document, &snapshot.archive, uuid)
        .ok()?
        .task;
    let key = key_of(&snapshot.document, task)?;
    let prefix = snapshot.key()?;
    if with_key(prefix, None).len() > 1 {
        let uuid = task.get("id").and_then(Value::as_str).unwrap_or_default();
        return Some(uuid.chars().take(8).collect());
    }
    Some(if key_final(task) {
        key
    } else {
        format!("~{key}")
    })
}

/// The project IDs the working tasks of `project` depend on (`<project id>:<UUID>` in `dependencies`), each once: what
/// its rules read of other projects.
#[must_use]
pub fn referenced(project: &Project) -> Vec<String> {
    let mut ids: Vec<String> = project
        .store()
        .list_tasks()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|t| t.get("dependencies").and_then(Value::as_array))
        .flatten()
        .filter_map(Value::as_str)
        .filter(|d| atm_common::rules::is_foreign_reference(d))
        .filter_map(|d| d.split_once(':').map(|(id, _)| id.to_string()))
        .collect();
    ids.sort();
    ids.dedup();
    ids
}

/// What the projects with these IDs are now, as a value that changes when their task data does (null for one that
/// can't be found): for a cache of what was worked out from them (a project's summary).
#[must_use]
pub fn version(ids: &[String]) -> Value {
    json!(
        ids.iter()
            .map(|id| project_by_id(id).map_or(Value::Null, |o| json!(o.snapshot.generation)))
            .collect::<Vec<_>>()
    )
}

/// Other projects' tasks as a task's view shows them (ADR-007 §7), looked up once each for one read: the registered
/// projects are checked once, and each reference looked up once.
#[derive(Default)]
pub struct Lookup {
    projects: Option<Vec<Other>>,
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
fn look_up(projects: &[Other], reference: &str) -> Value {
    let Some((id, uuid)) = reference.split_once(':') else {
        return json!({"project": reference, "found": false, "reason": "unregistered"});
    };
    let Some(other) = projects
        .iter()
        .find(|o| o.snapshot.meta_id.as_deref() == Some(id))
    else {
        return json!({"project": id, "found": false, "reason": "unregistered"});
    };
    let snapshot = &other.snapshot;
    let document = &snapshot.document;
    let name = document
        .get("meta")
        .and_then(|m| m.get("project"))
        .cloned()
        .unwrap_or(Value::Null);
    let places = &other.places;
    let Ok(found) = resolve(document, &snapshot.archive, uuid) else {
        return json!({
            "project": id, "project_name": name, "places": places, "found": false, "reason": "missing",
        });
    };
    let task = found.task;
    let kind = if found.archived {
        Some(Kind::Done)
    } else {
        snapshot
            .vocabulary
            .as_ref()
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
        let prefix = snapshot.key().unwrap_or_default();
        let sharing = projects
            .iter()
            .filter(|o| {
                o.snapshot
                    .key()
                    .is_some_and(|k| k.eq_ignore_ascii_case(prefix))
            })
            .count();
        view["ambiguous"] = json!(sharing > 1);
    }
    view
}

/// Another registered project's task by its UUID alone (not the one at `here`): `<project id>:<UUID>`.
#[must_use]
pub fn find_uuid(uuid: &str, here: Option<&Path>) -> Option<String> {
    registered(here).into_iter().find_map(|other| {
        let snapshot = &other.snapshot;
        resolve(&snapshot.document, &snapshot.archive, uuid).ok()?;
        Some(format!("{}:{uuid}", snapshot.meta_id.as_deref()?))
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
    let projects = with_key(prefix, here);
    let other = match projects.as_slice() {
        [] => return Ok(None),
        [one] => one,
        many => {
            return Err(Error::failed(format!(
                "{} projects use the key {prefix}: rename one with atm project key set, or give the task's UUID",
                many.len()
            )));
        }
    };
    let snapshot = &other.snapshot;
    if !keys_format(&snapshot.document) {
        return Ok(None);
    }
    let found = resolve(&snapshot.document, &snapshot.archive, reference)?;
    let id = snapshot.meta_id.clone().ok_or_else(|| {
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

#[cfg(test)]
mod tests;
