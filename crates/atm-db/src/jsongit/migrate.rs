//! `atm db migrate`, the JSON-in-git store's migrations of old layouts (the Python atm's `db/jsongit/migrate.py`):
//! move a legacy `tasks.json` into the task folder and set the folder up (`.gitignore`, merge driver), clean up after
//! per-project dashboards (0.10.0-0.14.0: the dashboard now comes from the plugin, served by `atm service` for all
//! projects), and bring old task data up to date (finish times, phases to sprints, `with: user`). Never commits.

mod phases;

use std::path::Path;

use atm_common::Shown;

use atm_common::archive::archive_hint;
use atm_common::home::write_text_atomic;
use atm_common::py::{to_json, truthy};
use atm_common::{Error, Kind, Result, Vocabulary};
use serde_json::{Map, Value, json};

use super::claims::{CLAIMS_SNAPSHOT, read_store, write_claims_snapshot};
use super::files::{archive_document, load_tasks, read_json};
use super::lock::lock;
use super::paths::{DASHBOARD_FILE, Paths, legacy_files};
use super::setup::{ensure_merge_driver, in_git, is_tracked, rel, stage};
use crate::git::git;

pub use phases::{describe_sprints_from_phases, phases_to_sprints};

/// Runtime files in the task folder that must not be committed (the dashboard and server files are from before
/// 0.15.0; they stay listed so leftovers stay out of git).
const IGNORED: [&str; 6] = [
    "tasks-dashboard.html",
    ".server.json",
    ".server.log",
    "claims.json",
    "claims-store.json",
    ".lock/",
];
const OLD_SERVER_RECORD: &str = ".server.json";
const OLD_SERVER_LOG: &str = ".server.log";
/// Recorded in `meta.migrations` once `interactive` labels became `with: user`.
const WITH_USER_MIGRATION: &str = "interactive-label";

fn failed(path: &Path, e: &std::io::Error) -> Error {
    Error::failed(format!("{}: {e}", path.shown()))
}

fn write_json(path: &Path, value: &Value) -> Result<()> {
    write_text_atomic(path, &(to_json(value) + "\n")).map_err(|e| failed(path, &e))
}

/// Whether the file is owned by the user running atm (the owner of atm's own process).
#[cfg(unix)]
fn owned_by_user(path: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    let current = || -> Option<u32> {
        let pid = sysinfo::get_current_pid().ok()?;
        let mut system = sysinfo::System::new().ok()?;
        // The user is read only when asked for (on Linux a plain refresh leaves it out)
        system.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::Some(&[pid]),
            true,
            sysinfo::ProcessRefreshKind::nothing().with_user(sysinfo::UpdateKind::Always),
        );
        system.process(pid)?.user_id().map(|u| **u)
    };
    current() == Some(meta.uid())
}

/// Without user IDs (Windows), no file counts as the user's: an old server is never contacted there.
#[cfg(not(unix))]
fn owned_by_user(_path: &Path) -> bool {
    false
}

/// Whether an old server record is one atm wrote here: owned by this user and not tracked by git (atm always kept it
/// out of git). A record that came with the repository is only deleted: atm never contacts its port or signals its
/// pid (docs/security-review.md F14).
fn written_by_atm(root: &Path, path: &Path) -> bool {
    owned_by_user(path) && !is_tracked(root, path)
}

/// Ask the atm server on this port who it is (on localhost); none if nothing, or not an atm server, answers.
fn whoami(port: u64) -> Option<Map<String, Value>> {
    atm_common::dashboard::whoami(port, std::time::Duration::from_secs(1))
}

/// Stop process `pid` (SIGTERM); whether the signal was sent.
fn terminate(pid: u64) -> bool {
    let Ok(pid) = usize::try_from(pid) else {
        return false;
    };
    let pid = sysinfo::Pid::from(pid);
    let Ok(mut system) = sysinfo::System::new() else {
        return false;
    };
    system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
    system
        .process(pid)
        .and_then(|p| p.kill_with(sysinfo::Signal::Term))
        .unwrap_or(false)
}

/// Per-project dashboards (0.10.0-0.14.0): delete the project's copy of the dashboard, stop its server if one still
/// runs (only when whoami confirms it serves this folder), and remove the server's record and log. What was done
/// (`{dashboard, server}`), or none when there was nothing left.
fn clean_up_old_dashboard(paths: &Paths) -> Result<Option<Value>> {
    let task_dir = &paths.dir;
    let copy = task_dir.join(DASHBOARD_FILE);
    let removed = copy.is_file();
    if removed {
        std::fs::remove_file(&copy).map_err(|e| failed(&copy, &e))?;
    }
    let mut stopped = None;
    let record_path = task_dir.join(OLD_SERVER_RECORD);
    let record = if written_by_atm(&paths.root, &record_path) {
        read_json(&record_path)
    } else {
        None
    };
    if let Some(record) = record.as_ref().and_then(Value::as_object) {
        let info = record.get("port").and_then(Value::as_u64).and_then(whoami);
        let pid = record.get("pid").and_then(Value::as_u64);
        if let (Some(info), Some(pid)) = (info, pid) {
            let folder = task_dir.shown();
            if info.get("folder").and_then(Value::as_str) == Some(folder.as_str())
                && info.get("pid").and_then(Value::as_u64) == Some(pid)
                && terminate(pid)
            {
                stopped = Some(pid);
            }
        }
    }
    let mut had_files = false;
    for name in [OLD_SERVER_RECORD, OLD_SERVER_LOG] {
        let p = task_dir.join(name);
        if std::fs::symlink_metadata(&p).is_ok() {
            std::fs::remove_file(&p).map_err(|e| failed(&p, &e))?;
            had_files = true;
        }
    }
    if !(removed || stopped.is_some() || had_files) {
        return Ok(None);
    }
    Ok(Some(json!({"dashboard": removed, "server": stopped})))
}

/// The field a finished task's finish time goes in, by its status's kind.
fn finish_field(vocab: &Vocabulary, task: &Value) -> Option<&'static str> {
    match vocab.kind_of(task.get("status"))? {
        Kind::Done => Some("completed_at"),
        Kind::Skipped => Some("skipped_at"),
        _ => None,
    }
}

/// Give the finished tasks without a finish time their `updated_at`; how many it filled in.
fn fill_finish_times(vocab: &Vocabulary, tasks: &mut [Value]) -> usize {
    let mut filled = 0;
    for task in tasks.iter_mut().filter(|t| t.is_object()) {
        let Some(field) = finish_field(vocab, task) else {
            continue;
        };
        if task.get(field).is_some_and(truthy) {
            continue;
        }
        let Some(updated) = task.get("updated_at").filter(|u| truthy(u)).cloned() else {
            continue;
        };
        task[field] = updated;
        filled += 1;
    }
    filled
}

fn tasks_mut(document: &mut Value) -> &mut Vec<Value> {
    if !document["tasks"].is_array() {
        document["tasks"] = json!([]);
    }
    document["tasks"]
        .as_array_mut()
        .expect("made a list just above")
}

/// Finished tasks without a finish time get their `updated_at`, which is what the dashboard showed for them anyway,
/// so a later edit (which moves `updated_at`) doesn't move them in time. In `tasks.json` and the archive; once per
/// task. How many tasks were filled in.
fn backfill_finish_times(paths: &Paths) -> Result<usize> {
    let precheck = || -> Result<bool> {
        let loaded = load_tasks(paths)?;
        let vocab = &loaded.vocabulary;
        let mut tasks = loaded.document["tasks"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let mut archived = archive_document(paths)?["tasks"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        Ok(fill_finish_times(vocab, &mut tasks) + fill_finish_times(vocab, &mut archived) > 0)
    };
    // An unreadable file: the command the skill runs next reports the problem
    if !precheck().unwrap_or(false) {
        return Ok(0);
    }
    let _lock = lock(&paths.dir, None)?;
    let loaded = load_tasks(paths)?;
    let vocab = loaded.vocabulary;
    let mut document = loaded.document;
    let mut filled = fill_finish_times(&vocab, tasks_mut(&mut document));
    if filled > 0 {
        write_json(&paths.tasks, &document)?;
    }
    let archive = paths.archive();
    if archive.is_file() {
        let mut archived = archive_document(paths)?;
        let more = fill_finish_times(&vocab, tasks_mut(&mut archived));
        if more > 0 {
            write_json(&archive, &archived)?;
            filled += more;
        }
    }
    Ok(filled)
}

fn has_phases(document: &Value, archive: &Value) -> bool {
    document
        .get("meta")
        .and_then(|m| m.get("phases"))
        .is_some_and(truthy)
        || [document, archive].iter().any(|d| {
            d["tasks"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|t| t.get("phase").is_some())
        })
}

/// [`phases_to_sprints`] on the files under the lock, staging them in git (not committing). The sprints it added, or
/// none when there was nothing to convert.
fn migrate_phases(paths: &Paths) -> Result<Option<Vec<Value>>> {
    let precheck = || -> Result<bool> {
        Ok(has_phases(
            &load_tasks(paths)?.document,
            &archive_document(paths)?,
        ))
    };
    if !precheck().unwrap_or(false) {
        return Ok(None);
    }
    let archive = paths.archive();
    let written = {
        let _lock = lock(&paths.dir, None)?;
        let loaded = load_tasks(paths)?;
        let mut document = loaded.document;
        let mut archived = archive_document(paths)?;
        let added = phases_to_sprints(
            &mut document,
            tasks_mut(&mut archived),
            &loaded.vocabulary,
            &atm_common::time::now_iso(),
        );
        if let Some(meta) = archived.get_mut("meta").and_then(Value::as_object_mut) {
            meta.shift_remove("phases");
        }
        Vocabulary::of_document(&document)?;
        write_json(&paths.tasks, &document)?;
        let mut written = vec![paths.tasks.clone()];
        if archive.is_file() {
            write_json(&archive, &archived)?;
            written.push(archive.clone());
        }
        (written, added)
    };
    if in_git(&paths.root) {
        let files: Vec<&Path> = written.0.iter().map(std::path::PathBuf::as_path).collect();
        stage(&paths.root, &files);
    }
    Ok(Some(written.1))
}

/// The unfinished tasks labelled `interactive` without a `with`, unless the migration ran already.
fn interactive_tasks(document: &Value, vocab: &Vocabulary) -> Vec<usize> {
    let done = document
        .get("meta")
        .and_then(|m| m.get("migrations"))
        .and_then(Value::as_array)
        .is_some_and(|m| m.iter().any(|x| x == WITH_USER_MIGRATION));
    if done {
        return Vec::new();
    }
    document["tasks"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
        .filter(|(_, t)| {
            t.is_object()
                && t.get("labels")
                    .and_then(Value::as_array)
                    .is_some_and(|l| l.iter().any(|x| x == "interactive"))
                && !t.get("with").is_some_and(truthy)
                && !matches!(
                    vocab.kind_of(t.get("status")),
                    Some(Kind::Done | Kind::Skipped)
                )
        })
        .map(|(i, _)| i)
        .collect()
}

/// Tasks labelled `interactive` (work done with the user, T-240) get `with: user` (SR-6.5), once: the change is
/// recorded in `meta.migrations`, so a `with` taken off later isn't put back. The label stays. Only unfinished tasks
/// in `tasks.json`; nothing is written when none has the label. How many tasks it changed.
fn migrate_interactive(paths: &Paths) -> Result<usize> {
    let precheck = || -> Result<bool> {
        let loaded = load_tasks(paths)?;
        Ok(!interactive_tasks(&loaded.document, &loaded.vocabulary).is_empty())
    };
    if !precheck().unwrap_or(false) {
        return Ok(0);
    }
    let count = {
        let _lock = lock(&paths.dir, None)?;
        let loaded = load_tasks(paths)?;
        let mut document = loaded.document;
        let found = interactive_tasks(&document, &loaded.vocabulary);
        for &i in &found {
            document["tasks"][i]["with"] = json!("user");
        }
        if !document.get("meta").is_some_and(Value::is_object) {
            document["meta"] = json!({});
        }
        let mut migrations = document["meta"]
            .get("migrations")
            .filter(|m| truthy(m))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        migrations.push(json!(WITH_USER_MIGRATION));
        document["meta"]["migrations"] = Value::Array(migrations);
        write_json(&paths.tasks, &document)?;
        found.len()
    };
    if in_git(&paths.root) {
        stage(&paths.root, &[&paths.tasks]);
    }
    Ok(count)
}

/// Add any missing runtime entries to the folder's `.gitignore`, keeping the user's lines. Whether it changed.
fn ensure_gitignore(task_dir: &Path) -> Result<bool> {
    let p = task_dir.join(".gitignore");
    let mut text = std::fs::read_to_string(&p).unwrap_or_default();
    let missing: Vec<&str> = IGNORED
        .into_iter()
        .filter(|entry| !text.lines().any(|l| l == *entry))
        .collect();
    if missing.is_empty() {
        return Ok(false);
    }
    if text.is_empty() {
        text = "# atm runtime files (managed by atm)\n".into();
    } else if !text.ends_with('\n') {
        text.push('\n');
    }
    for entry in missing {
        text.push_str(entry);
        text.push('\n');
    }
    std::fs::write(&p, text).map_err(|e| failed(&p, &e))?;
    Ok(true)
}

/// Move a legacy `tasks.json` into the task folder (staged with `git mv` when git tracks it), set the folder up, and
/// bring the task data up to date. What it did: `{moved, removed_dashboard, warning, gitignore_updated,
/// merge_driver_updated, cleaned_up, finish_times_filled, sprints_from_phases, with_user_from_label[, suggest]}`.
///
/// # Errors
/// When a file can't be moved, read or written.
pub fn migrate(paths: &Paths) -> Result<Map<String, Value>> {
    let root = &paths.root;
    let mut result: Map<String, Value> = json!({
        "moved": null,
        "removed_dashboard": null,
        "warning": null,
        "gitignore_updated": false,
        "merge_driver_updated": false,
        "cleaned_up": null,
        "finish_times_filled": 0,
        "sprints_from_phases": null,
        "with_user_from_label": 0,
    })
    .as_object()
    .cloned()
    .unwrap_or_default();
    let legacy = legacy_files(root);
    if let Some(legacy) = &legacy {
        let (old, new) = (rel(root, &legacy.tasks), rel(root, &paths.tasks));
        if paths.tasks.is_file() {
            result.insert(
                "warning".into(),
                json!(format!(
                    "both {old} and {new} exist; using {new}. Remove the old file when you no longer need it."
                )),
            );
        } else {
            std::fs::create_dir_all(&paths.dir).map_err(|e| failed(&paths.dir, &e))?;
            let tracked = is_tracked(root, &legacy.tasks);
            if tracked {
                if git(&["mv", "--", &old, &new], root).is_none() {
                    return Err(Error::failed(format!("git mv {old} failed")));
                }
            } else {
                std::fs::rename(&legacy.tasks, &paths.tasks)
                    .map_err(|e| failed(&legacy.tasks, &e))?;
            }
            result.insert(
                "moved".into(),
                json!({"from": old, "to": new, "git": tracked}),
            );
            if let Some(dashboard) = &legacy.dashboard {
                if is_tracked(root, dashboard) {
                    let _ = git(&["rm", "--cached", "-q", "--", &rel(root, dashboard)], root);
                }
                std::fs::remove_file(dashboard).map_err(|e| failed(dashboard, &e))?;
                result.insert("removed_dashboard".into(), json!(rel(root, dashboard)));
            }
        }
    }
    if !paths.tasks.is_file() {
        return Ok(result);
    }
    if ensure_gitignore(&paths.dir)? {
        result.insert("gitignore_updated".into(), json!(true));
        if in_git(root) {
            stage(root, &[&paths.dir.join(".gitignore")]);
        }
    }
    result.insert(
        "merge_driver_updated".into(),
        json!(ensure_merge_driver(paths)?),
    );
    if !paths.dir.join(CLAIMS_SNAPSHOT).exists() {
        // So the dashboard finds the file from the start instead of logging a 404 on every refresh
        write_claims_snapshot(paths, read_store(paths).get("claims"))?;
    }
    result.insert(
        "cleaned_up".into(),
        clean_up_old_dashboard(paths)?.unwrap_or(Value::Null),
    );
    result.insert(
        "finish_times_filled".into(),
        json!(backfill_finish_times(paths)?),
    );
    result.insert(
        "sprints_from_phases".into(),
        migrate_phases(paths)?.map_or(Value::Null, Value::Array),
    );
    result.insert(
        "with_user_from_label".into(),
        json!(migrate_interactive(paths)?),
    );
    // An unreadable file: the command the skill runs next reports the problem
    if let Ok(loaded) = load_tasks(paths) {
        let size = std::fs::metadata(&paths.tasks).map_or(0, |m| m.len());
        if let Some(hint) = archive_hint(&loaded.document, &loaded.vocabulary, size) {
            result.insert("suggest".into(), json!({"archive": hint}));
        }
    }
    Ok(result)
}

/// What a migration did, in words, a line each ("Nothing to migrate." when nothing).
#[must_use]
pub fn describe_migration(r: &Map<String, Value>, paths: &Paths) -> Vec<String> {
    let mut lines = Vec::new();
    let text = |key: &str| r.get(key).and_then(Value::as_str).map(String::from);
    if let Some(warning) = text("warning") {
        lines.push(format!("warning: {warning}"));
    }
    if let Some(moved) = r.get("moved").and_then(Value::as_object) {
        let mut line = format!(
            "Moved {} to {}",
            atm_common::py::py_str_opt(moved.get("from")),
            atm_common::py::py_str_opt(moved.get("to"))
        );
        if moved.get("git").is_some_and(truthy) {
            line.push_str(" (staged with git mv, not committed)");
        }
        if let Some(dashboard) = text("removed_dashboard") {
            line += "; removed ";
            line += &dashboard;
            line += " (the dashboard now comes with the plugin: /atm:serve)";
        }
        lines.push(line + ".");
    } else if !paths.tasks.is_file() {
        lines.push("No tasks.json found. Run /atm:create-tasks to create one.".into());
    }
    if let Some(cleaned) = r.get("cleaned_up").filter(|c| truthy(c)) {
        let mut line = "Removed this project's old dashboard files".to_string();
        if let Some(pid) = cleaned.get("server").filter(|s| truthy(s)) {
            line += " and stopped its old dashboard server (pid ";
            line += &atm_common::py::py_str(pid);
            line += ")";
        }
        lines.push(
            line + ": one dashboard for all your projects now comes with the plugin (/atm:serve).",
        );
    }
    let count = |key: &str| r.get(key).and_then(Value::as_u64).unwrap_or(0);
    let filled = count("finish_times_filled");
    if filled > 0 {
        let tasks = if filled == 1 {
            "1 finished task".to_string()
        } else {
            format!("{filled} finished tasks")
        };
        lines.push(format!(
            "Filled in the finish time of {tasks} that had none, from the last update (completed_at/skipped_at)."
        ));
    }
    if let Some(sprints) = r.get("sprints_from_phases").and_then(Value::as_array) {
        lines.extend(describe_sprints_from_phases(sprints));
    }
    let together = count("with_user_from_label");
    if together > 0 {
        let tasks = if together == 1 {
            "1 task".to_string()
        } else {
            format!("{together} tasks")
        };
        lines.push(format!(
            "Gave {tasks} labelled interactive `with: user` (done together with you; unattended loops skip them)."
        ));
    }
    if lines.is_empty() {
        lines.push("Nothing to migrate.".into());
    }
    if let Some(hint) = r.get("suggest").and_then(|s| s.get("archive")) {
        lines.push(atm_common::py::py_str_opt(hint.get("text")));
    }
    lines
}

#[cfg(test)]
mod tests;
