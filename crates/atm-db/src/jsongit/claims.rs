//! The JSON-in-git store's claims store (the Python atm's `db/jsongit/claims.py`, docs/parallel-agents-design.md):
//! which agent works on which task until when, what was finished, waits or was decided in another worktree, and when
//! hints were last shown. It lives in the git common folder, so all worktrees of a clone see it; outside git, in the
//! task folder. Lock order: the task folder, then the claims store, so two commands can never wait on each other.

use std::path::{Path, PathBuf};

use atm_common::Shown;

use atm_common::home::write_text_atomic;
use atm_common::py::to_json;
use atm_common::time::now_iso;
use atm_common::{Error, Result};
use serde_json::{Map, Value, json};

use super::files::read_json;
use super::lock::{Lock, lock};
use super::paths::Paths;
use crate::git::git;

const CLAIMS_STORE: &str = "claims.json"; // in <git common dir>/atm/, shared by all worktrees of a clone
const CLAIMS_STORE_NO_GIT: &str = "claims-store.json"; // in the task folder, outside git
/// The snapshot of the live claims the dashboard reads, in the task folder (gitignored).
pub const CLAIMS_SNAPSHOT: &str = "claims.json";

/// Where the claims store is: `<git common dir>/atm/`, or the task folder outside git.
#[must_use]
pub fn claims_dir(paths: &Paths) -> PathBuf {
    match git(&["rev-parse", "--git-common-dir"], &paths.root) {
        Some(common) => normalize(&paths.root.join(common)).join("atm"),
        None => paths.dir.clone(),
    }
}

/// A path with `.` and `..` worked out, without reading links (Python's `os.path.normpath`).
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

/// The claims store's file.
#[must_use]
pub fn claims_store_path(paths: &Paths) -> PathBuf {
    let dir = claims_dir(paths);
    let name = if dir == paths.dir {
        CLAIMS_STORE_NO_GIT
    } else {
        CLAIMS_STORE
    };
    dir.join(name)
}

/// The locks a change holds: the task folder's, and the claims store's when it is elsewhere (always in this order).
#[derive(Debug)]
pub struct Locks {
    _task_folder: Lock,
    _claims: Option<Lock>,
}

/// Take the task folder's lock and, if it is elsewhere, the claims store's.
///
/// # Errors
/// When either lock can't be taken in time.
pub fn locked_with_claims(paths: &Paths) -> Result<Locks> {
    let task_folder = lock(&paths.dir, None)?;
    let dir = claims_dir(paths);
    let claims = if dir == paths.dir {
        None
    } else {
        Some(lock(&dir, None)?)
    };
    Ok(Locks {
        _task_folder: task_folder,
        _claims: claims,
    })
}

/// The shared store, each section an object: live `claims`, `completed` tasks (done in some worktree), `waiting`
/// ones (blocked or in review in some worktree), `resolved` ones (decided in another worktree than they wait in), and
/// `hints` (when each hint was last shown).
#[must_use]
pub fn read_store(paths: &Paths) -> Map<String, Value> {
    let data = read_json(&claims_store_path(paths))
        .and_then(|d| d.as_object().cloned())
        .unwrap_or_default();
    // `claims` and `completed` as Python's `or {}` reads them: anything falsy is empty
    let or_empty = |key: &str| {
        data.get(key)
            .filter(|v| atm_common::py::truthy(v))
            .cloned()
            .unwrap_or_else(|| json!({}))
    };
    let section = |key: &str| {
        data.get(key)
            .filter(|v| v.is_object())
            .cloned()
            .unwrap_or_else(|| json!({}))
    };
    let mut store = Map::new();
    store.insert("claims".into(), or_empty("claims"));
    store.insert("completed".into(), or_empty("completed"));
    store.insert("waiting".into(), section("waiting"));
    store.insert("resolved".into(), section("resolved"));
    store.insert("hints".into(), section("hints"));
    store
}

/// Write the claims store, and the gitignored snapshot of its live claims the dashboard polls.
///
/// # Errors
/// When a file can't be written.
pub fn write_store(paths: &Paths, store: &Map<String, Value>) -> Result<()> {
    let dir = claims_dir(paths);
    std::fs::create_dir_all(&dir).map_err(|e| Error::failed(format!("{}: {e}", dir.shown())))?;
    let file = claims_store_path(paths);
    write_text_atomic(&file, &(to_json(&Value::Object(store.clone())) + "\n"))
        .map_err(|e| Error::failed(format!("{}: {e}", file.shown())))?;
    write_claims_snapshot(paths, store.get("claims"))
}

/// The gitignored copy of the live claims the dashboard polls, as a list by task.
///
/// # Errors
/// When the file can't be written.
pub fn write_claims_snapshot(paths: &Paths, claims: Option<&Value>) -> Result<()> {
    if !paths.dir.is_dir() {
        return Ok(());
    }
    let mut list: Vec<Value> = claims
        .and_then(Value::as_object)
        .map(|c| c.values().cloned().collect())
        .unwrap_or_default();
    list.sort_by(|a, b| {
        atm_common::py::py_str_opt(a.get("task")).cmp(&atm_common::py::py_str_opt(b.get("task")))
    });
    let file = paths.dir.join(CLAIMS_SNAPSHOT);
    write_text_atomic(
        &file,
        &(to_json(&json!({"generated_at": now_iso(), "claims": list})) + "\n"),
    )
    .map_err(|e| Error::failed(format!("{}: {e}", file.shown())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jsongit::paths::find_paths;
    use crate::test_dir::TestDir;
    use atm_common::TASK_DIR;

    #[test]
    fn outside_git_the_store_is_in_the_task_folder_and_reads_empty() {
        let dir = TestDir::new();
        std::fs::create_dir(dir.path().join(TASK_DIR)).unwrap();
        let paths = find_paths(dir.path()).unwrap();
        assert_eq!(
            claims_store_path(&paths),
            paths.dir.join("claims-store.json")
        );
        let store = read_store(&paths);
        assert_eq!(
            store.keys().collect::<Vec<_>>(),
            ["claims", "completed", "waiting", "resolved", "hints"]
        );
        assert!(store.values().all(|v| v == &json!({})));
    }

    #[test]
    fn written_claims_are_read_back_with_a_snapshot_for_the_dashboard() {
        let dir = TestDir::new();
        std::fs::create_dir(dir.path().join(TASK_DIR)).unwrap();
        let paths = find_paths(dir.path()).unwrap();
        let mut store = read_store(&paths);
        store.insert(
            "claims".into(),
            json!({"T-2": {"task": "T-2"}, "T-1": {"task": "T-1"}}),
        );
        write_store(&paths, &store).unwrap();
        assert_eq!(read_store(&paths)["claims"]["T-1"], json!({"task": "T-1"}));
        let snapshot = read_json(&paths.dir.join("claims.json")).unwrap();
        assert_eq!(
            snapshot["claims"],
            json!([{"task": "T-1"}, {"task": "T-2"}])
        );
        let _locks = locked_with_claims(&paths).unwrap();
        assert!(paths.dir.join(".lock").is_dir());
    }

    #[test]
    fn paths_are_normalized_without_reading_links() {
        assert_eq!(
            normalize(Path::new("/a/b/../c/./d")),
            PathBuf::from("/a/c/d")
        );
    }
}
