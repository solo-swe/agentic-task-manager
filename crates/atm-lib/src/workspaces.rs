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
use atm_db::{OnChange, Subscription};
use serde_json::{Map, Value, json};

use crate::Project;
use crate::archive::{project_stamp, task_snapshot};
use crate::documents::{read_document, read_image};
use crate::foreign;
use crate::query::Filters;
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

    /// The registered projects, in any workspace, whose short name (`meta.key`, ADR-007 §2) is `key` in any case, other
    /// than the one in `except`: `(workspace, project's name)`, for `atm project key set`'s warning. A project that
    /// can't be read is left out.
    #[must_use]
    pub fn key_users(&self, key: &str, except: &Path) -> Vec<(String, String)> {
        let except = std::fs::canonicalize(except).unwrap_or_else(|_| except.to_path_buf());
        let mut found = Vec::new();
        for id in self.ids() {
            let Some(workspace) = self.get(&id) else {
                continue;
            };
            for entry in workspace.entries() {
                let path = folder(&entry);
                if std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone()) == except {
                    continue;
                }
                let Ok(project) = Workspace::project_tolerant(&entry) else {
                    continue;
                };
                let Ok(document) = project.store().document() else {
                    continue;
                };
                if atm_common::keys::project_key(document)
                    .is_some_and(|k| k.eq_ignore_ascii_case(key))
                {
                    let name = entry
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or_else(|| entry["id"].as_str().unwrap_or_default());
                    found.push((id.clone(), name.to_string()));
                }
            }
        }
        found
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
/// changed, or the day did (the completions count by day), or a project its tasks depend on did (T-553).
pub struct Workspace {
    registry: FileRegistry,
    summaries: Mutex<HashMap<String, Summarized>>,
}

/// A project's summary, and what it was worked out from: the project's stamp, and the other projects its tasks
/// depend on, by ID, with their [`foreign::version`].
struct Summarized {
    stamp: Value,
    others: Vec<String>,
    version: Value,
    summary: Value,
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

/// A full git object ID: 40 hex digits (SHA-1) or 64 (SHA-256), in lower case as git writes them. Anything shorter
/// could name another object later; a name (`HEAD`, a branch) moves; an expression (`HEAD~1`, `:/fix`) is git's to
/// work out. A file is read at a revision a page names only by one (ATM-671, audit H6), the REST routes and the app's
/// commands alike.
#[must_use]
pub fn is_object_id(revision: &str) -> bool {
    matches!(revision.len(), 40 | 64)
        && revision
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// `revision` as a page may name one, or why not.
fn checked_revision(revision: &str) -> Result<()> {
    if is_object_id(revision) {
        Ok(())
    } else {
        Err(Error::Invalid(format!(
            "at={revision}: a revision is a full object ID"
        )))
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

    /// The project, opened to act as the user at the machine (the app's and the browser's review requests, ATM-572):
    /// never an agent's identity, whatever environment the server runs in.
    ///
    /// # Errors
    /// As [`Project::open`].
    pub fn project_as_user(&self) -> Result<Project> {
        let root = folder(&self.entry);
        Ok(Project::open(&root)?.with_actor(Some(crate::actor::user_identity(&root))))
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

    /// Its stats, progress and forecast for `query`'s slice and period, now ([`crate::stats::project_stats`]); read
    /// tolerantly, as its tasks are.
    ///
    /// # Errors
    /// As [`Member::tasks`].
    pub fn stats(&self, query: &crate::stats::StatsQuery) -> Result<Value> {
        if too_large(&self.stamp) {
            return Err(Error::TooLarge(format!(
                "larger than {} MB",
                MAX_SERVED_BYTES / 1_000_000
            )));
        }
        let now = atm_common::time::now() * 1000;
        crate::stats::project_stats(&Workspace::project_tolerant(&self.entry)?, query, now)
    }

    /// What the other projects its tasks name are now ([`foreign::named_version`]): with [`Member::stamp`], what its
    /// task views depend on, for the tasks route's `ETag` (docs/cross-project-freshness-design.md §5.3 D). Null when
    /// it can't be opened.
    #[must_use]
    pub fn others_version(&self) -> Value {
        Workspace::project_tolerant(&self.entry).map_or(Value::Null, |p| foreign::named_version(&p))
    }

    /// The claims of all its worktrees held now, from the shared claims store: an expired lease is left out, by atm's
    /// clock (ATM-724).
    #[must_use]
    pub fn claims(&self) -> Vec<Value> {
        self.project()
            .map(|p| crate::query::held_claims(&p, atm_common::time::now()))
            .unwrap_or_default()
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

    /// The document at `path` as the commit or tree `revision` has it (a review's round's side).
    ///
    /// # Errors
    /// As [`Member::document`], and a revision that isn't one, or doesn't have the file.
    pub fn document_at(&self, path: &str, revision: &str) -> Result<Value> {
        checked_revision(revision)?;
        crate::documents::read_document_at(self.project()?.vcs().as_ref(), revision, path)
    }

    /// Its reviews the query keeps, the latest activity first, which need the user.
    ///
    /// # Errors
    /// As [`crate::review_pages::list_reviews`].
    pub fn reviews(&self, query: &crate::review_pages::ReviewQuery) -> Result<Value> {
        crate::review_pages::list_reviews(&self.project_as_user()?, query)
    }

    /// A review's page for one view: round `round` (the latest when none), against round `against` when given.
    ///
    /// # Errors
    /// As [`crate::review_pages::review_page`].
    pub fn review(
        &self,
        review: &crate::reviews::ReviewRef,
        round: Option<u32>,
        against: Option<u32>,
    ) -> Result<Value> {
        crate::review_pages::review_page(&self.project_as_user()?, review, round, against)
    }

    /// What a live review's page of its working copy now is made from (ATM-607); none for any other page.
    ///
    /// # Errors
    /// As [`crate::review_pages::live_page_stamp`].
    pub fn live_review_stamp(
        &self,
        review: &crate::reviews::ReviewRef,
        round: Option<u32>,
        against: Option<u32>,
    ) -> Result<Option<Value>> {
        crate::review_pages::live_page_stamp(&self.project_as_user()?, review, round, against)
    }

    /// The image at `path` in its working copy, and its content type.
    ///
    /// # Errors
    /// As [`Member::document`], and a file that isn't an image.
    pub fn image(&self, path: &str) -> Result<(Vec<u8>, &'static str)> {
        read_image(self.project()?.vcs().as_ref(), path)
    }

    /// The image at `path` as the commit or tree `revision` has it, and its content type (a review's image file, its
    /// sides, ATM-689).
    ///
    /// # Errors
    /// As [`Member::document_at`], and a file that isn't an image.
    pub fn image_at(&self, path: &str, revision: &str) -> Result<(Vec<u8>, &'static str)> {
        checked_revision(revision)?;
        crate::documents::read_image_at(self.project()?.vcs().as_ref(), revision, path)
    }

    /// Be told when its data changes (its store's subscription), until the subscription is dropped. `on_change` runs
    /// on the store's thread.
    ///
    /// # Errors
    /// As [`Project::open`]; a store that can't be watched.
    pub fn subscribe(&self, on_change: OnChange) -> Result<Subscription> {
        // Only what the page shows changing (ATM-767): a lease renewed alone tells nothing
        let shown = crate::shown::on_shown_change(folder(&self.entry), on_change);
        self.project()?.store().subscribe(shown)
    }

    /// Be told when its working copy changes as a live review shows it (ATM-607), until the subscription is dropped.
    ///
    /// # Errors
    /// As [`crate::reviews::subscribe_working_copy`].
    pub fn subscribe_working_copy(&self, on_change: OnChange) -> Result<Subscription> {
        crate::reviews::subscribe_working_copy(&self.project()?, on_change)
    }
}

/// A project entry's folder.
#[must_use]
pub fn folder(entry: &Value) -> PathBuf {
    PathBuf::from(entry["path"].as_str().unwrap_or_default())
}

/// What the summary depends on: the task data, the claims, the day, the checked-out branch (on the card), the reviews
/// and the inbox (its open items, ATM-761).
fn summary_stamp(stamp: &Value) -> Value {
    json!([
        stamp["tasks"],
        stamp["claims"],
        &format_iso(now())[..10],
        stamp["head"],
        stamp["reviews"],
        stamp["inbox"]
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

    /// The project's store stamp (`{tasks, archive, claims, head, copies}`) and its reviews' (`reviews`, ATM-606: what
    /// waits on the user is in its summary), or none when it is missing or refused.
    #[must_use]
    pub fn data_stamp(entry: &Value) -> Option<Value> {
        let project = Self::project(entry).ok()?;
        let mut stamp = project_stamp(&project);
        if stamp["tasks"].is_null() {
            return None;
        }
        if let Some(map) = stamp.as_object_mut() {
            map.insert(
                "reviews".into(),
                crate::review_pages::reviews_stamp(&project),
            );
        }
        Some(stamp)
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

    /// Be told when it changes, until the subscription is dropped: its list (and the list of workspaces), and the data
    /// of each project it lists now that can be opened, their subscriptions merged. `on_change` runs on the stores'
    /// threads, and atm-lib holds no lock while it runs.
    ///
    /// # Errors
    /// Its list or a project's store can't be watched.
    pub fn subscribe(&self, on_change: OnChange) -> Result<Subscription> {
        let mut parts = Vec::new();
        for entry in self.entries() {
            if let Ok(project) = Self::project(&entry) {
                // Only what the page shows changing (ATM-767): a lease renewed alone tells nothing
                let shown = crate::shown::on_shown_change(folder(&entry), Arc::clone(&on_change));
                parts.push(project.store().subscribe(shown)?);
                // Its reviews too, when it has some (ATM-606: what waits on the user is in its summary); a project
                // without a reviews folder isn't given one
                let has_reviews = project
                    .root()
                    .is_some_and(|r| r.join(atm_common::TASK_DIR).join("reviews").is_dir());
                if has_reviews && let Ok(store) = crate::reviews::store(&project) {
                    use crate::reviews::ReviewStore as _;
                    parts.push(store.subscribe(Arc::clone(&on_change))?);
                }
            }
        }
        parts.push(self.registry.subscribe(on_change)?);
        Ok(Subscription::merged(parts))
    }

    fn summary(&self, entry: &Value, stamp: &Value) -> Value {
        let key = entry["path"].as_str().unwrap_or_default().to_string();
        let mut cache = self
            .summaries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(cached) = cache.get(&key)
            && cached.stamp == *stamp
            && foreign::version(&cached.others) == cached.version
        {
            return cached.summary.clone();
        }
        // An unreadable tasks.json or an unusable vocabulary: listed as missing
        let project = Self::project(entry);
        let others = project
            .as_ref()
            .map(foreign::referenced)
            .unwrap_or_default();
        // Before the summary, so a change in another project meanwhile is seen on the next use
        let version = foreign::version(&others);
        let summary = project
            .and_then(|p| project_summary(&p))
            .unwrap_or(Value::Null);
        cache.insert(
            key,
            Summarized {
                stamp: stamp.clone(),
                others,
                version,
                summary: summary.clone(),
            },
        );
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
        // The other projects each one's tasks name: their summaries (waiting, next) show them
        let others: Vec<Value> = entries
            .iter()
            .zip(&stamps)
            .map(|(e, s)| {
                s.as_ref()
                    .and_then(|_| Self::project(e).ok())
                    .map_or(Value::Null, |p| foreign::named_version(&p))
            })
            .collect();
        let stamp = json!([self.stamp(), stamps, trees, logo_stamps, others]);
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

    #[test]
    fn task_data_is_too_large_to_show_only_past_the_limit() {
        for (stamp, expected) in [
            (json!({"tasks": [MAX_SERVED_BYTES, "1"]}), false),
            (json!({"tasks": [MAX_SERVED_BYTES + 1, "1"]}), true),
            (json!({"tasks": null}), false),
            (json!({}), false),
            (json!({"tasks": ["big", "1"]}), false),
        ] {
            assert_eq!(too_large(&stamp), expected, "{stamp}");
        }
    }

    #[test]
    fn a_files_stamp_is_its_size_and_time_or_null_when_it_isnt_there() {
        let home = state("stamp");
        let file = home.join("f.txt");
        assert_eq!(file_stamp(&file), Value::Null);
        std::fs::write(&file, "abc").unwrap();
        let stamp = file_stamp(&file);
        assert_eq!(stamp[0], json!(3));
        assert!(stamp[1].as_str().is_some_and(|t| t.parse::<u128>().is_ok()));
        assert_eq!(
            folder(&json!({})),
            PathBuf::new(),
            "an entry without a path"
        );
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn a_logo_is_served_at_the_v1_address_in_default_and_the_v2_one_elsewhere() {
        let home = state("logo");
        let workspaces = Workspaces::at(&home);
        workspaces.create("work").unwrap();
        assert_eq!(workspaces.default_workspace().logo_route("p1"), "p/p1/logo");
        assert_eq!(
            workspaces.get("work").unwrap().logo_route("p1"),
            "api/v2/workspaces/work/projects/p1/logo"
        );
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn a_workspace_search_finds_each_projects_tasks_and_ignores_a_blank_query() {
        let home = state("search");
        let project = home.join("project");
        let tasks = project.join(atm_common::TASK_DIR).join("tasks.json");
        std::fs::create_dir_all(tasks.parent().unwrap()).unwrap();
        std::fs::write(
            &tasks,
            r#"{"meta": {"id": "one"}, "tasks": [{"id": "T-1", "name": "Parse config"}, {"id": "T-2", "name": "Other"}]}"#,
        )
        .unwrap();
        let workspaces = Workspaces::at(&home);
        let workspace = workspaces.default_workspace();
        workspace
            .add_project(Some(&project.to_string_lossy()), &home)
            .unwrap();
        assert_eq!(
            Value::Object(workspace.search("CONFIG")),
            json!({"one": ["T-1"]})
        );
        assert!(workspace.search("   ").is_empty());
        assert!(workspace.search("nothing-matches").is_empty());
        // past the query's cap, the rest doesn't count
        let long = format!("config{}", " ".repeat(SEARCH_QUERY_CAP)) + "nothing-matches";
        assert_eq!(
            Value::Object(workspace.search(&long)),
            json!({"one": ["T-1"]})
        );
        assert_eq!(
            workspaces.summary()["workspaces"],
            json!([{"id": "default", "projects": 1}])
        );
        assert!(
            matches!(workspace.member("nope"), Err(Error::NotFound(m)) if m == "unknown project")
        );
        assert!(workspace.member("one").is_ok());
        std::fs::remove_dir_all(&home).unwrap();
    }

    /// T-555: a workspace's subscription is its members' stores' and its list's, merged: a change in a member's task
    /// file and in the workspace's file are both told, and nothing once it is dropped
    #[test]
    fn a_workspace_subscription_tells_its_members_changes_and_its_lists() {
        use std::sync::mpsc::{Receiver, channel};
        use std::time::Duration;
        /// Whether nothing more is told for a second, what was told so far dropped first
        fn quiet(received: &Receiver<()>) -> bool {
            while received.try_recv().is_ok() {}
            received.recv_timeout(Duration::from_secs(1)).is_err()
        }
        let home = state("subscribe");
        let project = home.join("project");
        let tasks = project.join(atm_common::TASK_DIR).join("tasks.json");
        std::fs::create_dir_all(tasks.parent().unwrap()).unwrap();
        std::fs::write(&tasks, r#"{"meta": {"id": "one"}, "tasks": []}"#).unwrap();
        let workspaces = Workspaces::at(&home);
        let workspace = workspaces.default_workspace();
        workspace
            .add_project(Some(&project.to_string_lossy()), &home)
            .unwrap();
        let (sent, received) = channel();
        let on_change: OnChange = Arc::new(move || {
            let _ = sent.send(());
        });
        let subscription = workspace.subscribe(on_change).unwrap();
        std::thread::sleep(Duration::from_millis(200));
        std::fs::write(
            &tasks,
            r#"{"meta": {"id": "one"}, "tasks": [{"id": "T-1"}]}"#,
        )
        .unwrap();
        received
            .recv_timeout(Duration::from_secs(10))
            .expect("told of the member's change");
        let _ = quiet(&received);
        workspace.remove_project("one", &home).unwrap();
        received
            .recv_timeout(Duration::from_secs(10))
            .expect("told of the list's change");
        drop(subscription);
        std::thread::sleep(Duration::from_millis(100));
        std::fs::write(&tasks, r#"{"meta": {"id": "one"}, "tasks": []}"#).unwrap();
        assert!(quiet(&received), "nothing once dropped");
        std::fs::remove_dir_all(&home).unwrap();
    }

    /// ATM-767: through the real watcher, a lease renewed (the claims store rewritten, only the lease moved) tells a
    /// project's subscription nothing; another holder tells it
    #[test]
    fn a_project_subscription_is_told_of_what_the_page_shows_not_a_lease_renewed() {
        use std::sync::mpsc::channel;
        use std::time::Duration;
        let home = state("subscribe-lease");
        let project = home.join("project");
        let folder = project.join(atm_common::TASK_DIR);
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(
            folder.join("tasks.json"),
            r#"{"meta": {"id": "one"}, "tasks": [{"id": "T-1", "status": "in_progress"}]}"#,
        )
        .unwrap();
        let claims = |agent: &str, until: &str| {
            std::fs::write(
                folder.join("claims-store.json"),
                json!({"claims": {"T-1": {"task": "T-1", "agent": agent,
                    "lease_expires_at": until, "renewed_at": until}}})
                .to_string(),
            )
            .unwrap();
        };
        claims("claude:a", "2026-11-24T10:00:00Z");
        let workspaces = Workspaces::at(&home);
        let workspace = workspaces.default_workspace();
        workspace
            .add_project(Some(&project.to_string_lossy()), &home)
            .unwrap();
        let member = workspace.member("one").unwrap();
        let (sent, received) = channel();
        let on_change: OnChange = Arc::new(move || {
            let _ = sent.send(());
        });
        let subscription = member.subscribe(on_change).unwrap();
        std::thread::sleep(Duration::from_millis(200));
        claims("claude:a", "2026-11-24T10:05:00Z");
        assert!(
            received.recv_timeout(Duration::from_secs(2)).is_err(),
            "a renewal tells nothing"
        );
        claims("claude:b", "2026-11-24T10:05:00Z");
        received
            .recv_timeout(Duration::from_secs(10))
            .expect("told of the new holder");
        drop(subscription);
        std::fs::remove_dir_all(&home).unwrap();
    }
}
