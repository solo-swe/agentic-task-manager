//! Adding tasks (the Python atm's `lib/edit.py`: `atm task add`, one task or many). IDs come from `meta.next_id`
//! inside the store's change, so two agents adding at once never get the same one (docs/v4/data-model.md DM-2.3).
//! Status and timestamps aren't given here: atm sets them.

use std::collections::{HashMap, HashSet};

use atm_common::components::{component_ids, suggest_components};
use atm_common::edits::{set_sprint, status_for};
use atm_common::py::{py_repr, py_str, truthy};
use atm_common::time::now_iso;
use atm_common::vocabulary::REVIEW_MODES;
use atm_common::{Error, Kind, Result, Task, Vocabulary};
use atm_db::working::Working;
use serde_json::{Map, Value, json};

use crate::project::Project;

/// A project's ID pattern when it has no numbered task yet: `T-001`.
const DEFAULT_ID: (&str, usize) = ("T-", 3);

/// `(prefix, digits)` of an ID that ends in a number (`T-001`: `("T-", "001")`).
fn split_id(id: &str) -> Option<(&str, &str)> {
    let digits = id.len() - id.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    (digits > 0).then(|| id.split_at(id.len() - digits))
}

pub(crate) fn task_ids(document: &Value) -> impl Iterator<Item = &str> {
    document["tasks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|t| t.get("id").and_then(Value::as_str))
}

/// The project's ID pattern: the prefix and digit count of its last numbered task ID (`T-001`: `("T-", 3)`).
fn id_format(document: &Value) -> (String, usize) {
    let ids: Vec<&str> = task_ids(document).collect();
    ids.iter()
        .rev()
        .find_map(|id| split_id(id))
        .map_or((DEFAULT_ID.0.to_string(), DEFAULT_ID.1), |(p, d)| {
            (p.to_string(), d.len())
        })
}

/// `meta.next_id`, but never behind the highest ID in the file (tasks added by hand, or by an older atm).
fn next_number(document: &Value, prefix: &str) -> i128 {
    let floor = task_ids(document)
        .filter_map(split_id)
        .filter(|(p, _)| *p == prefix)
        .filter_map(|(_, d)| d.parse::<i128>().ok())
        .max()
        .unwrap_or(0)
        + 1;
    match document.get("meta").and_then(|m| m.get("next_id")) {
        Some(Value::Number(n)) if n.is_i64() || n.is_u64() => n
            .as_i64()
            .map_or(floor, |stored| i128::from(stored).max(floor)),
        _ => floor,
    }
}

/// The number the next new task gets, in the project's pattern (`meta.next_id`, never behind the file's IDs).
pub(crate) fn next_free_number(document: &Value) -> i128 {
    let (prefix, _) = id_format(document);
    next_number(document, &prefix)
}

/// `count` new IDs in the project's pattern; advances `meta.next_id`.
fn allocate_ids(document: &mut Value, count: usize) -> Vec<String> {
    let (prefix, width) = id_format(document);
    let first = next_number(document, &prefix);
    let ids = (0..count)
        .map(|n| format!("{prefix}{:0width$}", first + n as i128))
        .collect();
    if !document.get("meta").is_some_and(Value::is_object) {
        document["meta"] = json!({});
    }
    document["meta"]["next_id"] = json!(first + count as i128);
    ids
}

/// A field read as Python's `list(value or [])` reads it: a list's items, a text's characters, an object's keys.
fn py_list(value: Option<&Value>, key: &str) -> Result<Vec<Value>> {
    match value {
        None => Ok(Vec::new()),
        Some(v) if !truthy(v) => Ok(Vec::new()),
        Some(Value::Array(items)) => Ok(items.clone()),
        Some(Value::String(text)) => Ok(text.chars().map(|c| json!(c.to_string())).collect()),
        Some(Value::Object(map)) => Ok(map.keys().map(|k| json!(k)).collect()),
        Some(other) => Err(Error::Invalid(format!(
            "{key} {}: expected a list",
            py_repr(other)
        ))),
    }
}

fn check_labels(labels: &[Value]) -> Result<()> {
    let valid = |label: &str| {
        (1..=32).contains(&label.len())
            && label
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
    };
    for label in labels {
        if !label.as_str().is_some_and(valid) {
            return Err(Error::Invalid(format!(
                "label {}: use lowercase letters, digits, - and _ (at most 32 characters)",
                py_repr(label)
            )));
        }
    }
    Ok(())
}

/// The ID of the sprint called `name`, which a task can be put in: it must exist and not be closed (SR-2.5).
pub(crate) fn open_sprint(vocab: &Vocabulary, name: &Value) -> Result<String> {
    let state = |s: &Map<String, Value>| s.get("state").and_then(Value::as_str) == Some("closed");
    let Some(sprint) = vocab.find_sprint(Some(name)) else {
        let open: Vec<String> = vocab
            .sprints
            .iter()
            .filter(|s| !state(s))
            .map(|s| py_str(&s["id"]))
            .collect();
        let known = if open.is_empty() {
            "none yet: atm sprint add".to_string()
        } else {
            open.join(", ")
        };
        return Err(Error::Invalid(format!(
            "no sprint {}; the open sprints: {known}",
            py_repr(name)
        )));
    };
    let id = sprint.get("id").cloned().unwrap_or(Value::Null);
    if state(sprint) {
        return Err(Error::Invalid(format!(
            "the sprint {} is closed; a task can only go into a planned or active sprint",
            py_repr(&id)
        )));
    }
    Ok(py_str(&id))
}

/// `with`: who the work is done together with; `user` for now (SR-6.5).
fn check_with(value: &Value) -> Result<()> {
    if value == "user" {
        return Ok(());
    }
    Err(Error::Invalid(format!(
        "with {}: use user (work done together with the user)",
        py_repr(value)
    )))
}

fn check_review(value: &Value) -> Result<()> {
    if value.as_str().is_some_and(|v| REVIEW_MODES.contains(&v)) {
        return Ok(());
    }
    Err(Error::Invalid(format!(
        "review {}: use one of {}",
        py_repr(value),
        REVIEW_MODES.join(", ")
    )))
}

/// A complete new task from the given fields, checked against the vocabularies; its ID comes later.
fn new_task(vocab: &Vocabulary, fields: &Task, actor: Option<&str>, stamp: &str) -> Result<Task> {
    // Tasks go into sprints (or the backlog): phases became sprints (SR-5.5, SR-8.3); a "phase" given is ignored
    for key in ["name", "type"] {
        if fields.get(key).is_none_or(|v| v.is_null() || v == "") {
            return Err(Error::Invalid(format!("a new task needs a {key}")));
        }
    }
    let type_ = &fields["type"];
    if !vocab.types.iter().any(|t| type_ == t.id.as_str()) {
        let known: Vec<&str> = vocab.types.iter().map(|t| t.id.as_str()).collect();
        return Err(Error::Invalid(format!(
            "unknown type {}; use one of {}",
            py_repr(type_),
            known.join(", ")
        )));
    }
    let priority = fields.get("priority").filter(|p| !p.is_null());
    if let Some(priority) = priority
        && !vocab.priorities.iter().any(|p| priority == p.id.as_str())
    {
        let priorities: Vec<&str> = vocab.priorities.iter().map(|p| p.id.as_str()).collect();
        return Err(Error::Invalid(format!(
            "unknown priority {}; use one of {}",
            py_repr(priority),
            priorities.join(", ")
        )));
    }
    let mut task = Map::new();
    task.insert("id".into(), Value::Null);
    task.insert("name".into(), json!(py_str(&fields["name"])));
    task.insert("type".into(), type_.clone());
    if let Some(priority) = priority {
        task.insert("priority".into(), priority.clone());
    }
    let labels = py_list(fields.get("labels"), "labels")?;
    check_labels(&labels)?;
    if !labels.is_empty() {
        task.insert("labels".into(), Value::Array(labels));
    }
    let description = fields
        .get("description")
        .filter(|d| truthy(d))
        .map(py_str)
        .unwrap_or_default();
    task.insert("description".into(), json!(description));
    for key in ["requirements", "use_cases"] {
        task.insert(key.into(), Value::Array(py_list(fields.get(key), key)?));
    }
    task.insert("status".into(), json!(status_for(vocab, Kind::Todo, None)?));
    for key in ["dependencies", "files"] {
        task.insert(key.into(), Value::Array(py_list(fields.get(key), key)?));
    }
    task.insert("created_at".into(), json!(stamp));
    task.insert("updated_at".into(), json!(stamp));
    if let Some(actor) = actor.filter(|a| !a.is_empty()) {
        task.insert("created_by".into(), json!(actor));
        task.insert("updated_by".into(), json!(actor));
    }
    if let Some(sprint) = fields.get("sprint").filter(|s| truthy(s)) {
        set_sprint(&mut task, Some(&open_sprint(vocab, sprint)?), stamp);
    }
    if let Some(release) = fields.get("release").filter(|r| truthy(r)) {
        vocab.known_release(&py_str(release))?;
        task.insert("release".into(), release.clone());
    }
    // "auto" (--components auto): the components its files are in (SR-2.7)
    let requested = fields.get("components");
    let components: Vec<Value> = if requested.is_some_and(|r| r == "auto") {
        let files = task["files"].as_array().cloned().unwrap_or_default();
        suggest_components(vocab, &files)
            .into_iter()
            .map(Value::String)
            .collect()
    } else {
        component_ids(vocab, &py_list(requested, "components")?)?
    };
    if !components.is_empty() {
        task.insert("components".into(), Value::Array(components));
    }
    if let Some(review) = fields.get("review").filter(|r| truthy(r)) {
        check_review(review)?;
        task.insert("review".into(), review.clone());
    }
    if let Some(with) = fields.get("with").filter(|w| truthy(w)) {
        check_with(with)?;
        task.insert("with".into(), with.clone());
    }
    if let Some(related) = fields.get("related").filter(|r| truthy(r)) {
        // Checked once the new IDs are known (add_tasks)
        task.insert(
            "related".into(),
            Value::Array(py_list(Some(related), "related")?),
        );
    }
    Ok(task)
}

/// A dependency cycle among `tasks` (as a list of IDs), if there is one. Depth-first with an explicit stack, not
/// recursion: a chain of thousands of tasks (or a hostile file) can't overflow it (docs/security-review.md F12).
pub(crate) fn find_cycle(tasks: &[&Task]) -> Option<Vec<String>> {
    let mut order: Vec<String> = Vec::new();
    let mut deps: HashMap<String, Vec<String>> = HashMap::new();
    for t in tasks {
        let Some(id) = t.get("id").and_then(Value::as_str) else {
            continue;
        };
        let list: Vec<String> = t
            .get("dependencies")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(String::from)
            .collect();
        if deps.insert(id.to_string(), list).is_none() {
            order.push(id.to_string());
        }
    }
    let mut done: HashSet<&str> = HashSet::new();
    for start in &order {
        if done.contains(start.as_str()) {
            continue;
        }
        // `path` is the current chain, `on_path` its positions, `stack` where each one's dependencies are at
        let mut path: Vec<&str> = vec![start];
        let mut on_path: HashMap<&str, usize> = HashMap::from([(start.as_str(), 0)]);
        let mut stack: Vec<(&str, usize)> = vec![(start, 0)];
        while let Some((node, next)) = stack.last_mut() {
            let Some(dep) = deps[*node].get(*next).map(String::as_str) else {
                let node = path.pop().unwrap_or_default();
                on_path.remove(node);
                done.insert(node);
                stack.pop();
                continue;
            };
            *next += 1;
            if let Some(&at) = on_path.get(dep) {
                let mut cycle: Vec<String> = path[at..].iter().map(|s| (*s).to_string()).collect();
                cycle.push(dep.to_string());
                return Some(cycle);
            }
            if let Some((dep, _)) = deps.get_key_value(dep)
                && !done.contains(dep.as_str())
            {
                on_path.insert(dep, path.len());
                path.push(dep);
                stack.push((dep, 0));
            }
        }
    }
    None
}

/// The archived tasks' IDs, read from the change's own archive the first time something asks.
#[derive(Default)]
struct Archived(Option<HashSet<String>>);

impl Archived {
    fn has(&mut self, work: &mut Working, id: &Value) -> Result<bool> {
        if self.0.is_none() {
            let ids = task_ids(work.archive()?).map(String::from).collect();
            self.0 = Some(ids);
        }
        Ok(id
            .as_str()
            .is_some_and(|id| self.0.as_ref().is_some_and(|ids| ids.contains(id))))
    }
}

fn join(values: &[Value]) -> String {
    values.iter().map(py_str).collect::<Vec<_>>().join(", ")
}

/// Add `entries` (task fields; an `id` there is a placeholder the other entries' dependencies and related may use) to
/// the change's document, all or none. Returns the new tasks.
fn add_tasks(work: &mut Working, entries: &[Task], actor: Option<&str>) -> Result<Vec<Task>> {
    let vocab = Vocabulary::of_document(&work.document)?;
    let stamp = now_iso();
    let mut tasks = entries
        .iter()
        .map(|e| new_task(&vocab, e, actor, &stamp))
        .collect::<Result<Vec<Task>>>()?;
    let placeholders: Vec<&Value> = entries
        .iter()
        .map(|e| e.get("id").unwrap_or(&Value::Null))
        .collect();
    for p in placeholders.iter().filter(|p| !p.is_null()) {
        if placeholders.iter().filter(|q| q == &p).count() > 1 {
            return Err(Error::Invalid(format!(
                "placeholder ID {} is used twice",
                py_repr(p)
            )));
        }
    }
    let ids = allocate_ids(&mut work.document, tasks.len());
    let rename = |value: &Value| -> Value {
        placeholders
            .iter()
            .zip(&ids)
            .find(|(p, _)| !p.is_null() && **p == value)
            .map_or_else(|| value.clone(), |(_, real)| json!(real))
    };
    let existing: HashSet<String> = task_ids(&work.document).map(String::from).collect();
    let known = |id: &Value| {
        id.as_str()
            .is_some_and(|id| existing.contains(id) || ids.iter().any(|n| n == id))
    };
    let mut archived = Archived::default();
    for (task, real) in tasks.iter_mut().zip(&ids) {
        task.insert("id".into(), json!(real));
        let deps: Vec<Value> = task["dependencies"]
            .as_array()
            .into_iter()
            .flatten()
            .map(&rename)
            .collect();
        let mut missing = Vec::new();
        for d in &deps {
            // An archived task is done: depending on it is fine, as atm doctor says (T-356)
            if !known(d) && !archived.has(work, d)? {
                missing.push(d.clone());
            }
        }
        task.insert("dependencies".into(), Value::Array(deps));
        if !missing.is_empty() {
            return Err(Error::Invalid(format!(
                "{real} ({}) depends on {}, which doesn't exist",
                py_str(&task["name"]),
                join(&missing)
            )));
        }
        if let Some(Value::Array(related)) = task.get("related") {
            let related: Vec<Value> = related.iter().map(&rename).collect();
            for other in &related {
                if other == real.as_str() {
                    return Err(Error::Invalid(format!("{real} can't be related to itself")));
                }
                if !known(other) && !archived.has(work, other)? {
                    return Err(Error::Invalid(format!(
                        "{real} can't be related to {}, which doesn't exist",
                        py_str(other)
                    )));
                }
            }
            task.insert("related".into(), Value::Array(related));
        }
    }
    let current: Vec<Task> = work.document["tasks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
        .cloned()
        .collect();
    let all: Vec<&Task> = current.iter().chain(tasks.iter()).collect();
    if let Some(cycle) = find_cycle(&all) {
        return Err(Error::Invalid(format!(
            "these dependencies would make a cycle: {}",
            cycle.join(" -> ")
        )));
    }
    work.tasks()
        .extend(tasks.iter().cloned().map(Value::Object));
    Ok(tasks)
}

/// What `atm task add` added: the new tasks, and the vocabularies (for the components their files suggest).
#[derive(Debug, Clone)]
pub struct Added {
    /// The new tasks, with their IDs.
    pub tasks: Vec<Task>,
    /// The project's vocabularies.
    pub vocab: Vocabulary,
}

/// `atm task add`: the tasks `entries` describe (task fields; an `id` there is a placeholder the others'
/// dependencies may use), with the next IDs, all or none, as the project's actor.
///
/// # Errors
/// The first entry that can't be added (an unknown type, priority or sprint, a bad label, a dependency that doesn't
/// exist, a cycle): nothing is written then.
pub fn add_new_tasks(project: &mut Project, entries: &[Task]) -> Result<Added> {
    let actor = project.actor().map(String::from);
    let mut added: Option<Added> = None;
    project.store_mut().change(&mut |work: &mut Working| {
        let tasks = add_tasks(work, entries, actor.as_deref())?;
        let vocab = Vocabulary::of_document(&work.document)?;
        added = Some(Added { tasks, vocab });
        Ok(())
    })?;
    added.ok_or_else(|| Error::failed("no tasks added"))
}

/// What `atm task edit` changes: what isn't given (`None`, or an empty list) changes nothing; an empty `release`,
/// `review` or `with_` removes it; `backlog` takes the task out of its sprint; `components_auto` adds the components its
/// files are in.
#[derive(Debug, Clone, Default)]
#[allow(clippy::struct_excessive_bools)] // reason: each is one of the command's flags, as the Python atm has them
pub struct TaskEdit {
    /// A new name.
    pub name: Option<String>,
    /// A new description.
    pub description: Option<String>,
    /// The sprint to move it to.
    pub sprint: Option<String>,
    /// Out of its sprint, into the backlog.
    pub backlog: bool,
    /// A new type.
    pub type_: Option<String>,
    /// A new priority.
    pub priority: Option<String>,
    /// A new release (empty: none).
    pub release: Option<String>,
    /// A new review mode (empty: none).
    pub review: Option<String>,
    /// Who it is done together with (empty: no one).
    pub with_: Option<String>,
    /// Add the components its files are in.
    pub components_auto: bool,
    /// Files to add.
    pub add_files: Vec<String>,
    /// Files to remove.
    pub remove_files: Vec<String>,
    /// Dependencies to add.
    pub add_dependencies: Vec<String>,
    /// Dependencies to remove.
    pub remove_dependencies: Vec<String>,
    /// Labels to add.
    pub add_labels: Vec<String>,
    /// Labels to remove.
    pub remove_labels: Vec<String>,
    /// Components to add.
    pub add_components: Vec<String>,
    /// Components to remove.
    pub remove_components: Vec<String>,
    /// Related tasks to add.
    pub add_related: Vec<String>,
    /// Related tasks to remove.
    pub remove_related: Vec<String>,
    /// Requirements to add.
    pub add_requirements: Vec<String>,
    /// Requirements to remove.
    pub remove_requirements: Vec<String>,
    /// Use cases to add.
    pub add_use_cases: Vec<String>,
    /// Use cases to remove.
    pub remove_use_cases: Vec<String>,
}

/// What `atm task edit` did: the task after it, the fields that changed (in the order they were changed), and the
/// vocabularies (for the components new files suggest).
#[derive(Debug, Clone)]
pub struct Edited {
    /// The task after the edit.
    pub task: Task,
    /// The names of the fields that changed; nothing is written when none did.
    pub changed: Vec<String>,
    /// The project's vocabularies.
    pub vocab: Vocabulary,
}

fn strings(values: &[String]) -> Vec<Value> {
    values.iter().map(|v| json!(v)).collect()
}

/// Apply the edit to `task` (a copy: the caller writes it back only when this succeeds). Returns the fields changed.
#[allow(clippy::too_many_lines)] // reason: one step per option, in the order the Python edit_task applies them
fn edit_task(
    work: &mut Working,
    task: &mut Task,
    edit: &TaskEdit,
    archived: &mut Archived,
) -> Result<Vec<String>> {
    let vocab = Vocabulary::of_document(&work.document)?;
    let mut changed: Vec<String> = Vec::new();
    let task_id = task.get("id").map(py_str).unwrap_or_default();
    let set_field = |task: &mut Task, changed: &mut Vec<String>, key: &str, value: Value| {
        if task.get(key) != Some(&value) {
            task.insert(key.into(), value);
            changed.push(key.into());
        }
    };
    let edit_list = |task: &mut Task,
                     changed: &mut Vec<String>,
                     key: &str,
                     add: &[Value],
                     remove: &[Value]|
     -> Result<()> {
        let current = py_list(task.get(key), key)?;
        let mut updated: Vec<Value> = current
            .iter()
            .filter(|v| !remove.contains(v))
            .cloned()
            .collect();
        updated.extend(add.iter().filter(|v| !current.contains(v)).cloned());
        if updated != current {
            task.insert(key.into(), Value::Array(updated));
            changed.push(key.into());
        }
        Ok(())
    };

    if let Some(name) = &edit.name {
        set_field(task, &mut changed, "name", json!(name));
    }
    if let Some(description) = &edit.description {
        set_field(task, &mut changed, "description", json!(description));
    }
    if edit.sprint.is_some() || edit.backlog {
        let sprint = match (&edit.sprint, edit.backlog) {
            (_, true) | (None, _) => None,
            (Some(name), false) => Some(open_sprint(&vocab, &json!(name))?),
        };
        if set_sprint(task, sprint.as_deref(), &now_iso()) {
            changed.push("sprint".into());
        }
    }
    if let Some(type_) = &edit.type_ {
        if !vocab.types.iter().any(|t| &t.id == type_) {
            let known: Vec<&str> = vocab.types.iter().map(|t| t.id.as_str()).collect();
            return Err(Error::Invalid(format!(
                "unknown type {}; use one of {}",
                py_repr(&json!(type_)),
                known.join(", ")
            )));
        }
        set_field(task, &mut changed, "type", json!(type_));
    }
    if let Some(priority) = &edit.priority {
        if !vocab.priorities.iter().any(|p| &p.id == priority) {
            let known: Vec<&str> = vocab.priorities.iter().map(|p| p.id.as_str()).collect();
            return Err(Error::Invalid(format!(
                "unknown priority {}; use one of {}",
                py_repr(&json!(priority)),
                known.join(", ")
            )));
        }
        set_field(task, &mut changed, "priority", json!(priority));
    }
    edit_list(
        task,
        &mut changed,
        "files",
        &strings(&edit.add_files),
        &strings(&edit.remove_files),
    )?;
    let labels = strings(&edit.add_labels);
    check_labels(&labels)?;
    edit_list(
        task,
        &mut changed,
        "labels",
        &labels,
        &strings(&edit.remove_labels),
    )?;
    // An entry is an ID or a Markdown link to where it is defined (T-391): any text but an empty one
    for (key, add, remove) in [
        (
            "requirements",
            &edit.add_requirements,
            &edit.remove_requirements,
        ),
        ("use_cases", &edit.add_use_cases, &edit.remove_use_cases),
    ] {
        if add.iter().any(|v| atm_common::py::strip(v).is_empty()) {
            let what = key[..key.len() - 1].replace('_', " ");
            return Err(Error::Invalid(format!("an empty {what} can't be added")));
        }
        edit_list(task, &mut changed, key, &strings(add), &strings(remove))?;
    }
    for (key, value) in [
        ("release", &edit.release),
        ("review", &edit.review),
        ("with", &edit.with_),
    ] {
        match value.as_deref() {
            Some("") => {
                if task.shift_remove(key).is_some() {
                    changed.push(key.into());
                }
            }
            Some(value) => {
                let value = json!(value);
                match key {
                    "release" => vocab.known_release(&py_str(&value))?,
                    "review" => check_review(&value)?,
                    _ => check_with(&value)?,
                }
                set_field(task, &mut changed, key, value);
            }
            None => {}
        }
    }
    if edit.components_auto {
        let files = py_list(task.get("files"), "files")?;
        let suggested: Vec<Value> = suggest_components(&vocab, &files)
            .into_iter()
            .map(Value::String)
            .collect();
        edit_list(task, &mut changed, "components", &suggested, &[])?;
    }
    let components = component_ids(&vocab, &strings(&edit.add_components))?;
    edit_list(
        task,
        &mut changed,
        "components",
        &components,
        &strings(&edit.remove_components),
    )?;
    let known: HashSet<String> = task_ids(&work.document).map(String::from).collect();
    for other in &edit.add_related {
        if *other == task_id {
            return Err(Error::Invalid(format!(
                "{task_id} can't be related to itself"
            )));
        }
        if !known.contains(other) && !archived.has(work, &json!(other))? {
            return Err(Error::Invalid(format!(
                "{task_id} can't be related to {other}, which doesn't exist"
            )));
        }
    }
    edit_list(
        task,
        &mut changed,
        "related",
        &strings(&edit.add_related),
        &strings(&edit.remove_related),
    )?;
    for key in ["components", "related"] {
        if task.get(key).is_some_and(|v| !truthy(v)) {
            task.shift_remove(key);
        }
    }
    let mut missing = Vec::new();
    for dep in &edit.add_dependencies {
        if !known.contains(dep) && !archived.has(work, &json!(dep))? {
            missing.push(dep.clone());
        }
    }
    if !missing.is_empty() {
        return Err(Error::Invalid(format!(
            "{task_id} can't depend on {}, which doesn't exist",
            missing.join(", ")
        )));
    }
    if edit.add_dependencies.contains(&task_id) {
        return Err(Error::Invalid(format!("{task_id} can't depend on itself")));
    }
    edit_list(
        task,
        &mut changed,
        "dependencies",
        &strings(&edit.add_dependencies),
        &strings(&edit.remove_dependencies),
    )?;
    Ok(changed)
}

/// `atm task edit`: change a task's definition (never its status), as the project's actor.
///
/// # Errors
/// An unknown task (it says when the task is archived), an invalid value, a dependency that doesn't exist or makes a
/// cycle: nothing is written then.
pub fn edit_existing_task(project: &mut Project, task_id: &str, edit: &TaskEdit) -> Result<Edited> {
    let actor = project.actor().map(String::from);
    let mut edited: Option<Edited> = None;
    project.store_mut().change(&mut |work: &mut Working| {
        let mut archived = Archived::default();
        let at = work.document["tasks"].as_array().and_then(|list| {
            list.iter()
                .position(|t| t.get("id").and_then(Value::as_str) == Some(task_id))
        });
        let Some(at) = at else {
            let hint = if archived.has(work, &json!(task_id))? {
                " (it is archived: atm db unarchive brings it back)"
            } else {
                ""
            };
            return Err(Error::failed(format!("no task {task_id}{hint}")));
        };
        let mut task = work.document["tasks"][at]
            .as_object()
            .cloned()
            .unwrap_or_default();
        let changed = edit_task(work, &mut task, edit, &mut archived)?;
        // The cycle check sees the task as edited
        let mut all: Vec<Task> = work.document["tasks"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|t| t.as_object().cloned().unwrap_or_default())
            .collect();
        all[at].clone_from(&task);
        if let Some(cycle) = find_cycle(&all.iter().collect::<Vec<_>>()) {
            return Err(Error::Invalid(format!(
                "these dependencies would make a cycle: {}",
                cycle.join(" -> ")
            )));
        }
        if !changed.is_empty() {
            atm_common::edits::touch(&mut task, actor.as_deref(), &now_iso());
            work.tasks()[at] = Value::Object(task.clone());
        }
        let vocab = Vocabulary::of_document(&work.document)?;
        edited = Some(Edited {
            task,
            changed,
            vocab,
        });
        Ok(())
    })?;
    edited.ok_or_else(|| Error::failed(format!("no change to {task_id}")))
}

#[cfg(test)]
mod tests;
