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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, mpsc};

    fn told() -> (OnChange, mpsc::Receiver<()>) {
        let (send, receive) = mpsc::channel();
        let send = std::sync::Mutex::new(send);
        let on_change: OnChange = Arc::new(move || {
            if let Ok(send) = send.lock() {
                let _ = send.send(());
            }
        });
        (on_change, receive)
    }

    #[test]
    fn a_folder_that_isnt_there_is_skipped_and_nothing_is_told() {
        let dir = crate::test_dir::TestDir::new();
        let (on_change, received) = told();
        let subscription = watch(&[dir.path().join("missing")], on_change).unwrap();
        std::fs::write(dir.path().join("a.txt"), "a").unwrap();
        assert!(
            received.recv_timeout(SETTLE * 3).is_err(),
            "no folder watched"
        );
        drop(subscription);
    }

    #[test]
    fn a_change_in_a_watched_folder_is_told_once_it_settles_until_dropped() {
        let dir = crate::test_dir::TestDir::new();
        let (on_change, received) = told();
        let subscription = watch(&[dir.path().to_path_buf()], on_change).unwrap();
        std::thread::sleep(Duration::from_millis(200));
        std::fs::write(dir.path().join("a.txt"), "a").unwrap();
        received
            .recv_timeout(Duration::from_secs(10))
            .expect("told of the change");
        drop(subscription);
        while received.recv_timeout(SETTLE * 2).is_ok() {}
        std::fs::write(dir.path().join("b.txt"), "b").unwrap();
        assert!(
            received.recv_timeout(SETTLE * 3).is_err(),
            "nothing once dropped"
        );
    }

    #[test]
    fn a_tree_watch_tells_a_change_deep_in_a_subfolder() {
        let dir = crate::test_dir::TestDir::new();
        let deep = dir.path().join("a/b");
        std::fs::create_dir_all(&deep).unwrap();
        let (on_change, received) = told();
        let subscription = watch_tree(&[dir.path().to_path_buf()], on_change).unwrap();
        std::thread::sleep(Duration::from_millis(200));
        std::fs::write(deep.join("c.txt"), "c").unwrap();
        received
            .recv_timeout(Duration::from_secs(10))
            .expect("told of the change below");
        drop(subscription);
    }
}
