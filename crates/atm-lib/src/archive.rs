//! The archive (the Python atm's `lib/archive.py`; docs/v4/data-model.md §5): finished tasks moved out of the working
//! tasks into the archive, so the working file stays small, and brought back.
//!
//! A move is one store change that tells the store it moves tasks into the archive (`moving_into_archive`): the
//! JSON-in-git store writes the archive first, then `tasks.json`; a restore the other way round. A task found in both
//! keeps its working copy and loses the archive's: an archive that crashed halfway simply hasn't happened, an
//! unarchive that crashed halfway is complete, and no task is ever lost.

use std::collections::HashSet;

use atm_common::archive::{archivable_by, archive_settings, finished_at, in_open_pr, recent_since};
use atm_common::py::{py_str, py_str_opt};
use atm_common::time::{format_iso, now_iso, parse_iso};
use atm_common::{Error, Kind, Result, Task, Vocabulary};
use atm_db::working::Working;
use serde_json::{Value, json};

use crate::edit::next_free_number;
use crate::project::Project;

/// What `atm db archive` did, or would do.
#[derive(Debug, Clone, PartialEq)]
pub struct Archived {
    /// The IDs of the tasks moved (or that would move).
    pub moved: Vec<Value>,
    /// The tasks found in both files, whose archive copy was dropped.
    pub duplicates_dropped: Vec<Value>,
    /// The finished tasks that stay, and why: `{id, reason}`.
    pub kept: Vec<Value>,
    /// The files' sizes afterwards, in bytes: (`tasks.json`, the archive).
    pub sizes: (u64, u64),
}

/// What `atm db archive` is asked for.
#[derive(Debug, Clone, Default)]
pub struct ArchiveRequest {
    /// Only the finished tasks of these sprints (`--sprint`).
    pub sprints: Vec<String>,
    /// Only those finished before this date or time (`--before`).
    pub before: Option<String>,
    /// Keep those finished in the last this many days (`--keep-days`), instead of `meta.archive.keep_days`.
    pub keep_days: Option<f64>,
    /// Only say what would move (`--dry-run`).
    pub dry_run: bool,
}

fn tasks_of(document: &Value) -> impl Iterator<Item = &Task> {
    document["tasks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
}

fn id_of(task: &Value) -> Value {
    task.get("id").cloned().unwrap_or(Value::Null)
}

/// Tasks in both files keep their `tasks.json` copy (see the module's documentation). The dropped IDs.
fn drop_duplicates(document: &Value, archive: &mut Value) -> Vec<Value> {
    let ids: Vec<Value> = document["tasks"]
        .as_array()
        .into_iter()
        .flatten()
        .map(id_of)
        .collect();
    let Some(archived) = archive["tasks"].as_array_mut() else {
        return Vec::new();
    };
    let dropped: Vec<Value> = archived
        .iter()
        .map(id_of)
        .filter(|id| ids.contains(id))
        .collect();
    archived.retain(|t| !ids.contains(&id_of(t)));
    dropped
}

/// A number of days as Python prints it: `--keep-days 3` is a float (`3.0`), `meta.archive.keep_days` as written.
fn days_text(document: &Value, keep: Option<f64>) -> String {
    if let Some(keep) = keep {
        return if keep.fract() == 0.0 {
            format!("{keep:.1}")
        } else {
            format!("{keep}")
        };
    }
    let given = document
        .get("meta")
        .and_then(|m| m.get("archive"))
        .and_then(|a| a.get("keep_days"))
        .filter(|d| d.as_f64().is_some_and(|n| n >= 0.0));
    given.map_or_else(
        || {
            let days = archive_settings(document).keep_days;
            format!("{days}")
        },
        py_str,
    )
}

/// The finished tasks that stay in `tasks.json`, and why (T-352), as [`archivable_by`] decides: in an open PR,
/// finished in the last keep days, not before `--before`, not in the `--sprint` chosen. `[{id, reason}]`, in file
/// order.
fn kept_finished(
    document: &Value,
    vocab: &Vocabulary,
    moving: &[Value],
    before: Option<i64>,
    keep: Option<f64>,
    sprints: Option<&HashSet<String>>,
) -> Vec<Value> {
    let days = days_text(document, keep);
    let recent = recent_since(document, keep);
    let moved: Vec<Value> = moving.iter().map(id_of).collect();
    let mut kept = Vec::new();
    for t in tasks_of(document) {
        let id = t.get("id").cloned().unwrap_or(Value::Null);
        if moved.contains(&id)
            || !matches!(
                vocab.kind_of(t.get("status")),
                Some(Kind::Done | Kind::Skipped)
            )
        {
            continue;
        }
        let finished = finished_at(t);
        let reason = if in_open_pr(t) {
            format!(
                "in open PR #{}",
                py_str_opt(t.get("pr").and_then(|p| p.get("number")))
            )
        } else if finished.is_some_and(|f| f > recent) {
            format!("finished in the last {days} days")
        } else if let Some(before) = before
            && finished.is_none_or(|f| f >= before)
        {
            format!("finished on or after {}", &format_iso(before)[..10])
        } else if sprints.is_some() {
            "not in the sprints chosen".to_string()
        } else {
            continue;
        };
        kept.push(json!({"id": id, "reason": reason}));
    }
    kept
}

/// `--before` as a time: a date (`2026-05-01`) or a full ISO time; none without one.
fn parse_before(before: Option<&str>) -> Result<Option<i64>> {
    let Some(before) = before.filter(|b| !b.is_empty()) else {
        return Ok(None);
    };
    let text = if before.contains('T') {
        before.to_string()
    } else {
        format!("{before}T00:00:00Z")
    };
    parse_iso(&text)
        .map(Some)
        .ok_or_else(|| Error::Invalid(format!("--before {before}: use a date like 2026-05-01")))
}

/// What an archive would do: (moving, dropped, kept). Drops the archive's duplicates.
fn plan_archive(
    document: &Value,
    archive: &mut Value,
    request: &ArchiveRequest,
    before: Option<i64>,
) -> Result<(Vec<Value>, Vec<Value>, Vec<Value>)> {
    let vocab = Vocabulary::of_document(document)?;
    let dropped = drop_duplicates(document, archive);
    let mut moving: Vec<Value> = archivable_by(document, &vocab, before, request.keep_days)
        .into_iter()
        .map(|t| Value::Object(t.clone()))
        .collect();
    let mut names = None;
    if !request.sprints.is_empty() {
        let mut chosen = HashSet::new();
        for name in &request.sprints {
            let Some(sprint) = vocab.find_sprint(Some(&json!(name))) else {
                return Err(Error::Failed {
                    message: format!("no sprint {}", atm_common::py::repr_str(name)),
                    code: 2,
                });
            };
            chosen.insert(sprint.get("id").map(py_str).unwrap_or_default());
        }
        moving.retain(|t| {
            vocab
                .find_sprint(t.get("sprint"))
                .and_then(|s| s.get("id"))
                .is_some_and(|id| chosen.contains(&py_str(id)))
        });
        names = Some(chosen);
    }
    let kept = kept_finished(
        document,
        &vocab,
        &moving,
        before,
        request.keep_days,
        names.as_ref(),
    );
    Ok((moving, dropped, kept))
}

/// `atm db archive`: move the finished tasks (DM-5.1) to the archive, those of the sprints asked for only and
/// finished before `before` if given, keeping those finished in the last keep days. With `dry_run`, only what it
/// would do.
///
/// # Errors
/// A `--before` that isn't a date, an unknown sprint, a store error.
pub fn archive_finished(project: &mut Project, request: &ArchiveRequest) -> Result<Archived> {
    let before = parse_before(request.before.as_deref())?;
    if request.dry_run {
        // Nothing changes: worked out on copies of what the store reads
        let store = project.store();
        let tasks: Vec<Value> = store
            .list_tasks()?
            .into_iter()
            .map(|t| Value::Object(t.clone()))
            .collect();
        let document = json!({"meta": store.document()?.get("meta"), "tasks": tasks});
        let archived: Vec<Value> = store
            .list_archived()?
            .into_iter()
            .map(|t| Value::Object(t.clone()))
            .collect();
        let mut archive = json!({"meta": {}, "tasks": archived});
        let (moving, dropped, kept) = plan_archive(&document, &mut archive, request, before)?;
        return Ok(Archived {
            moved: moving.iter().map(id_of).collect(),
            duplicates_dropped: dropped,
            kept,
            sizes: store.sizes(),
        });
    }
    let mut result = None;
    project.store_mut().change(&mut |work: &mut Working| {
        let document = work.document.clone();
        let mut archive = work.archive()?.clone();
        let (moving, dropped, kept) = plan_archive(&document, &mut archive, request, before)?;
        let ids: Vec<Value> = moving.iter().map(id_of).collect();
        if !moving.is_empty() || !dropped.is_empty() {
            work.moving_into_archive();
            // New IDs must never come back to an archived ID (DM-2.3)
            let next = next_free_number(&work.document);
            if !work.document.get("meta").is_some_and(Value::is_object) {
                work.document["meta"] = json!({});
            }
            work.document["meta"]["next_id"] = json!(next);
            if !archive.get("meta").is_some_and(Value::is_object) {
                archive["meta"] = json!({});
            }
            archive["meta"]["archived_at"] = json!(now_iso());
            let count = {
                let list = archive["tasks"]
                    .as_array_mut()
                    .ok_or_else(|| Error::failed("the archive's tasks aren't a list"))?;
                list.extend(moving);
                list.len()
            };
            work.tasks().retain(|t| !ids.contains(&id_of(t)));
            // The dashboard shows the count without loading the archive (dashboard D-5.4)
            work.document["meta"]["archived"] = json!(count);
            *work.archive()? = archive;
        }
        result = Some((ids, dropped, kept));
        Ok(())
    })?;
    let (moved, duplicates_dropped, kept) = result.unwrap_or_default();
    Ok(Archived {
        moved,
        duplicates_dropped,
        kept,
        sizes: project.store().sizes(),
    })
}

/// What `atm db unarchive` did.
#[derive(Debug, Clone, PartialEq)]
pub struct Unarchived {
    /// The IDs brought back.
    pub restored: Vec<String>,
    /// Those the next archive would move straight back (finished and old, T-357).
    pub moves_back: Vec<Value>,
}

/// `atm db unarchive`: bring archived tasks back.
///
/// # Errors
/// A task that isn't in the archive (already back, or never there), a store error.
pub fn unarchive_tasks(project: &mut Project, ids: &[String]) -> Result<Unarchived> {
    let mut result = None;
    project.store_mut().change(&mut |work: &mut Working| {
        let mut archive = work.archive()?.clone();
        drop_duplicates(&work.document, &mut archive);
        let found = |id: &str| {
            archive["tasks"]
                .as_array()
                .into_iter()
                .flatten()
                .rev()
                .find(|t| t.get("id").and_then(Value::as_str) == Some(id))
                .cloned()
        };
        let missing: Vec<&String> = ids.iter().filter(|i| found(i).is_none()).collect();
        if !missing.is_empty() {
            // Already back, or never there at all: two different mistakes (T-357)
            let live: Vec<Value> = work.document["tasks"]
                .as_array()
                .into_iter()
                .flatten()
                .map(id_of)
                .collect();
            let (here, unknown): (Vec<&String>, Vec<&String>) =
                missing.iter().partition(|i| live.contains(&json!(i)));
            let join = |l: &[&String]| l.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ");
            let mut parts = Vec::new();
            if !here.is_empty() {
                parts.push(format!("{} (already in tasks.json)", join(&here)));
            }
            if !unknown.is_empty() {
                parts.push(format!("{} (no such task)", join(&unknown)));
            }
            return Err(Error::failed(format!(
                "not in the archive: {}",
                parts.join("; ")
            )));
        }
        let back: Vec<Value> = ids.iter().filter_map(|i| found(i)).collect();
        work.tasks().extend(back);
        // Still finished and old: the next atm db archive would move it straight back (T-357)
        let vocab = Vocabulary::of_document(&work.document)?;
        let again: Vec<Value> = archivable_by(&work.document, &vocab, None, None)
            .into_iter()
            .filter_map(|t| t.get("id").and_then(Value::as_str))
            .filter(|id| ids.iter().any(|i| i == id))
            .map(|id| json!(id))
            .collect();
        let unique: HashSet<&String> = ids.iter().collect();
        let left = archive["tasks"].as_array().map_or(0, Vec::len);
        if !work.document.get("meta").is_some_and(Value::is_object) {
            work.document["meta"] = json!({});
        }
        work.document["meta"]["archived"] = json!(left.saturating_sub(unique.len()));
        if let Some(list) = archive["tasks"].as_array_mut() {
            list.retain(|t| {
                !t.get("id")
                    .and_then(Value::as_str)
                    .is_some_and(|id| ids.iter().any(|i| i == id))
            });
        }
        *work.archive()? = archive;
        result = Some(again);
        Ok(())
    })?;
    Ok(Unarchived {
        restored: ids.to_vec(),
        moves_back: result.unwrap_or_default(),
    })
}

/// The suggestion to archive (CLI-5.1), or none: when the working tasks are over the threshold.
///
/// # Errors
/// Task data that can't be read.
pub fn archive_hint_now(project: &Project) -> Result<Option<Value>> {
    let store = project.store();
    Ok(atm_common::archive::archive_hint(
        store.document()?,
        store.vocabulary()?,
        store.sizes().0,
    ))
}

/// The project's tasks as the REST API's v1 route gives them (docs/multi-project-api.md §2.5), in one snapshot:
/// `meta` (as read: a tolerant read leaves out what doesn't work), `bytes` (how big the task data is), `tasks` (each
/// with what atm derives for it), `problems` (what a tolerant read found wrong with the file) and, for the working
/// tasks, `archive_hint` when atm suggests archiving.
///
/// # Errors
/// A filter naming no such sprint or release (exit code 2), task data that can't be read.
pub fn task_snapshot(project: &Project, filters: &crate::query::Filters) -> Result<Value> {
    let store = project.store();
    let meta = store
        .document()?
        .get("meta")
        .filter(|m| atm_common::py::truthy(m))
        .cloned()
        .unwrap_or_else(|| json!({}));
    let mut snapshot = json!({
        "meta": meta,
        "bytes": store.sizes().0,
        "tasks": crate::query::task_views(project, filters)?,
        "problems": crate::query::read_problems(project)?,
    });
    if !filters.archived
        && let Some(hint) = archive_hint_now(project)?
    {
        snapshot["archive_hint"] = hint;
    }
    Ok(snapshot)
}

/// What changes when the project's data does (`{tasks, archive, claims, head, copies}`), for the REST API's `ETag`s.
#[must_use]
pub fn project_stamp(project: &Project) -> Value {
    project.store().stamp()
}

#[cfg(test)]
mod tests;
