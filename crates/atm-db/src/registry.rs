//! The user's project registry, the `default` workspace (`<state>/workspaces/default.json`, ADR-003; it was
//! `projects.json` before, and is renamed the first time it is used): the projects the dashboard server shows (its
//! format is docs/multi-project-api.md §5.1). T-491 gives atm-lib its workspaces on top of it.
//!
//! One file per state folder, guarded by the folder's lock (never taken while holding a project's), written
//! atomically. A linked git worktree registers its repository's main worktree.

use std::path::{Path, PathBuf};

use atm_common::Shown;

use atm_common::home::{ensure_home, state_dir, write_text_atomic};
use atm_common::py::to_json;
use atm_common::sha1::sha1_hex;
use atm_common::time::{format_iso, now, now_iso, parse_iso};
use atm_common::{Error, Result, TASK_DIR, TASKS_FILE};
use serde_json::{Map, Value, json};

use crate::jsongit::files::read_json;
use crate::jsongit::lock::lock;
use crate::jsongit::paths::{Paths, find_paths, main_worktree};

/// The registry's file, the `default` workspace, in the state folder.
pub const REGISTRY_FILE: &str = "workspaces/default.json";
/// Its name before ADR-003, renamed to [`REGISTRY_FILE`] when found.
pub const LEGACY_REGISTRY_FILE: &str = "projects.json";

/// The registry's file in the state folder `home`: an old `projects.json` there is renamed into place first (one
/// atomic step; if two processes race, one renames and the other finds it done).
#[must_use]
pub fn registry_file(home: &Path) -> PathBuf {
    let file = home.join(REGISTRY_FILE);
    let legacy = home.join(LEGACY_REGISTRY_FILE);
    if !file.exists() && legacy.is_file() {
        if let Some(folder) = file.parent() {
            let _ = std::fs::create_dir_all(folder);
        }
        let _ = std::fs::rename(&legacy, &file);
    }
    file
}
const REGISTRY_VERSION: u64 = 1;
/// Seconds between `seen_at` updates, so the registry isn't rewritten on every command.
const SEEN_EVERY: i64 = 24 * 3600;

/// A project ID is a key in URLs (`#project=<id>`, `p/<id>/…`): letters, digits, `.`, `_` and `-`.
fn valid_project_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

fn text(path: &Path) -> String {
    path.shown()
}

fn entries(registry: &Value) -> &[Value] {
    registry["projects"].as_array().map_or(&[], Vec::as_slice)
}

fn entries_mut(registry: &mut Value) -> &mut Vec<Value> {
    if !registry["projects"].is_array() {
        registry["projects"] = json!([]);
    }
    registry["projects"]
        .as_array_mut()
        .expect("made a list just above")
}

/// The ID for the project at `path`: `wanted` unless another project already has it or it isn't a valid ID; then one
/// made from the path, so two projects never shadow each other (F18).
fn unique_id(registry: &Value, path: &str, wanted: &str) -> String {
    let taken = entries(registry)
        .iter()
        .filter(|e| e.get("path").and_then(Value::as_str) != Some(path))
        .any(|e| e.get("id").and_then(Value::as_str) == Some(wanted));
    if valid_project_id(wanted) && !taken {
        return wanted.to_string();
    }
    let suffix = &sha1_hex(path.as_bytes())[..7];
    let base = if valid_project_id(wanted) && wanted.len() <= 56 {
        wanted
    } else {
        "p"
    };
    format!("{base}-{suffix}")
}

/// The meta of the project at `path`, or none without a readable tasks.json.
fn project_meta(path: &Path) -> Option<Map<String, Value>> {
    let data = read_json(&path.join(TASK_DIR).join(TASKS_FILE))?;
    if !data.get("tasks").is_some_and(Value::is_array) {
        return None;
    }
    Some(
        data.get("meta")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default(),
    )
}

/// `meta.id`, else `p` and 7 hex characters of the path's SHA-1 (stable until `meta.id` is set).
fn project_id(meta: &Map<String, Value>, path: &str) -> String {
    match meta
        .get("id")
        .and_then(Value::as_str)
        .filter(|i| !i.is_empty())
    {
        Some(id) => id.to_string(),
        None => format!("p{}", &sha1_hex(path.as_bytes())[..7]),
    }
}

fn project_name(meta: &Map<String, Value>, path: &Path) -> String {
    meta.get("project")
        .and_then(Value::as_str)
        .filter(|p| !p.is_empty())
        .map_or_else(
            || {
                path.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default()
            },
            String::from,
        )
}

/// Whether the project shows on the dashboard: unless `meta.dashboard.listed` is false.
fn listed(meta: &Map<String, Value>) -> bool {
    meta.get("dashboard").and_then(|d| d.get("listed")) != Some(&Value::Bool(false))
}

fn is_missing(entry: &Value) -> bool {
    entry
        .get("path")
        .and_then(Value::as_str)
        .is_none_or(|p| project_meta(Path::new(p)).is_none())
}

fn empty_registry() -> Value {
    json!({"version": REGISTRY_VERSION, "projects": []})
}

/// The registry and, when its file was unreadable, where it was moved (`default.json.damaged-<time>`; an empty
/// registry replaces it). Called holding the registry's lock.
fn read_registry(path: &Path) -> Result<(Value, Option<String>)> {
    if !path.exists() {
        return Ok((empty_registry(), None));
    }
    let data = read_json(path);
    let usable = data
        .as_ref()
        .and_then(|d| d.get("projects"))
        .and_then(Value::as_array)
        .is_some_and(|list| {
            list.iter()
                .all(|e| e.get("path").is_some_and(Value::is_string))
        });
    if let (true, Some(data)) = (usable, data) {
        return Ok((data, None));
    }
    let stamp: String = format_iso(now())
        .chars()
        .filter(|c| *c != '-' && *c != ':')
        .collect();
    let damaged = format!("{}.damaged-{stamp}", path.shown());
    std::fs::rename(path, &damaged).map_err(|e| Error::failed(format!("{}: {e}", path.shown())))?;
    Ok((empty_registry(), Some(damaged)))
}

fn write_registry(path: &Path, registry: &Value) -> Result<()> {
    if let Some(folder) = path.parent() {
        ensure_home(folder).map_err(|e| Error::failed(format!("{}: {e}", folder.shown())))?;
    }
    write_text_atomic(path, &(to_json(registry) + "\n"))
        .map_err(|e| Error::failed(format!("{}: {e}", path.shown())))
}

fn position(registry: &Value, path: &str) -> Option<usize> {
    entries(registry)
        .iter()
        .position(|e| e.get("path").and_then(Value::as_str) == Some(path))
}

/// Whether the entry's `seen_at` is a day old (or unreadable): time to refresh it.
fn seen_long_ago(entry: &Value) -> bool {
    entry
        .get("seen_at")
        .and_then(Value::as_str)
        .and_then(parse_iso)
        .is_none_or(|seen| now() - seen >= SEEN_EVERY)
}

/// Add, refresh or (when the project opted out) remove the entry for `path`. Returns what happened: `added`,
/// `updated`, `unchanged`, `removed` or `unlisted`.
fn update_entry(registry: &mut Value, path: &Path, meta: &Map<String, Value>) -> &'static str {
    let key = text(path);
    let at = position(registry, &key);
    if !listed(meta) {
        if let Some(at) = at {
            entries_mut(registry).remove(at);
            return "removed";
        }
        return "unlisted";
    }
    let stamp = now_iso();
    let id = unique_id(registry, &key, &project_id(meta, &key));
    let name = project_name(meta, path);
    let Some(at) = at else {
        entries_mut(registry).push(json!({
            "id": id, "name": name, "path": key, "added_at": stamp, "seen_at": stamp,
        }));
        return "added";
    };
    let entry = &mut entries_mut(registry)[at];
    let changed = entry.get("id").and_then(Value::as_str) != Some(id.as_str())
        || entry.get("name").and_then(Value::as_str) != Some(name.as_str());
    if !changed && !seen_long_ago(entry) {
        return "unchanged";
    }
    entry["id"] = json!(id);
    entry["name"] = json!(name);
    entry["seen_at"] = json!(stamp);
    "updated"
}

/// The project of `paths`, by its main worktree, and its meta; none without a tasks.json.
fn registered_folder(paths: &Paths) -> Option<(PathBuf, Map<String, Value>)> {
    let path = main_worktree(&paths.root);
    let meta = project_meta(&path).or_else(|| project_meta(&paths.root))?;
    Some((path, meta))
}

fn home() -> Result<PathBuf> {
    let home = state_dir();
    ensure_home(&home).map_err(|e| Error::failed(format!("{}: {e}", home.shown())))?;
    Ok(home)
}

/// List the project of `paths` on the dashboard (its main worktree). With `wait` false the registry's lock is tried
/// once and the project is skipped if it is taken (the plugin's hook never waits). Returns what the registry did,
/// `busy` when skipped, or none when the project has no tasks.json.
///
/// # Errors
/// The registry's lock or file, when waiting.
pub fn register(paths: &Paths, wait: bool) -> Result<Option<&'static str>> {
    let Some((path, meta)) = registered_folder(paths) else {
        return Ok(None);
    };
    let home = home()?;
    let held = match lock(&home, if wait { None } else { Some(0.0) }) {
        Ok(held) => held,
        Err(_) if !wait => return Ok(Some("busy")),
        Err(e) => return Err(e),
    };
    let file = registry_file(&home);
    let (mut registry, _) = read_registry(&file)?;
    let result = update_entry(&mut registry, &path, &meta);
    if !matches!(result, "unchanged" | "unlisted") {
        write_registry(&file, &registry)?;
    }
    drop(held);
    Ok(Some(result))
}

/// (id, listed) of the project of `paths` as the registry has it: the ID it gave the project (`meta.id`, or another
/// when that one is taken), and whether it is listed on the dashboard.
#[must_use]
pub fn registered_id(paths: &Paths) -> (String, bool) {
    let path = main_worktree(&paths.root);
    let meta = project_meta(&path)
        .or_else(|| project_meta(&paths.root))
        .unwrap_or_default();
    let key = text(&path);
    let registry = read_json(&registry_file(&state_dir())).unwrap_or(Value::Null);
    let given = registry
        .get("projects")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|e| e.get("path").and_then(Value::as_str) == Some(key.as_str()))
        .and_then(|e| e.get("id"))
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .map(String::from);
    (
        given.unwrap_or_else(|| project_id(&meta, &key)),
        listed(&meta),
    )
}

/// The plugin hook's cheap path, run after every tool call: one read of the registry, and registering (without
/// waiting for the lock) only when the project isn't listed yet or was last seen a day ago.
///
/// # Errors
/// The registry's file, when it has to be written.
pub fn register_if_needed(paths: &Paths) -> Result<Option<&'static str>> {
    let path = text(&main_worktree(&paths.root));
    let registry = read_json(&registry_file(&state_dir())).unwrap_or(Value::Null);
    let entry = registry
        .get("projects")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|e| e.get("path").and_then(Value::as_str) == Some(path.as_str()));
    if entry.is_some_and(|e| !seen_long_ago(e)) {
        return Ok(Some("unchanged"));
    }
    register(paths, false)
}

/// The main worktree folder of the project at `target` (a folder in it; here when none), and its meta.
fn resolve_project(target: Option<&str>, cwd: &Path) -> Result<(PathBuf, Map<String, Value>)> {
    let folder = match target.filter(|t| !t.is_empty()) {
        Some(t) => crate::jsongit::logo::normalize(&cwd.join(expand_home(t))),
        None => cwd.to_path_buf(),
    };
    if !folder.is_dir() {
        return Err(Error::Failed {
            message: format!("{}: not a directory", target.unwrap_or_default()),
            code: 2,
        });
    }
    let paths = find_paths(&folder)?;
    let path = main_worktree(&paths.root);
    let meta = project_meta(&path)
        .or_else(|| project_meta(&paths.root))
        .ok_or_else(|| {
            Error::failed(format!(
                "no tasks.json in {}. Run /atm-create-tasks there first.",
                path.join(TASK_DIR).shown()
            ))
        })?;
    Ok((path, meta))
}

/// `~` and `~/…` as the user's home folder, as Python's `expanduser` reads them.
fn expand_home(path: &str) -> PathBuf {
    match (path.strip_prefix('~'), atm_common::home::home_dir()) {
        (Some(rest), Some(home)) if rest.is_empty() || rest.starts_with('/') => {
            home.join(rest.trim_start_matches('/'))
        }
        _ => PathBuf::from(path),
    }
}

/// The workspace every project atm meets joins, which always exists (ADR-003 §1).
pub const DEFAULT_WORKSPACE: &str = "default";
/// The folder of the workspaces' files in the state folder.
pub const WORKSPACES_DIR: &str = "workspaces";

/// A workspace ID is a file name and a key in URLs: letters, digits, `.`, `_` and `-`, not starting with `.`.
#[must_use]
pub fn valid_workspace_id(id: &str) -> bool {
    valid_project_id(id) && !id.starts_with('.')
}

/// A workspace's file in the state folder `home`: `workspaces/<id>.json` (`default`'s renamed from `projects.json`
/// first, [`registry_file`]).
#[must_use]
pub fn workspace_file(home: &Path, id: &str) -> PathBuf {
    if id == DEFAULT_WORKSPACE {
        return registry_file(home);
    }
    home.join(WORKSPACES_DIR).join(format!("{id}.json"))
}

/// A workspace's list of projects, its file in the state folder (`workspaces/<id>.json`; the `default` one is the
/// registry). Each method holds the state folder's lock while it reads and writes, and says where an unreadable file
/// was moved (`damaged`).
#[derive(Debug, Clone)]
pub struct FileRegistry {
    home: PathBuf,
    workspace: String,
}

impl Default for FileRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl FileRegistry {
    /// The registry (the `default` workspace) of atm's state folder.
    #[must_use]
    pub fn new() -> Self {
        Self::at(&state_dir())
    }

    /// The registry (the `default` workspace) of the state folder `home` (the server's, given on its command line).
    #[must_use]
    pub fn at(home: &Path) -> Self {
        Self::workspace(home, DEFAULT_WORKSPACE)
    }

    /// The workspace `id` of the state folder `home` (a valid ID: [`valid_workspace_id`]).
    #[must_use]
    pub fn workspace(home: &Path, id: &str) -> Self {
        Self {
            home: home.to_path_buf(),
            workspace: id.to_string(),
        }
    }

    /// Its ID.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.workspace
    }

    /// Whether its file is there (`default` always counts as there).
    #[must_use]
    pub fn exists(&self) -> bool {
        self.workspace == DEFAULT_WORKSPACE || self.file().is_file()
    }

    fn file(&self) -> PathBuf {
        workspace_file(&self.home, &self.workspace)
    }

    /// The entries as the file has them now, without the lock (the server reads them on every request): those with
    /// a `path`.
    #[must_use]
    pub fn entries(&self) -> Vec<Value> {
        let registry = read_json(&self.file()).unwrap_or(Value::Null);
        registry
            .get("projects")
            .and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .filter(|e| e.get("path").is_some_and(Value::is_string))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The file's size and modification time, null while there is none: it changes when the list does.
    #[must_use]
    pub fn stamp(&self) -> Value {
        let Ok(meta) = std::fs::metadata(self.file()) else {
            return Value::Null;
        };
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_nanos());
        json!([meta.len(), mtime.to_string()])
    }

    fn locked(&self) -> Result<crate::jsongit::lock::Lock> {
        ensure_home(&self.home)
            .map_err(|e| Error::failed(format!("{}: {e}", self.home.shown())))?;
        lock(&self.home, None)
    }

    /// `{projects: the entries, each with missing (no readable tasks.json there now), damaged}`.
    ///
    /// # Errors
    /// The registry's lock or file.
    pub fn list_projects(&self) -> Result<Value> {
        let held = self.locked()?;
        let (registry, damaged) = read_registry(&self.file())?;
        if damaged.is_some() {
            write_registry(&self.file(), &registry)?;
        }
        drop(held);
        let projects: Vec<Value> = entries(&registry)
            .iter()
            .map(|e| {
                let mut entry = e.clone();
                entry["missing"] = json!(is_missing(e));
                entry
            })
            .collect();
        Ok(json!({"projects": projects, "damaged": damaged}))
    }

    /// Register the project at `target` (a folder in it; here when none): `{result, project: its entry, damaged}`.
    ///
    /// # Errors
    /// Not a folder, no tasks.json there, a project that opted out of the dashboard, the registry's lock or file.
    pub fn add_project(&self, target: Option<&str>, cwd: &Path) -> Result<Value> {
        let (path, meta) = resolve_project(target, cwd)?;
        if !listed(&meta) {
            return Err(Error::failed(format!(
                "{} opts out of the dashboard (meta.dashboard.listed is false)",
                path.shown()
            )));
        }
        let held = self.locked()?;
        let (mut registry, damaged) = read_registry(&self.file())?;
        let result = update_entry(&mut registry, &path, &meta);
        write_registry(&self.file(), &registry)?;
        drop(held);
        let project = position(&registry, &text(&path))
            .map_or(Value::Null, |at| entries(&registry)[at].clone());
        Ok(json!({"result": result, "project": project, "damaged": damaged}))
    }

    /// Unregister the project with the ID or path `target`: `{removed: its entry, damaged}`.
    ///
    /// # Errors
    /// No target, no such project, the registry's lock or file.
    pub fn remove_project(&self, target: &str, cwd: &Path) -> Result<Value> {
        if target.is_empty() {
            return Err(Error::Failed {
                message: "remove needs a project ID or path (see `atm project list`)".into(),
                code: 2,
            });
        }
        let mut candidates = vec![
            target.to_string(),
            text(&crate::jsongit::logo::normalize(
                &cwd.join(expand_home(target)),
            )),
        ];
        let folder = cwd.join(target);
        if folder.is_dir()
            && let Ok(paths) = find_paths(&atm_common::home::absolute(&folder))
        {
            candidates.push(text(&main_worktree(&paths.root)));
        }
        let held = self.locked()?;
        let (mut registry, damaged) = read_registry(&self.file())?;
        let matches = |e: &Value, key: &str| {
            e.get(key)
                .and_then(Value::as_str)
                .is_some_and(|v| candidates.iter().any(|c| c == v))
        };
        let at = entries(&registry)
            .iter()
            .position(|e| matches(e, "id") || matches(e, "path"));
        let removed = at.map(|at| entries_mut(&mut registry).remove(at));
        if removed.is_some() || damaged.is_some() {
            write_registry(&self.file(), &registry)?;
        }
        drop(held);
        let removed = removed.ok_or_else(|| {
            Error::failed(format!(
                "no registered project {target} (see `atm project list`)"
            ))
        })?;
        Ok(json!({"removed": removed, "damaged": damaged}))
    }

    /// Unregister the projects whose task folder is gone: `{removed: their entries, damaged}`.
    ///
    /// # Errors
    /// The registry's lock or file.
    pub fn prune(&self) -> Result<Value> {
        let held = self.locked()?;
        let (mut registry, damaged) = read_registry(&self.file())?;
        let (gone, kept): (Vec<Value>, Vec<Value>) =
            entries(&registry).iter().cloned().partition(is_missing);
        if !gone.is_empty() || damaged.is_some() {
            *entries_mut(&mut registry) = kept;
            write_registry(&self.file(), &registry)?;
        }
        drop(held);
        Ok(json!({"removed": gone, "damaged": damaged}))
    }

    /// Make the workspace's file, with no projects, unless it is there already. Whether it made it.
    ///
    /// # Errors
    /// An invalid ID, the state folder's lock or the file.
    pub fn create(&self) -> Result<bool> {
        if !valid_workspace_id(&self.workspace) {
            return Err(Error::Invalid(format!(
                "{:?} isn't a workspace ID (letters, digits, '.', '_' and '-')",
                self.workspace
            )));
        }
        let held = self.locked()?;
        let file = self.file();
        let made = !file.exists();
        if made {
            write_registry(&file, &empty_registry())?;
        }
        drop(held);
        Ok(made)
    }
}

/// The IDs of the workspaces in the state folder `home`, sorted, `default` first and always there.
#[must_use]
pub fn workspace_ids(home: &Path) -> Vec<String> {
    let mut ids: Vec<String> = std::fs::read_dir(home.join(WORKSPACES_DIR))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            name.strip_suffix(".json").map(String::from)
        })
        .filter(|id| valid_workspace_id(id) && id != DEFAULT_WORKSPACE)
        .collect();
    ids.sort();
    ids.insert(0, DEFAULT_WORKSPACE.into());
    ids
}

/// What was opened last, in the state folder: `recent.json`.
pub const RECENT_FILE: &str = "recent.json";

/// What `recent.json` says (`{workspace, projects: {<workspace>: <project id>}}`), an empty object without one.
#[must_use]
pub fn read_recent(home: &Path) -> Map<String, Value> {
    read_json(&home.join(RECENT_FILE))
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}

/// Write `recent.json`, atomically, under the state folder's lock.
///
/// # Errors
/// The state folder's lock or the file.
pub fn write_recent(home: &Path, recent: &Map<String, Value>) -> Result<()> {
    ensure_home(home).map_err(|e| Error::failed(format!("{}: {e}", home.shown())))?;
    let held = lock(home, None)?;
    let file = home.join(RECENT_FILE);
    let written = write_text_atomic(&file, &(to_json(&Value::Object(recent.clone())) + "\n"))
        .map_err(|e| Error::failed(format!("{}: {e}", file.shown())));
    drop(held);
    written
}

#[cfg(test)]
mod tests;
