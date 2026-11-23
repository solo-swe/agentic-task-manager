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

/// Which changed paths count, for [`watch_tree_where`]: called with every path a settled burst touched.
pub type Keep = std::sync::Arc<dyn Fn(&[PathBuf]) -> bool + Send + Sync>;

/// [`watch_tree`], told only of a burst `keep` says counts (given its paths): a working copy, whose `.git/` and ignored
/// files change all the time without changing the work (ATM-607).
///
/// # Errors
/// The watcher can't be started, or a folder can't be watched.
pub fn watch_tree_where(
    folders: &[PathBuf],
    keep: Keep,
    on_change: OnChange,
) -> Result<Subscription> {
    let failed = |e: notify::Error| Error::failed(format!("watching for changes: {e}"));
    let mut watcher = new_debouncer(SETTLE, None, move |result: DebounceEventResult| {
        // Events missed: whatever changed is unknown, so it counts, whatever `keep` would say (ATM-727)
        let Ok(events) = result else {
            on_change();
            return;
        };
        if events.iter().any(|e| e.need_rescan()) {
            on_change();
            return;
        }
        let paths: Vec<PathBuf> = events
            .iter()
            .filter(|e| is_change(e))
            .flat_map(|e| e.paths.iter().cloned())
            .collect();
        if !paths.is_empty() && keep(&paths) {
            on_change();
        }
    })
    .map_err(failed)?;
    for folder in folders.iter().filter(|f| f.is_dir()) {
        watcher
            .watch(folder, RecursiveMode::Recursive)
            .map_err(failed)?;
    }
    Ok(Subscription::new(watcher))
}

fn watch_with(
    folders: &[PathBuf],
    mode: RecursiveMode,
    on_change: OnChange,
) -> Result<Subscription> {
    let failed = |e: notify::Error| Error::failed(format!("watching for changes: {e}"));
    let mut watcher = new_debouncer(SETTLE, None, move |result: DebounceEventResult| {
        // Events missed (an error, or the system's queue overflowed: a rescan) count as a change (ATM-727): a watch
        // that looks only when told mustn't miss one
        if result.is_err() || result.is_ok_and(|events| events.iter().any(|e| is_change(e))) {
            on_change();
        }
    })
    .map_err(failed)?;
    for folder in folders.iter().filter(|f| f.is_dir()) {
        watcher.watch(folder, mode).map_err(failed)?;
    }
    Ok(Subscription::new(watcher))
}

/// Whether an event changed something. Linux reports a file opened, read or closed unwritten as an event too (access),
/// and what is told reads files: git checking the ignore rules reads `.gitignore` and `.git/`, a store reads its own
/// files. Counting those, a watch would wake itself, again and again.
fn is_change(event: &notify::Event) -> bool {
    !event.kind.is_access()
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

    /// Once the watch is quiet: what was written just before it began may still come in late (`FSEvents` on macOS,
    /// inotify's first events on Linux), a few bursts at most; one that never goes quiet (a watch waking itself) fails
    fn quiet_after(received: &mpsc::Receiver<()>, what: &str) {
        for _ in 0..10 {
            if received.recv_timeout(SETTLE * 2).is_err() {
                return;
            }
        }
        panic!("{what}: never quiet");
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
    fn a_filtered_tree_watch_tells_only_the_changes_it_keeps() {
        let dir = crate::test_dir::TestDir::new();
        std::fs::create_dir_all(dir.path().join(".git")).unwrap();
        let (on_change, received) = told();
        let keep: Keep = Arc::new(|paths: &[PathBuf]| {
            paths
                .iter()
                .any(|p| !p.components().any(|c| c.as_os_str() == ".git"))
        });
        let subscription = watch_tree_where(&[dir.path().to_path_buf()], keep, on_change).unwrap();
        std::thread::sleep(Duration::from_millis(200));
        quiet_after(&received, "the start");
        std::fs::write(dir.path().join(".git/index"), "i").unwrap();
        assert!(
            received.recv_timeout(SETTLE * 4).is_err(),
            "a change in .git/ only is left out"
        );
        std::fs::write(dir.path().join("a.txt"), "a").unwrap();
        received
            .recv_timeout(Duration::from_secs(10))
            .expect("told of a change it keeps");
        drop(subscription);
    }

    /// Linux reports a read as an event (access): a watch tells changes only, so reading a watched file tells nothing,
    /// and a `keep` that reads files (git checking its ignore rules) doesn't wake its own watch again and again
    #[test]
    fn reading_a_watched_file_tells_nothing_and_a_keep_that_reads_never_wakes_itself() {
        let dir = crate::test_dir::TestDir::new();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, "a").unwrap();
        let read = file.clone();
        let (on_change, received) = told();
        let keep: Keep = Arc::new(move |_: &[PathBuf]| {
            let _ = std::fs::read_to_string(&read);
            true
        });
        let tree = watch_tree_where(&[dir.path().to_path_buf()], keep, on_change.clone()).unwrap();
        let flat = watch(&[dir.path().to_path_buf()], on_change).unwrap();
        std::thread::sleep(Duration::from_millis(200));
        quiet_after(&received, "the start");
        for _ in 0..3 {
            let _ = std::fs::read_to_string(&file).unwrap();
        }
        assert!(
            received.recv_timeout(SETTLE * 4).is_err(),
            "a read is no change"
        );
        std::fs::write(&file, "b").unwrap();
        received
            .recv_timeout(Duration::from_secs(10))
            .expect("a write is told");
        // Told for the write by each watch, then quiet: the keep's own read doesn't start another round
        quiet_after(&received, "after the write");
        drop((tree, flat));
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
