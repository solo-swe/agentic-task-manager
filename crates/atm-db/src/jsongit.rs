//! The JSON-in-git store (docs/layered-architecture.md §3): the task files in the project's `.agentic-task-manager/`
//! folder, committed with the code. The Python atm's `db/jsongit/`; this slice reads them.

pub mod files;
pub mod paths;

use std::cell::OnceCell;
use std::collections::HashMap;

use atm_common::{Result, Task, Vocabulary};

use crate::interface::Store;
use files::{Loaded, load_archive, load_tasks};
use paths::Paths;

/// The task files of one project, read once (one atm command works on one snapshot of them).
#[derive(Debug)]
pub struct JsonGitStore {
    paths: Paths,
    loaded: OnceCell<Result<Loaded>>,
    archive: OnceCell<Result<HashMap<String, Task>>>,
}

impl JsonGitStore {
    /// The store of the task files at `paths`; nothing is read until something is asked for.
    #[must_use]
    pub fn new(paths: Paths) -> Self {
        Self {
            paths,
            loaded: OnceCell::new(),
            archive: OnceCell::new(),
        }
    }

    /// Where its files are.
    #[must_use]
    pub fn paths(&self) -> &Paths {
        &self.paths
    }

    fn loaded(&self) -> Result<&Loaded> {
        self.loaded
            .get_or_init(|| load_tasks(&self.paths))
            .as_ref()
            .map_err(Clone::clone)
    }

    fn archive(&self) -> Result<&HashMap<String, Task>> {
        self.archive
            .get_or_init(|| load_archive(&self.paths))
            .as_ref()
            .map_err(Clone::clone)
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
        Ok(self.archive()?.get(id))
    }
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

    #[test]
    fn an_error_reading_is_the_same_on_every_ask() {
        let dir = TestDir::new();
        let store = JsonGitStore::new(paths::find_paths(dir.path()).unwrap());
        let first = store.list_tasks().unwrap_err();
        assert_eq!(store.get_task("T-1").unwrap_err(), first);
    }
}
