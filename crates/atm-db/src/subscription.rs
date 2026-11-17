//! Being told when a store's data changes (ADR-008 §5, T-555): what [`crate::Store::subscribe`] and the registry's
//! [`crate::registry::subscribe`] hand out. How a store hears of a change is its own business, push or poll: the JSON
//! store watches its files ([`watch`]), the memory store is told by its own writes, and a store that can't push may
//! poll its stamp ([`poll_stamp`]). What subscribes can't tell which.

use std::path::PathBuf;
#[cfg(test)]
use std::sync::Arc;
use std::sync::mpsc::{RecvTimeoutError, channel};
use std::time::Duration;

use atm_common::{Error, Result};
use notify::RecursiveMode;
use notify_debouncer_full::{DebounceEventResult, new_debouncer};
use serde_json::Value;

// atm-common's, shared with the review store (atm-review, ATM-561), so atm-lib can merge the two
pub use atm_common::subscription::{OnChange, Subscription};

/// How long a file watcher waits for a burst of changes (an atm command writes a file or two, git many) to settle.
pub const SETTLE: Duration = Duration::from_millis(300);

/// Watch `folders` (those that are there; not their subfolders), calling `on_change` once each burst of changes in
/// them has settled ([`SETTLE`]), on the watcher's thread.
///
/// # Errors
/// The watcher can't be started, or a folder can't be watched.
pub fn watch(folders: &[PathBuf], on_change: OnChange) -> Result<Subscription> {
    let failed = |e: notify::Error| Error::failed(format!("watching for changes: {e}"));
    let mut watcher = new_debouncer(SETTLE, None, move |result: DebounceEventResult| {
        if result.is_ok_and(|events| !events.is_empty()) {
            on_change();
        }
    })
    .map_err(failed)?;
    for folder in folders.iter().filter(|f| f.is_dir()) {
        watcher
            .watch(folder, RecursiveMode::NonRecursive)
            .map_err(failed)?;
    }
    Ok(Subscription::new(watcher))
}

/// Poll a store's stamp (`stamp`, a value that changes when the data does, as [`crate::Store::stamp`]) every `every`,
/// calling `on_change` when it differs from the one before, on the poller's own thread: for a store that can't push.
/// The interval is the store's choice (a remote one can poll more slowly). The stamp is read once now, as the first
/// to compare with.
///
/// # Errors
/// The poller's thread can't be started.
pub fn poll_stamp(
    mut stamp: impl FnMut() -> Value + Send + 'static,
    every: Duration,
    on_change: OnChange,
) -> Result<Subscription> {
    let (stop, stopped) = channel::<()>();
    let mut last = stamp();
    std::thread::Builder::new()
        .name("atm-poll-stamp".into())
        .spawn(move || {
            // Until the subscription is dropped (its sender with it)
            while let Err(RecvTimeoutError::Timeout) = stopped.recv_timeout(every) {
                let now = stamp();
                if now != last {
                    last = now;
                    on_change();
                }
            }
        })
        .map_err(|e| Error::failed(format!("polling for changes: {e}")))?;
    Ok(Subscription::new(stop))
}

/// An `OnChange` that sends on a channel, and its receiving end: for the tests.
#[cfg(test)]
pub(crate) fn told() -> (OnChange, std::sync::mpsc::Receiver<()>) {
    let (sent, received) = channel();
    let on_change: OnChange = Arc::new(move || {
        let _ = sent.send(());
    });
    (on_change, received)
}

/// Whether nothing more is told on `received` for a while (a second): what was told so far is dropped first.
#[cfg(test)]
pub(crate) fn quiet(received: &std::sync::mpsc::Receiver<()>) -> bool {
    while received.try_recv().is_ok() {}
    received.recv_timeout(Duration::from_secs(1)).is_err()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    #[test]
    fn polling_a_stamp_tells_when_it_changes_and_stops_when_dropped() {
        let version = Arc::new(AtomicU64::new(0));
        let read = Arc::clone(&version);
        let (on_change, received) = told();
        let subscription = poll_stamp(
            move || Value::from(read.load(Ordering::SeqCst)),
            Duration::from_millis(10),
            on_change,
        )
        .unwrap();
        // Unchanged: nothing told
        assert!(received.recv_timeout(Duration::from_millis(100)).is_err());
        version.store(1, Ordering::SeqCst);
        received
            .recv_timeout(Duration::from_secs(10))
            .expect("told of the change");
        // Once per change
        assert!(received.recv_timeout(Duration::from_millis(100)).is_err());
        drop(subscription);
        // The poller has ended: its callback, and the channel's sender with it, are gone
        assert!(matches!(
            received.recv_timeout(Duration::from_secs(10)),
            Err(RecvTimeoutError::Disconnected)
        ));
    }

    #[test]
    fn merged_subscriptions_end_together() {
        let (on_change, received) = told();
        let parts = [Duration::from_millis(10), Duration::from_millis(20)]
            .map(|every| poll_stamp(|| Value::Null, every, Arc::clone(&on_change)).unwrap());
        drop(on_change);
        let merged = Subscription::merged(parts);
        assert!(format!("{merged:?}").contains("parts: 2"));
        drop(merged);
        assert!(matches!(
            received.recv_timeout(Duration::from_secs(10)),
            Err(RecvTimeoutError::Disconnected)
        ));
    }
}
