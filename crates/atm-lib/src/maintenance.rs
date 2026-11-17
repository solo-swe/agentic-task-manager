//! What only the JSON-in-git store has (ADR-003 §4, `project.store_maintenance()`): its file locations, bringing an
//! old task folder up to date, the merge driver's setup, and a lock test hook. A store without such things has no
//! `StoreMaintenance`; the CLI's `atm db` group reaches these through it, never through atm-db.

use std::path::{Path, PathBuf};

use atm_common::{Result, Shown};
use atm_db::jsongit::paths::{DASHBOARD_FILE, Paths, find_paths, legacy_files};

/// The JSON-in-git store's own operations, for the project at `root`.
#[derive(Debug, Clone)]
pub struct StoreMaintenance {
    root: PathBuf,
}

/// What `atm db init` set up in the store's working copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetUp {
    /// Whether the project is in a git repository.
    pub git: bool,
    /// Whether `.gitattributes` gained the merge driver's line (staged).
    pub gitattributes_added: bool,
}

/// What `atm db migrate` did: its result, the same in words, and whether there is a task file now.
#[derive(Debug, Clone, PartialEq)]
pub struct Migration {
    /// What it did (see [`atm_db::jsongit::migrate::migrate`]).
    pub result: serde_json::Map<String, serde_json::Value>,
    /// The same, a line each.
    pub lines: Vec<String>,
    /// Whether the project has a task file now.
    pub tasks_exists: bool,
}

impl StoreMaintenance {
    /// The store's operations for the project at or above `folder`.
    #[must_use]
    pub fn at(folder: &Path) -> Self {
        Self {
            root: folder.to_path_buf(),
        }
    }

    fn paths(&self) -> Result<Paths> {
        find_paths(&self.root)
    }

    /// `atm project config`: where the task files are: root, dir, tasks (and whether it exists), the old dashboard's
    /// place, and a task file left in a legacy location.
    ///
    /// # Errors
    /// The task folder's checks.
    pub fn task_paths(&self) -> Result<serde_json::Value> {
        let paths = self.paths()?;
        let text = |p: &Path| p.shown();
        let legacy = legacy_files(&paths.root).map(|l| {
            serde_json::json!({
                "tasks": text(&l.tasks),
                "dashboard": l.dashboard.as_deref().map(text),
            })
        });
        Ok(serde_json::json!({
            "root": text(&paths.root),
            "dir": text(&paths.dir),
            "tasks": text(&paths.tasks),
            "tasks_exists": paths.tasks.is_file(),
            "dashboard": text(&paths.dir.join(DASHBOARD_FILE)),
            "legacy": legacy,
        }))
    }

    /// `atm db init`: in git, the merge driver for the task files (registered in the clone, `.gitattributes` staged).
    ///
    /// # Errors
    /// The task folder's checks, a `.gitattributes` that can't be written.
    pub fn set_up(&self) -> Result<SetUp> {
        let paths = self.paths()?;
        let git = atm_vcs::GitVcs::new(&paths.root).in_repository();
        let gitattributes_added = git && atm_db::jsongit::setup::ensure_merge_driver(&paths)?;
        Ok(SetUp {
            git,
            gitattributes_added,
        })
    }

    /// `atm db migrate` (and `atm service`, before it shows a project): bring the task folder up to date. With
    /// `register_it`, the project joins the default workspace too (every skill runs migrate first). Task IDs are left
    /// as they are: giving them keys is `atm project key set` ([`StoreMaintenance::migrate_to_keys`]).
    ///
    /// # Errors
    /// The task folder's checks, a file that can't be moved, read or written.
    pub fn migrate(&self, register_it: bool) -> Result<Migration> {
        let paths = self.paths()?;
        let result = atm_db::jsongit::migrate::migrate(&paths)?;
        let tasks_exists = paths.tasks.is_file();
        if register_it && tasks_exists {
            atm_db::registry::register(&paths, true)?;
        }
        let lines = atm_db::jsongit::migrate::describe_migration(&result, &paths);
        Ok(Migration {
            result,
            lines,
            tasks_exists,
        })
    }

    /// The first task-keys migration (ADR-007 §6), which `atm project key set` runs on a project of IDs (`T-527`):
    /// every task stored by UUID, numbered, the references and claims rewritten, the files staged. Its report as JSON
    /// (`{key, old_prefix, tasks, archived, renumbered}`) and in words; none when the project has keys already.
    ///
    /// # Errors
    /// The task folder's checks, files that can't be read or written.
    pub fn migrate_to_keys(&self) -> Result<Option<(serde_json::Value, Vec<String>)>> {
        let paths = self.paths()?;
        let Some(r) = atm_db::jsongit::migrate::migrate_to_keys(&paths)? else {
            return Ok(None);
        };
        let json = serde_json::json!({
            "key": r.key,
            "old_prefix": r.old_prefix,
            "tasks": r.tasks,
            "archived": r.archived,
            "renumbered": r.renumbered.iter()
                .map(|(old, n)| serde_json::json!({"from": old, "number": n}))
                .collect::<Vec<_>>(),
        });
        Ok(Some((
            json,
            atm_db::jsongit::migrate::describe_keys_migration(&r),
        )))
    }

    /// `atm _lock-test <seconds>`: hold the task folder's lock for `seconds`, logging `acquire <pid>` and
    /// `release <pid>` to `.lock-test.log` in it, so tests can check that two processes never hold it at once.
    ///
    /// # Errors
    /// When the lock can't be taken in time, or the log can't be written.
    pub fn hold_lock_for_test(&self, seconds: f64) -> Result<()> {
        use std::io::Write;
        let dir = self.paths()?.dir;
        let log = dir.join(".lock-test.log");
        let append = |line: String| -> Result<()> {
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log)
                .map_err(|e| atm_common::Error::failed(format!("{}: {e}", log.shown())))?;
            writeln!(file, "{line}")
                .map_err(|e| atm_common::Error::failed(format!("{}: {e}", log.shown())))
        };
        let held = atm_db::jsongit::lock::lock(&dir, None)?;
        append(format!("acquire {}", std::process::id()))?;
        std::thread::sleep(std::time::Duration::from_secs_f64(seconds.max(0.0)));
        append(format!("release {}", std::process::id()))?;
        drop(held);
        Ok(())
    }
}

/// `atm db merge-driver` (git runs it, on three files, outside any opened project): merge the versions of a task file
/// task by task, or of a review's `comments.md` or `review.md` (by `path`, the file git merges; none from a driver
/// registered before reviews, which merged only task files), writing the result over `ours`. Returns the conflicts
/// (this branch's version was kept for each).
///
/// # Errors
/// A version that isn't task data (or a review header), or a result that can't be written.
pub fn merge_versions(
    base: &Path,
    ours: &Path,
    theirs: &Path,
    path: Option<&str>,
) -> Result<Vec<String>> {
    let name = path.map(|p| p.rsplit('/').next().unwrap_or(p));
    if !matches!(name, Some("comments.md" | "review.md")) {
        return atm_db::jsongit::merge::merge_files(base, ours, theirs);
    }
    let read = |p: &Path| {
        std::fs::read_to_string(p)
            .map_err(|e| atm_common::Error::failed(format!("{}: {e}", p.display())))
    };
    // The base is empty when both branches added the file
    let (b, o, t) = (
        std::fs::read_to_string(base).unwrap_or_default(),
        read(ours)?,
        read(theirs)?,
    );
    let (text, conflicts) = if name == Some("comments.md") {
        atm_review::merge::merge_comments(&b, &o, &t)
    } else {
        (atm_review::merge::merge_review(&b, &o, &t)?, Vec::new())
    };
    atm_common::home::write_text_atomic(ours, &text)
        .map_err(|e| atm_common::Error::failed(format!("{}: {e}", ours.display())))?;
    Ok(conflicts)
}
