//! The JSON-in-git store's merge driver (the Python atm's `db/jsongit/merge.py`): merging the task files task by task.
//!
//! Registered as a git merge driver (`tasks.json merge=atm`), so branches that changed tasks.json merge task by task
//! instead of line by line. Design: docs/parallel-agents-design.md, option E and layer L3. Every rule is the Python
//! atm's, in its order, so both merge a file the same way.

use std::path::Path;

use atm_common::home::write_text_atomic;
use atm_common::py::{py_repr, py_str, to_json};
use atm_common::{Error, Kind, Result, Vocabulary};
use serde_json::{Map, Value, json};

use crate::jsongit::files::read_json;

/// Fields that move with the status: they come from the side whose progress wins.
const PROGRESS_FIELDS: [&str; 18] = [
    "status",
    "updated_at",
    "started_at",
    "completed_at",
    "skipped_at",
    "last_failure",
    "agent",
    "started_by",
    "completed_by",
    "updated_by",
    "commit",
    "review_requested_at",
    "review_state",
    "reviewed_at",
    "sprint_review",
    "waiting_on",
    "questions",
    "blocked_at",
];

/// How far along a status's kind is (DM-3.3); a status without a kind ranks like todo.
fn kind_rank(kind: Option<Kind>) -> u8 {
    match kind {
        Some(Kind::Active | Kind::Blocked | Kind::Review) => 1,
        Some(Kind::Done | Kind::Skipped) => 2,
        _ => 0,
    }
}

/// The vocabulary to rank statuses by: this branch's, else the other's, else the defaults. A merge never fails over
/// a vocabulary; the next atm command reports it.
fn merge_vocabulary(ours: &Value, theirs: &Value) -> Vocabulary {
    Vocabulary::of_document(ours)
        .or_else(|_| Vocabulary::of_document(theirs))
        .or_else(|_| Vocabulary::of_meta(&Map::new()))
        .expect("the default vocabularies are valid")
}

/// Three-way merge of one value (`None`: the key is missing): the value, and whether it conflicts (ours is kept).
fn merge_value(
    base: Option<&Value>,
    ours: Option<&Value>,
    theirs: Option<&Value>,
) -> (Option<Value>, bool) {
    if ours == theirs || theirs == base {
        return (ours.cloned(), false);
    }
    if ours == base {
        return (theirs.cloned(), false);
    }
    (ours.cloned(), true)
}

fn text_of(value: Option<&Value>) -> String {
    value.map(py_str).unwrap_or_default()
}

/// Both sides moved a task along: the one further along wins, then the newer update, then ours.
fn progress_winner<'t>(
    ours: &'t Map<String, Value>,
    theirs: &'t Map<String, Value>,
    vocab: &Vocabulary,
) -> &'t Map<String, Value> {
    let rank = |task: &Map<String, Value>| kind_rank(vocab.kind_of(task.get("status")));
    let (rank_o, rank_t) = (rank(ours), rank(theirs));
    if rank_o != rank_t {
        return if rank_o > rank_t { ours } else { theirs };
    }
    if text_of(theirs.get("updated_at")) > text_of(ours.get("updated_at")) {
        theirs
    } else {
        ours
    }
}

fn list(value: Option<&Value>) -> Vec<Value> {
    value.and_then(Value::as_array).cloned().unwrap_or_default()
}

/// Sort by a text key, keeping the order of equal keys (Python's sort is stable too).
fn sorted_by(mut items: Vec<Value>, key: &str) -> Vec<Value> {
    items.sort_by_cached_key(|i| {
        if i.is_object() {
            text_of(i.get(key))
        } else {
            String::new()
        }
    });
    items
}

/// Notes only grow: both sides' notes, each once, oldest first.
fn merge_notes(ours: Option<&Value>, theirs: Option<&Value>) -> Value {
    let ours = list(ours);
    let mut merged = ours.clone();
    merged.extend(list(theirs).into_iter().filter(|n| !ours.contains(n)));
    Value::Array(sorted_by(merged, "at"))
}

/// Past blocks and reviews only grow: both sides', one per start, oldest first.
fn merge_intervals(ours: Option<&Value>, theirs: Option<&Value>) -> Value {
    let ours = list(ours);
    let starts: Vec<Option<Value>> = ours
        .iter()
        .filter(|i| i.is_object())
        .map(|i| i.get("from").cloned())
        .collect();
    let mut merged = ours.clone();
    merged.extend(
        list(theirs)
            .into_iter()
            .filter(|i| i.is_object() && !starts.contains(&i.get("from").cloned())),
    );
    Value::Array(sorted_by(merged, "from"))
}

/// A task's `sprint_history` only grows: one entry per start (`from`); an entry ended on either side (`to`) is kept
/// ended, so a sprint change on one branch closes the entry the other branch still has open.
fn merge_history(ours: Option<&Value>, theirs: Option<&Value>) -> Value {
    let mut by_start: Vec<(Option<Value>, Value)> = Vec::new();
    for entry in list(ours)
        .into_iter()
        .chain(list(theirs))
        .filter(Value::is_object)
    {
        let start = entry.get("from").cloned();
        match by_start.iter_mut().find(|(s, _)| *s == start) {
            None => by_start.push((start, entry)),
            Some((_, known)) => {
                if entry.get("to").is_some() && known.get("to").is_none() {
                    *known = entry;
                }
            }
        }
    }
    Value::Array(sorted_by(
        by_start.into_iter().map(|(_, e)| e).collect(),
        "from",
    ))
}

/// A list merged as a set (`related`, SR-2.8): what either side added stays, what either side removed goes, in ours'
/// order, then theirs' additions. None when nothing is left.
fn merge_set(base: Option<&Value>, ours: Option<&Value>, theirs: Option<&Value>) -> Option<Value> {
    let (base, ours, theirs) = (list(base), list(ours), list(theirs));
    let removed: Vec<&Value> = base
        .iter()
        .filter(|v| !ours.contains(v) || !theirs.contains(v))
        .collect();
    let mut merged: Vec<Value> = ours
        .iter()
        .filter(|v| !removed.contains(v))
        .cloned()
        .collect();
    for v in theirs {
        if !removed.contains(&&v) && !merged.contains(&v) {
            merged.push(v);
        }
    }
    (!merged.is_empty()).then_some(Value::Array(merged))
}

/// The keys of three versions: ours', then theirs' new ones, then the base's the others both dropped.
fn keys_of(
    base: &Map<String, Value>,
    ours: &Map<String, Value>,
    theirs: &Map<String, Value>,
) -> Vec<String> {
    let mut keys: Vec<String> = ours.keys().cloned().collect();
    keys.extend(theirs.keys().filter(|k| !ours.contains_key(*k)).cloned());
    keys.extend(
        base.keys()
            .filter(|k| !ours.contains_key(*k) && !theirs.contains_key(*k))
            .cloned(),
    );
    keys
}

/// Merge one task changed on both sides: the task, and the names of the fields that conflict.
fn merge_task(
    base: &Map<String, Value>,
    ours: &Map<String, Value>,
    theirs: &Map<String, Value>,
    vocab: &Vocabulary,
) -> (Map<String, Value>, Vec<String>) {
    let changed =
        |side: &Map<String, Value>| PROGRESS_FIELDS.iter().any(|f| side.get(*f) != base.get(*f));
    let progress = if !changed(theirs) {
        ours
    } else if !changed(ours) {
        theirs
    } else {
        progress_winner(ours, theirs, vocab)
    };
    let (mut merged, mut conflicts) = (Map::new(), Vec::new());
    for key in keys_of(base, ours, theirs) {
        let (b, o, t) = (base.get(&key), ours.get(&key), theirs.get(&key));
        let value = if PROGRESS_FIELDS.contains(&key.as_str()) {
            progress.get(&key).cloned()
        } else if key == "notes" {
            Some(merge_notes(o, t))
        } else if key == "blocks" || key == "reviews" {
            Some(merge_intervals(o, t))
        } else if key == "sprint_history" {
            Some(merge_history(o, t))
        } else if key == "related" {
            merge_set(b, o, t)
        } else {
            let (mut value, mut conflict) = merge_value(b, o, t);
            if conflict
                && key == "phase"
                && (ours.contains_key("sprint") || theirs.contains_key("sprint"))
            {
                // One side has been migrated to sprints (SR-8.5): its sprint stands for the phase
                (value, conflict) = (None, false);
            }
            if conflict {
                conflicts.push(key.clone());
            }
            value
        };
        if let Some(value) = value {
            merged.insert(key, value);
        }
    }
    (merged, conflicts)
}

/// How far along each state of a meta list's items is: changed on both sides, the further state wins.
fn state_rank(name: &str) -> Option<&'static [(&'static str, u8)]> {
    match name {
        "sprints" => Some(&[("planned", 0), ("active", 1), ("closed", 2)]),
        "releases" => Some(&[("planned", 0), ("released", 1), ("archived", 2)]),
        _ => None,
    }
}

const META_LISTS: [&str; 3] = ["sprints", "releases", "components"];

/// An item's key in a meta list: its ID; for sprints, case-folded (two names that differ only in case are the same
/// sprint, SR-2.3).
fn item_key(name: &str, item: &Map<String, Value>) -> Value {
    match item.get("id") {
        Some(Value::String(id)) if name == "sprints" => json!(id.to_lowercase()),
        other => other.cloned().unwrap_or(Value::Null),
    }
}

/// The items of a meta list by key (the last of a key wins, as a Python dict).
fn keyed(name: &str, items: Option<&Value>) -> Vec<(Value, Map<String, Value>)> {
    let mut out: Vec<(Value, Map<String, Value>)> = Vec::new();
    for item in items
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
    {
        let key = item_key(name, item);
        match out.iter_mut().find(|(k, _)| *k == key) {
            Some(slot) => slot.1.clone_from(item),
            None => out.push((key, item.clone())),
        }
    }
    out
}

fn find<'a>(
    items: &'a [(Value, Map<String, Value>)],
    key: &Value,
) -> Option<&'a Map<String, Value>> {
    items.iter().find(|(k, _)| k == key).map(|(_, i)| i)
}

/// One sprint, release or component changed on both sides, field by field; where both changed a field and both changed
/// the state, the side further along wins it (a sprint closed on one branch stays closed with its report); otherwise
/// ours, as a conflict.
fn merge_item(
    base: &Map<String, Value>,
    ours: &Map<String, Value>,
    theirs: &Map<String, Value>,
    rank: Option<&[(&str, u8)]>,
) -> (Map<String, Value>, Vec<String>) {
    let mut winner = None;
    if let Some(rank) = rank
        && ours.get("state") != base.get("state")
        && theirs.get("state") != base.get("state")
    {
        let of = |item: &Map<String, Value>| {
            let state = item.get("state").and_then(Value::as_str);
            rank.iter()
                .find(|(s, _)| Some(*s) == state)
                .map_or(0, |(_, r)| *r)
        };
        let (o, t) = (of(ours), of(theirs));
        if o != t {
            winner = Some(if o > t { ours } else { theirs });
        }
    }
    let (mut merged, mut conflicts) = (Map::new(), Vec::new());
    for key in keys_of(base, ours, theirs) {
        let (mut value, mut conflict) =
            merge_value(base.get(&key), ours.get(&key), theirs.get(&key));
        if conflict && let Some(winner) = winner {
            (value, conflict) = (winner.get(&key).cloned(), false);
        }
        if conflict {
            conflicts.push(key.clone());
        }
        if let Some(value) = value {
            merged.insert(key, value);
        }
    }
    (merged, conflicts)
}

/// Merge three versions of a meta list of `{id, …}` items: the items, and the conflicts in words.
fn merge_items(
    name: &str,
    base: Option<&Value>,
    ours: Option<&Value>,
    theirs: Option<&Value>,
) -> (Value, Vec<String>) {
    let (b, o, t) = (keyed(name, base), keyed(name, ours), keyed(name, theirs));
    let (mut items, mut conflicts) = (Vec::new(), Vec::new());
    let id = |item: &Map<String, Value>| py_repr(item.get("id").unwrap_or(&Value::Null));
    for (key, item) in &o {
        match (find(&b, key), find(&t, key)) {
            (None, other) => {
                if other.is_some_and(|other| other != item) {
                    conflicts.push(format!(
                        "meta.{name}: {} was added on both branches, differently",
                        id(item)
                    ));
                }
                items.push(Value::Object(item.clone()));
            }
            (Some(base_item), None) => {
                if item != base_item {
                    conflicts.push(format!(
                        "meta.{name}: {} changed on this branch, deleted on the other",
                        id(item)
                    ));
                    items.push(Value::Object(item.clone()));
                }
            }
            (Some(base_item), Some(other)) => {
                let (merged, fields) = merge_item(base_item, item, other, state_rank(name));
                items.push(Value::Object(merged));
                if !fields.is_empty() {
                    conflicts.push(format!(
                        "meta.{name}: both branches changed {} ({})",
                        id(item),
                        fields.join(", ")
                    ));
                }
            }
        }
    }
    for (key, item) in &t {
        let in_base = find(&b, key);
        if find(&o, key).is_none() && in_base.is_none_or(|base_item| item != base_item) {
            if in_base.is_some() {
                conflicts.push(format!(
                    "meta.{name}: {} deleted on this branch, changed on the other",
                    id(item)
                ));
            }
            items.push(Value::Object(item.clone()));
        }
    }
    (Value::Array(items), conflicts)
}

/// `(prefix, digits)` of an ID that ends in a number.
fn split_id(id: &str) -> Option<(&str, &str)> {
    let digits = id.len() - id.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    (digits > 0).then(|| id.split_at(id.len() - digits))
}

/// A new ID for a task theirs added with an ID ours added too: the next number in its pattern, or `<id>-2`, `-3`, ….
fn next_free_id(task_id: &str, used: &[String]) -> String {
    let Some((prefix, digits)) = split_id(task_id) else {
        let mut n = 2;
        while used.contains(&format!("{task_id}-{n}")) {
            n += 1;
        }
        return format!("{task_id}-{n}");
    };
    let highest = used
        .iter()
        .filter_map(|x| x.strip_prefix(prefix))
        .filter(|rest| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()))
        .filter_map(|rest| rest.parse::<u128>().ok())
        .max()
        .unwrap_or(0);
    format!("{prefix}{:0width$}", highest + 1, width = digits.len())
}

fn tasks_of(document: &Value) -> Vec<Map<String, Value>> {
    document
        .get("tasks")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
        .cloned()
        .collect()
}

fn id_of(task: &Map<String, Value>) -> Value {
    task.get("id").cloned().unwrap_or(Value::Null)
}

/// The tasks by ID (the last of an ID wins, as a Python dict).
fn by_id(tasks: &[Map<String, Value>]) -> Vec<(Value, &Map<String, Value>)> {
    let mut out: Vec<(Value, &Map<String, Value>)> = Vec::new();
    for task in tasks {
        let id = id_of(task);
        match out.iter_mut().find(|(k, _)| *k == id) {
            Some(slot) => slot.1 = task,
            None => out.push((id, task)),
        }
    }
    out
}

fn lookup<'a>(
    tasks: &[(Value, &'a Map<String, Value>)],
    id: &Value,
) -> Option<&'a Map<String, Value>> {
    tasks.iter().find(|(k, _)| k == id).map(|(_, t)| *t)
}

/// Merge three versions of tasks.json: the merged document, and the conflicts in words (ours was kept for each).
#[must_use]
#[allow(clippy::too_many_lines)] // reason: one pass per side, then meta, in the order the Python merge_task_files runs
pub fn merge_task_files(base: &Value, ours: &Value, theirs: &Value) -> (Value, Vec<String>) {
    let vocab = merge_vocabulary(ours, theirs);
    let (base_list, our_list, their_list) = (tasks_of(base), tasks_of(ours), tasks_of(theirs));
    let (base_tasks, our_tasks, their_tasks) =
        (by_id(&base_list), by_id(&our_list), by_id(&their_list));
    let mut used: Vec<String> = base_tasks
        .iter()
        .chain(&our_tasks)
        .chain(&their_tasks)
        .map(|(id, _)| py_str(id))
        .collect();
    let (mut conflicts, mut merged_tasks, mut renamed, mut theirs_new) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for task in &our_list {
        let id = id_of(task);
        match (lookup(&base_tasks, &id), lookup(&their_tasks, &id)) {
            // Added here (if added there too, theirs is renumbered below)
            (None, _) => merged_tasks.push(Value::Object(task.clone())),
            (Some(b), None) => {
                if task != b {
                    conflicts.push(format!(
                        "{}: changed on this branch, deleted on the other",
                        py_str(&id)
                    ));
                    merged_tasks.push(Value::Object(task.clone()));
                }
            }
            (Some(b), Some(t)) => {
                let (merged, fields) = merge_task(b, task, t, &vocab);
                merged_tasks.push(Value::Object(merged));
                if !fields.is_empty() {
                    conflicts.push(format!(
                        "{}: both branches changed {}",
                        py_str(&id),
                        fields.join(", ")
                    ));
                }
            }
        }
    }
    for task in &their_list {
        let id = id_of(task);
        if let Some(b) = lookup(&base_tasks, &id) {
            if lookup(&our_tasks, &id).is_none() && task != b {
                conflicts.push(format!(
                    "{}: deleted on this branch, changed on the other",
                    py_str(&id)
                ));
                merged_tasks.push(Value::Object(task.clone()));
            }
            continue;
        }
        let mut task = task.clone();
        if let Some(ours_task) = lookup(&our_tasks, &id) {
            if *ours_task == task {
                continue; // the same task added on both sides
            }
            let new_id = next_free_id(&py_str(&id), &used);
            used.push(new_id.clone());
            renamed.push((id.clone(), json!(new_id)));
            task.insert("id".into(), json!(new_id));
        }
        theirs_new.push(task);
    }
    for mut task in theirs_new {
        if !renamed.is_empty()
            && let Some(Value::Array(deps)) = task
                .get("dependencies")
                .filter(|d| atm_common::py::truthy(d))
        {
            let deps: Vec<Value> = deps
                .iter()
                .map(|d| {
                    renamed
                        .iter()
                        .find(|(old, _)| old == d)
                        .map_or_else(|| d.clone(), |(_, new)| new.clone())
                })
                .collect();
            task.insert("dependencies".into(), Value::Array(deps));
        }
        merged_tasks.push(Value::Object(task));
    }
    let empty = Map::new();
    let (ours_doc, theirs_doc) = (
        ours.as_object().unwrap_or(&empty),
        theirs.as_object().unwrap_or(&empty),
    );
    let mut result = Map::new();
    let mut keys: Vec<String> = ours_doc.keys().cloned().collect();
    keys.extend(
        theirs_doc
            .keys()
            .filter(|k| !ours_doc.contains_key(*k))
            .cloned(),
    );
    for key in keys {
        if key == "tasks" {
            result.insert(key, Value::Array(merged_tasks.clone()));
        } else if let (true, Some(o), Some(t)) = (
            key == "meta",
            ours_doc.get(&key).and_then(Value::as_object),
            theirs_doc.get(&key).and_then(Value::as_object),
        ) {
            let b = base
                .get(&key)
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            let mut meta = Map::new();
            let mut meta_keys: Vec<String> = o.keys().cloned().collect();
            meta_keys.extend(t.keys().filter(|k| !o.contains_key(*k)).cloned());
            for k in meta_keys {
                let value = if META_LISTS.contains(&k.as_str()) {
                    let (value, item_conflicts) = merge_items(&k, b.get(&k), o.get(&k), t.get(&k));
                    conflicts.extend(item_conflicts);
                    Some(value)
                } else {
                    merge_value(b.get(&k), o.get(&k), t.get(&k)).0
                };
                if let Some(value) = value {
                    meta.insert(k, value);
                }
            }
            if meta.contains_key("sprints") {
                meta.shift_remove("phases"); // a project migrated on either side has no phases left (SR-8.5)
            }
            result.insert(key, Value::Object(meta));
        } else {
            let value = ours_doc
                .get(&key)
                .or_else(|| theirs_doc.get(&key))
                .cloned()
                .unwrap_or(Value::Null);
            result.insert(key, value);
        }
    }
    (Value::Object(result), conflicts)
}

/// What git's merge driver does (`atm db merge-driver %O %A %B`): merge the three versions task by task and write the
/// result over `ours`. Returns the conflicts (this branch's version was kept for each).
///
/// # Errors
/// A version that isn't task data (resolve the conflict by hand), or one that can't be written.
pub fn merge_files(base: &Path, ours: &Path, theirs: &Path) -> Result<Vec<String>> {
    let mut versions: Vec<Option<Value>> =
        [base, ours, theirs].iter().map(|p| read_json(p)).collect();
    if versions[0].is_none() && std::fs::metadata(base).is_ok_and(|m| m.len() == 0) {
        versions[0] = Some(json!({"tasks": []})); // the file was added on both branches
    }
    let usable = |v: &Option<Value>| {
        v.as_ref()
            .is_some_and(|v| v.is_object() && v.get("tasks").is_none_or(Value::is_array))
    };
    if !versions.iter().all(usable) {
        return Err(Error::failed(
            "tasks.json merge: one side is not valid JSON; resolve the conflict by hand",
        ));
    }
    let [Some(b), Some(o), Some(t)] = [&versions[0], &versions[1], &versions[2]] else {
        return Err(Error::failed("tasks.json merge: one side is missing"));
    };
    let (merged, conflicts) = merge_task_files(b, o, t);
    write_text_atomic(ours, &(to_json(&merged) + "\n"))
        .map_err(|e| Error::failed(format!("{}: {e}", ours.display())))?;
    Ok(conflicts)
}

#[cfg(test)]
mod tests;
