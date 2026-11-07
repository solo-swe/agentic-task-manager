//! The store's interface (docs/layered-architecture.md §3.1): what atm-lib reads a project's task data through,
//! whatever holds it. The Python atm's `db/interface.py`; this slice has its reads.

use std::path::Path;

use atm_common::{Error, Result, Task, Vocabulary};
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

    /// Whether the project has task data yet.
    fn exists(&self) -> bool {
        true
    }

    /// Write the project's first task data (`atm db init`).
    ///
    /// # Errors
    /// When it has some already, or it can't be written.
    fn create(&mut self, _document: &Value) -> Result<()> {
        Err(Error::failed("the project has task data already"))
    }

    /// The task data as stored, unchecked, for `atm doctor` to report on.
    ///
    /// # Errors
    /// When there is no task data at all.
    fn raw(&self) -> Result<Raw> {
        Ok(Raw {
            document: Some(self.document()?.clone()),
            archive: self.list_archived()?.into_iter().cloned().collect(),
            archive_error: None,
        })
    }

    /// What a tolerant read found wrong with the task data (T-377); nothing for a strict one.
    ///
    /// # Errors
    /// When the task data can't be read.
    fn problems(&self) -> Result<Problems> {
        Ok(Problems::default())
    }

    /// The branch checked out in the working copy (a detached `HEAD`'s short SHA), for a store that has one.
    fn branch(&self) -> Option<String> {
        None
    }

    /// What changes when the data does, for the REST API's `ETag`s: per part (`tasks`, `archive`, `claims`, `head`,
    /// `copies`), a value that changes with it, null for a part that isn't there.
    fn stamp(&self) -> Value {
        Value::Null
    }

    /// The project's working copies, main first: `[{path, branch, main}]`; none for a store without them.
    fn working_copies(&self) -> Vec<Value> {
        Vec::new()
    }

    /// The tasks in the working copy at `folder`, as its task file has them (entries that aren't tasks left out).
    fn copy_tasks(&self, _folder: &str) -> Vec<Value> {
        Vec::new()
    }

    /// The working copy at `folder` as a store of its own, read tolerantly, for its tasks' derived values.
    ///
    /// # Errors
    /// For a store without working copies, or a copy whose task folder fails its checks.
    fn copy_store(&self, _folder: &str) -> Result<Box<dyn Store>> {
        Err(Error::failed("this store has no working copies"))
    }
}

/// What a tolerant read found wrong ([`Store::problems`]), in `atm doctor`'s words.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Problems {
    /// About the file as a whole: entries left out, vocabularies that fell back to their defaults.
    pub file: Vec<String>,
    /// About single tasks, by ID.
    pub tasks: Map<String, Value>,
}

/// The task data as stored ([`Store::raw`]).
#[derive(Debug, Clone, PartialEq)]
pub struct Raw {
    /// The task document, none when it isn't task data (an object with a `tasks` list).
    pub document: Option<Value>,
    /// The archived tasks (empty when the archive can't be read).
    pub archive: Vec<Task>,
    /// Why the archive can't be read, if it can't.
    pub archive_error: Option<String>,
}
