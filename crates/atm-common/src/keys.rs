//! Task keys (ADR-007): in a format-2 file a task is stored by its UUID (`id`) and shown by its key, the project's
//! short name and the task's number (`ATM-527`). This module reads them: a task's key, the next number, the order by
//! number, and [`resolve`], the one way a reference people or agents type (a key, a key with an earlier prefix, an
//! alias, a UUID) becomes a task. A file from before task keys (format 1) has only IDs, matched exactly, as before.

use std::cmp::Ordering;

use serde_json::Value;

use crate::ids::{KEYS_FORMAT, file_format, task_key, valid_task_uuid};
use crate::py::repr_str;
use crate::rules::natural_cmp;
use crate::{Error, Result, Task};

/// Whether the document is in the keys format (tasks stored by UUID, shown by key).
#[must_use]
pub fn keys_format(document: &Value) -> bool {
    file_format(document) == Some(KEYS_FORMAT)
}

/// The project's short name (`meta.key`), if it has one.
#[must_use]
pub fn project_key(document: &Value) -> Option<&str> {
    document
        .get("meta")
        .and_then(|m| m.get("key"))
        .and_then(Value::as_str)
}

/// A task's number in its project (`number`), if it has one.
#[must_use]
pub fn number_of(task: &Task) -> Option<u64> {
    task.get("number").and_then(Value::as_u64)
}

/// A task's key (`ATM-527`) in a keys-format document; None in a legacy one, or for a task without a number.
#[must_use]
pub fn key_of(document: &Value, task: &Task) -> Option<String> {
    if !keys_format(document) {
        return None;
    }
    Some(task_key(project_key(document)?, number_of(task)?))
}

/// A copy of a task to show (ADR-007 §1): in a keys-format document, its `key` right after its `id` (worked out, never
/// stored); before task keys, the task as it is.
#[must_use]
pub fn with_key(document: &Value, task: &Task) -> Task {
    let Some(key) = key_of(document, task) else {
        return task.clone();
    };
    let mut out = Task::new();
    for (k, v) in task {
        if k == "key" {
            continue;
        }
        out.insert(k.clone(), v.clone());
        if k == "id" {
            out.insert("key".into(), Value::String(key.clone()));
        }
    }
    out
}

/// How a task reads to people: its key, plain from the start (ADR-009: no provisional keys, no `~`), or its `id` when
/// it has no key (a task as [`with_key`] gives it, or as stored before task keys).
#[must_use]
pub fn label_of(task: &Value) -> String {
    match task.get("key").and_then(Value::as_str) {
        Some(key) => key.to_string(),
        None => crate::py::py_str_opt(task.get("id")),
    }
}

/// A reference as people or agents typed it, without the `~` atm once printed before a provisional key (ADR-007,
/// gone with ADR-009): old notes, commits and review files still have it, and still find their task.
#[must_use]
pub fn without_draft_mark(reference: &str) -> &str {
    let trimmed = reference.trim();
    match trimmed.strip_prefix('~') {
        Some(rest) if !rest.is_empty() => rest,
        _ => trimmed,
    }
}

/// The order tasks are listed and taken in: by number when both have one (`ATM-2` before `ATM-10`), else by ID as
/// before task keys (`T-2` before `T-10`).
#[must_use]
pub fn task_order(a: &Task, b: &Task) -> Ordering {
    match (number_of(a), number_of(b)) {
        (Some(x), Some(y)) => x.cmp(&y),
        _ => natural_cmp(
            a.get("id").and_then(Value::as_str).unwrap_or_default(),
            b.get("id").and_then(Value::as_str).unwrap_or_default(),
        ),
    }
}

/// The number the next new task gets: `meta.next_number`, but never at or below a number a task (working or archived)
/// already has, so no number is ever given twice (ADR-007 §1).
#[must_use]
pub fn next_task_number(document: &Value, archive: &[Task]) -> u64 {
    let tasks = document["tasks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_object);
    let highest = tasks
        .chain(archive.iter())
        .filter_map(number_of)
        .max()
        .unwrap_or(0);
    let stored = document
        .get("meta")
        .and_then(|m| m.get("next_number"))
        .and_then(Value::as_u64)
        .unwrap_or(1);
    stored.max(highest + 1)
}

/// How a reference found its task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Via {
    /// A legacy file's ID, matched exactly.
    Id,
    /// The task's UUID.
    Uuid,
    /// Its current key.
    Key,
    /// A key with one of the project's earlier prefixes (`meta.key_history`).
    EarlierPrefix,
    /// One of the task's own aliases (a number that moved, an ID from before task keys).
    Alias,
}

/// A task a reference found.
#[derive(Debug, Clone, Copy)]
pub struct Found<'a> {
    /// The task, as stored.
    pub task: &'a Task,
    /// Whether it is in the archive.
    pub archived: bool,
    /// How the reference found it.
    pub via: Via,
}

/// `PREFIX-NUMBER` (`ATM-527`): the prefix and the number, or None when the text isn't one.
fn split_key(reference: &str) -> Option<(&str, u64)> {
    let (prefix, digits) = reference.rsplit_once('-')?;
    if prefix.is_empty() || digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some((prefix, digits.parse().ok()?))
}

fn tasks_of(document: &Value) -> impl Iterator<Item = &Task> {
    document["tasks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
}

/// The tasks matching `test`: the working ones, then the archived ones not also working (a task is in one or the
/// other; a later archive entry wins, as a dict of them by ID has it).
fn matching<'a>(
    document: &'a Value,
    archive: &'a [Task],
    test: &dyn Fn(&Task) -> bool,
) -> Vec<(&'a Task, bool)> {
    let mut found: Vec<(&Task, bool)> = tasks_of(document)
        .filter(|t| test(t))
        .map(|t| (t, false))
        .collect();
    if found.is_empty()
        && let Some(t) = archive.iter().rev().find(|t| test(t))
    {
        found.push((t, true));
    }
    found
}

fn one<'a>(found: &[(&'a Task, bool)], reference: &str, via: Via) -> Result<Option<Found<'a>>> {
    match found {
        [] => Ok(None),
        [(task, archived)] => Ok(Some(Found {
            task,
            archived: *archived,
            via,
        })),
        many => Err(Error::failed(format!(
            "{} names {} tasks ({}): use one's UUID, and run atm doctor",
            repr_str(reference),
            many.len(),
            many.iter()
                .filter_map(|(t, _)| t.get("id").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

/// The task a reference names (ADR-007 §5). In a keys-format document, in this order: the UUID; the current key
/// (`ATM-527`, any case); a key with an earlier prefix (`T-527`, from `meta.key_history`); a task's own alias (a key
/// that moved, an ID from before task keys). A current key wins over an alias: when a provisional number moved, the
/// task that holds the number now is the one its key names, and the alias finds the other only once no task holds
/// it. In a legacy document, the ID exactly. The archive too, after the working tasks.
///
/// # Errors
/// [`Error::NotFound`] when nothing matches, saying why (another project's key, no such number); a failure when the
/// reference names more than one task (a file to repair).
pub fn resolve<'a>(document: &'a Value, archive: &'a [Task], reference: &str) -> Result<Found<'a>> {
    // `~ATM-5`, as atm printed a provisional key before ADR-009, is `ATM-5`
    let reference = without_draft_mark(reference);
    let id_is = |t: &Task| t.get("id").and_then(Value::as_str) == Some(reference);
    if !keys_format(document) {
        return one(&matching(document, archive, &id_is), reference, Via::Id)?
            .ok_or_else(|| Error::NotFound(format!("no task {}", repr_str(reference))));
    }
    if valid_task_uuid(reference) {
        return one(&matching(document, archive, &id_is), reference, Via::Uuid)?
            .ok_or_else(|| Error::NotFound(format!("no task with the UUID {reference}")));
    }
    let key = project_key(document).unwrap_or_default();
    let history: Vec<&str> = document
        .get("meta")
        .and_then(|m| m.get("key_history"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let parsed = split_key(reference);
    if let Some((prefix, number)) = parsed {
        let has_number = |t: &Task| number_of(t) == Some(number);
        if prefix.eq_ignore_ascii_case(key)
            && let Some(found) = one(
                &matching(document, archive, &has_number),
                reference,
                Via::Key,
            )?
        {
            return Ok(found);
        }
        if history.iter().any(|h| prefix.eq_ignore_ascii_case(h))
            && let Some(found) = one(
                &matching(document, archive, &has_number),
                reference,
                Via::EarlierPrefix,
            )?
        {
            return Ok(found);
        }
    }
    let alias_is = |t: &Task| {
        t.get("aliases").and_then(Value::as_array).is_some_and(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .any(|x| x.eq_ignore_ascii_case(reference))
        })
    };
    if let Some(found) = one(
        &matching(document, archive, &alias_is),
        reference,
        Via::Alias,
    )? {
        return Ok(found);
    }
    Err(Error::NotFound(match parsed {
        Some((prefix, _))
            if !prefix.eq_ignore_ascii_case(key)
                && !history.iter().any(|h| prefix.eq_ignore_ascii_case(h)) =>
        {
            format!(
                "no task {}: {prefix} isn't this project's key ({key}) nor one it had",
                repr_str(reference)
            )
        }
        Some(_) => format!("no task {} in this project", repr_str(reference)),
        None => format!(
            "no task {}: give its key ({key}-527), or its UUID",
            repr_str(reference)
        ),
    }))
}

#[cfg(test)]
mod tests;
