//! The store's interface (docs/layered-architecture.md §3.1): what atm-lib reads a project's task data through,
//! whatever holds it. The Python atm's `db/interface.py`; this slice has its reads.

use std::path::Path;

use atm_common::{Result, Task, Vocabulary};

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
}
