//! The user's project registry (`$ATM_HOME/projects.json`; the Python atm's `db/registry.py`): the projects the
//! dashboard server shows (the `Registry` of docs/layered-architecture.md §3.2; its format is docs/multi-project-api.md
//! §5.1).
//!
//! One file per user, guarded by `$ATM_HOME`'s lock (never taken while holding a project's), written atomically. A
//! linked git worktree registers its repository's main worktree.

use std::path::{Path, PathBuf};

use atm_common::home::{atm_home, ensure_home, write_text_atomic};
use atm_common::py::to_json;
use atm_common::sha1::sha1_hex;
use atm_common::time::{format_iso, now, now_iso, parse_iso};
use atm_common::{Error, Result, TASK_DIR, TASKS_FILE};
use serde_json::{Map, Value, json};

use crate::jsongit::files::read_json;
use crate::jsongit::lock::lock;
use crate::jsongit::paths::{Paths, find_paths, main_worktree};

/// The registry's file, in `$ATM_HOME`.
pub const REGISTRY_FILE: &str = "projects.json";
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
    path.display().to_string()
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

/// The registry and, when `projects.json` was unreadable, where it was moved (`projects.json.damaged-<time>`; an
/// empty registry replaces it). Called holding the registry's lock.
fn read_registry(home: &Path) -> Result<(Value, Option<String>)> {
    let path = home.join(REGISTRY_FILE);
    if !path.exists() {
        return Ok((empty_registry(), None));
    }
    let data = read_json(&path);
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
    let damaged = format!("{}.damaged-{stamp}", path.display());
    std::fs::rename(&path, &damaged)
        .map_err(|e| Error::failed(format!("{}: {e}", path.display())))?;
    Ok((empty_registry(), Some(damaged)))
}

fn write_registry(home: &Path, registry: &Value) -> Result<()> {
    let path = home.join(REGISTRY_FILE);
    write_text_atomic(&path, &(to_json(registry) + "\n"))
        .map_err(|e| Error::failed(format!("{}: {e}", path.display())))
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
    let home = atm_home();
    ensure_home(&home).map_err(|e| Error::failed(format!("{}: {e}", home.display())))?;
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
    let (mut registry, _) = read_registry(&home)?;
    let result = update_entry(&mut registry, &path, &meta);
    if !matches!(result, "unchanged" | "unlisted") {
        write_registry(&home, &registry)?;
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
    let registry = read_json(&atm_home().join(REGISTRY_FILE)).unwrap_or(Value::Null);
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

/// The plugin hook's cheap path, run after every tool call: one read of projects.json, and registering (without
/// waiting for the lock) only when the project isn't listed yet or was last seen a day ago.
///
/// # Errors
/// The registry's file, when it has to be written.
pub fn register_if_needed(paths: &Paths) -> Result<Option<&'static str>> {
    let path = text(&main_worktree(&paths.root));
    let registry = read_json(&atm_home().join(REGISTRY_FILE)).unwrap_or(Value::Null);
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
                "no tasks.json in {}. Run /atm:create-tasks there first.",
                path.join(TASK_DIR).display()
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

/// The registry in `$ATM_HOME/projects.json`. Each method holds the registry's lock while it reads and writes, and
/// says where an unreadable projects.json was moved (`damaged`).
#[derive(Debug, Clone)]
pub struct FileRegistry {
    home: PathBuf,
}

impl Default for FileRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl FileRegistry {
    /// The registry of `$ATM_HOME`.
    #[must_use]
    pub fn new() -> Self {
        Self { home: atm_home() }
    }

    /// The registry of the atm folder `home` (the server's, given on its command line).
    #[must_use]
    pub fn at(home: &Path) -> Self {
        Self {
            home: home.to_path_buf(),
        }
    }

    /// The entries as the file has them now, without the lock (the server reads them on every request): those with
    /// a `path`.
    #[must_use]
    pub fn entries(&self) -> Vec<Value> {
        let registry = read_json(&self.home.join(REGISTRY_FILE)).unwrap_or(Value::Null);
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
        let Ok(meta) = std::fs::metadata(self.home.join(REGISTRY_FILE)) else {
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
            .map_err(|e| Error::failed(format!("{}: {e}", self.home.display())))?;
        lock(&self.home, None)
    }

    /// `{projects: the entries, each with missing (no readable tasks.json there now), damaged}`.
    ///
    /// # Errors
    /// The registry's lock or file.
    pub fn list_projects(&self) -> Result<Value> {
        let held = self.locked()?;
        let (registry, damaged) = read_registry(&self.home)?;
        if damaged.is_some() {
            write_registry(&self.home, &registry)?;
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
                path.display()
            )));
        }
        let held = self.locked()?;
        let (mut registry, damaged) = read_registry(&self.home)?;
        let result = update_entry(&mut registry, &path, &meta);
        write_registry(&self.home, &registry)?;
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
        let (mut registry, damaged) = read_registry(&self.home)?;
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
            write_registry(&self.home, &registry)?;
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
        let (mut registry, damaged) = read_registry(&self.home)?;
        let (gone, kept): (Vec<Value>, Vec<Value>) =
            entries(&registry).iter().cloned().partition(is_missing);
        if !gone.is_empty() || damaged.is_some() {
            *entries_mut(&mut registry) = kept;
            write_registry(&self.home, &registry)?;
        }
        drop(held);
        Ok(json!({"removed": gone, "damaged": damaged}))
    }
}

#[cfg(test)]
mod tests;
