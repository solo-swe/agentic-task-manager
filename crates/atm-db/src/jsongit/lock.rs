//! The lock folder (the Python atm's `db/jsongit/lock.py`, docs/parallel-agents-design.md §4): the same protocol, so
//! the Python and the Rust atm can work on one project at once during the switch.
//!
//! A lock is a folder: creating it succeeds for exactly one process, on every system and file system. The holder
//! writes `owner.json` in it and removes the folder when done. A crashed holder leaves the folder behind, so a waiter
//! breaks the lock when its owner is provably gone (same host, process not running) or it is older than a minute.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use std::{fs, io};

use atm_common::home::write_text_atomic;
use atm_common::py::to_json;
use atm_common::time::{now_iso, parse_iso, real_now};
use atm_common::{Error, Result};
use serde_json::{Value, json};

use super::files::read_json;

/// The lock's folder, in the folder it locks.
pub const LOCK_DIR: &str = ".lock";
const LOCK_OWNER: &str = "owner.json";
/// Seconds a command waits for the lock (`ATM_LOCK_TIMEOUT` overrides).
const LOCK_TIMEOUT: f64 = 10.0;
/// A lock older than this is abandoned: commands hold it for milliseconds.
const LOCK_STALE: f64 = 60.0;
const LOCK_POLL: Duration = Duration::from_millis(50);

fn lock_timeout() -> f64 {
    std::env::var("ATM_LOCK_TIMEOUT")
        .ok()
        .and_then(|t| t.trim().parse().ok())
        .unwrap_or(LOCK_TIMEOUT)
}

/// This machine's name, as `owner.json` records it.
fn host_name() -> String {
    sysinfo::System::host_name().unwrap_or_default()
}

/// Whether a process with this ID runs (a zombie, exited but not reaped, doesn't). When the system can't be asked,
/// it counts as running: a lock is broken only on proof that its owner is gone, or by its age.
fn pid_alive(pid: u32) -> bool {
    let pid = sysinfo::Pid::from_u32(pid);
    let Ok(mut system) = sysinfo::System::new() else {
        return true;
    };
    system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
    system
        .process(pid)
        .is_some_and(|p| p.status() != sysinfo::ProcessStatus::Zombie)
}

fn owner_of(lock: &Path) -> Option<Value> {
    read_json(&lock.join(LOCK_OWNER))
}

/// Seconds since the lock was taken: from `owner.json`, or the folder itself if `owner.json` isn't written yet.
fn lock_age(lock: &Path, owner: Option<&Value>) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    // reason: seconds since 1970 fit an f64 exactly for millions of years
    let now = real_now() as f64;
    let since = match owner {
        Some(owner) => owner
            .get("acquired_at")
            .and_then(Value::as_str)
            .and_then(parse_iso)
            .map(|s| {
                #[allow(clippy::cast_precision_loss)] // reason: as above
                let s = s as f64;
                s
            }),
        None => fs::metadata(lock)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs_f64()),
    };
    since.map_or(0.0, |since| now - since)
}

fn lock_is_stale(lock: &Path, owner: Option<&Value>) -> bool {
    if let Some(o) = owner
        && o.get("host").and_then(Value::as_str) == Some(host_name().as_str())
        && let Some(pid) = o
            .get("pid")
            .and_then(Value::as_u64)
            .and_then(|p| u32::try_from(p).ok())
        && !pid_alive(pid)
    {
        return true;
    }
    lock_age(lock, owner) > LOCK_STALE
}

/// Move a stale lock aside; true if this process broke it. Rename first, then check the owner inside the renamed
/// folder (docs/security-review.md F13): if what was moved isn't the stale lock judged here, it goes back.
fn break_lock(lock: &Path, owner: Option<&Value>) -> bool {
    let name = lock
        .file_name()
        .map_or_else(|| LOCK_DIR.into(), |n| n.to_string_lossy().into_owned());
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    let aside = lock.with_file_name(format!("{name}.stale.{}.{nanos:08x}", std::process::id()));
    if fs::rename(lock, &aside).is_err() {
        return false;
    }
    if owner_of(&aside).as_ref() != owner {
        // Someone else's live lock: put it back (unless a newer one is in place already)
        let _ = fs::rename(&aside, lock);
        return false;
    }
    let _ = fs::remove_dir_all(&aside);
    true
}

fn describe_owner(owner: Option<&Value>) -> String {
    match owner {
        None => "another process".into(),
        Some(o) => {
            let field = |key: &str| atm_common::py::py_str_opt(o.get(key));
            format!(
                "pid {} on {} since {}",
                field("pid"),
                field("host"),
                field("acquired_at")
            )
        }
    }
}

/// The locks this process holds, removed if it is stopped with SIGTERM (as Python's handler exits through them).
static HELD: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

/// Without SIGTERM (Windows) there is nothing to release on.
#[cfg(not(unix))]
fn release_on_sigterm() {}

#[cfg(unix)]
fn release_on_sigterm() {
    static ON_SIGTERM: std::sync::Once = std::sync::Once::new();
    ON_SIGTERM.call_once(|| {
        if let Ok(mut signals) = signal_hook::iterator::Signals::new([signal_hook::consts::SIGTERM])
        {
            std::thread::spawn(move || {
                if signals.forever().next().is_some() {
                    for lock in HELD.lock().map(|h| h.clone()).unwrap_or_default() {
                        let _ = fs::remove_dir_all(lock);
                    }
                    std::process::exit(128 + signal_hook::consts::SIGTERM);
                }
            });
        }
    });
}

/// The lock of a folder, held until it is dropped.
#[derive(Debug)]
pub struct Lock {
    path: PathBuf,
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
        if let Ok(mut held) = HELD.lock() {
            held.retain(|p| p != &self.path);
        }
    }
}

fn io_error(path: &Path) -> impl Fn(io::Error) -> Error + '_ {
    move |e| Error::failed(format!("{}: {e}", path.display()))
}

/// Take the lock of `directory` (creating the folder if needed), waiting for it up to `timeout` seconds
/// (`ATM_LOCK_TIMEOUT`, else 10, when None; 0 tries once).
///
/// # Errors
/// When the lock stays taken by a live holder until the time is up, or the folder can't be written.
pub fn lock(directory: &Path, timeout: Option<f64>) -> Result<Lock> {
    fs::create_dir_all(directory).map_err(io_error(directory))?;
    let lock = directory.join(LOCK_DIR);
    let wait = Duration::from_secs_f64(timeout.unwrap_or_else(lock_timeout).max(0.0));
    let deadline = Instant::now() + wait;
    release_on_sigterm();
    loop {
        match fs::create_dir(&lock) {
            Ok(()) => break,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                let owner = owner_of(&lock);
                if lock_is_stale(&lock, owner.as_ref()) && break_lock(&lock, owner.as_ref()) {
                    continue;
                }
                if Instant::now() >= deadline {
                    return Err(Error::failed(format!(
                        "timed out waiting for the lock {} (held by {})",
                        lock.display(),
                        describe_owner(owner.as_ref())
                    )));
                }
                std::thread::sleep(LOCK_POLL);
            }
            Err(e) => return Err(io_error(&lock)(e)),
        }
    }
    if let Ok(mut held) = HELD.lock() {
        held.push(lock.clone());
    }
    let held = Lock { path: lock };
    let owner = json!({"host": host_name(), "pid": std::process::id(), "acquired_at": now_iso()});
    write_text_atomic(&held.path.join(LOCK_OWNER), &(to_json(&owner) + "\n"))
        .map_err(io_error(&held.path))?;
    Ok(held)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_dir::TestDir;

    #[test]
    fn the_lock_is_a_folder_with_its_owner_removed_when_dropped() {
        let dir = TestDir::new();
        let held = lock(dir.path(), None).unwrap();
        let owner = owner_of(&dir.path().join(LOCK_DIR)).unwrap();
        assert_eq!(owner["pid"], json!(std::process::id()));
        assert_eq!(owner["host"], json!(host_name()));
        drop(held);
        assert!(!dir.path().join(LOCK_DIR).exists());
    }

    #[test]
    fn a_live_lock_times_out_naming_its_holder() {
        let dir = TestDir::new();
        let _held = lock(dir.path(), None).unwrap();
        let error = lock(dir.path(), Some(0.1)).unwrap_err().to_string();
        assert!(
            error.starts_with("timed out waiting for the lock "),
            "{error}"
        );
        assert!(
            error.contains(&format!("held by pid {} on ", std::process::id())),
            "{error}"
        );
    }

    #[test]
    fn a_lock_whose_owner_is_gone_or_old_is_broken() {
        let dir = TestDir::new();
        let planted = dir.path().join(LOCK_DIR);
        fs::create_dir(&planted).unwrap();
        fs::write(
            planted.join(LOCK_OWNER),
            json!({"host": host_name(), "pid": 999_999, "acquired_at": now_iso()}).to_string(),
        )
        .unwrap();
        drop(lock(dir.path(), Some(0.0)).unwrap());
        fs::create_dir(&planted).unwrap();
        fs::write(
            planted.join(LOCK_OWNER),
            json!({"host": "elsewhere", "pid": 1, "acquired_at": "2020-01-01T00:00:00Z"})
                .to_string(),
        )
        .unwrap();
        drop(lock(dir.path(), Some(0.0)).unwrap());
    }

    #[test]
    fn a_fresh_lock_without_an_owner_is_respected() {
        let dir = TestDir::new();
        fs::create_dir(dir.path().join(LOCK_DIR)).unwrap();
        assert!(lock(dir.path(), Some(0.0)).is_err());
        assert_eq!(describe_owner(None), "another process");
    }

    #[test]
    fn this_process_is_alive_and_a_made_up_one_isnt() {
        assert!(pid_alive(std::process::id()));
        assert!(!pid_alive(999_999));
    }
}
