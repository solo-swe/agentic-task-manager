//! When to suggest archiving, and what would move (the Python atm's `common/model.py`, DM-5, CLI-5.1): finished
//! tasks not finished in the last days and not in an open PR, once the task file is big or has many of them.

use serde_json::{Map, Value, json};

use crate::Task;
use crate::py::truthy;
use crate::time::{now, parse_iso};
use crate::vocabulary::{Kind, Vocabulary};

/// `meta.archive`, with the defaults for what it doesn't set (DM-2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArchiveSettings {
    /// Suggest archiving above this size of `tasks.json`, in KB.
    pub threshold_kb: f64,
    /// Suggest archiving above this many archivable tasks.
    pub threshold_tasks: f64,
    /// Keep tasks finished in the last this many days.
    pub keep_days: f64,
    /// Whether the skills archive on their own when atm suggests it (else they only relay the hint).
    pub auto: bool,
}

/// The project's archive settings.
#[must_use]
pub fn archive_settings(document: &Value) -> ArchiveSettings {
    let given = document
        .get("meta")
        .and_then(|m| m.get("archive"))
        .and_then(Value::as_object);
    let number = |key: &str, default: f64| {
        given
            .and_then(|g| g.get(key))
            .filter(|v| v.is_number())
            .and_then(Value::as_f64)
            .filter(|n| *n >= 0.0)
            .unwrap_or(default)
    };
    ArchiveSettings {
        threshold_kb: number("threshold_kb", 1024.0),
        threshold_tasks: number("threshold_tasks", 500.0),
        keep_days: number("keep_days", 14.0),
        auto: given
            .and_then(|g| g.get("auto"))
            .and_then(Value::as_bool)
            .unwrap_or(true),
    }
}

/// When a finished task finished: `completed_at` (done) or `skipped_at` (skipped), else `updated_at` for files from
/// before those were written.
#[must_use]
pub fn finished_at(task: &Task) -> Option<i64> {
    ["completed_at", "skipped_at", "updated_at"]
        .into_iter()
        .find_map(|key| task.get(key).and_then(Value::as_str).and_then(parse_iso))
}

fn tasks_of(document: &Value) -> impl Iterator<Item = &Map<String, Value>> {
    document["tasks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
}

fn finished(vocab: &Vocabulary, task: &Task) -> bool {
    matches!(
        vocab.kind_of(task.get("status")),
        Some(Kind::Done | Kind::Skipped)
    )
}

/// The tasks `atm db archive` would move (DM-5.1): done- and skipped-kind, not finished in the last `keep_days`, not
/// in an open PR.
#[must_use]
pub fn archivable<'a>(document: &'a Value, vocab: &Vocabulary) -> Vec<&'a Task> {
    let settings = archive_settings(document);
    #[allow(clippy::cast_possible_truncation)]
    // reason: days of seconds fit an i64 for any sane setting
    let recent = now() - (settings.keep_days * 86_400.0) as i64;
    tasks_of(document)
        .filter(|t| finished(vocab, t))
        // Not in an open PR: no PR recorded, or merged
        .filter(|t| {
            !t.get("pr").is_some_and(Value::is_object) || t.get("merged_at").is_some_and(truthy)
        })
        .filter(|t| finished_at(t).is_none_or(|f| f <= recent))
        .collect()
}

/// A size as atm says it: `1.2 MB`, or `512 KB` below a megabyte.
#[must_use]
pub fn format_size(size: u64) -> String {
    if size >= 1_048_576 {
        #[allow(clippy::cast_precision_loss)] // reason: one decimal of megabytes
        let mb = size as f64 / 1_048_576.0;
        format!("{mb:.1} MB")
    } else {
        format!("{} KB", size / 1024)
    }
}

/// The suggestion to archive (CLI-5.1), or None: when the task data (`size` bytes) is over `threshold_kb`, or has
/// more archivable tasks than `threshold_tasks`, and there is something to move. `{bytes, finished, archivable,
/// auto, text}`.
#[must_use]
pub fn archive_hint(document: &Value, vocab: &Vocabulary, size: u64) -> Option<Value> {
    let settings = archive_settings(document);
    let movable = archivable(document, vocab).len();
    #[allow(clippy::cast_precision_loss)] // reason: sizes and counts far below 2^52
    let (over_size, over_count) = (
        size as f64 > settings.threshold_kb * 1024.0,
        movable as f64 > settings.threshold_tasks,
    );
    if movable == 0 || !(over_size || over_count) {
        return None;
    }
    let done = tasks_of(document).filter(|t| finished(vocab, t)).count();
    Some(json!({
        "bytes": size,
        "finished": done,
        "archivable": movable,
        "auto": settings.auto,
        "text": format!(
            "tasks.json is {} with {done} finished tasks; `atm db archive` would move {movable} of them to tasks-archive.json.",
            format_size(size)
        ),
    }))
}

/// Unfinished tasks whose sprint is closed (T-330): reopened there, or left by a merge; the loop's default selection
/// never takes them. `[{id, name, status, sprint}]`.
#[must_use]
pub fn open_in_closed_sprints(vocab: &Vocabulary, document: &Value) -> Vec<Value> {
    tasks_of(document)
        .filter_map(|t| {
            let sprint = vocab.find_sprint(t.get("sprint"))?;
            (sprint.get("state").and_then(Value::as_str) == Some("closed") && !finished(vocab, t))
                .then(|| {
                    json!({
                        "id": t.get("id"),
                        "name": t.get("name").cloned().unwrap_or_else(|| json!("")),
                        "status": t.get("status"),
                        "sprint": sprint.get("id"),
                    })
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vocab(document: &Value) -> Vocabulary {
        Vocabulary::of_document(document).unwrap()
    }

    #[test]
    fn settings_have_defaults_and_ignore_what_cant_work() {
        let s = archive_settings(
            &json!({"meta": {"archive": {"keep_days": 3, "threshold_kb": -1, "auto": "no"}}}),
        );
        assert_eq!(
            s,
            ArchiveSettings {
                threshold_kb: 1024.0,
                threshold_tasks: 500.0,
                keep_days: 3.0,
                auto: true
            }
        );
    }

    #[test]
    fn archivable_tasks_are_finished_long_enough_ago_and_not_in_an_open_pr() {
        let doc = json!({"tasks": [
            {"id": "T-1", "status": "done", "completed_at": "2020-01-01T00:00:00Z"},
            {"id": "T-2", "status": "skipped", "skipped_at": "2020-01-01T00:00:00Z"},
            {"id": "T-3", "status": "done", "completed_at": "2020-01-01T00:00:00Z", "pr": {"number": 1}},
            {"id": "T-4", "status": "done", "completed_at": "2999-01-01T00:00:00Z"},
            {"id": "T-5", "status": "todo"},
            {"id": "T-6", "status": "done"}
        ]});
        let ids: Vec<&Value> = archivable(&doc, &vocab(&doc))
            .iter()
            .map(|t| &t["id"])
            .collect();
        assert_eq!(ids, [&json!("T-1"), &json!("T-2"), &json!("T-6")]);
    }

    #[test]
    fn the_hint_comes_when_the_file_is_big_or_holds_many() {
        let doc = json!({"meta": {"archive": {"threshold_tasks": 0}}, "tasks": [{"id": "T-1", "status": "done"}]});
        let hint = archive_hint(&doc, &vocab(&doc), 2048).unwrap();
        assert_eq!(
            hint["text"],
            json!(
                "tasks.json is 2 KB with 1 finished tasks; `atm db archive` would move 1 of them to tasks-archive.json."
            )
        );
        let small = json!({"tasks": [{"id": "T-1", "status": "done"}]});
        assert_eq!(archive_hint(&small, &vocab(&small), 2048), None);
        assert_eq!(format_size(3 * 1_048_576 / 2), "1.5 MB");
    }

    #[test]
    fn open_tasks_in_closed_sprints_are_found() {
        let doc = json!({"meta": {"sprints": [{"id": "Old", "state": "closed"}]}, "tasks": [
            {"id": "T-1", "status": "todo", "sprint": "old", "name": "x"}, {"id": "T-2", "status": "done", "sprint": "Old"}
        ]});
        assert_eq!(
            open_in_closed_sprints(&vocab(&doc), &doc),
            [json!({"id": "T-1", "name": "x", "status": "todo", "sprint": "Old"})]
        );
    }
}
