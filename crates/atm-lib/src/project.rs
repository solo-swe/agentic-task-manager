//! A project: its store and the actor working on it (and, as later slices come, its version control). The Python
//! atm's `lib/project.py`.

use atm_db::Store;

/// A project, as atm-lib works on it: everything goes through its store; changes are recorded as its actor's.
pub struct Project {
    store: Box<dyn Store>,
    actor: Option<String>,
}

impl Project {
    /// The project whose task data `store` holds, with no actor (reads need none).
    #[must_use]
    pub fn new(store: Box<dyn Store>) -> Self {
        Self { store, actor: None }
    }

    /// The same project, worked on by `actor` (who changes are recorded as, DM-4.5).
    #[must_use]
    pub fn with_actor(mut self, actor: Option<String>) -> Self {
        self.actor = actor;
        self
    }

    /// Who works on it.
    #[must_use]
    pub fn actor(&self) -> Option<&str> {
        self.actor.as_deref()
    }

    /// Its store.
    #[must_use]
    pub fn store(&self) -> &dyn Store {
        self.store.as_ref()
    }

    /// Its store, to change.
    pub fn store_mut(&mut self) -> &mut dyn Store {
        self.store.as_mut()
    }
}
