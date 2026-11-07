//! The JSON-in-git store (docs/layered-architecture.md §3): the task files in the project's `.agentic-task-manager/`
//! folder, committed with the code, and the claims store. The Python atm's `db/jsongit/`.

pub mod claims;
pub mod files;
pub mod lock;
pub mod logo;
pub mod merge;
pub mod migrate;
pub mod paths;
pub mod setup;
pub mod worktrees;

use std::cell::OnceCell;

use atm_common::Shown;

use atm_common::home::write_text_atomic;
use atm_common::py::to_json;
use atm_common::{Error, Result, Task, Vocabulary};
use serde_json::Value;

use crate::git::checked_out_branch;
use crate::interface::{Problems, Raw, Store};
use crate::working::{Here, Working};
use claims::{claims_store_path, locked_with_claims, read_store, write_store};
use files::{
    Loaded, archive_document, last_with_id, load_archive, load_tasks, load_tasks_tolerant,
    read_json,
};
use paths::Paths;

/// The task files of one project, read once (one atm command works on one snapshot of them).
#[derive(Debug)]
pub struct JsonGitStore {
    paths: Paths,
    tolerant: bool,
    loaded: OnceCell<Result<(Loaded, Problems)>>,
    archive: OnceCell<Result<Vec<Task>>>,
}

impl JsonGitStore {
    /// The store of the task files at `paths`; nothing is read until something is asked for.
    #[must_use]
    pub fn new(paths: Paths) -> Self {
        Self {
            paths,
            tolerant: false,
            loaded: OnceCell::new(),
            archive: OnceCell::new(),
        }
    }

    /// The same, reading a malformed task file as far as it can be read and saying what is wrong
    /// ([`Store::problems`]), instead of refusing it: for showing it (the REST API's reads, T-377). Writes are strict
    /// either way.
    #[must_use]
    pub fn tolerant(paths: Paths) -> Self {
        Self {
            tolerant: true,
            ..Self::new(paths)
        }
    }

    /// Where its files are.
    #[must_use]
    pub fn paths(&self) -> &Paths {
        &self.paths
    }

    fn read(&self) -> Result<&(Loaded, Problems)> {
        self.loaded
            .get_or_init(|| {
                if self.tolerant {
                    load_tasks_tolerant(&self.paths)
                } else {
                    load_tasks(&self.paths).map(|l| (l, Problems::default()))
                }
            })
            .as_ref()
            .map_err(Clone::clone)
    }

    fn loaded(&self) -> Result<&Loaded> {
        self.read().map(|(loaded, _)| loaded)
    }

    /// The task file's path relative to the project's root (the same in every working copy).
    fn relative_tasks(&self) -> std::path::PathBuf {
        self.paths
            .tasks
            .strip_prefix(&self.paths.root)
            .map_or_else(|_| self.paths.tasks.clone(), std::path::Path::to_path_buf)
    }

    fn archive(&self) -> Result<&[Task]> {
        self.archive
            .get_or_init(|| load_archive(&self.paths))
            .as_deref()
            .map_err(Clone::clone)
    }

    /// `tasks.json`, written only when its text changed: a change that changed nothing (`loop next`, a heartbeat)
    /// leaves the file and its time alone, so open dashboards don't reload for nothing. The same data in another
    /// layout (a file edited by hand) is unchanged too.
    fn write_tasks(&self, document: &Value) -> Result<()> {
        let text = to_json(document) + "\n";
        let stored = std::fs::read_to_string(&self.paths.tasks).unwrap_or_default();
        let same =
            stored == text || serde_json::from_str::<Value>(&stored).is_ok_and(|s| &s == document);
        if same {
            return Ok(());
        }
        write_text_atomic(&self.paths.tasks, &text)
            .map_err(|e| Error::failed(format!("{}: {e}", self.paths.tasks.shown())))
    }

    fn write_archive(&self, archive: &Value) -> Result<()> {
        let file = self.paths.archive();
        write_text_atomic(&file, &(to_json(archive) + "\n"))
            .map_err(|e| Error::failed(format!("{}: {e}", file.shown())))
    }
}

impl JsonGitStore {
    /// Run `change` on the task data under the task folder's lock (and the claims store's), as every atm write
    /// always has: read `tasks.json` and the claims store, let `change` change them, write what changed. If `change`
    /// fails, nothing is written. A move into the archive writes the archive first; anything else `tasks.json` first
    /// (DM-5.2). An error the change deferred is returned after the writes.
    ///
    /// # Errors
    /// The lock can't be taken, the files can't be read or written, or `change` fails.
    pub fn apply(&mut self, change: &mut dyn FnMut(&mut Working) -> Result<()>) -> Result<()> {
        let _locks = locked_with_claims(&self.paths)?;
        let document = load_tasks(&self.paths)?.document;
        let size = std::fs::metadata(&self.paths.tasks).map_or(0, |m| m.len());
        let root = self.paths.root.clone();
        let here = Here {
            worktree: Some(root.shown()),
            branch: checked_out_branch(&root),
        };
        let paths = self.paths.clone();
        let mut work = Working::new(
            document,
            read_store(&self.paths),
            here,
            size,
            Box::new(move || archive_document(&paths)),
        );
        change(&mut work)?;
        let archive = work.changed_archive().cloned();
        if work.archive_first() {
            if let Some(archive) = &archive {
                self.write_archive(archive)?;
                // Tasks moved in: the archive merges task by task too
                setup::ensure_archive_merge(&self.paths)?;
            }
            self.write_tasks(&work.document)?;
        } else {
            self.write_tasks(&work.document)?;
            if let Some(archive) = &archive {
                self.write_archive(archive)?;
            }
        }
        if work.store_changed() {
            write_store(&self.paths, &work.claims_store)?;
        }
        // This store's snapshot is out of date now
        self.loaded = OnceCell::new();
        self.archive = OnceCell::new();
        work.take_deferred().map_or(Ok(()), Err)
    }
}

impl Store for JsonGitStore {
    fn vocabulary(&self) -> Result<&Vocabulary> {
        Ok(&self.loaded()?.vocabulary)
    }

    fn list_tasks(&self) -> Result<Vec<&Task>> {
        Ok(self.loaded()?.tasks().collect())
    }

    fn get_task(&self, id: &str) -> Result<Option<&Task>> {
        Ok(self
            .loaded()?
            .tasks()
            .find(|t| t.get("id").and_then(serde_json::Value::as_str) == Some(id)))
    }

    fn get_archived_task(&self, id: &str) -> Result<Option<&Task>> {
        Ok(last_with_id(self.archive()?, id))
    }

    fn list_archived(&self) -> Result<Vec<&Task>> {
        Ok(self.archive()?.iter().collect())
    }

    fn working_copy(&self) -> Option<&std::path::Path> {
        Some(&self.paths.root)
    }

    fn document(&self) -> Result<&Value> {
        Ok(&self.loaded()?.document)
    }

    fn read_claims(&self) -> serde_json::Map<String, Value> {
        read_store(&self.paths)
    }

    fn sizes(&self) -> (u64, u64) {
        let size = |p: &std::path::Path| {
            std::fs::metadata(p)
                .ok()
                .filter(std::fs::Metadata::is_file)
                .map_or(0, |m| m.len())
        };
        (size(&self.paths.tasks), size(&self.paths.archive()))
    }

    fn change(&mut self, change: &mut dyn FnMut(&mut Working) -> Result<()>) -> Result<()> {
        self.apply(change)
    }

    fn exists(&self) -> bool {
        self.paths.tasks.is_file()
    }

    fn create(&mut self, document: &Value) -> Result<()> {
        let folder = &self.paths.dir;
        let _lock = if folder.is_dir() {
            Some(lock::lock(folder, None)?)
        } else {
            None
        };
        if self.exists() {
            return Err(Error::failed(format!(
                "{} exists already",
                self.paths.tasks.shown()
            )));
        }
        std::fs::create_dir_all(folder)
            .map_err(|e| Error::failed(format!("{}: {e}", folder.shown())))?;
        write_text_atomic(&self.paths.tasks, &(to_json(document) + "\n"))
            .map_err(|e| Error::failed(format!("{}: {e}", self.paths.tasks.shown())))?;
        self.loaded = OnceCell::new();
        Ok(())
    }

    fn raw(&self) -> Result<Raw> {
        if !self.exists() {
            return Err(Error::failed(format!(
                "no tasks.json in {}. Run /atm:create-tasks first.",
                self.paths.dir.shown()
            )));
        }
        let document = files::read_json(&self.paths.tasks)
            .filter(|d| d.is_object() && d.get("tasks").is_some_and(Value::is_array));
        let (archive, archive_error) = match load_archive(&self.paths) {
            Ok(archive) => (archive, None),
            Err(e) => (Vec::new(), Some(e.to_string())),
        };
        Ok(Raw {
            document,
            archive,
            archive_error,
        })
    }

    fn problems(&self) -> Result<Problems> {
        self.read().map(|(_, problems)| problems.clone())
    }

    fn branch(&self) -> Option<String> {
        checked_out_branch(&self.paths.root)
    }

    fn stamp(&self) -> Value {
        let relative = self.relative_tasks();
        let copies: Vec<Value> = worktrees::worktrees(&self.paths.root)
            .into_iter()
            .map(|tree| {
                let path = tree
                    .get("path")
                    .and_then(Value::as_str)
                    .map(std::path::PathBuf::from)
                    .unwrap_or_default();
                let stamp = file_stamp(&path.join(&relative));
                serde_json::json!([tree, stamp])
            })
            .collect();
        serde_json::json!({
            "tasks": file_stamp(&self.paths.tasks),
            "archive": file_stamp(&self.paths.archive()),
            "claims": file_stamp(&claims_store_path(&self.paths)),
            "head": file_stamp(&self.paths.root.join(".git").join("HEAD")),
            "copies": copies,
        })
    }

    fn working_copies(&self) -> Vec<Value> {
        worktrees::worktrees(&self.paths.root)
    }

    fn copy_tasks(&self, folder: &str) -> Vec<Value> {
        let data = read_json(&std::path::Path::new(folder).join(self.relative_tasks()));
        data.as_ref()
            .and_then(|d| d.get("tasks"))
            .and_then(Value::as_array)
            .map(|tasks| {
                tasks
                    .iter()
                    .filter(|t| t.get("id").is_some_and(Value::is_string))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    fn copy_store(&self, folder: &str) -> Result<Box<dyn Store>> {
        let paths = paths::find_paths(std::path::Path::new(folder))?;
        Ok(Box::new(JsonGitStore::tolerant(paths)))
    }
}

/// `[size, nanosecond mtime]` of a file, or null if it doesn't exist: it changes with the file.
fn file_stamp(path: &std::path::Path) -> Value {
    let Ok(meta) = std::fs::metadata(path) else {
        return Value::Null;
    };
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos());
    serde_json::json!([meta.len(), mtime.to_string()])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_dir::TestDir;
    use atm_common::TASK_DIR;

    #[test]
    fn the_store_reads_tasks_and_the_archive_on_first_use() {
        let dir = TestDir::new();
        let folder = dir.path().join(TASK_DIR);
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(folder.join("tasks.json"), r#"{"tasks": [{"id": "T-2"}]}"#).unwrap();
        std::fs::write(
            folder.join("tasks-archive.json"),
            r#"{"tasks": [{"id": "T-1"}]}"#,
        )
        .unwrap();
        let store = JsonGitStore::new(paths::find_paths(dir.path()).unwrap());
        assert_eq!(store.list_tasks().unwrap().len(), 1);
        assert!(store.get_task("T-2").unwrap().is_some());
        assert!(store.get_task("T-1").unwrap().is_none());
        assert!(store.get_archived_task("T-1").unwrap().is_some());
        assert_eq!(store.vocabulary().unwrap().default_priority, "P2");
        assert_eq!(store.paths().tasks, folder.join("tasks.json"));
    }

    fn project_with(tasks: &str) -> (TestDir, JsonGitStore) {
        let dir = TestDir::new();
        let folder = dir.path().join(TASK_DIR);
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(folder.join("tasks.json"), tasks).unwrap();
        let store = JsonGitStore::new(paths::find_paths(dir.path()).unwrap());
        (dir, store)
    }

    #[test]
    fn a_change_writes_what_changed_and_the_store_reads_it_again() {
        let (_dir, mut store) = project_with("{\"tasks\": [{\"id\": \"T-1\"}]}");
        assert_eq!(store.list_tasks().unwrap().len(), 1);
        store
            .change(&mut |work| {
                work.tasks().push(serde_json::json!({"id": "T-2"}));
                work.claims_store
                    .insert("claims".into(), serde_json::json!({"T-2": {"task": "T-2"}}));
                Ok(())
            })
            .unwrap();
        assert_eq!(store.list_tasks().unwrap().len(), 2);
        let text = std::fs::read_to_string(&store.paths().tasks).unwrap();
        assert_eq!(
            text,
            "{\n  \"tasks\": [\n    {\n      \"id\": \"T-1\"\n    },\n    {\n      \"id\": \"T-2\"\n    }\n  ]\n}\n"
        );
        assert_eq!(
            claims::read_store(store.paths())["claims"]["T-2"]["task"],
            "T-2"
        );
        assert!(
            !store.paths().dir.join(".lock").exists(),
            "the lock is released"
        );
    }

    #[test]
    fn a_change_that_changes_nothing_leaves_the_file_as_it_was() {
        let (_dir, mut store) = project_with("{\"tasks\":[{\"id\":\"T-1\"}]}");
        store.change(&mut |_| Ok(())).unwrap();
        assert_eq!(
            std::fs::read_to_string(&store.paths().tasks).unwrap(),
            "{\"tasks\":[{\"id\":\"T-1\"}]}"
        );
        assert!(
            !claims::claims_store_path(store.paths()).exists(),
            "an unchanged claims store isn't written"
        );
    }

    #[test]
    fn a_failed_change_writes_nothing_and_a_deferred_error_comes_after_the_write() {
        let (_dir, mut store) = project_with("{\"tasks\": []}");
        let error = store
            .change(&mut |work| {
                work.tasks().push(serde_json::json!({"id": "T-1"}));
                Err(Error::failed("no"))
            })
            .unwrap_err();
        assert_eq!(error, Error::failed("no"));
        assert_eq!(store.list_tasks().unwrap().len(), 0);
        let error = store
            .change(&mut |work| {
                work.tasks().push(serde_json::json!({"id": "T-1"}));
                work.defer(Error::failed("later"));
                Ok(())
            })
            .unwrap_err();
        assert_eq!(error, Error::failed("later"));
        assert_eq!(store.list_tasks().unwrap().len(), 1);
    }

    #[test]
    fn a_move_into_the_archive_writes_the_archive() {
        let (_dir, mut store) =
            project_with("{\"tasks\": [{\"id\": \"T-1\", \"status\": \"done\"}]}");
        store
            .change(&mut |work| {
                let task = work.tasks().remove(0);
                work.moving_into_archive();
                work.archive()?["tasks"]
                    .as_array_mut()
                    .expect("a list")
                    .push(task);
                Ok(())
            })
            .unwrap();
        assert_eq!(store.list_tasks().unwrap().len(), 0);
        assert!(store.get_archived_task("T-1").unwrap().is_some());
    }

    #[test]
    fn an_error_reading_is_the_same_on_every_ask() {
        let dir = TestDir::new();
        let store = JsonGitStore::new(paths::find_paths(dir.path()).unwrap());
        let first = store.list_tasks().unwrap_err();
        assert_eq!(store.get_task("T-1").unwrap_err(), first);
    }
}
