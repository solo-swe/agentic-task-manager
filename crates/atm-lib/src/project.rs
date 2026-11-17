//! A project: its store, its version control and the actor working on it. The Python atm's `lib/project.py`.
//!
//! Opened from a folder in it (`Project::open`, ADR-003 §4): the project's settings file says which store and which
//! version control it uses, and atm-db's and atm-vcs's factories build them. Tests give it a store of their own
//! (`Project::with_store`).

use std::path::{Path, PathBuf};

use atm_common::Result;
use atm_db::{ProjectSettings, Store};
use atm_vcs::GitVcs;

use crate::maintenance::StoreMaintenance;

/// A project, as atm-lib works on it: everything goes through its store; changes are recorded as its actor's.
pub struct Project {
    store: Box<dyn Store>,
    actor: Option<String>,
    /// Where it was opened from, and what its settings say: none for a store given directly.
    opened: Option<(PathBuf, ProjectSettings)>,
}

impl Project {
    /// The project whose task data `store` holds, with no actor (reads need none).
    #[must_use]
    pub fn new(store: Box<dyn Store>) -> Self {
        Self {
            store,
            actor: None,
            opened: None,
        }
    }

    /// The project on `store`, without a folder: tests' way in, on the in-memory store.
    #[must_use]
    pub fn with_store(store: Box<dyn Store>) -> Self {
        Self::new(store)
    }

    /// The project at or above `cwd`, on the store its settings name. Nothing is read until its data is asked for.
    ///
    /// # Errors
    /// The task folder's checks, an unusable settings file, a store atm doesn't know.
    pub fn open(cwd: &Path) -> Result<Self> {
        Self::opened(atm_db::open(cwd, false)?)
    }

    /// The project at or above `cwd`, its task file read as far as it can be, saying what is wrong instead of refusing
    /// it (T-377): for showing it (the REST API, `atm task list --derived --tolerant`).
    ///
    /// # Errors
    /// As [`Project::open`].
    pub fn open_tolerant(cwd: &Path) -> Result<Self> {
        Self::opened(atm_db::open(cwd, true)?)
    }

    fn opened(opened: atm_db::Opened) -> Result<Self> {
        // The version control is checked on open, so a project naming one atm doesn't know is refused at once
        atm_vcs::open(&opened.root, &opened.settings.vcs)?;
        // Another project's task, a dependency of one here, is looked up among the registered projects (ADR-007 §7)
        crate::foreign::install();
        Ok(Self {
            store: opened.store,
            actor: None,
            opened: Some((opened.root, opened.settings)),
        })
    }

    /// The same project, worked on by `actor` (who changes are recorded as, DM-4.5).
    #[must_use]
    pub fn with_actor(mut self, actor: Option<String>) -> Self {
        self.actor = actor;
        self
    }

    /// Who works on it.
    #[must_use]
    pub fn actor(&self) -> Option<&str> {
        self.actor.as_deref()
    }

    /// Its store.
    #[must_use]
    pub fn store(&self) -> &dyn Store {
        self.store.as_ref()
    }

    /// Its store, to change.
    pub fn store_mut(&mut self) -> &mut dyn Store {
        self.store.as_mut()
    }

    /// The stored ID of the task a reference names (ADR-007 §5): in a keys-format file, a key, a key with an earlier
    /// prefix, an alias or a UUID becomes the task's UUID (archived tasks too); in a file from before task keys the
    /// reference is the ID, given back as it is (the command then says what it says of an unknown one).
    ///
    /// # Errors
    /// In a keys-format file, a reference that names no task (saying why), or several; task data that can't be read.
    pub fn resolve_task(&self, reference: &str) -> Result<String> {
        let document = self.store.document()?;
        if !atm_common::keys::keys_format(document) {
            return Ok(reference.to_string());
        }
        let archive: Vec<atm_common::Task> =
            self.store.list_archived()?.into_iter().cloned().collect();
        let found = atm_common::keys::resolve(document, &archive, reference)?;
        Ok(found
            .task
            .get("id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(reference)
            .to_string())
    }

    /// A dependency or related task to store (ADR-007 §5, §7): a task of this project by [`Project::resolve_task`], or,
    /// when the key's prefix is another registered project's, that task as `<project id>:<UUID>`.
    ///
    /// # Errors
    /// No task of either; several projects with that key.
    pub fn resolve_dependency(&self, reference: &str) -> Result<String> {
        let reference = reference.trim();
        // Already another project's task (`<project id>:<UUID>`): kept when that task can be found (ADR-007 §8, the
        // way to name one whose key two projects share)
        if atm_common::rules::is_foreign_reference(reference) {
            return match crate::foreign::status(reference) {
                Some(_) => Ok(reference.to_string()),
                None => Err(atm_common::Error::NotFound(format!(
                    "no task {reference} among your registered projects"
                ))),
            };
        }
        match self.resolve_task(reference) {
            Ok(id) => Ok(id),
            Err(e) => {
                if !atm_common::keys::keys_format(self.store.document()?) {
                    return Err(e);
                }
                if atm_common::ids::valid_task_uuid(reference) {
                    // Another registered project's task by its UUID alone
                    return crate::foreign::find_uuid(reference, self.root()).ok_or(e);
                }
                match crate::foreign::resolve_in_other_project(reference, self.root())? {
                    Some(id) => Ok(id),
                    None => Err(e),
                }
            }
        }
    }

    /// [`Project::resolve_dependency`] for several.
    ///
    /// # Errors
    /// As [`Project::resolve_dependency`].
    pub fn resolve_dependencies(&self, references: &[String]) -> Result<Vec<String>> {
        references
            .iter()
            .map(|r| self.resolve_dependency(r))
            .collect()
    }

    /// [`Project::resolve_task`] for several.
    ///
    /// # Errors
    /// As [`Project::resolve_task`].
    pub fn resolve_tasks(&self, references: &[String]) -> Result<Vec<String>> {
        references.iter().map(|r| self.resolve_task(r)).collect()
    }

    /// How a task is shown to people (ADR-007 §1, §3): its key, `~` before a provisional one (`~ATM-538`), in a
    /// keys-format file; its ID before task keys.
    #[must_use]
    pub fn task_label(&self, task: &atm_common::Task) -> String {
        let id = || {
            task.get("id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string()
        };
        let Ok(document) = self.store.document() else {
            return id();
        };
        match atm_common::keys::key_of(document, task) {
            Some(key) if atm_common::keys::key_final(task) => key,
            Some(key) => format!("~{key}"),
            None => id(),
        }
    }

    /// Its root folder, when it was opened from one.
    #[must_use]
    pub fn root(&self) -> Option<&Path> {
        self.opened.as_ref().map(|(root, _)| root.as_path())
    }

    /// What its settings file says (the defaults without one), when it was opened from a folder.
    #[must_use]
    pub fn settings(&self) -> Option<&ProjectSettings> {
        self.opened.as_ref().map(|(_, settings)| settings)
    }

    /// Its version control, as its settings name it, when it was opened from a folder.
    #[must_use]
    pub fn vcs(&self) -> Option<GitVcs> {
        let (root, settings) = self.opened.as_ref()?;
        atm_vcs::open(root, &settings.vcs).ok()
    }

    /// List it in the `default` workspace, which every project atm meets joins (refreshing its entry), and say the ID
    /// it has there (`meta.id`, or another when that one is taken) and whether it is listed (`meta.dashboard.listed`).
    ///
    /// # Errors
    /// A project given its store (no folder), the task folder's checks, the state folder's lock or file.
    pub fn join_default_workspace(&self) -> Result<(String, bool)> {
        let root = self.root().ok_or_else(|| {
            atm_common::Error::Invalid("a project without a folder joins no workspace".into())
        })?;
        let paths = atm_db::jsongit::paths::find_paths(root)?;
        atm_db::registry::register(&paths, true)?;
        Ok(atm_db::registry::registered_id(&paths))
    }

    /// The harness hook's cheap way into `default` (after every tool call): one read of the workspace's file, and
    /// joining it, without waiting for the lock, only when the project isn't in it yet or was last seen a day ago.
    /// Never fails: the hook must not disturb a tool call.
    pub fn join_default_workspace_if_needed(&self) {
        if let Some(root) = self.root()
            && let Ok(paths) = atm_db::jsongit::paths::find_paths(root)
        {
            let _ = atm_db::registry::register_if_needed(&paths);
        }
    }

    /// What only its store has (the JSON-in-git store's merge driver, migrations and file locations), or none for a
    /// store without such things.
    #[must_use]
    pub fn store_maintenance(&self) -> Option<StoreMaintenance> {
        let (root, settings) = self.opened.as_ref()?;
        (settings.store == atm_db::open::DEFAULT_STORE).then(|| StoreMaintenance::at(root))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_project_given_its_store_has_no_folder_and_no_maintenance() {
        let store = atm_db::MemoryStore::new(vec![], vec![], &serde_json::Map::new()).unwrap();
        let project = Project::with_store(Box::new(store));
        assert!(project.root().is_none() && project.vcs().is_none());
        assert!(project.store_maintenance().is_none());
    }

    #[test]
    fn a_project_opened_from_its_folder_is_json_git_on_git() {
        let dir = std::env::temp_dir().join(format!("atm-lib-project-{}", std::process::id()));
        std::fs::create_dir_all(dir.join(atm_common::TASK_DIR)).unwrap();
        let project = Project::open(&dir).unwrap();
        assert_eq!(project.settings(), Some(&ProjectSettings::default()));
        assert!(project.vcs().is_some() && project.store_maintenance().is_some());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
