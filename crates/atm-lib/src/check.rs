//! `atm doctor` (the Python atm's `lib/edit.py` `check_data` and `check_project`): what is wrong with the task data
//! as stored, as errors (exit code 1) and warnings.

use std::collections::HashSet;
use std::path::Path;

use atm_common::archive::open_in_closed_sprints;
use atm_common::components::suggest_components;
use atm_common::ids::valid_task_id;
use atm_common::py::{py_repr, py_str, repr_str};
use atm_common::{Kind, Result, Task, Vocabulary};
use serde_json::{Map, Value, json};

use crate::edit::find_cycle;
use crate::project::Project;

/// What `atm doctor` found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checked {
    /// What stops the data from working.
    pub errors: Vec<String>,
    /// What is worth a look.
    pub warnings: Vec<String>,
    /// How many tasks there are; none when the data isn't task data at all.
    pub count: Option<usize>,
}

/// The resolutions a finished task can have (SR-2.8).
const RESOLUTIONS: [&str; 4] = ["done", "wont_do", "duplicate", "obsolete"];

fn valid_label(label: &str) -> bool {
    (1..=32).contains(&label.len())
        && label
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

fn is_list_of_strings(value: &Value) -> bool {
    value
        .as_array()
        .is_some_and(|items| items.iter().all(Value::is_string))
}

/// A task's resolution, `duplicate_of` and `related` (SR-2.8): a resolution only on a done or skipped task, one of
/// [`RESOLUTIONS`]; `duplicate_of` only with the duplicate resolution, naming another task that exists (in either
/// file).
fn check_resolution(
    vocab: &Vocabulary,
    task: &Task,
    known: &HashSet<String>,
    errors: &mut Vec<String>,
) {
    let task_id = py_str(&task["id"]);
    let resolution = task.get("resolution");
    if let Some(resolution) = resolution {
        if !resolution
            .as_str()
            .is_some_and(|r| RESOLUTIONS.contains(&r))
        {
            errors.push(format!(
                "{task_id}: the resolution {} is not one of {}",
                py_repr(resolution),
                RESOLUTIONS.join(", ")
            ));
        } else if !matches!(
            vocab.kind_of(task.get("status")),
            Some(Kind::Done | Kind::Skipped)
        ) {
            errors.push(format!(
                "{task_id}: has a resolution ({}) but isn't done or skipped",
                py_str(resolution)
            ));
        }
    }
    let is_other = |v: &Value| {
        v.as_str()
            .is_some_and(|o| o != task_id && known.contains(o))
    };
    if let Some(other) = task.get("duplicate_of") {
        if resolution.is_none_or(|r| r != "duplicate") {
            errors.push(format!(
                "{task_id}: duplicate_of needs the resolution duplicate"
            ));
        }
        if !is_other(other) {
            errors.push(format!(
                "{task_id}: duplicate_of {} isn't another task",
                py_repr(other)
            ));
        }
    }
    match task.get("related") {
        None => {}
        Some(related) if !is_list_of_strings(related) => {
            errors.push(format!("{task_id}: 'related' must be a list of task IDs"));
        }
        Some(related) => {
            for other in related.as_array().into_iter().flatten() {
                if !is_other(other) {
                    errors.push(format!(
                        "{task_id}: related to {}, which isn't another task",
                        py_str(other)
                    ));
                }
            }
        }
    }
}

fn find_release<'v>(vocab: &'v Vocabulary, release: &Value) -> Option<&'v Map<String, Value>> {
    vocab.releases.iter().find(|r| r.get("id") == Some(release))
}

/// The checks of one task (well formed: an object with a string ID).
#[allow(clippy::too_many_lines)] // reason: one check after another, as the Python atm has them
fn check_task(
    vocab: &Vocabulary,
    task: &Task,
    known: &HashSet<String>,
    root: Option<&Path>,
    files: bool,
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
) {
    let task_id = py_str(&task["id"]);
    if !valid_task_id(&task_id) {
        errors.push(format!(
            "{}: an ID is letters, digits, '.', '_' and '-', at most 64, not starting with '-'",
            repr_str(&task_id)
        ));
    }
    for key in ["name", "type", "status"] {
        if !task.get(key).is_some_and(Value::is_string) {
            errors.push(format!("{task_id}: needs a {} (a string)", repr_str(key)));
        }
    }
    if let Some(status) = task.get("status").filter(|s| s.is_string())
        && vocab.kind_of(Some(status)).is_none()
    {
        errors.push(format!(
            "{task_id}: the status {} is not one of the project's statuses",
            py_repr(status)
        ));
    }
    if let Some(type_) = task.get("type").and_then(Value::as_str)
        && !vocab.types.iter().any(|t| t.id == type_)
    {
        warnings.push(format!(
            "{task_id}: the type {} is not one of the project's types",
            repr_str(type_)
        ));
    }
    if let Some(priority) = task.get("priority")
        && !vocab.priorities.iter().any(|p| priority == p.id.as_str())
    {
        warnings.push(format!(
            "{task_id}: the priority {} is not one of the project's priorities",
            py_repr(priority)
        ));
    }
    for key in [
        "dependencies",
        "files",
        "labels",
        "requirements",
        "use_cases",
    ] {
        if task.get(key).is_some_and(|v| !is_list_of_strings(v)) {
            errors.push(format!(
                "{task_id}: {} must be a list of strings",
                repr_str(key)
            ));
        }
    }
    if let Some(sprint) = task.get("sprint")
        && vocab.find_sprint(Some(sprint)).is_none()
    {
        errors.push(format!(
            "{task_id}: the sprint {} is not one of meta.sprints",
            py_repr(sprint)
        ));
    }
    if let Some(release) = task.get("release")
        && find_release(vocab, release).is_none()
    {
        errors.push(format!(
            "{task_id}: the release {} is not one of meta.releases",
            py_repr(release)
        ));
    }
    if let Some(history) = task.get("sprint_history") {
        let ok = history.as_array().is_some_and(|entries| {
            entries
                .iter()
                .all(|e| e.get("sprint").is_some_and(Value::is_string) && e.is_object())
        });
        if !ok {
            errors.push(format!(
                "{task_id}: \"sprint_history\" must be a list of {{\"sprint\", \"from\", \"to\"}}"
            ));
        }
    }
    let list = |key: &str| -> Vec<Value> {
        task.get(key)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };
    let task_files: Vec<Value> = list("files").into_iter().filter(Value::is_string).collect();
    if !vocab.components.is_empty()
        && !task_files.is_empty()
        && suggest_components(vocab, &task_files).is_empty()
    {
        warnings.push(format!(
            "{task_id}: its files match no component (meta.components' paths)"
        ));
    }
    check_resolution(vocab, task, known, errors);
    for dep in list("dependencies") {
        if let Some(dep) = dep.as_str()
            && !known.contains(dep)
        {
            errors.push(format!("{task_id}: depends on {dep}, which doesn't exist"));
        }
    }
    for label in list("labels") {
        if let Some(label) = label.as_str()
            && !valid_label(label)
        {
            errors.push(format!(
                "{task_id}: the label {} isn't lowercase letters, digits, - and _",
                repr_str(label)
            ));
        }
    }
    // Tasks list the files they will create, so only a finished task's missing files are worth a word
    if files
        && vocab.kind_of(task.get("status")) == Some(Kind::Done)
        && let Some(root) = root
    {
        for f in list("files") {
            if let Some(f) = f.as_str()
                && !root.join(f).exists()
            {
                warnings.push(format!("{task_id}: the file {f} doesn't exist"));
            }
        }
    }
}

/// Problems in a task document: (errors, warnings). `archived` are the archive's tasks: dependencies on them are
/// fine, and an ID in both files is an error. With `files`, also a warning for each file a finished task lists that
/// doesn't exist under `root` (often just history: files get renamed and deleted later).
///
/// # Panics
/// Never: the default vocabularies always work.
#[must_use]
pub fn check_data(
    document: &Value,
    root: Option<&Path>,
    files: bool,
    archived: &[Task],
) -> (Vec<String>, Vec<String>) {
    let (mut errors, mut warnings) = (Vec::new(), Vec::new());
    let vocab = Vocabulary::of_document(document).unwrap_or_else(|e| {
        errors.push(e.to_string());
        Vocabulary::of_document(&json!({})).expect("the defaults work")
    });
    let tasks: Vec<Value> = document["tasks"].as_array().cloned().unwrap_or_default();
    let ids: Vec<Value> = tasks
        .iter()
        .filter(|t| t.is_object())
        .map(|t| t.get("id").cloned().unwrap_or(Value::Null))
        .collect();
    let mut repeated: Vec<&Value> = ids
        .iter()
        .filter(|i| ids.iter().filter(|j| j == i).count() > 1)
        .collect();
    repeated.sort_by_key(|i| py_str(i));
    repeated.dedup();
    for id in repeated {
        let count = ids.iter().filter(|j| *j == id).count();
        errors.push(format!("{}: the ID is used by {count} tasks", py_str(id)));
    }
    let archived_ids: HashSet<String> = archived
        .iter()
        .filter_map(|t| t.get("id").and_then(Value::as_str))
        .map(String::from)
        .collect();
    let live_ids: HashSet<String> = ids
        .iter()
        .filter_map(Value::as_str)
        .map(String::from)
        .collect();
    let mut both: Vec<&String> = live_ids.intersection(&archived_ids).collect();
    both.sort();
    for id in both {
        errors.push(format!(
            "{id}: in both tasks.json and the archive (atm db archive keeps the tasks.json copy)"
        ));
    }
    let known: HashSet<String> = live_ids.union(&archived_ids).cloned().collect();
    for (i, task) in tasks.iter().enumerate() {
        let Some(task) = task.as_object().filter(|t| {
            t.get("id")
                .and_then(Value::as_str)
                .is_some_and(|id| !id.is_empty())
        }) else {
            errors.push(format!(
                "task #{}: needs an \"id\" (a non-empty string)",
                i + 1
            ));
            continue;
        };
        check_task(
            &vocab,
            task,
            &known,
            root,
            files,
            &mut errors,
            &mut warnings,
        );
    }
    let well_formed: Vec<Value> = tasks
        .iter()
        .filter(|t| t.get("id").is_some_and(Value::is_string) && t.is_object())
        .cloned()
        .collect();
    for t in open_in_closed_sprints(&vocab, &json!({"tasks": well_formed})) {
        let (id, sprint) = (py_str(&t["id"]), py_str(&t["sprint"]));
        warnings.push(format!(
            "{id}: {} in the closed sprint {}, where the loop won't take it: atm sprint move {id} --to backlog (or \
             --to active)",
            py_str(&t["status"]),
            repr_str(&sprint)
        ));
    }
    for sprint in &vocab.sprints {
        if let Some(release) = sprint.get("release").filter(|r| atm_common::py::truthy(r))
            && find_release(&vocab, release).is_none()
        {
            errors.push(format!(
                "the sprint {}: the release {} is not one of meta.releases",
                py_repr(sprint.get("id").unwrap_or(&Value::Null)),
                py_repr(release)
            ));
        }
    }
    let graph: Vec<Task> = tasks
        .iter()
        .filter_map(Value::as_object)
        .filter(|t| t.get("dependencies").is_none_or(Value::is_array))
        .cloned()
        .collect();
    let refs: Vec<&Task> = graph.iter().collect();
    if let Some(cycle) = find_cycle(&refs) {
        errors.push(format!("dependency cycle: {}", cycle.join(" -> ")));
    }
    (errors, warnings)
}

/// `atm doctor`: what is wrong with the task data as stored. With `files`, a finished task's files that don't exist
/// are warned about too (relative to the working copy).
///
/// # Errors
/// When there is no task data at all.
pub fn check_project(project: &Project, files: bool) -> Result<Checked> {
    let raw = project.store().raw()?;
    let Some(document) = raw.document else {
        return Ok(Checked {
            errors: vec![
                "tasks.json is not a task file (expected an object with a 'tasks' list)".into(),
            ],
            warnings: Vec::new(),
            count: None,
        });
    };
    let (found, warnings) = check_data(
        &document,
        project.store().working_copy(),
        files,
        &raw.archive,
    );
    let mut errors: Vec<String> = raw.archive_error.into_iter().collect();
    errors.extend(found);
    Ok(Checked {
        errors,
        warnings,
        count: document["tasks"].as_array().map(Vec::len),
    })
}

#[cfg(test)]
mod tests;
