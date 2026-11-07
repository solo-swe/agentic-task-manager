//! A store in memory (the Python atm's `db/memory.py`): for atm-lib's tests, and other tools' that shouldn't need files.

use atm_common::{Result, Task, Vocabulary};
use serde_json::{Map, Value};

use crate::interface::Store;

/// Task data held in memory.
#[derive(Debug, Clone)]
pub struct MemoryStore {
    tasks: Vec<Task>,
    archive: Vec<Task>,
    vocabulary: Vocabulary,
}

impl MemoryStore {
    /// A store with these working and archived tasks, and the vocabularies of `meta`.
    ///
    /// # Errors
    /// When `meta`'s vocabularies can't work.
    pub fn new(tasks: Vec<Task>, archive: Vec<Task>, meta: &Map<String, Value>) -> Result<Self> {
        Ok(Self {
            tasks,
            archive,
            vocabulary: Vocabulary::of_meta(meta)?,
        })
    }
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
        // The last one with the ID, as the archive's lookup has it
        Ok(self
            .archive
            .iter()
            .rev()
            .find(|t| t.get("id").and_then(Value::as_str) == Some(id)))
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
