//! A store in memory (the Python atm's `db/memory.py`): for atm-lib's tests, and other tools' that shouldn't need files.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};

use atm_common::{Result, Task, Vocabulary};
use serde_json::{Map, Value, json};

use crate::interface::Store;
use crate::subscription::{OnChange, Subscription};
use crate::working::{Here, Working};

/// Task data held in memory.
#[derive(Debug, Clone)]
pub struct MemoryStore {
    document: Value,
    archive: Vec<Task>,
    claims_store: Map<String, Value>,
    vocabulary: Vocabulary,
    listeners: Listeners,
    here: Here,
}

/// Who is told of a memory store's changes: each live subscription's callback, by a number of its own. A clone of the
/// store is another store, with none.
#[derive(Default)]
struct Listeners(Arc<Mutex<Vec<(u64, OnChange)>>>, Arc<AtomicU64>);

impl Clone for Listeners {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl std::fmt::Debug for Listeners {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Listeners({})", lock(&self.0).len())
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// A memory store's subscription: dropping it takes its callback off the list.
struct Listening {
    listeners: Weak<Mutex<Vec<(u64, OnChange)>>>,
    id: u64,
}

impl Drop for Listening {
    fn drop(&mut self) {
        if let Some(listeners) = self.listeners.upgrade() {
            lock(&listeners).retain(|(id, _)| *id != self.id);
        }
    }
}

impl Listeners {
    fn add(&self, on_change: OnChange) -> Subscription {
        let id = self.1.fetch_add(1, Ordering::Relaxed);
        lock(&self.0).push((id, on_change));
        Subscription::new(Listening {
            listeners: Arc::downgrade(&self.0),
            id,
        })
    }

    /// Tell each one, on the writer's thread, without holding the list's lock.
    fn tell(&self) {
        let callbacks: Vec<OnChange> = lock(&self.0).iter().map(|(_, c)| Arc::clone(c)).collect();
        for on_change in callbacks {
            on_change();
        }
    }
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
            listeners: Listeners::default(),
            here: Here {
                worktree: None,
                branch: None,
            },
        })
    }

    /// The same store, as the working copy in this folder, on this branch: so a test can play one worktree of a clone
    /// whose claims store (`with_claims`) another worktree wrote (T-202). Without it, the store is in no working copy.
    #[must_use]
    pub fn with_here(mut self, worktree: &str, branch: &str) -> Self {
        self.here = Here {
            worktree: Some(worktree.into()),
            branch: Some(branch.into()),
        };
        self
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
        let mut work = Working::new(
            self.document.clone(),
            self.claims_store.clone(),
            self.here.clone(),
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
        self.listeners.tell();
        work.take_deferred().map_or(Ok(()), Err)
    }

    fn subscribe(&self, on_change: OnChange) -> Result<Subscription> {
        Ok(self.listeners.add(on_change))
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

    #[test]
    fn a_change_is_made_in_the_working_copy_the_store_was_given() {
        let here = |store: &mut MemoryStore| {
            let mut seen = None;
            store
                .change(&mut |work| {
                    seen = Some(work.here().clone());
                    Ok(())
                })
                .unwrap();
            seen.unwrap()
        };
        let mut none = MemoryStore::new(Vec::new(), Vec::new(), &Map::new()).unwrap();
        assert_eq!(
            here(&mut none),
            Here {
                worktree: None,
                branch: None
            }
        );
        let mut linked = none.with_here("/wt/agent-2", "agent-2");
        assert_eq!(
            here(&mut linked),
            Here {
                worktree: Some("/wt/agent-2".into()),
                branch: Some("agent-2".into())
            }
        );
    }

    /// T-555: a memory store is told by its own writes; a failed one, a clone's or a dropped subscription's isn't
    #[test]
    fn a_memory_store_tells_its_writes_to_its_subscriptions() {
        let mut store = MemoryStore::new(Vec::new(), Vec::new(), &Map::new()).unwrap();
        let (on_change, received) = crate::subscription::told();
        let subscription = store.subscribe(on_change).unwrap();
        let mut clone = store.clone();
        clone.change(&mut |_| Ok(())).unwrap();
        assert!(received.try_recv().is_err(), "a clone is another store");
        let _ = store.change(&mut |_| Err(atm_common::Error::failed("no")));
        assert!(received.try_recv().is_err(), "nothing written");
        store
            .change(&mut |work| {
                work.tasks().push(json!({"id": "T-1"}));
                Ok(())
            })
            .unwrap();
        assert!(received.try_recv().is_ok(), "told, on the writer's thread");
        drop(subscription);
        store.change(&mut |_| Ok(())).unwrap();
        assert!(received.try_recv().is_err(), "nothing once dropped");
    }
}
