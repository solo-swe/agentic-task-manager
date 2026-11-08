//! Workspaces (ADR-003 §1): named lists of projects in atm's state folder, `default` always there. `Workspaces` is the
//! state folder's set of them and what was opened last; a `Workspace` is one list, with its projects' summaries,
//! worktrees and logos (what the dashboard's project list shows), search across them, and adding and removing
//! projects. A project whose files are broken is listed as missing, never breaking the list (F18).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use atm_common::home::state_dir;
use atm_common::time::{format_iso, now};
use atm_common::{Error, Result};
use atm_db::registry::{DEFAULT_WORKSPACE, FileRegistry, read_recent, workspace_ids, write_recent};
use serde_json::{Map, Value, json};

use crate::Project;
use crate::archive::{project_stamp, task_snapshot};
use crate::documents::{read_document, read_image};
use crate::query::{Filters, list_claims};
use crate::settings::get_logo;
use crate::summary::project_summary;
use crate::worktrees::working_copies;
use crate::worktrees::worktree_copies;

/// Task data atm will read into memory to show (a big project's archive is a few MB).
pub const MAX_SERVED_BYTES: u64 = 100_000_000;
/// Characters of a search query that count.
pub const SEARCH_QUERY_CAP: usize = 200;
/// Task IDs per project in a search's answer.
pub const SEARCH_RESULTS_CAP: usize = 200;

/// The workspaces of a state folder. Each one is opened once and kept (with its summary cache) while it exists.
pub struct Workspaces {
    home: PathBuf,
    opened: Mutex<HashMap<String, Arc<Workspace>>>,
}

impl Workspaces {
    /// The workspaces of atm's state folder.
    #[must_use]
    pub fn open() -> Self {
        Self::at(&state_dir())
    }

    /// The workspaces of the state folder `home` (the server's, given on its command line).
    #[must_use]
    pub fn at(home: &Path) -> Self {
        Self {
            home: home.to_path_buf(),
            opened: Mutex::new(HashMap::new()),
        }
    }

    /// The folder their files are in: it changes when one is made, removed or its list changes.
    #[must_use]
    pub fn folder(&self) -> PathBuf {
        self.home.join(atm_db::registry::WORKSPACES_DIR)
    }

    /// Their IDs, `default` first.
    #[must_use]
    pub fn ids(&self) -> Vec<String> {
        workspace_ids(&self.home)
    }

    /// `{workspaces: [{id, projects: how many}], recent: the one opened last}`.
    #[must_use]
    pub fn summary(&self) -> Value {
        let list: Vec<Value> = self
            .ids()
            .iter()
            .filter_map(|id| self.get(id))
            .map(|w| json!({"id": w.id(), "projects": w.entries().len()}))
            .collect();
        json!({"workspaces": list, "recent": self.recent()})
    }

    /// The workspace `id`, if there is one.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<Arc<Workspace>> {
        let registry = FileRegistry::workspace(&self.home, id);
        if !atm_db::registry::valid_workspace_id(id) || !registry.exists() {
            return None;
        }
        let mut opened = self
            .opened
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Some(Arc::clone(
            opened
                .entry(id.to_string())
                .or_insert_with(|| Arc::new(Workspace::new(registry))),
        ))
    }

    /// Make workspace `id`, with no projects. Whether it was made (false: it was there already).
    ///
    /// # Errors
    /// An invalid ID, the state folder's lock or the file.
    pub fn create(&self, id: &str) -> Result<bool> {
        FileRegistry::workspace(&self.home, id).create()
    }

    /// Delete workspace `id`; its projects stay where they are, and in the other workspaces that list them.
    ///
    /// # Errors
    /// `default`, no such workspace, the state folder's lock or the file.
    pub fn delete(&self, id: &str) -> Result<()> {
        FileRegistry::workspace(&self.home, id).delete()?;
        self.forget(id);
        Ok(())
    }

    /// Rename workspace `id` to `new`; the last opened follows it.
    ///
    /// # Errors
    /// `default` on either side, an invalid or taken new ID, no such workspace, the state folder's lock or the file.
    pub fn rename(&self, id: &str, new: &str) -> Result<()> {
        FileRegistry::workspace(&self.home, id).rename(new)?;
        self.forget(id);
        if read_recent(&self.home)
            .get("workspace")
            .and_then(Value::as_str)
            == Some(id)
        {
            self.set_recent(new, None)?;
        }
        Ok(())
    }

    fn forget(&self, id: &str) {
        self.opened
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(id);
    }

    /// The `default` workspace, which every project atm meets joins.
    ///
    /// # Panics
    /// Never: `default` is always there.
    #[must_use]
    pub fn default_workspace(&self) -> Arc<Workspace> {
        self.get(DEFAULT_WORKSPACE)
            .expect("default is always there")
    }

    /// The workspace opened last, if it is still there; else `default`.
    #[must_use]
    pub fn recent(&self) -> String {
        read_recent(&self.home)
            .get("workspace")
            .and_then(Value::as_str)
            .filter(|id| self.get(id).is_some())
            .unwrap_or(DEFAULT_WORKSPACE)
            .to_string()
    }

    /// Workspace `id` was opened (in the dashboard or the app): remember it, writing only when it wasn't already the
    /// last opened, so a poll doesn't rewrite the file.
    ///
    /// # Errors
    /// The state folder's lock or `recent.json`.
    pub fn opened(&self, id: &str) -> Result<()> {
        let recent = read_recent(&self.home);
        if recent.get("workspace").and_then(Value::as_str) == Some(id) {
            return Ok(());
        }
        self.set_recent(id, None)
    }

    /// The project opened last in workspace `id`, if any.
    #[must_use]
    pub fn recent_project(&self, id: &str) -> Option<String> {
        read_recent(&self.home)
            .get("projects")
            .and_then(|p| p.get(id))
            .and_then(Value::as_str)
            .map(String::from)
    }

    /// Remember that workspace `id` was opened (and in it, `project`).
    ///
    /// # Errors
    /// The state folder's lock or `recent.json`.
    pub fn set_recent(&self, id: &str, project: Option<&str>) -> Result<()> {
        let mut recent = read_recent(&self.home);
        recent.insert("workspace".into(), json!(id));
        if let Some(project) = project {
            let projects = recent
                .entry("projects")
                .or_insert_with(|| Value::Object(Map::new()));
            if !projects.is_object() {
                *projects = Value::Object(Map::new());
            }
            projects[id] = json!(project);
        }
        write_recent(&self.home, &recent)
    }
}

/// One workspace: its list of projects, and each one's summary, computed again only when its task data or claims
/// changed, or the day did (the completions count by day).
pub struct Workspace {
    registry: FileRegistry,
    summaries: Mutex<HashMap<String, (Value, Value)>>,
}

/// A workspace's projects as the dashboard lists them, read once: what the list depends on (`stamp`, for an `ETag`)
/// and, computed only when asked, the list itself.
pub struct Listing<'a> {
    workspace: &'a Workspace,
    entries: Vec<Value>,
    stamps: Vec<Option<Value>>,
    trees: Vec<Value>,
    logos: Vec<Option<(PathBuf, &'static str)>>,
    /// What the list depends on: the workspace's file, and each project's task data, claims, worktrees, logo and the
    /// day.
    pub stamp: Value,
}

impl Listing<'_> {
    /// Each project: `id`, `name`, `path`, `missing`, `summary`, `worktrees` and `logo` (its route, relative to the
    /// dashboard, when it has one).
    #[must_use]
    pub fn projects(&self) -> Vec<Value> {
        self.entries
            .iter()
            .zip(&self.stamps)
            .zip(self.trees.iter().zip(&self.logos))
            .map(|((entry, stamp), (tree, logo))| {
                let summary = stamp
                    .as_ref()
                    .map_or(Value::Null, |s| self.workspace.summary(entry, s));
                let shown = !summary.is_null();
                json!({
                    "id": entry.get("id"),
                    "name": entry.get("name"),
                    "path": entry["path"],
                    "missing": !shown,
                    "summary": summary,
                    "worktrees": if shown { tree.clone() } else { json!([]) },
                    // Relative to the dashboard, like the project's files; a new one shows as a new ETag
                    "logo": (logo.is_some() && shown)
                        .then(|| self.workspace.logo_route(&atm_common::py::py_str_opt(entry.get("id")))),
                })
            })
            .collect()
    }
}

/// One of a workspace's projects, found by its ID with its data there: what each read about one project (the REST
/// API's routes, the app's commands) is answered from, so both answer the same.
pub struct Member {
    /// Its entry in the workspace (`id`, `name`, `path`, …).
    pub entry: Value,
    /// Its store's stamp (`{tasks, archive, claims, head, copies}`), for `ETag`s.
    pub stamp: Value,
}

impl Member {
    /// The project, opened.
    ///
    /// # Errors
    /// As [`Project::open`].
    pub fn project(&self) -> Result<Project> {
        Workspace::project(&self.entry)
    }

    /// Its tasks the filters keep, each with its derived values, and in the same snapshot its meta and how big its
    /// task data is; read tolerantly (a malformed task file is shown with what is wrong with it, T-377).
    ///
    /// # Errors
    /// `TooLarge` for task data over [`MAX_SERVED_BYTES`]; a filter naming no such sprint or release; an unreadable
    /// task file.
    pub fn tasks(&self, filters: &Filters) -> Result<Value> {
        if too_large(&self.stamp) {
            return Err(Error::TooLarge(format!(
                "larger than {} MB",
                MAX_SERVED_BYTES / 1_000_000
            )));
        }
        task_snapshot(&Workspace::project_tolerant(&self.entry)?, filters)
    }

    /// The claims of all its worktrees, from the shared claims store.
    #[must_use]
    pub fn claims(&self) -> Vec<Value> {
        self.project().map(|p| list_claims(&p)).unwrap_or_default()
    }

    /// Its working copies, main first, and the tasks that differ in the others.
    #[must_use]
    pub fn worktrees(&self) -> Vec<Value> {
        self.project()
            .map(|p| worktree_copies(&p))
            .unwrap_or_default()
    }

    /// Its logo's file and content type, if it has one.
    #[must_use]
    pub fn logo(&self) -> Option<(PathBuf, &'static str)> {
        Workspace::logo(&self.entry)
    }

    /// The document at `path` in its working copy, as the viewer shows it.
    ///
    /// # Errors
    /// A path that is malformed, not there, too large or one atm doesn't show.
    pub fn document(&self, path: &str) -> Result<Value> {
        read_document(self.project()?.vcs().as_ref(), path)
    }

    /// The image at `path` in its working copy, and its content type.
    ///
    /// # Errors
    /// As [`Member::document`], and a file that isn't an image.
    pub fn image(&self, path: &str) -> Result<(Vec<u8>, &'static str)> {
        read_image(self.project()?.vcs().as_ref(), path)
    }

    /// The folders to watch for its data to change.
    #[must_use]
    pub fn watch_folders(&self) -> Vec<PathBuf> {
        self.project()
            .map(|p| p.store().watch_folders())
            .unwrap_or_default()
    }
}

/// A project entry's folder.
#[must_use]
pub fn folder(entry: &Value) -> PathBuf {
    PathBuf::from(entry["path"].as_str().unwrap_or_default())
}

/// What the summary depends on: the task data, the claims, the day, and the checked-out branch (on the card).
fn summary_stamp(stamp: &Value) -> Value {
    json!([
        stamp["tasks"],
        stamp["claims"],
        &format_iso(now())[..10],
        stamp["head"]
    ])
}

/// `[size, nanosecond mtime]` of a file, null if it doesn't exist.
#[must_use]
pub fn file_stamp(path: &Path) -> Value {
    let Ok(meta) = std::fs::metadata(path) else {
        return Value::Null;
    };
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos());
    json!([meta.len(), mtime.to_string()])
}

/// Whether a project's task data (by its store stamp) is too large to read in to show.
#[must_use]
pub fn too_large(stamp: &Value) -> bool {
    stamp["tasks"][0]
        .as_u64()
        .is_some_and(|size| size > MAX_SERVED_BYTES)
}

impl Workspace {
    fn new(registry: FileRegistry) -> Self {
        Self {
            registry,
            summaries: Mutex::new(HashMap::new()),
        }
    }

    /// Its ID.
    #[must_use]
    pub fn id(&self) -> &str {
        self.registry.id()
    }

    /// Where its project `id`'s logo is served, relative to the dashboard: `p/<id>/logo` in `default` (the v1
    /// address), `api/v2/workspaces/<w>/projects/<id>/logo` in another.
    #[must_use]
    pub fn logo_route(&self, id: &str) -> String {
        if self.id() == DEFAULT_WORKSPACE {
            format!("p/{id}/logo")
        } else {
            format!("api/v2/workspaces/{}/projects/{id}/logo", self.id())
        }
    }

    /// Its projects' entries as its file has them now (read without the lock, on every request).
    #[must_use]
    pub fn entries(&self) -> Vec<Value> {
        self.registry.entries()
    }

    /// Its file's stamp: it changes when the list does.
    #[must_use]
    pub fn stamp(&self) -> Value {
        self.registry.stamp()
    }

    /// The entry of its project `id`.
    #[must_use]
    pub fn find(&self, id: &str) -> Option<Value> {
        self.entries()
            .into_iter()
            .find(|e| e.get("id").and_then(Value::as_str) == Some(id))
    }

    /// The project of `entry`.
    ///
    /// # Errors
    /// As [`Project::open`].
    pub fn project(entry: &Value) -> Result<Project> {
        Project::open(&folder(entry))
    }

    /// The project of `entry`, read tolerantly (to show it).
    ///
    /// # Errors
    /// As [`Project::open_tolerant`].
    pub fn project_tolerant(entry: &Value) -> Result<Project> {
        Project::open_tolerant(&folder(entry))
    }

    /// The project's store stamp (`{tasks, archive, claims, head, copies}`), or none when it is missing or refused.
    #[must_use]
    pub fn data_stamp(entry: &Value) -> Option<Value> {
        let project = Self::project(entry).ok()?;
        let stamp = project_stamp(&project);
        (!stamp["tasks"].is_null()).then_some(stamp)
    }

    /// The file and content type of a project's logo (`meta.logo`, or one found), if it has one.
    #[must_use]
    pub fn logo(entry: &Value) -> Option<(PathBuf, &'static str)> {
        let project = Self::project_tolerant(entry).ok()?;
        let (found, _) = get_logo(&project).ok()?;
        found.map(|logo| (logo.file, logo.content_type))
    }

    /// Its project `id`, there and readable: where a request about one project starts.
    ///
    /// # Errors
    /// `NotFound`: "unknown project" when it has no such project, "not found" when the project's task data is missing
    /// or refused.
    pub fn member(&self, id: &str) -> Result<Member> {
        let entry = self
            .find(id)
            .ok_or_else(|| Error::NotFound("unknown project".into()))?;
        let stamp = Self::data_stamp(&entry).ok_or_else(|| Error::NotFound("not found".into()))?;
        Ok(Member { entry, stamp })
    }

    /// The folders to watch for its list to change: its file's, and each listed project's.
    #[must_use]
    pub fn watch_folders(&self) -> Vec<PathBuf> {
        let mut folders: Vec<PathBuf> = self
            .registry
            .file()
            .parent()
            .map(Path::to_path_buf)
            .into_iter()
            .collect();
        for entry in self.entries() {
            if let Ok(project) = Self::project(&entry) {
                folders.extend(project.store().watch_folders());
            }
        }
        folders.sort();
        folders.dedup();
        folders
    }

    fn summary(&self, entry: &Value, stamp: &Value) -> Value {
        let key = entry["path"].as_str().unwrap_or_default().to_string();
        let mut cache = self
            .summaries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some((cached, summary)) = cache.get(&key)
            && cached == stamp
        {
            return summary.clone();
        }
        // An unreadable tasks.json or an unusable vocabulary: listed as missing
        let summary = Self::project(entry)
            .and_then(|p| project_summary(&p))
            .unwrap_or(Value::Null);
        cache.insert(key, (stamp.clone(), summary.clone()));
        summary
    }

    /// Its projects, as the dashboard lists them (see [`Listing`]).
    #[must_use]
    pub fn listing(&self) -> Listing<'_> {
        let entries = self.entries();
        let stamps: Vec<Option<Value>> = entries
            .iter()
            .map(|e| Self::data_stamp(e).map(|s| summary_stamp(&s)))
            .collect();
        let trees: Vec<Value> = entries
            .iter()
            .zip(&stamps)
            .map(|(e, s)| {
                let list = s
                    .as_ref()
                    .and_then(|_| Self::project(e).ok())
                    .map(|p| working_copies(&p))
                    .unwrap_or_default();
                Value::Array(list)
            })
            .collect();
        let logos: Vec<Option<(PathBuf, &'static str)>> = entries
            .iter()
            .zip(&stamps)
            .map(|(e, s)| s.as_ref().and_then(|_| Self::logo(e)))
            .collect();
        let logo_stamps: Vec<Value> = logos
            .iter()
            .map(|l| l.as_ref().map_or(Value::Null, |(file, _)| file_stamp(file)))
            .collect();
        let stamp = json!([self.stamp(), stamps, trees, logo_stamps]);
        Listing {
            workspace: self,
            entries,
            stamps,
            trees,
            logos,
            stamp,
        }
    }

    /// Per project, the IDs of the working tasks (not the archive) in which every word of `query` appears,
    /// case-insensitively (the project's name counts as part of every task's text); at most
    /// [`SEARCH_RESULTS_CAP`] each, and only the first [`SEARCH_QUERY_CAP`] characters of the query count. Projects
    /// that are missing, broken or too large are left out.
    #[must_use]
    pub fn search(&self, query: &str) -> Map<String, Value> {
        let q: String = query.chars().take(SEARCH_QUERY_CAP).collect();
        let mut results = Map::new();
        if q.split_whitespace().next().is_none() {
            return results;
        }
        for entry in self.entries() {
            let Some(stamp) = Self::data_stamp(&entry) else {
                continue;
            };
            if too_large(&stamp) {
                continue;
            }
            let name = entry
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let ids = Self::project(&entry)
                .and_then(|p| crate::query::search_tasks(&p, &q, name))
                .unwrap_or_default();
            if !ids.is_empty() {
                let id = atm_common::py::py_str_opt(entry.get("id"));
                results.insert(
                    id,
                    json!(ids.into_iter().take(SEARCH_RESULTS_CAP).collect::<Vec<_>>()),
                );
            }
        }
        results
    }

    /// `{projects: the entries, each with missing, damaged}`.
    ///
    /// # Errors
    /// The state folder's lock or the workspace's file.
    pub fn list_projects(&self) -> Result<Value> {
        self.registry.list_projects()
    }

    /// Add the project at `target` (a folder in it; `cwd` when none): `{result, project, damaged}`.
    ///
    /// # Errors
    /// Not a folder, no task file there, a project that opted out of the dashboard, the lock or the file.
    pub fn add_project(&self, target: Option<&str>, cwd: &Path) -> Result<Value> {
        self.registry.add_project(target, cwd)
    }

    /// Remove the project with the ID or path `target`: `{removed, damaged}`.
    ///
    /// # Errors
    /// No such project, the lock or the file.
    pub fn remove_project(&self, target: &str, cwd: &Path) -> Result<Value> {
        self.registry.remove_project(target, cwd)
    }

    /// Remove the projects whose task folder is gone: `{removed, damaged}`.
    ///
    /// # Errors
    /// The lock or the file.
    pub fn prune(&self) -> Result<Value> {
        self.registry.prune()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("atm-lib-ws-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn default_is_always_there_and_others_once_made() {
        let home = state("ids");
        let workspaces = Workspaces::at(&home);
        assert_eq!(workspaces.ids(), vec!["default".to_string()]);
        assert!(workspaces.get("default").is_some() && workspaces.get("work").is_none());
        assert!(FileRegistry::workspace(&home, "work").create().unwrap());
        assert!(!FileRegistry::workspace(&home, "work").create().unwrap());
        assert!(FileRegistry::workspace(&home, "../x").create().is_err());
        assert_eq!(workspaces.ids(), vec!["default".to_string(), "work".into()]);
        assert_eq!(workspaces.get("work").unwrap().id(), "work");
        assert!(workspaces.get("..").is_none());
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn workspaces_are_made_renamed_and_deleted_but_default_stays() {
        let home = state("manage");
        let workspaces = Workspaces::at(&home);
        assert!(workspaces.create("work").unwrap());
        workspaces.opened("work").unwrap();
        workspaces.rename("work", "client").unwrap();
        assert_eq!(
            workspaces.ids(),
            vec!["default".to_string(), "client".into()]
        );
        assert_eq!(workspaces.recent(), "client");
        assert!(workspaces.get("work").is_none());
        assert!(matches!(
            workspaces.rename("client", "default"),
            Err(Error::Refused(_))
        ));
        assert!(matches!(
            workspaces.rename("default", "x"),
            Err(Error::Refused(_))
        ));
        assert!(matches!(
            workspaces.rename("nope", "x"),
            Err(Error::NotFound(_))
        ));
        assert!(workspaces.create("other").unwrap());
        assert!(matches!(
            workspaces.rename("client", "other"),
            Err(Error::Invalid(_))
        ));
        assert!(matches!(
            workspaces.rename("client", "../x"),
            Err(Error::Invalid(_))
        ));
        assert!(matches!(
            workspaces.delete("default"),
            Err(Error::Refused(_))
        ));
        workspaces.delete("client").unwrap();
        assert!(matches!(
            workspaces.delete("client"),
            Err(Error::NotFound(_))
        ));
        assert_eq!(workspaces.recent(), "default");
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn the_recent_workspace_falls_back_to_default() {
        let home = state("recent");
        let workspaces = Workspaces::at(&home);
        assert_eq!(workspaces.recent(), "default");
        workspaces.set_recent("gone", Some("p1")).unwrap();
        assert_eq!(workspaces.recent(), "default");
        assert_eq!(workspaces.recent_project("gone").as_deref(), Some("p1"));
        FileRegistry::workspace(&home, "work").create().unwrap();
        workspaces.set_recent("work", None).unwrap();
        assert_eq!(workspaces.recent(), "work");
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn a_missing_project_is_listed_as_missing_and_never_searched() {
        let home = state("missing");
        let registry = serde_json::json!({"version": 1, "projects": [
            {"id": "gone", "name": "Gone", "path": home.join("nowhere").to_string_lossy()}
        ]});
        std::fs::create_dir_all(home.join("workspaces")).unwrap();
        std::fs::write(home.join("workspaces/default.json"), registry.to_string()).unwrap();
        let workspace = Workspaces::at(&home).default_workspace();
        let listing = workspace.listing();
        let projects = listing.projects();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0]["missing"], json!(true));
        assert_eq!(projects[0]["worktrees"], json!([]));
        assert!(workspace.search("anything").is_empty());
        assert!(workspace.find("gone").is_some() && workspace.find("x").is_none());
        std::fs::remove_dir_all(&home).unwrap();
    }
}
