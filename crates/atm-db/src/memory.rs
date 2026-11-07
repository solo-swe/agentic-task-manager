//! A store in memory (the Python atm's `db/memory.py`): for atm-lib's tests, and other tools' that shouldn't need files.

use atm_common::{Result, Task, Vocabulary};
use serde_json::{Map, Value};

use crate::interface::Store;
use crate::working::{Here, Working};

/// Task data held in memory.
#[derive(Debug, Clone)]
pub struct MemoryStore {
    meta: Map<String, Value>,
    tasks: Vec<Task>,
    archive: Vec<Task>,
    claims_store: Map<String, Value>,
    vocabulary: Vocabulary,
}

impl MemoryStore {
    /// A store with these working and archived tasks, and the vocabularies of `meta`.
    ///
    /// # Errors
    /// When `meta`'s vocabularies can't work.
    pub fn new(tasks: Vec<Task>, archive: Vec<Task>, meta: &Map<String, Value>) -> Result<Self> {
        let claims_store = ["claims", "completed", "waiting", "resolved", "hints"]
            .into_iter()
            .map(|k| (k.to_string(), Value::Object(Map::new())))
            .collect();
        Ok(Self {
            meta: meta.clone(),
            tasks,
            archive,
            claims_store,
            vocabulary: Vocabulary::of_meta(meta)?,
        })
    }

    /// The claims store as the last change left it.
    #[must_use]
    pub fn claims_store(&self) -> &Map<String, Value> {
        &self.claims_store
    }
}

fn objects(list: &Value) -> Vec<Task> {
    list.as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
        .cloned()
        .collect()
}

fn find<'a>(tasks: &'a [Task], id: &str) -> Option<&'a Task> {
    tasks
        .iter()
        .find(|t| t.get("id").and_then(Value::as_str) == Some(id))
}

impl Store for MemoryStore {
    fn vocabulary(&self) -> Result<&Vocabulary> {
        Ok(&self.vocabulary)
    }

    fn list_tasks(&self) -> Result<Vec<&Task>> {
        Ok(self.tasks.iter().collect())
    }

    fn get_task(&self, id: &str) -> Result<Option<&Task>> {
        Ok(find(&self.tasks, id))
    }

    fn working_copy(&self) -> Option<&std::path::Path> {
        None
    }

    fn get_archived_task(&self, id: &str) -> Result<Option<&Task>> {
        Ok(crate::jsongit::files::last_with_id(&self.archive, id))
    }

    fn list_archived(&self) -> Result<Vec<&Task>> {
        Ok(self.archive.iter().collect())
    }

    fn change(&mut self, change: &mut dyn FnMut(&mut Working) -> Result<()>) -> Result<()> {
        let document = serde_json::json!({"meta": self.meta, "tasks": self.tasks});
        let archive = serde_json::json!({"meta": {}, "tasks": self.archive});
        let here = Here {
            worktree: None,
            branch: None,
        };
        let mut work = Working::new(
            document,
            self.claims_store.clone(),
            here,
            0,
            Box::new(move || Ok(archive.clone())),
        );
        change(&mut work)?;
        if let Some(archive) = work.changed_archive() {
            self.archive = objects(&archive["tasks"]);
        }
        self.meta = work.document["meta"]
            .as_object()
            .cloned()
            .unwrap_or_default();
        self.vocabulary = Vocabulary::of_meta(&self.meta)?;
        self.tasks = objects(&work.document["tasks"]);
        self.claims_store = std::mem::take(&mut work.claims_store);
        work.take_deferred().map_or(Ok(()), Err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_memory_store_holds_what_it_was_given() {
        let task = |v: Value| v.as_object().unwrap().clone();
        let store = MemoryStore::new(
            vec![task(json!({"id": "T-2"}))],
            vec![task(json!({"id": "T-1"}))],
            &Map::new(),
        )
        .unwrap();
        assert_eq!(store.list_tasks().unwrap().len(), 1);
        assert!(store.get_task("T-2").unwrap().is_some());
        assert!(store.get_archived_task("T-1").unwrap().is_some());
        assert_eq!(store.vocabulary().unwrap().default_priority, "P2");
    }
}
