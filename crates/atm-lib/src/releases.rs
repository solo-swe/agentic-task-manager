//! Releases (the Python atm's `lib/releases.py`; docs/sprints-and-releases-design.md §3.3, SR-2.2): list them with
//! their tasks by kind and stage, add one (with pull requests, giving it their tasks, archived ones too), edit and
//! archive one, and draft its notes. A task's release is its own, else its sprint's.

use atm_common::ids::valid_short_id;
use atm_common::py::{py_str, repr_str};
use atm_common::time::parse_iso;
use atm_common::vocabulary::is_date;
use atm_common::{Error, Kind, Result, Task, Vocabulary};
use atm_db::working::Working;
use serde_json::{Map, Value, json};

use crate::project::Project;

/// A done task's stages, in order (DM-4.3).
const STAGES: [&str; 3] = ["done", "in_pr", "merged"];

fn invalid(message: impl Into<String>) -> Error {
    Error::Invalid(message.into())
}

/// A done task's stage: merged once its PR merged, `in_pr` while its PR is open, else done; none when it isn't done.
fn stage_of(vocab: &Vocabulary, task: &Task) -> Option<&'static str> {
    if vocab.kind_of(task.get("status")) != Some(Kind::Done) {
        return None;
    }
    let pr = task.get("pr").is_some_and(Value::is_object);
    if pr && task.get("merged_at").is_some_and(atm_common::py::truthy) {
        return Some("merged");
    }
    Some(if pr { "in_pr" } else { "done" })
}

fn release_id(release: &Map<String, Value>) -> String {
    release.get("id").map(py_str).unwrap_or_default()
}

fn find_release<'v>(vocab: &'v Vocabulary, id: &str) -> Option<&'v Map<String, Value>> {
    vocab
        .releases
        .iter()
        .find(|r| r.get("id").and_then(Value::as_str) == Some(id))
}

/// No such release, naming the releases (exit code 2).
fn unknown_release(vocab: &Vocabulary, id: &str) -> Error {
    let ids: Vec<String> = vocab.releases.iter().map(release_id).collect();
    let known = if ids.is_empty() {
        "none yet: atm release add <R>".to_string()
    } else {
        ids.join(", ")
    };
    Error::Failed {
        message: format!("no release {}; the releases: {known}", repr_str(id)),
        code: 2,
    }
}

/// The releases in order, each with its tasks (by effective release, both files) by kind and by stage.
fn release_overview(vocab: &Vocabulary, tasks: &[&Task], include_archived: bool) -> Vec<Value> {
    vocab
        .releases
        .iter()
        .filter(|r| include_archived || r.get("state").and_then(Value::as_str) != Some("archived"))
        .map(|r| {
            let id = json!(release_id(r));
            let (mut by_kind, mut by_stage) = ([0usize; 6], [0usize; 3]);
            for task in tasks
                .iter()
                .filter(|t| vocab.effective_release(t).as_ref() == Some(&id))
            {
                if let Some(kind) = vocab.kind_of(task.get("status"))
                    && let Some(at) = Kind::ALL.iter().position(|k| *k == kind)
                {
                    by_kind[at] += 1;
                }
                if let Some(stage) = stage_of(vocab, task)
                    && let Some(at) = STAGES.iter().position(|s| *s == stage)
                {
                    by_stage[at] += 1;
                }
            }
            let mut out = r.clone();
            out.insert("tasks".into(), json!(by_kind.iter().sum::<usize>()));
            let kinds: Map<String, Value> = Kind::ALL
                .iter()
                .zip(by_kind)
                .filter(|(_, n)| *n > 0)
                .map(|(k, n)| (k.as_str().to_string(), json!(n)))
                .collect();
            let stages: Map<String, Value> = STAGES
                .iter()
                .zip(by_stage)
                .filter(|(_, n)| *n > 0)
                .map(|(s, n)| ((*s).to_string(), json!(n)))
                .collect();
            out.insert("by_kind".into(), Value::Object(kinds));
            out.insert("by_stage".into(), Value::Object(stages));
            Value::Object(out)
        })
        .collect()
}

/// `atm release list`: the releases in order with their tasks by kind and stage, and whether there are any at all.
///
/// # Errors
/// A task file that can't be read.
pub fn list_releases(project: &Project, include_archived: bool) -> Result<(Vec<Value>, bool)> {
    let store = project.store();
    let vocab = store.vocabulary()?;
    let mut tasks = store.list_tasks()?;
    tasks.extend(store.list_archived()?);
    Ok((
        release_overview(vocab, &tasks, include_archived),
        !vocab.releases.is_empty(),
    ))
}

/// `name` and `target` on a release (`None` leaves one as it is); an empty value removes the field. Returns the names
/// changed.
fn release_fields(
    release: &mut Map<String, Value>,
    name: Option<&str>,
    target: Option<&str>,
) -> Result<Vec<String>> {
    if let Some(target) = target.filter(|t| !t.is_empty())
        && !is_date(&json!(target))
    {
        return Err(invalid(format!(
            "--target {}: use a date, YYYY-MM-DD",
            repr_str(target)
        )));
    }
    let mut changed = Vec::new();
    for (key, value) in [("name", name), ("target", target)] {
        let Some(value) = value else { continue };
        if value.is_empty() {
            if release.shift_remove(key).is_some() {
                changed.push(key.to_string());
            }
        } else if release.get(key).and_then(Value::as_str) != Some(value) {
            release.insert(key.into(), json!(value));
            changed.push(key.to_string());
        }
    }
    Ok(changed)
}

/// The position in `meta.releases` of the release `id`, or an error naming the releases.
fn stored_release(document: &Value, vocab: &Vocabulary, id: &str) -> Result<usize> {
    if find_release(vocab, id).is_none() {
        return Err(unknown_release(vocab, id));
    }
    document["meta"]["releases"]
        .as_array()
        .and_then(|list| {
            list.iter()
                .position(|r| r.get("id").and_then(Value::as_str) == Some(id))
        })
        .ok_or_else(|| unknown_release(vocab, id))
}

fn release_at(document: &mut Value, at: usize) -> &mut Map<String, Value> {
    let entry = &mut document["meta"]["releases"][at];
    if !entry.is_object() {
        *entry = json!({});
    }
    entry.as_object_mut().expect("made an object just above")
}

/// What `atm release add` added: the release, and the tasks its pull requests gave it.
#[derive(Debug, Clone)]
pub struct AddedRelease {
    /// The new release.
    pub release: Value,
    /// The tasks given to it, by ID.
    pub tasks: Vec<String>,
}

/// Give the release `id` every task of pull request `number` in `tasks`; the release records the PR's address.
fn take_pr_tasks(
    tasks: &mut [Value],
    number: i64,
    id: &str,
    release: &mut Map<String, Value>,
) -> Vec<String> {
    let mut own = Vec::new();
    for task in tasks.iter_mut().filter_map(Value::as_object_mut) {
        let Some(pr) = task.get("pr").and_then(Value::as_object).cloned() else {
            continue;
        };
        if pr.get("number").and_then(Value::as_i64) != Some(number) {
            continue;
        }
        task.insert("release".into(), json!(id));
        own.push(task.get("id").map(py_str).unwrap_or_default());
        if let Some(url) = pr.get("url").filter(|u| u.is_string()) {
            release.insert("pr".into(), json!({"number": number, "url": url}));
        }
    }
    own
}

/// `atm release add`: a planned release; with `prs` (pull request numbers) every task of those PRs, in both files, goes
/// into it; with `released_at` it shipped already.
///
/// # Errors
/// A malformed ID, PR number or time, a release already there, a PR with no tasks, a bad target date.
pub fn add_release(
    project: &mut Project,
    id: &str,
    name: Option<&str>,
    target: Option<&str>,
    prs: &[i64],
    released_at: Option<&str>,
) -> Result<AddedRelease> {
    if !valid_short_id(id) {
        return Err(invalid(format!(
            "{}: a release ID is letters, digits, '.', '-' and '_' (at most 64)",
            repr_str(id)
        )));
    }
    if prs.iter().any(|n| *n <= 0) {
        return Err(invalid("--pr needs a pull request number"));
    }
    if let Some(at) = released_at
        && parse_iso(at).is_none()
    {
        return Err(invalid(format!(
            "--released-at needs a time like 2026-10-24T18:00:00Z, not {}",
            repr_str(at)
        )));
    }
    let mut outcome: Option<AddedRelease> = None;
    project.store_mut().change(&mut |work: &mut Working| {
        let vocab = Vocabulary::of_document(&work.document)?;
        if find_release(&vocab, id).is_some() {
            return Err(invalid(format!(
                "there is already a release {}",
                repr_str(id)
            )));
        }
        let mut release = Map::new();
        release.insert("id".into(), json!(id));
        release.insert("state".into(), json!("planned"));
        release_fields(&mut release, name, target)?;
        let mut given = Vec::new();
        // A release recovered from pull requests (SR-8.2): their tasks, in both files, go into it; the release records
        // the last PR given (the one that shipped it)
        for number in prs {
            let mut own = Vec::new();
            if let Some(tasks) = work.document["tasks"].as_array_mut() {
                own.extend(take_pr_tasks(tasks, *number, id, &mut release));
            }
            if let Some(tasks) = work.archive()?["tasks"].as_array_mut() {
                own.extend(take_pr_tasks(tasks, *number, id, &mut release));
            }
            if own.is_empty() {
                return Err(invalid(format!("no task is in pull request #{number}")));
            }
            given.extend(own);
        }
        if let Some(at) = released_at {
            // One that shipped already: released at that time (its PR's merge, for one recovered from PRs)
            release.insert("state".into(), json!("released"));
            release.insert("released_at".into(), json!(at));
        }
        if !work.document.get("meta").is_some_and(Value::is_object) {
            work.document["meta"] = json!({});
        }
        let meta = &mut work.document["meta"];
        if !meta.get("releases").is_some_and(Value::is_array) {
            meta["releases"] = json!([]);
        }
        if let Some(list) = meta["releases"].as_array_mut() {
            list.push(Value::Object(release.clone()));
        }
        Vocabulary::of_document(&work.document)?;
        outcome = Some(AddedRelease {
            release: Value::Object(release),
            tasks: given,
        });
        Ok(())
    })?;
    outcome.ok_or_else(|| invalid(format!("no release {}", repr_str(id))))
}

/// `atm release edit`: a release's name or target (`None` leaves it, an empty value removes it). Returns the release
/// and the fields changed.
///
/// # Errors
/// An unknown release, a bad target date.
pub fn edit_release(
    project: &mut Project,
    id: &str,
    name: Option<&str>,
    target: Option<&str>,
) -> Result<(Value, Vec<String>)> {
    let mut outcome = (Value::Null, Vec::new());
    project.store_mut().change(&mut |work: &mut Working| {
        let vocab = Vocabulary::of_document(&work.document)?;
        let at = stored_release(&work.document, &vocab, id)?;
        let mut release = release_at(&mut work.document, at).clone();
        let changed = release_fields(&mut release, name, target)?;
        if !changed.is_empty() {
            release_at(&mut work.document, at).clone_from(&release);
            Vocabulary::of_document(&work.document)?;
        }
        outcome = (Value::Object(release), changed);
        Ok(())
    })?;
    Ok(outcome)
}

/// What a release's rename changed: its IDs, and how many tasks and sprints follow it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenamedRelease {
    /// Its ID before.
    pub from: String,
    /// Its ID now.
    pub to: String,
    /// The tasks in tasks.json whose own release it is.
    pub tasks: usize,
    /// The archived tasks whose own release it is.
    pub archived: usize,
    /// The sprints delivering to it.
    pub sprints: usize,
}

/// The objects in `list` whose `release` is `from` take `to`; how many.
fn rename_release_in(list: Option<&mut Vec<Value>>, from: &str, to: &str) -> usize {
    let mut n = 0;
    for entry in list.into_iter().flatten().filter_map(Value::as_object_mut) {
        if entry.get("release").and_then(Value::as_str) == Some(from) {
            entry.insert("release".into(), json!(to));
            n += 1;
        }
    }
    n
}

/// `atm release rename` (T-497): a release's ID; the tasks (in tasks.json and the archive) and the sprints that name it
/// follow. Its name, state and dates stay; a task's `updated_at` too, as the rename is the release's.
///
/// # Errors
/// An unknown release, a new ID that isn't one or is another release's.
pub fn rename_release(project: &mut Project, id: &str, new: &str) -> Result<RenamedRelease> {
    if !valid_short_id(new) {
        return Err(invalid(format!(
            "{}: a release ID is letters, digits, '.', '-' and '_' (at most 64)",
            repr_str(new)
        )));
    }
    let mut outcome: Option<RenamedRelease> = None;
    project.store_mut().change(&mut |work: &mut Working| {
        let vocab = Vocabulary::of_document(&work.document)?;
        let at = stored_release(&work.document, &vocab, id)?;
        if new == id {
            return Err(invalid(format!("the release is already {}", repr_str(id))));
        }
        if find_release(&vocab, new).is_some() {
            return Err(invalid(format!(
                "there is already a release {}",
                repr_str(new)
            )));
        }
        release_at(&mut work.document, at).insert("id".into(), json!(new));
        let tasks = rename_release_in(work.document["tasks"].as_array_mut(), id, new);
        let sprints = rename_release_in(
            work.document["meta"]
                .get_mut("sprints")
                .and_then(Value::as_array_mut),
            id,
            new,
        );
        let archived = rename_release_in(
            work.archive()?
                .get_mut("tasks")
                .and_then(Value::as_array_mut),
            id,
            new,
        );
        Vocabulary::of_document(&work.document)?;
        outcome = Some(RenamedRelease {
            from: id.to_string(),
            to: new.to_string(),
            tasks,
            archived,
            sprints,
        });
        Ok(())
    })?;
    outcome.ok_or_else(|| invalid(format!("no release {}", repr_str(id))))
}

/// `atm release archive`: it leaves the open releases (`atm release list --all` shows it). Returns the release.
///
/// # Errors
/// An unknown release, or one archived already.
pub fn archive_release(project: &mut Project, id: &str) -> Result<Value> {
    let mut archived = Value::Null;
    project.store_mut().change(&mut |work: &mut Working| {
        let vocab = Vocabulary::of_document(&work.document)?;
        let at = stored_release(&work.document, &vocab, id)?;
        let release = release_at(&mut work.document, at);
        if release.get("state").and_then(Value::as_str) == Some("archived") {
            return Err(invalid(format!(
                "the release {} is already archived",
                repr_str(&release_id(release))
            )));
        }
        release.insert("state".into(), json!("archived"));
        archived = Value::Object(release.clone());
        Ok(())
    })?;
    Ok(archived)
}

/// The task's last outcome note (what `atm task state done --note` said).
fn outcome_of(task: &Task) -> Value {
    task.get("notes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .rfind(|n| n.get("kind").and_then(Value::as_str) == Some("outcome"))
        .map_or(Value::Null, |n| {
            n.get("text").cloned().unwrap_or(Value::Null)
        })
}

/// `atm release notes`: a draft of the release's changes (§3.3): its done tasks (by effective release, both files)
/// grouped by type in the project's order, each with its ID, name and outcome note; and its tasks not done yet.
/// Returns `{release, groups, open}`.
///
/// # Errors
/// An unknown release, a task file that can't be read.
pub fn release_notes(project: &Project, id: &str) -> Result<Value> {
    let store = project.store();
    let vocab = store.vocabulary()?;
    let release = find_release(vocab, id).ok_or_else(|| unknown_release(vocab, id))?;
    let mut tasks = store.list_tasks()?;
    tasks.extend(store.list_archived()?);
    let wanted = json!(id);
    let own: Vec<&Task> = tasks
        .into_iter()
        .filter(|t| vocab.effective_release(t).as_ref() == Some(&wanted))
        .collect();
    let done: Vec<&Task> = own
        .iter()
        .copied()
        .filter(|t| vocab.kind_of(t.get("status")) == Some(Kind::Done))
        .collect();
    let mut types: Vec<(Value, Value)> = vocab
        .types
        .iter()
        .map(|t| (json!(t.id), t.label.clone()))
        .collect();
    for task in &done {
        let own_type = task.get("type").cloned().unwrap_or(Value::Null);
        if !types.iter().any(|(id, _)| *id == own_type) {
            types.push((own_type.clone(), own_type));
        }
    }
    let field = |t: &Task, key: &str| t.get(key).cloned().unwrap_or(Value::Null);
    let groups: Vec<Value> = types
        .iter()
        .filter_map(|(type_id, label)| {
            let items: Vec<Value> = done
                .iter()
                .filter(|t| t.get("type").unwrap_or(&Value::Null) == type_id)
                .map(|t| json!({"id": field(t, "id"), "name": field(t, "name"), "outcome": outcome_of(t)}))
                .collect();
            (!items.is_empty()).then(|| json!({"type": type_id, "label": label, "tasks": items}))
        })
        .collect();
    let open: Vec<Value> = own
        .iter()
        .filter(|t| !matches!(vocab.kind_of(t.get("status")), Some(Kind::Done | Kind::Skipped)))
        .map(|t| json!({"id": field(t, "id"), "name": field(t, "name"), "status": field(t, "status")}))
        .collect();
    Ok(json!({"release": release, "groups": groups, "open": open}))
}

#[cfg(test)]
mod tests;
