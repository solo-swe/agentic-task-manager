//! A store's working copy for one change (docs/layered-architecture.md §3.3; the Python atm's `db/working.py`):
//! what atm-lib reads and changes inside a store's `change`, and what the store then writes. Until the code that
//! still takes the whole task document has moved, the copy is the whole document.

use atm_common::{Error, Result};
use serde_json::{Map, Value, json};

/// Where a change runs: its working copy and the branch checked out there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Here {
    /// The working copy's folder, for a store that has them.
    pub worktree: Option<String>,
    /// The branch checked out there (a detached `HEAD`'s short SHA), outside git none.
    pub branch: Option<String>,
}

/// The data one change works on, to read and change in place.
pub struct Working {
    /// The task document (`{meta, tasks}`).
    pub document: Value,
    /// The claims store: `claims`, `completed`, `waiting`, `resolved`, `hints`.
    pub claims_store: Map<String, Value>,
    here: Here,
    size: u64,
    load_archive: Box<dyn Fn() -> Result<Value>>,
    archive: Option<Value>,
    before_archive: Option<Value>,
    before_store: Map<String, Value>,
    archive_first: bool,
    deferred: Option<Error>,
    /// The inbox and its archive (docs/inbox-design.md, ATM-759), each read on first use
    inbox: Lazy,
    inbox_archive: Lazy,
    inbox_archive_first: bool,
}

/// A document a change reads only if it asks for it, and the store writes only if it changed.
struct Lazy {
    load: Box<dyn Fn() -> Result<Value>>,
    value: Option<Value>,
    before: Option<Value>,
}

impl Lazy {
    fn new(load: Box<dyn Fn() -> Result<Value>>) -> Self {
        Self {
            load,
            value: None,
            before: None,
        }
    }

    fn get(&mut self) -> Result<&mut Value> {
        if self.value.is_none() {
            let value = (self.load)()?;
            self.before = Some(value.clone());
            self.value = Some(value);
        }
        Ok(self
            .value
            .as_mut()
            .expect("the document was read just above"))
    }

    fn changed(&self) -> Option<&Value> {
        self.value
            .as_ref()
            .filter(|v| Some(*v) != self.before.as_ref())
    }
}

/// An empty inbox, for a store that has none (the memory store, until it is given one)
fn no_inbox() -> Box<dyn Fn() -> Result<Value>> {
    Box::new(|| Ok(atm_common::inbox::empty_inbox()))
}

impl Working {
    /// The working copy of a change: the document and claims store as read, where it runs, the task data's size,
    /// and how to read the archive document (only when the change asks for it).
    #[must_use]
    pub fn new(
        document: Value,
        claims_store: Map<String, Value>,
        here: Here,
        size: u64,
        archive: Box<dyn Fn() -> Result<Value>>,
    ) -> Self {
        let before_store = claims_store.clone();
        Self {
            document,
            claims_store,
            here,
            size,
            load_archive: archive,
            archive: None,
            before_archive: None,
            before_store,
            archive_first: false,
            deferred: None,
            inbox: Lazy::new(no_inbox()),
            inbox_archive: Lazy::new(no_inbox()),
            inbox_archive_first: false,
        }
    }

    /// The same, reading the inbox and its archive with `inbox` and `archive` (only when the change asks for them).
    #[must_use]
    pub fn with_inbox(
        mut self,
        inbox: Box<dyn Fn() -> Result<Value>>,
        archive: Box<dyn Fn() -> Result<Value>>,
    ) -> Self {
        self.inbox = Lazy::new(inbox);
        self.inbox_archive = Lazy::new(archive);
        self
    }

    /// The inbox document (`{version, next, items}`), to read and change in place; read on first use.
    ///
    /// # Errors
    /// When the inbox exists but can't be read.
    pub fn inbox(&mut self) -> Result<&mut Value> {
        self.inbox.get()
    }

    /// The inbox's archive, the same way.
    ///
    /// # Errors
    /// When it exists but can't be read.
    pub fn inbox_archive(&mut self) -> Result<&mut Value> {
        self.inbox_archive.get()
    }

    /// This change moves items into the inbox's archive: it is written first, so a crash between the two writes
    /// leaves an item in both, never in neither (as the tasks' archive, DM-5.2).
    pub fn moving_into_inbox_archive(&mut self) {
        self.inbox_archive_first = true;
    }

    /// Whether the inbox's archive goes first.
    #[must_use]
    pub fn inbox_archive_first(&self) -> bool {
        self.inbox_archive_first
    }

    /// The inbox, when it was asked for and changed: the store writes it then, and only then.
    #[must_use]
    pub fn changed_inbox(&self) -> Option<&Value> {
        self.inbox.changed()
    }

    /// The inbox's archive, the same way.
    #[must_use]
    pub fn changed_inbox_archive(&self) -> Option<&Value> {
        self.inbox_archive.changed()
    }

    /// The working tasks, to read and change in place.
    ///
    /// # Panics
    /// Never: `tasks` is made a list first when it isn't one.
    pub fn tasks(&mut self) -> &mut Vec<Value> {
        if !self.document["tasks"].is_array() {
            self.document["tasks"] = json!([]);
        }
        self.document["tasks"]
            .as_array_mut()
            .expect("the tasks were made a list just above")
    }

    /// Where the change runs.
    #[must_use]
    pub fn here(&self) -> &Here {
        &self.here
    }

    /// How big the task data was when it was read, in bytes (the archive hint).
    #[must_use]
    pub fn size(&self) -> u64 {
        self.size
    }

    /// The archive document (`{meta, tasks}`), to read and change in place; read on first use.
    ///
    /// # Errors
    /// When the archive exists but can't be read.
    pub fn archive(&mut self) -> Result<&mut Value> {
        let archive = if let Some(archive) = self.archive.take() {
            archive
        } else {
            let archive = (self.load_archive)()?;
            self.before_archive = Some(archive.clone());
            archive
        };
        Ok(self.archive.insert(archive))
    }

    /// This change moves tasks from the working tasks into the archive: the archive is written first, so a crash
    /// between the two writes leaves a task in both, never in neither (DM-5.2).
    pub fn moving_into_archive(&mut self) {
        self.archive_first = true;
    }

    /// Whether the archive goes first.
    #[must_use]
    pub fn archive_first(&self) -> bool {
        self.archive_first
    }

    /// The archive, when it was asked for and changed: the store writes it then, and only then.
    #[must_use]
    pub fn changed_archive(&self) -> Option<&Value> {
        self.archive
            .as_ref()
            .filter(|a| Some(*a) != self.before_archive.as_ref())
    }

    /// Whether the claims store changed (keys in another order are no change).
    #[must_use]
    pub fn store_changed(&self) -> bool {
        self.claims_store != self.before_store
    }

    /// Report `error` once the change is written (a lease that expired is written as expired, then reported).
    pub fn defer(&mut self, error: Error) {
        self.deferred = Some(error);
    }

    /// The error a change deferred, if any.
    #[must_use]
    pub fn take_deferred(&mut self) -> Option<Error> {
        self.deferred.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn working() -> Working {
        let store = json!({"claims": {}, "hints": {}});
        Working::new(
            json!({"meta": {}, "tasks": [{"id": "T-1"}]}),
            store.as_object().unwrap().clone(),
            Here {
                worktree: None,
                branch: None,
            },
            10,
            Box::new(|| Ok(json!({"meta": {}, "tasks": []}))),
        )
    }

    #[test]
    fn what_changed_is_known_and_key_order_is_no_change() {
        let mut w = working();
        assert!(!w.store_changed());
        w.claims_store = json!({"hints": {}, "claims": {}})
            .as_object()
            .unwrap()
            .clone();
        assert!(!w.store_changed(), "the same store in another order");
        w.claims_store.insert("claims".into(), json!({"T-1": {}}));
        assert!(w.store_changed());
    }

    #[test]
    fn the_archive_is_read_only_when_asked_and_written_only_when_changed() {
        let mut w = working();
        assert!(w.changed_archive().is_none());
        w.archive().unwrap();
        assert!(w.changed_archive().is_none());
        w.archive().unwrap()["tasks"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id": "T-0"}));
        w.moving_into_archive();
        assert!(w.changed_archive().is_some() && w.archive_first());
    }

    #[test]
    fn the_inbox_and_its_archive_are_read_only_when_asked_and_written_only_when_changed() {
        let reads = std::rc::Rc::new(std::cell::Cell::new(0));
        let counted = std::rc::Rc::clone(&reads);
        let mut w = working().with_inbox(
            Box::new(move || {
                counted.set(counted.get() + 1);
                Ok(json!({"version": 1, "next": 2, "items": [{"id": "IN-1", "state": "created"}]}))
            }),
            Box::new(|| Err(Error::failed("unreadable"))),
        );
        assert!(w.changed_inbox().is_none() && w.changed_inbox_archive().is_none());
        assert_eq!(reads.get(), 0, "nothing read until asked");
        w.inbox().unwrap();
        w.inbox().unwrap();
        assert_eq!(reads.get(), 1, "read once");
        assert!(w.changed_inbox().is_none(), "read, not changed");
        w.inbox().unwrap()["next"] = json!(3);
        assert_eq!(w.changed_inbox().unwrap()["next"], 3);
        assert!(
            w.inbox_archive().is_err(),
            "an archive that can't be read is an error"
        );
        assert!(!w.inbox_archive_first());
        w.moving_into_inbox_archive();
        assert!(w.inbox_archive_first());
        // Without one given, an empty inbox
        let mut plain = working();
        assert_eq!(plain.inbox().unwrap()["items"], json!([]));
    }

    #[test]
    fn tasks_are_changed_in_place_and_errors_wait_for_the_write() {
        let mut w = working();
        w.tasks().push(json!({"id": "T-2"}));
        assert_eq!(w.document["tasks"].as_array().unwrap().len(), 2);
        w.defer(Error::failed("later"));
        assert_eq!(w.take_deferred(), Some(Error::failed("later")));
        assert_eq!((w.size(), w.here().branch.clone()), (10, None));
    }
}
