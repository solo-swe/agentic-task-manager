//! Watching folders for changes, for the stores that keep their data in files (the task store's, ATM-555; the review
//! store's, ATM-563).

use std::path::PathBuf;
use std::time::Duration;

use atm_common::subscription::{OnChange, Subscription};
use atm_common::{Error, Result};
use notify::RecursiveMode;
use notify_debouncer_full::{DebounceEventResult, new_debouncer};

/// How long a file watcher waits for a burst of changes (an atm command writes a file or two, git many) to settle.
pub const SETTLE: Duration = Duration::from_millis(300);

/// Watch `folders` (those that are there; not their subfolders), calling `on_change` once each burst of changes in
/// them has settled ([`SETTLE`]), on the watcher's thread.
///
/// # Errors
/// The watcher can't be started, or a folder can't be watched.
pub fn watch(folders: &[PathBuf], on_change: OnChange) -> Result<Subscription> {
    watch_with(folders, RecursiveMode::NonRecursive, on_change)
}

/// [`watch`], with the folders' subfolders too: a store whose records are folders of files (the reviews').
///
/// # Errors
/// The watcher can't be started, or a folder can't be watched.
pub fn watch_tree(folders: &[PathBuf], on_change: OnChange) -> Result<Subscription> {
    watch_with(folders, RecursiveMode::Recursive, on_change)
}

fn watch_with(
    folders: &[PathBuf],
    mode: RecursiveMode,
    on_change: OnChange,
) -> Result<Subscription> {
    let failed = |e: notify::Error| Error::failed(format!("watching for changes: {e}"));
    let mut watcher = new_debouncer(SETTLE, None, move |result: DebounceEventResult| {
        if result.is_ok_and(|events| !events.is_empty()) {
            on_change();
        }
    })
    .map_err(failed)?;
    for folder in folders.iter().filter(|f| f.is_dir()) {
        watcher.watch(folder, mode).map_err(failed)?;
    }
    Ok(Subscription::new(watcher))
}
