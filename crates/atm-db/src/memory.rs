//! A store in memory (the Python atm's `db/memory.py`): for atm-lib's tests, and other tools' that shouldn't need files.

use atm_common::{Result, Task, Vocabulary};
use serde_json::{Map, Value, json};

use crate::interface::Store;
use crate::working::{Here, Working};

/// Task data held in memory.
#[derive(Debug, Clone)]
pub struct MemoryStore {
    document: Value,
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
            document: json!({"meta": meta, "tasks": Value::Array(tasks.into_iter().map(Value::Object).collect())}),
            archive,
            claims_store,
            vocabulary: Vocabulary::of_meta(meta)?,
        })
    }

    /// The same store with this claims store.
    #[must_use]
    pub fn with_claims(mut self, claims_store: Map<String, Value>) -> Self {
        self.claims_store = claims_store;
        self
    }

    /// The claims store as the last change left it.
    #[must_use]
    pub fn claims_store(&self) -> &Map<String, Value> {
        &self.claims_store
    }

    fn tasks(&self) -> impl Iterator<Item = &Task> {
        self.document["tasks"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_object)
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

impl Store for MemoryStore {
    fn vocabulary(&self) -> Result<&Vocabulary> {
        Ok(&self.vocabulary)
    }

    fn list_tasks(&self) -> Result<Vec<&Task>> {
        Ok(self.tasks().collect())
    }

    fn get_task(&self, id: &str) -> Result<Option<&Task>> {
        Ok(self
            .tasks()
            .find(|t| t.get("id").and_then(Value::as_str) == Some(id)))
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

    fn document(&self) -> Result<&Value> {
        Ok(&self.document)
    }

    fn read_claims(&self) -> Map<String, Value> {
        self.claims_store.clone()
    }

    fn sizes(&self) -> (u64, u64) {
        (0, 0)
    }

    fn change(&mut self, change: &mut dyn FnMut(&mut Working) -> Result<()>) -> Result<()> {
        let archive = json!({"meta": {}, "tasks": self.archive});
        let here = Here {
            worktree: None,
            branch: None,
        };
        let mut work = Working::new(
            self.document.clone(),
            self.claims_store.clone(),
            here,
            0,
            Box::new(move || Ok(archive.clone())),
        );
        change(&mut work)?;
        if let Some(archive) = work.changed_archive() {
            self.archive = objects(&archive["tasks"]);
        }
        self.vocabulary = Vocabulary::of_document(&work.document)?;
        self.document = std::mem::take(&mut work.document);
        self.claims_store = std::mem::take(&mut work.claims_store);
        work.take_deferred().map_or(Ok(()), Err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(v: Value) -> Task {
        match v {
            Value::Object(map) => map,
            other => panic!("not a task: {other}"),
        }
    }

    #[test]
    fn a_memory_store_holds_what_it_was_given() {
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
        assert_eq!(store.document().unwrap()["tasks"][0]["id"], "T-2");
        assert_eq!(store.sizes(), (0, 0));
    }

    #[test]
    fn a_change_is_kept_in_memory() {
        let mut store = MemoryStore::new(Vec::new(), Vec::new(), &Map::new()).unwrap();
        store
            .change(&mut |work| {
                work.tasks().push(json!({"id": "T-1"}));
                work.claims_store
                    .insert("claims".into(), json!({"T-1": {}}));
                Ok(())
            })
            .unwrap();
        assert!(store.get_task("T-1").unwrap().is_some());
        assert_eq!(store.read_claims()["claims"], json!({"T-1": {}}));
    }
}
