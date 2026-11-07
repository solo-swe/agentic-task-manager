//! The store's interface (docs/layered-architecture.md §3.1): what atm-lib reads a project's task data through,
//! whatever holds it. The Python atm's `db/interface.py`; this slice has its reads.

use std::path::Path;

use atm_common::{Result, Task, Vocabulary};
use serde_json::{Map, Value};

use crate::working::Working;

/// A project's task data, as atm-lib reads it. One store is one snapshot: the data is read once, on first use.
pub trait Store {
    /// The project's vocabularies, checked.
    ///
    /// # Errors
    /// When the task data can't be read, or its vocabularies can't work.
    fn vocabulary(&self) -> Result<&Vocabulary>;

    /// The working tasks, in their order.
    ///
    /// # Errors
    /// When the task data can't be read.
    fn list_tasks(&self) -> Result<Vec<&Task>>;

    /// The working task with this ID, if there is one.
    ///
    /// # Errors
    /// When the task data can't be read.
    fn get_task(&self, id: &str) -> Result<Option<&Task>>;

    /// The archived task with this ID, if there is one.
    ///
    /// # Errors
    /// When the archive exists but can't be read.
    fn get_archived_task(&self, id: &str) -> Result<Option<&Task>>;

    /// The archived tasks, in the archive's order.
    ///
    /// # Errors
    /// When the archive exists but can't be read.
    fn list_archived(&self) -> Result<Vec<&Task>>;

    /// The folder the project's files are in (its working copy), when the store has one.
    fn working_copy(&self) -> Option<&Path>;

    /// The whole task document (`{meta, tasks}`), for the rules that still take it.
    ///
    /// # Errors
    /// When the task data can't be read.
    fn document(&self) -> Result<&Value>;

    /// The claims store, read without the lock (it is replaced atomically): `claims`, `completed`, `waiting`,
    /// `resolved`, `hints`.
    fn read_claims(&self) -> Map<String, Value>;

    /// The task data's sizes in bytes: (`tasks.json`, the archive).
    fn sizes(&self) -> (u64, u64);

    /// Run `change` on the task data under the store's isolation, and write what it changed; nothing when it fails.
    /// The store's snapshot is read again afterwards.
    ///
    /// # Errors
    /// The data can't be read or written, or `change` fails (or deferred an error).
    fn change(&mut self, change: &mut dyn FnMut(&mut Working) -> Result<()>) -> Result<()>;
}
