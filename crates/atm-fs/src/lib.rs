//! What atm's file stores share (ATM-563): running git for a working copy's place, the folder lock, and the folder
//! watcher. The task store (atm-db) and the review store (atm-review) both build on it, so neither depends on the
//! other (docs/code-review-design.md §3.1).

pub mod git;
pub mod lock;
pub mod watch;

#[cfg(test)]
mod test_dir;

use std::path::Path;

use serde_json::Value;

/// A JSON file's value, none when it is missing or not JSON.
#[must_use]
pub fn read_json(path: &Path) -> Option<Value> {
    let bytes = std::fs::read(path).ok()?;
    let text = std::str::from_utf8(&bytes).ok()?;
    serde_json::from_str(text).ok()
}
