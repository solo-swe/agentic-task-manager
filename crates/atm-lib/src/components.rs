//! The project's components (the Python atm's `lib/components.py`; docs/sprints-and-releases-design.md §2, SR-2.7):
//! list them, define one, change one; moving a label onto a component, or giving it to the tasks its globs match
//! (T-324), changes the archive's tasks too, in the same store change. The rules (which files a component has, the IDs
//! a task may name) are atm-common's ([`atm_common::components`]).

use atm_common::components::matches;
use atm_common::ids::valid_short_id;
use atm_common::py::{py_str, repr_str};
use atm_common::vocabulary::REVIEW_MODES;
use atm_common::{Error, Result, Vocabulary};
use atm_db::working::Working;
use serde_json::{Map, Value, json};

use crate::project::Project;

fn invalid(message: impl Into<String>) -> Error {
    Error::Invalid(message.into())
}

/// `meta.components` in order, each with how many tasks have it, archived ones too.
///
/// # Errors
/// A task file that can't be read.
pub fn list_components(project: &Project) -> Result<Vec<Value>> {
    let store = project.store();
    let mut tasks = store.list_tasks()?;
    tasks.extend(store.list_archived()?);
    Ok(store
        .vocabulary()?
        .components
        .iter()
        .map(|c| {
            let id = c.get("id").cloned().unwrap_or(Value::Null);
            let count = tasks
                .iter()
                .filter(|t| {
                    t.get("components")
                        .and_then(Value::as_array)
                        .is_some_and(|list| list.contains(&id))
                })
                .count();
            let mut out = c.clone();
            out.insert("tasks".into(), json!(count));
            Value::Object(out)
        })
        .collect())
}

/// A `meta.labels` entry's ID: the entry itself when it is a text, else its `id`.
fn label_entry(entry: &Value) -> Option<&str> {
    entry
        .as_str()
        .or_else(|| entry.get("id").and_then(Value::as_str))
}

/// The text `meta.labels` gives a label (e.g. "Dashboard" for dashboard).
fn label_text(meta: &Value, label: &str) -> Option<String> {
    meta.get("labels")
        .and_then(Value::as_array)?
        .iter()
        .find(|e| label_entry(e) == Some(label))?
        .get("label")
        .and_then(Value::as_str)
        .map(String::from)
}

fn check_options(globs: &[String], review: Option<&str>) -> Result<()> {
    if globs.iter().any(String::is_empty) {
        return Err(invalid("--path needs a glob, e.g. 'dashboard/**'"));
    }
    if let Some(review) = review.filter(|r| !r.is_empty() && !REVIEW_MODES.contains(r)) {
        return Err(invalid(format!(
            "--review {}: use one of {}",
            repr_str(review),
            REVIEW_MODES.join(", ")
        )));
    }
    Ok(())
}

/// What a label's move and an assignment did: the tasks that had the label, and those given it by their files.
#[derive(Debug, Clone, Default)]
pub struct Moved {
    /// The tasks that had the label, now the component.
    pub from_label: Vec<String>,
    /// The tasks given the component by their files.
    pub by_files: Vec<String>,
}

/// One task of either file: the label (if given) becomes the component, and with `assign` a task whose files the
/// component's globs match gets it too. Returns whether the label moved and whether the task changed.
fn move_one(
    task: &mut Map<String, Value>,
    component: &Map<String, Value>,
    label: Option<&str>,
    assign: bool,
) -> (bool, bool) {
    let id = component.get("id").cloned().unwrap_or(Value::Null);
    let labels = task
        .get("labels")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let moved = label.is_some_and(|l| labels.iter().any(|x| x == l));
    let current = task
        .get("components")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let matched = assign
        && !current.contains(&id)
        && task
            .get("files")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .any(|f| matches(component, f));
    if !(moved || matched) {
        return (false, false);
    }
    if moved {
        let kept: Vec<Value> = labels.into_iter().filter(|x| x.as_str() != label).collect();
        if kept.is_empty() {
            task.shift_remove("labels");
        } else {
            task.insert("labels".into(), Value::Array(kept));
        }
    }
    if !current.contains(&id) {
        let mut list = current;
        list.push(id);
        task.insert("components".into(), Value::Array(list));
    }
    (moved, true)
}

/// On every task of both files: `label` (if given) becomes the component, and with `assign` a task whose files its
/// globs match gets it too; `label` leaves `meta.labels`. A label no task has and `meta.labels` doesn't define (most
/// likely a typo) is refused before anything changes.
fn move_and_assign(
    work: &mut Working,
    component: &Map<String, Value>,
    label: Option<&str>,
    assign: bool,
) -> Result<Moved> {
    let label = label.filter(|l| !l.is_empty());
    let archive_tasks = work.archive()?["tasks"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if let Some(label) = label {
        let known = work.document["meta"]
            .get("labels")
            .and_then(Value::as_array)
            .is_some_and(|list| list.iter().any(|e| label_entry(e) == Some(label)));
        let has = |tasks: &[Value]| {
            tasks.iter().any(|t| {
                t.get("labels")
                    .and_then(Value::as_array)
                    .is_some_and(|l| l.iter().any(|x| x == label))
            })
        };
        let current = work.document["tasks"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        if !known && !has(&current) && !has(&archive_tasks) {
            return Err(invalid(format!(
                "no task has the label {}",
                repr_str(label)
            )));
        }
    }
    let mut result = Moved::default();
    let mut record = |task: &Map<String, Value>, moved: bool| {
        let id = task.get("id").map(py_str).unwrap_or_default();
        if moved {
            result.from_label.push(id);
        } else {
            result.by_files.push(id);
        }
    };
    if let Some(tasks) = work.document["tasks"].as_array_mut() {
        for task in tasks.iter_mut().filter_map(Value::as_object_mut) {
            if let (moved, true) = move_one(task, component, label, assign) {
                record(task, moved);
            }
        }
    }
    if let Some(tasks) = work.archive()?["tasks"].as_array_mut() {
        for task in tasks.iter_mut().filter_map(Value::as_object_mut) {
            if let (moved, true) = move_one(task, component, label, assign) {
                record(task, moved);
            }
        }
    }
    if let Some(label) = label
        && let Some(meta) = work.document["meta"].as_object_mut()
        && let Some(entries) = meta.get_mut("labels").and_then(Value::as_array_mut)
    {
        entries.retain(|e| label_entry(e) != Some(label));
        if entries.is_empty() {
            meta.shift_remove("labels");
        }
    }
    Ok(result)
}

fn ensure_meta(document: &mut Value) {
    if !document.get("meta").is_some_and(Value::is_object) {
        document["meta"] = json!({});
    }
}

/// `atm component add`: define a component; `from_label` moves that label onto it on every task (both files) and
/// drops it from `meta.labels`; `assign` gives it to every task whose files its globs match. One change.
///
/// # Errors
/// A malformed ID, glob or review mode, `--assign` without a glob, a component already there, a label no task has.
pub fn add_component(
    project: &mut Project,
    id: &str,
    label: Option<&str>,
    paths: &[String],
    review: Option<&str>,
    from_label: Option<&str>,
    assign: bool,
) -> Result<(Value, Moved)> {
    if !valid_short_id(id) {
        return Err(invalid(format!(
            "{}: a component ID is letters, digits, '.', '-' and '_' (at most 64)",
            repr_str(id)
        )));
    }
    check_options(paths, review)?;
    if assign && paths.is_empty() {
        return Err(invalid(
            "--assign gives the component to the tasks its globs match: it needs --path",
        ));
    }
    let mut outcome = (Value::Null, Moved::default());
    project.store_mut().change(&mut |work: &mut Working| {
        let vocab = Vocabulary::of_document(&work.document)?;
        if vocab
            .components
            .iter()
            .any(|c| c.get("id").and_then(Value::as_str) == Some(id))
        {
            return Err(invalid(format!(
                "there is already a component {} (atm component edit changes it)",
                repr_str(id)
            )));
        }
        ensure_meta(&mut work.document);
        let mut component = Map::new();
        component.insert("id".into(), json!(id));
        // The name shown: the label given, else the label's own text (meta.labels), else none
        let shown = label
            .filter(|l| !l.is_empty())
            .map(String::from)
            .or_else(|| {
                from_label
                    .filter(|l| !l.is_empty())
                    .and_then(|l| label_text(&work.document["meta"], l))
            });
        if let Some(shown) = shown {
            component.insert("label".into(), json!(shown));
        }
        component.insert("paths".into(), json!(paths));
        if let Some(review) = review.filter(|r| !r.is_empty()) {
            component.insert("review".into(), json!(review));
        }
        let meta = &mut work.document["meta"];
        if !meta.get("components").is_some_and(Value::is_array) {
            meta["components"] = json!([]);
        }
        if let Some(list) = meta["components"].as_array_mut() {
            list.push(Value::Object(component.clone()));
        }
        let moved = move_and_assign(work, &component, from_label, assign)?;
        Vocabulary::of_document(&work.document)?; // checks meta.components, as atm db init would
        outcome = (Value::Object(component), moved);
        Ok(())
    })?;
    Ok(outcome)
}

/// What `atm component edit` did: the component after it, the fields changed, and what moved.
#[derive(Debug, Clone)]
pub struct EditedComponent {
    /// The component after the edit.
    pub component: Value,
    /// The fields changed.
    pub changed: Vec<String>,
    /// What `--from-label` and `--assign` did.
    pub moved: Moved,
}

/// `atm component edit`: `label` and `review` (empty: remove them), `add_paths`; `from_label` and `assign` as for add,
/// the globs added included. One change.
///
/// # Errors
/// An unknown component, a malformed glob or review mode, `--assign` on a component without globs, a label no task
/// has.
pub fn edit_component(
    project: &mut Project,
    id: &str,
    label: Option<&str>,
    add_paths: &[String],
    review: Option<&str>,
    from_label: Option<&str>,
    assign: bool,
) -> Result<EditedComponent> {
    check_options(add_paths, review)?;
    let mut outcome: Option<EditedComponent> = None;
    project.store_mut().change(&mut |work: &mut Working| {
        let vocab = Vocabulary::of_document(&work.document)?;
        let known: Vec<String> = vocab
            .components
            .iter()
            .map(|c| c.get("id").map(py_str).unwrap_or_default())
            .collect();
        if !known.iter().any(|k| k == id) {
            let listed = if known.is_empty() {
                "none yet".to_string()
            } else {
                known.join(", ")
            };
            return Err(Error::Failed {
                message: format!("no component {}; the components: {listed}", repr_str(id)),
                code: 2,
            });
        }
        let at = work.document["meta"]["components"]
            .as_array()
            .and_then(|list| {
                list.iter()
                    .position(|c| c.get("id").and_then(Value::as_str) == Some(id))
            })
            .ok_or_else(|| invalid(format!("no component {}", repr_str(id))))?;
        let mut component = work.document["meta"]["components"][at]
            .as_object()
            .cloned()
            .unwrap_or_default();
        let mut changed = Vec::new();
        for (key, value) in [("label", label), ("review", review)] {
            match value {
                Some("") if component.contains_key(key) => {
                    component.shift_remove(key);
                    changed.push(key.to_string());
                }
                Some(value)
                    if !value.is_empty()
                        && component.get(key).and_then(Value::as_str) != Some(value) =>
                {
                    component.insert(key.into(), json!(value));
                    changed.push(key.to_string());
                }
                _ => {}
            }
        }
        let mut paths: Vec<Value> = component
            .get("paths")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut added = false;
        for glob in add_paths {
            if !paths.iter().any(|p| p == glob.as_str()) {
                paths.push(json!(glob));
                added = true;
            }
        }
        if added {
            component.insert("paths".into(), Value::Array(paths.clone()));
            changed.push("paths".into());
        }
        if assign && paths.is_empty() {
            return Err(invalid(format!(
                "--assign gives {id} to the tasks its globs match, and it has none: add --add-path"
            )));
        }
        work.document["meta"]["components"][at] = Value::Object(component.clone());
        let moved = move_and_assign(work, &component, from_label, assign)?;
        Vocabulary::of_document(&work.document)?; // checks meta.components, as atm db init would
        outcome = Some(EditedComponent {
            component: Value::Object(component),
            changed,
            moved,
        });
        Ok(())
    })?;
    outcome.ok_or_else(|| invalid(format!("no component {}", repr_str(id))))
}

#[cfg(test)]
mod tests;
