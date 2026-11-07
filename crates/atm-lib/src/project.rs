//! A project: its store (and, as later slices come, its version control and the actor working on it). The Python
//! atm's `lib/project.py`.

use atm_db::Store;

/// A project, as atm-lib works on it: everything goes through its store.
pub struct Project {
    store: Box<dyn Store>,
}

impl Project {
    /// The project whose task data `store` holds.
    #[must_use]
    pub fn new(store: Box<dyn Store>) -> Self {
        Self { store }
    }

    /// Its store.
    #[must_use]
    pub fn store(&self) -> &dyn Store {
        self.store.as_ref()
    }
}
