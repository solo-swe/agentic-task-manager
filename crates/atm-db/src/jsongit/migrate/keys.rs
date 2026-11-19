//! The first task-keys migration (ADR-007 §6): a file from before task keys (format 1, tasks known by IDs like
//! `T-527`) becomes format 2, tasks stored by UUID and shown by the project's key and a number (`ATM-527`).
//!
//! **The same in every clone.** Each clone and worktree migrates its own copy of the file, so a task's UUID can't be
//! random: two branches would give one task two UUIDs, and the merge driver, which matches tasks by UUID, would see
//! two tasks. A migrated task's UUID is a version-8 UUID made from the project's `meta.id` and the task's old ID
//! (SHA-256), and its number comes from the old ID: the same file gives the same result everywhere. Tasks added later
//! get random version-7 UUIDs (they exist on one branch first).

use std::collections::{HashMap, HashSet};

use atm_common::ids::{KEYS_FORMAT, valid_project_key};
use atm_common::sha256::sha256;
use serde_json::{Map, Value, json};

/// What the migration did, for `atm db migrate` to say.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeysReport {
    /// The project's key (`meta.key`) the tasks' keys start with.
    pub key: String,
    /// The old IDs' prefix kept in `meta.key_history` (`T`), if they had one in the `PREFIX-NUMBER` form.
    pub old_prefix: Option<String>,
    /// How many working tasks, and archived ones, it gave a UUID and a number.
    pub tasks: usize,
    /// How many archived ones.
    pub archived: usize,
    /// Old IDs that didn't fit the file's prefix (or took a number another had), and the number each got; they keep
    /// their old ID as an alias.
    pub renumbered: Vec<(String, u64)>,
}

/// The UUID a task with this old ID gets in this project: the same in every clone (see the module's doc).
#[must_use]
pub fn migrated_uuid(project: &str, old_id: &str) -> String {
    let digest = sha256(format!("atm task keys migration\0{project}\0{old_id}").as_bytes());
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    uuid::Builder::from_custom_bytes(bytes)
        .into_uuid()
        .hyphenated()
        .to_string()
}

/// `(prefix, number)` of an ID that ends in digits (`T-007`: `("T-", 7)`).
fn split(id: &str) -> Option<(&str, u64)> {
    let digits = id.len() - id.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    if digits == 0 {
        return None;
    }
    let (prefix, number) = id.split_at(id.len() - digits);
    Some((prefix, number.parse().ok()?))
}

/// The project's key: the initials of its name when they make 3 to 6 letters and digits (Agentic Task Manager:
/// `ATM`), else the old prefix when it is one, else the name's first letters and digits, else `TASK`.
#[must_use]
pub fn suggested_key(name: &str, old_prefix: Option<&str>) -> String {
    let initials: String = name
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter_map(|w| w.chars().next())
        .collect::<String>()
        .to_ascii_uppercase();
    if valid_project_key(&initials) {
        return initials;
    }
    if let Some(prefix) = old_prefix.filter(|p| valid_project_key(p)) {
        return prefix.to_ascii_uppercase();
    }
    let letters: String = name
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(6)
        .collect::<String>()
        .to_ascii_uppercase();
    if valid_project_key(&letters) {
        letters
    } else {
        "TASK".into()
    }
}

fn tasks_of(document: &mut Value) -> Vec<&mut Map<String, Value>> {
    document
        .get_mut("tasks")
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
        .filter_map(Value::as_object_mut)
        .collect()
}

fn old_ids(document: &Value) -> Vec<String> {
    document["tasks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|t| t.get("id").and_then(Value::as_str))
        .map(String::from)
        .collect()
}

/// The prefix most of the IDs share (`T-`), the later one on a tie, as atm gave IDs from the last task's.
fn common_prefix(ids: &[String]) -> Option<String> {
    let mut counts: HashMap<&str, (usize, usize)> = HashMap::new();
    for (i, id) in ids.iter().enumerate() {
        if let Some((prefix, _)) = split(id) {
            let entry = counts.entry(prefix).or_insert((0, 0));
            entry.0 += 1;
            entry.1 = i;
        }
    }
    counts
        .into_iter()
        .max_by_key(|(_, (count, last))| (*count, *last))
        .map(|(prefix, _)| prefix.to_string())
}

/// A list of task references (`dependencies`, `related`, a sprint report's lists) with old IDs replaced by UUIDs; an
/// ID no task has is kept as it is (`atm doctor` reports it).
fn rewrite_list(value: &mut Value, map: &HashMap<String, String>) {
    if let Some(items) = value.as_array_mut() {
        for item in items {
            if let Some(uuid) = item.as_str().and_then(|s| map.get(s)) {
                *item = json!(uuid);
            }
        }
    }
}

/// The task with its new fields first (`id`, `number`, `aliases`), then the rest as it was; no `key_final` (ADR-009:
/// keys are plain from creation).
fn rekeyed(
    task: &Map<String, Value>,
    uuid: &str,
    number: u64,
    alias: Option<&str>,
) -> Map<String, Value> {
    let mut out = Map::new();
    out.insert("id".into(), json!(uuid));
    out.insert("number".into(), json!(number));
    let mut aliases: Vec<Value> = task
        .get("aliases")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if let Some(alias) = alias
        && !aliases.iter().any(|a| a == alias)
    {
        aliases.push(json!(alias));
    }
    if !aliases.is_empty() {
        out.insert("aliases".into(), Value::Array(aliases));
    }
    for (k, v) in task {
        if !matches!(k.as_str(), "id" | "number" | "key_final" | "aliases") {
            out.insert(k.clone(), v.clone());
        }
    }
    out
}

/// [`assign_numbers`]' result: `(number, keeps its old ID as an alias)` per task, the renumbered IDs, the next number.
type Numbering = (Vec<(u64, bool)>, Vec<(String, u64)>, u64);

/// The tasks' numbers, in `all`'s order, with whether each keeps its old ID as an alias: the ID's own digits where it
/// has the file's `prefix` and the number is free, else the next number atm never gave (above the highest and
/// `next_id`: a task since deleted may have had it, and an old commit may name it). Also the renumbered IDs and the
/// next free number.
fn assign_numbers(
    all: &[String],
    prefix: Option<&str>,
    has_history: bool,
    given: u64,
) -> Numbering {
    let mut own: Vec<Option<u64>> = Vec::with_capacity(all.len());
    let mut taken: HashSet<u64> = HashSet::new();
    for id in all {
        let n = split(id)
            .filter(|(p, n)| Some(*p) == prefix && *n >= 1 && !taken.contains(n))
            .map(|(_, n)| n);
        if let Some(n) = n {
            taken.insert(n);
        }
        own.push(n);
    }
    let mut next = (taken.iter().max().copied().unwrap_or(0) + 1).max(given);
    let mut renumbered = Vec::new();
    let numbers = all
        .iter()
        .zip(own)
        .map(|(id, own)| {
            own.map_or_else(
                || {
                    let n = next;
                    next += 1;
                    renumbered.push((id.clone(), n));
                    (n, true)
                },
                // T-527 finds ATM-527 through the earlier prefix; without one, the old ID must stay an alias
                |n| (n, !has_history),
            )
        })
        .collect();
    (numbers, renumbered, next)
}

/// The sprints' reports' task lists, with old IDs as UUIDs.
fn rewrite_reports(document: &mut Value, map: &HashMap<String, String>) {
    if let Some(sprints) = document
        .get_mut("meta")
        .and_then(|m| m.get_mut("sprints"))
        .and_then(Value::as_array_mut)
    {
        for report in sprints.iter_mut().filter_map(|s| s.get_mut("report")) {
            for list in ["done", "carried"] {
                if let Some(v) = report.get_mut(list) {
                    rewrite_list(v, map);
                }
            }
        }
    }
}

/// The claims store, keyed by task and naming it in each entry, with old IDs as UUIDs (`hints` aren't tasks').
fn rewrite_claims(claims: &mut Map<String, Value>, map: &HashMap<String, String>) {
    for section in ["claims", "completed", "waiting", "resolved"] {
        let Some(entries) = claims.get_mut(section).and_then(Value::as_object_mut) else {
            continue;
        };
        let old: Vec<(String, Value)> = std::mem::take(entries).into_iter().collect();
        for (k, mut v) in old {
            if let Some(uuid) = v
                .get("task")
                .and_then(Value::as_str)
                .and_then(|t| map.get(t))
            {
                v["task"] = json!(uuid);
            }
            let k = map.get(&k).cloned().unwrap_or(k);
            entries.insert(k, v);
        }
    }
}

/// `meta` of the keys format: `format`, `key`, `key_history` with the old prefix, `next_number` (`next_id` goes).
fn keyed_meta(meta: &mut Map<String, Value>, key: &str, old_prefix: Option<&str>, next: u64) {
    meta.shift_remove("next_id");
    meta.insert("format".into(), json!(KEYS_FORMAT));
    meta.insert("key".into(), json!(key));
    let mut history: Vec<Value> = meta
        .get("key_history")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if let Some(p) = old_prefix
        && !p.eq_ignore_ascii_case(key)
        && !history
            .iter()
            .any(|h| h.as_str().is_some_and(|h| h.eq_ignore_ascii_case(p)))
    {
        history.push(json!(p));
    }
    if history.is_empty() {
        meta.shift_remove("key_history");
    } else {
        meta.insert("key_history".into(), Value::Array(history));
    }
    meta.insert("next_number".into(), json!(next));
}

/// Every task of `documents` (the task file, then the archive) rekeyed: its UUID, its number (`numbers`, in order),
/// final, its old ID as an alias where `numbers` says, and its references as UUIDs. A repeated ID (a hand-edited file)
/// can't share a UUID: the later ones get their own. How many tasks.
fn rekey_tasks(
    documents: [&mut Value; 2],
    numbers: &[(u64, bool)],
    map: &HashMap<String, String>,
    project: &str,
) -> usize {
    let mut used: HashSet<String> = HashSet::new();
    let mut i = 0;
    for doc in documents {
        for task in tasks_of(doc) {
            let id = task
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let (number, alias) = numbers[i];
            i += 1;
            let mut uuid = map[&id].clone();
            if !used.insert(uuid.clone()) {
                uuid = migrated_uuid(project, &format!("{id}\0{number}"));
                used.insert(uuid.clone());
            }
            let mut new = rekeyed(task, &uuid, number, alias.then_some(id.as_str()));
            for field in ["dependencies", "related"] {
                if let Some(v) = new.get_mut(field) {
                    rewrite_list(v, map);
                }
            }
            if let Some(uuid) = new
                .get("duplicate_of")
                .and_then(Value::as_str)
                .and_then(|d| map.get(d))
            {
                new.insert("duplicate_of".into(), json!(uuid));
            }
            *task = new;
        }
    }
    i
}

/// Turn a format-1 task file, its archive and the claims store into format 2 (ADR-007 §6): every task (working, then
/// archived) gets its UUID and a number (its ID's digits when the ID has the file's prefix and the number is free,
/// else the next free number, with its old ID kept as an alias), every task is final (they are on the base branch, or
/// on branches made from it, which migrate to the same), references become UUIDs (`dependencies`, `related`,
/// `duplicate_of`, the sprints' reports, the claims' keys and their `task`), and `meta` gets `format`, `key` (kept if
/// the file names a valid one), `key_history` and `next_number` (`next_id` goes). None, changing nothing, for a file
/// that isn't format 1.
pub fn to_keys_format(
    document: &mut Value,
    archive: &mut Value,
    claims: &mut Map<String, Value>,
) -> Option<KeysReport> {
    if atm_common::ids::file_format(document) != Some(atm_common::ids::LEGACY_FORMAT) {
        return None;
    }
    let meta = document
        .get("meta")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let project = meta
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let working_ids = old_ids(document);
    let archived_ids = old_ids(archive);
    let all: Vec<String> = working_ids.iter().chain(&archived_ids).cloned().collect();
    let prefix = common_prefix(&all);
    // `T-` keeps working as an earlier key prefix (T-527 finds ATM-527); a prefix without the dash can't
    let old_prefix = prefix
        .as_deref()
        .and_then(|p| p.strip_suffix('-'))
        .filter(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_alphanumeric()))
        .map(String::from);
    let key = meta
        .get("key")
        .and_then(Value::as_str)
        .filter(|k| valid_project_key(k))
        .map_or_else(
            || {
                suggested_key(
                    meta.get("project")
                        .and_then(Value::as_str)
                        .unwrap_or_default(),
                    old_prefix.as_deref(),
                )
            },
            String::from,
        );
    let (numbers, renumbered, next) = assign_numbers(
        &all,
        prefix.as_deref(),
        old_prefix.is_some(),
        meta.get("next_id").and_then(Value::as_u64).unwrap_or(0),
    );
    let map: HashMap<String, String> = all
        .iter()
        .map(|id| (id.clone(), migrated_uuid(&project, id)))
        .collect();
    let total = rekey_tasks([&mut *document, &mut *archive], &numbers, &map, &project);
    rewrite_reports(document, &map);
    rewrite_claims(claims, &map);
    if !document.get("meta").is_some_and(Value::is_object) {
        document["meta"] = json!({});
    }
    if let Some(meta) = document["meta"].as_object_mut() {
        keyed_meta(meta, &key, old_prefix.as_deref(), next);
    }
    if archive
        .get("tasks")
        .and_then(Value::as_array)
        .is_some_and(|t| !t.is_empty())
    {
        if !archive.get("meta").is_some_and(Value::is_object) {
            archive["meta"] = json!({});
        }
        archive["meta"]["format"] = json!(KEYS_FORMAT);
    }
    Some(KeysReport {
        key,
        old_prefix,
        tasks: total,
        archived: archived_ids.len(),
        renumbered,
    })
}

#[cfg(test)]
mod tests;
