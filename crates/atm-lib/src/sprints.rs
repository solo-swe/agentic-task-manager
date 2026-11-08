//! Sprints and the backlog (the Python atm's `lib/sprints.py`; docs/sprints-and-releases-design.md §3.1): list, add,
//! edit, start, close and delete sprints (`meta.sprints`), move tasks between sprints and the backlog, list the
//! backlog. Each write is one store change; a task's sprint changes only through `set_sprint`, which keeps its history.

use atm_common::edits::{set_sprint, touch};
use atm_common::py::{py_repr, py_str, repr_str, truthy};
use atm_common::sprint_names::generate_sprint_name;
use atm_common::time::now_iso;
use atm_common::vocabulary::{MAX_SPRINT_NAME, is_date};
use atm_common::{Error, Kind, Result, Task, Vocabulary};
use atm_db::working::Working;
use serde_json::{Map, Value, json};

use crate::edit::open_sprint;
use crate::project::Project;

/// A refusal of what was asked (exit code 2).
fn invalid(message: impl Into<String>) -> Error {
    Error::Invalid(message.into())
}

/// No such sprint (exit code 2, as the Python atm's `NotFound(…, 2)`).
fn not_found(message: impl Into<String>) -> Error {
    Error::Failed {
        message: message.into(),
        code: 2,
    }
}

fn id_of(value: &Value) -> Option<&str> {
    value.get("id").and_then(Value::as_str)
}

fn sprint_id(sprint: &Map<String, Value>) -> String {
    sprint.get("id").map(py_str).unwrap_or_default()
}

fn is_finished(kind: Option<Kind>) -> bool {
    matches!(kind, Some(Kind::Done | Kind::Skipped))
}

/// "is" or "are", "it" or "them": the words for one task or several.
fn one_or_many<'a>(n: usize, one: &'a str, many: &'a str) -> &'a str {
    if n == 1 { one } else { many }
}

/// The local time's (month, day, hour, minute), for sprints named by the time.
fn local_time() -> (i64, i64, i64, i64) {
    let (_, month, day, hour, minute) = atm_common::time::civil(atm_common::time::local_now());
    (month, day, hour, minute)
}

fn new_name(vocab: &Vocabulary, taken: &[String]) -> String {
    let seed = std::env::var("ATM_SEED").ok().filter(|s| !s.is_empty());
    generate_sprint_name(&vocab.sprint_names, taken, local_time(), seed.as_deref())
}

/// A sprint with its task counts: `{…sprint, tasks, by_kind, review_pending}`.
fn sprint_overview(vocab: &Vocabulary, tasks: &[&Task], include_closed: bool) -> Vec<Value> {
    vocab
        .sprints
        .iter()
        .filter(|s| include_closed || s.get("state").and_then(Value::as_str) != Some("closed"))
        .map(|s| {
            let id = sprint_id(s);
            let mut counts = [0usize; 6];
            let mut pending = 0;
            for task in tasks {
                let Some(found) = vocab.find_sprint(task.get("sprint")) else {
                    continue;
                };
                if sprint_id(found) != id {
                    continue;
                }
                if let Some(kind) = vocab.kind_of(task.get("status"))
                    && let Some(at) = Kind::ALL.iter().position(|k| *k == kind)
                {
                    counts[at] += 1;
                }
                if task.get("sprint_review").and_then(Value::as_str) == Some("pending") {
                    pending += 1;
                }
            }
            let mut out = s.clone();
            out.insert("tasks".into(), json!(counts.iter().sum::<usize>()));
            let by_kind: Map<String, Value> = Kind::ALL
                .iter()
                .zip(counts)
                .filter(|(_, n)| *n > 0)
                .map(|(k, n)| (k.as_str().to_string(), json!(n)))
                .collect();
            out.insert("by_kind".into(), Value::Object(by_kind));
            out.insert("review_pending".into(), json!(pending));
            Value::Object(out)
        })
        .collect()
}

/// Every sprint name in use: the sprints, and the sprints tasks in either file name or named.
fn taken_names(document: &Value, archived: &[Value]) -> Vec<String> {
    let mut names: Vec<String> = document
        .get("meta")
        .and_then(|m| m.get("sprints"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|s| id_of(s).map(String::from))
        .collect();
    let tasks = document["tasks"]
        .as_array()
        .into_iter()
        .flatten()
        .chain(archived);
    for task in tasks.filter(|t| t.is_object()) {
        names.extend(task.get("sprint").and_then(Value::as_str).map(String::from));
        names.extend(
            task.get("sprint_history")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|e| e.get("sprint").and_then(Value::as_str).map(String::from)),
        );
    }
    names
}

/// `goal`, `release` and `due` on a sprint (`None` leaves one as it is); an empty value removes the field; a release
/// must be one of `meta.releases` (SR-2.2). Returns the names changed.
fn apply_fields(
    sprint: &mut Map<String, Value>,
    vocab: &Vocabulary,
    goal: Option<&str>,
    release: Option<&str>,
    due: Option<&str>,
) -> Result<Vec<String>> {
    if let Some(release) = release.filter(|r| !r.is_empty()) {
        vocab.known_release(release)?;
    }
    if let Some(due) = due.filter(|d| !d.is_empty())
        && !is_date(&json!(due))
    {
        return Err(invalid(format!(
            "--due {}: use a date, YYYY-MM-DD",
            repr_str(due)
        )));
    }
    let mut changed = Vec::new();
    for (key, value) in [("goal", goal), ("release", release), ("due", due)] {
        let Some(value) = value else { continue };
        if value.is_empty() {
            if sprint.shift_remove(key).is_some() {
                changed.push(key.to_string());
            }
        } else if sprint.get(key).and_then(Value::as_str) != Some(value) {
            sprint.insert(key.into(), json!(value));
            changed.push(key.to_string());
        }
    }
    Ok(changed)
}

/// Whether a task is in no sprint; a finished one only with `finished`.
fn in_backlog(vocab: &Vocabulary, task: &Task, finished: bool) -> bool {
    !task.get("sprint").is_some_and(truthy)
        && (finished || !is_finished(vocab.kind_of(task.get("status"))))
}

/// The sprints with their counts, the backlog's size, and whether the project has sprints at all.
#[derive(Debug, Clone)]
pub struct SprintList {
    /// The sprints in order (the closed ones only when asked), with their task counts.
    pub sprints: Vec<Value>,
    /// How many unfinished tasks are in no sprint.
    pub backlog: usize,
    /// Whether the project has any sprint, closed ones included.
    pub has_sprints: bool,
}

/// `atm sprint list`: the sprints in order with their task counts (tasks.json and the archive).
///
/// # Errors
/// A task file that can't be read.
pub fn list_sprints(project: &Project, include_closed: bool) -> Result<SprintList> {
    let store = project.store();
    let vocab = store.vocabulary()?;
    let mut tasks = store.list_tasks()?;
    let current = tasks.len();
    tasks.extend(store.list_archived()?);
    Ok(SprintList {
        sprints: sprint_overview(vocab, &tasks, include_closed),
        backlog: tasks[..current]
            .iter()
            .filter(|t| in_backlog(vocab, t, false))
            .count(),
        has_sprints: !vocab.sprints.is_empty(),
    })
}

/// `atm sprint backlog`: the tasks in no sprint (the finished ones too with `finished`), with the vocabularies.
///
/// # Errors
/// A task file that can't be read.
pub fn backlog_tasks(project: &Project, finished: bool) -> Result<(Vec<Task>, Vocabulary)> {
    let store = project.store();
    let vocab = store.vocabulary()?;
    let tasks = store
        .list_tasks()?
        .into_iter()
        .filter(|t| in_backlog(vocab, t, finished))
        .cloned()
        .collect();
    Ok((tasks, vocab.clone()))
}

fn sprints_mut(document: &mut Value) -> &mut Vec<Value> {
    if !document.get("meta").is_some_and(Value::is_object) {
        document["meta"] = json!({});
    }
    let meta = &mut document["meta"];
    if !meta.get("sprints").is_some_and(Value::is_array) {
        meta["sprints"] = json!([]);
    }
    meta["sprints"]
        .as_array_mut()
        .expect("made a list just above")
}

/// The position in `meta.sprints` of the sprint called `name`, or an error naming the sprints.
fn stored_sprint(document: &Value, vocab: &Vocabulary, name: &str) -> Result<usize> {
    let Some(found) = vocab.find_sprint(Some(&json!(name))) else {
        let ids: Vec<String> = vocab.sprints.iter().map(sprint_id).collect();
        let known = if ids.is_empty() {
            "none yet: atm sprint add".to_string()
        } else {
            ids.join(", ")
        };
        return Err(not_found(format!(
            "no sprint {}; the sprints: {known}",
            repr_str(name)
        )));
    };
    let id = sprint_id(found);
    document["meta"]["sprints"]
        .as_array()
        .and_then(|list| list.iter().position(|s| id_of(s) == Some(id.as_str())))
        .ok_or_else(|| not_found(format!("no sprint {}", repr_str(name))))
}

fn sprint_at(document: &mut Value, at: usize) -> &mut Map<String, Value> {
    let entry = &mut sprints_mut(document)[at];
    if !entry.is_object() {
        *entry = json!({});
    }
    entry.as_object_mut().expect("made an object just above")
}

/// A name for a new sprint: the one given (trimmed), else a generated one; refused when taken or too long.
fn checked_name(
    vocab: &Vocabulary,
    given: Option<&str>,
    taken: impl FnOnce() -> Vec<String>,
) -> Result<String> {
    let name = match given.map(atm_common::py::strip).filter(|n| !n.is_empty()) {
        Some(name) => name.to_string(),
        None => new_name(vocab, &taken()),
    };
    if let Some(existing) = vocab.find_sprint(Some(&json!(name))) {
        return Err(invalid(format!(
            "there is already a sprint {}",
            py_repr(&json!(sprint_id(existing)))
        )));
    }
    if name.chars().count() > MAX_SPRINT_NAME {
        return Err(invalid(format!(
            "a sprint name is at most {MAX_SPRINT_NAME} characters"
        )));
    }
    Ok(name)
}

/// `atm sprint add`: a planned sprint, named `name` (generated without one), after the sprint `after` (default: last),
/// with its goal, release and due date. Returns the sprint.
///
/// # Errors
/// A name taken or too long, an unknown release or `after`, a bad date.
pub fn add_sprint(
    project: &mut Project,
    name: Option<&str>,
    goal: Option<&str>,
    release: Option<&str>,
    due: Option<&str>,
    after: Option<&str>,
) -> Result<Value> {
    let mut added = Value::Null;
    project.store_mut().change(&mut |work: &mut Working| {
        let vocab = Vocabulary::of_document(&work.document)?;
        let archived = work.archive()?["tasks"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let sprint_name = checked_name(&vocab, name, || taken_names(&work.document, &archived))?;
        let mut sprint = Map::new();
        sprint.insert("id".into(), json!(sprint_name));
        sprint.insert("state".into(), json!("planned"));
        apply_fields(&mut sprint, &vocab, goal, release, due)?;
        let sprints = sprints_mut(&mut work.document);
        let mut position = sprints.len();
        if let Some(after) = after.filter(|a| !a.is_empty()) {
            let Some(previous) = vocab.find_sprint(Some(&json!(after))) else {
                return Err(not_found(format!(
                    "no sprint {} to add it after",
                    repr_str(after)
                )));
            };
            let previous = sprint_id(previous);
            position = sprints
                .iter()
                .position(|s| id_of(s) == Some(previous.as_str()))
                .map_or(position, |i| i + 1);
        }
        sprints.insert(position, Value::Object(sprint.clone()));
        Vocabulary::of_document(&work.document)?;
        added = Value::Object(sprint);
        Ok(())
    })?;
    Ok(added)
}

/// `atm sprint edit`: a sprint's goal, release or due date (`None` leaves one, an empty value removes it). Returns the
/// sprint and the fields changed.
///
/// # Errors
/// An unknown sprint or release, a bad date.
pub fn edit_sprint(
    project: &mut Project,
    name: &str,
    goal: Option<&str>,
    release: Option<&str>,
    due: Option<&str>,
) -> Result<(Value, Vec<String>)> {
    let mut outcome = (Value::Null, Vec::new());
    project.store_mut().change(&mut |work: &mut Working| {
        let vocab = Vocabulary::of_document(&work.document)?;
        let at = stored_sprint(&work.document, &vocab, name)?;
        let mut sprint = sprint_at(&mut work.document, at).clone();
        let changed = apply_fields(&mut sprint, &vocab, goal, release, due)?;
        if !changed.is_empty() {
            sprint_at(&mut work.document, at).clone_from(&sprint);
            Vocabulary::of_document(&work.document)?;
        }
        outcome = (Value::Object(sprint), changed);
        Ok(())
    })?;
    Ok(outcome)
}

/// `atm sprint start`: make a planned sprint the active one (one at a time). Returns the sprint.
///
/// # Errors
/// An unknown sprint, a closed or active one, another sprint active.
pub fn start_sprint(project: &mut Project, name: &str) -> Result<Value> {
    let mut started = Value::Null;
    project.store_mut().change(&mut |work: &mut Working| {
        let vocab = Vocabulary::of_document(&work.document)?;
        let at = stored_sprint(&work.document, &vocab, name)?;
        let sprint = sprint_at(&mut work.document, at);
        let id = json!(sprint_id(sprint));
        match sprint
            .get("state")
            .and_then(Value::as_str)
            .unwrap_or("planned")
        {
            "closed" => {
                return Err(invalid(format!(
                    "the sprint {} is closed; closed sprints aren't started again",
                    py_repr(&id)
                )));
            }
            "active" => {
                return Err(invalid(format!(
                    "the sprint {} is already active",
                    py_repr(&id)
                )));
            }
            _ => {}
        }
        if let Some(active) = vocab
            .sprints
            .iter()
            .find(|s| s.get("state").and_then(Value::as_str) == Some("active"))
        {
            return Err(invalid(format!(
                "the sprint {} is active; close it first (one active sprint at a time)",
                py_repr(&json!(sprint_id(active)))
            )));
        }
        sprint.insert("state".into(), json!("active"));
        sprint.insert("start".into(), json!(now_iso()));
        started = Value::Object(sprint.clone());
        Ok(())
    })?;
    Ok(started)
}

/// Whether the task is in the sprint (names compare case-insensitively).
fn in_sprint(task: &Value, id: &str) -> bool {
    task.get("sprint")
        .and_then(Value::as_str)
        .is_some_and(|s| s.to_lowercase() == id.to_lowercase())
}

/// The task's history entries for the sprint.
fn history_in<'t>(task: &'t Value, id: &str) -> Vec<&'t Map<String, Value>> {
    task.get("sprint_history")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
        .filter(|e| {
            e.get("sprint")
                .and_then(Value::as_str)
                .is_some_and(|s| s.to_lowercase() == id.to_lowercase())
        })
        .collect()
}

/// Whether the task joined the sprint after it started: its last history entry for it begins after the start (SR-2.5).
fn added_mid_sprint(task: &Value, sprint: &Map<String, Value>) -> bool {
    let entries = history_in(task, &sprint_id(sprint));
    let start = sprint
        .get("start")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty());
    match (entries.last(), start) {
        (Some(last), Some(start)) => last
            .get("from")
            .and_then(Value::as_str)
            .is_some_and(|from| from > start),
        _ => false,
    }
}

/// What a sprint's close did: the sprint (with its report), the sprint created for the carried tasks, where they went
/// (`None`: the backlog), and whether it was the active one.
#[derive(Debug, Clone)]
pub struct Closed {
    /// The sprint, closed, with its report.
    pub sprint: Value,
    /// The new sprint the unfinished tasks went to, with `--carry-to new`.
    pub created: Option<Value>,
    /// Where the unfinished tasks went; none for the backlog.
    pub target: Option<String>,
    /// Whether the sprint was the active one.
    pub was_active: bool,
}

fn task_ids_of(tasks: &[usize], document: &Value) -> Vec<String> {
    tasks
        .iter()
        .map(|i| {
            document["tasks"][*i]
                .get("id")
                .map(py_str)
                .unwrap_or_default()
        })
        .collect()
}

fn indices_in_sprint(document: &Value, id: &str) -> Vec<usize> {
    document["tasks"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
        .filter(|(_, t)| in_sprint(t, id))
        .map(|(i, _)| i)
        .collect()
}

fn move_task(
    document: &mut Value,
    at: usize,
    target: Option<&str>,
    actor: Option<&str>,
    stamp: &str,
    always_touch: bool,
) -> bool {
    let Some(task) = document["tasks"][at].as_object_mut() else {
        return false;
    };
    let moved = set_sprint(task, target, stamp);
    if moved || always_touch {
        touch(task, actor, stamp);
    }
    moved
}

/// `atm sprint close` (SR-4.3): the unfinished tasks go to `carry_to` (a sprint, `new`, named `new_name`, or
/// `backlog`), keeping their status and history (SR-4.5); the sprint gets `end` and its report {done, carried, added,
/// to}; with `reviewed` its batch tasks are marked reviewed (SR-4.4).
///
/// # Errors
/// An unknown or closed sprint, a task in it in progress, batch tasks not reviewed, unfinished tasks and no
/// `carry_to`, a target that can't take them.
#[allow(clippy::too_many_lines)] // reason: one step per rule, in the order the Python close_sprint checks them
pub fn close_sprint(
    project: &mut Project,
    name: &str,
    carry_to: Option<&str>,
    new_sprint: Option<&str>,
    reviewed: bool,
) -> Result<Closed> {
    let actor = project.actor().map(String::from);
    let mut outcome: Option<Closed> = None;
    project.store_mut().change(&mut |work: &mut Working| {
        let vocab = Vocabulary::of_document(&work.document)?;
        let archived = work.archive()?["tasks"].as_array().cloned().unwrap_or_default();
        let at = stored_sprint(&work.document, &vocab, name)?;
        let sprint = sprint_at(&mut work.document, at).clone();
        let id = sprint_id(&sprint);
        if sprint.get("state").and_then(Value::as_str) == Some("closed") {
            return Err(invalid(format!("the sprint {} is already closed", repr_str(&id))));
        }
        let document = &work.document;
        let kind = |t: &Value| vocab.kind_of(t.get("status"));
        let own = indices_in_sprint(document, &id);
        let own_archived: Vec<&Value> = archived.iter().filter(|t| in_sprint(t, &id)).collect();
        let task = |i: &usize| &document["tasks"][*i];
        let claimed: Vec<usize> = own.iter().copied().filter(|i| kind(task(i)) == Some(Kind::Active)).collect();
        if !claimed.is_empty() {
            let n = claimed.len();
            return Err(invalid(format!(
                "{} {} in progress: finish or release {} before closing the sprint",
                task_ids_of(&claimed, document).join(", "),
                one_or_many(n, "is", "are"),
                one_or_many(n, "it", "them")
            )));
        }
        // Batch tasks wait for the end-of-sprint review (SR-4.4): /atm-close-sprint reviews them, then --reviewed
        let pending: Vec<usize> = own
            .iter()
            .copied()
            .filter(|i| task(i).get("sprint_review").and_then(Value::as_str) == Some("pending"))
            .collect();
        if !pending.is_empty() && !reviewed {
            let n = pending.len();
            return Err(invalid(format!(
                "{} {} for the end-of-sprint review: review {} with the user (/atm-close-sprint), then close with --reviewed",
                task_ids_of(&pending, document).join(", "),
                one_or_many(n, "waits", "wait"),
                one_or_many(n, "it", "them")
            )));
        }
        let unfinished: Vec<usize> = own.iter().copied().filter(|i| !is_finished(kind(task(i)))).collect();
        let carry_to = carry_to.filter(|c| !c.is_empty());
        if !unfinished.is_empty() && carry_to.is_none() {
            let n = unfinished.len();
            return Err(invalid(format!(
                "{} {}n't finished: say where {} with --carry-to <sprint>, new or backlog",
                task_ids_of(&unfinished, document).join(", "),
                one_or_many(n, "is", "are"),
                one_or_many(n, "it goes", "they go")
            )));
        }
        let mut report = Map::new();
        let done: Vec<String> = own
            .iter()
            .map(task)
            .chain(own_archived.iter().copied())
            .filter(|t| kind(t) == Some(Kind::Done))
            .map(|t| t.get("id").map(py_str).unwrap_or_default())
            .collect();
        let added: Vec<String> = own
            .iter()
            .map(task)
            .chain(own_archived.iter().copied())
            .filter(|t| added_mid_sprint(t, &sprint))
            .map(|t| t.get("id").map(py_str).unwrap_or_default())
            .collect();
        report.insert("done".into(), json!(done));
        report.insert("carried".into(), json!(task_ids_of(&unfinished, document)));
        report.insert("added".into(), json!(added));
        let (mut target, mut created) = (None, None);
        if let Some(to) = carry_to.filter(|_| !unfinished.is_empty()) {
            if new_sprint.is_some_and(|n| !n.is_empty()) && to != "new" {
                return Err(invalid("--name names a new sprint: use it with --carry-to new"));
            }
            match to {
                "backlog" => {}
                "new" => {
                    let taken = taken_names(document, &archived);
                    let fresh = checked_name(&vocab, new_sprint, || taken)?;
                    let mut sprint = Map::new();
                    sprint.insert("id".into(), json!(fresh));
                    sprint.insert("state".into(), json!("planned"));
                    sprints_mut(&mut work.document).insert(at + 1, Value::Object(sprint.clone()));
                    target = Some(fresh);
                    created = Some(Value::Object(sprint));
                }
                other => {
                    let to = open_sprint(&vocab, &json!(other))?;
                    if to == id {
                        return Err(invalid(format!(
                            "the tasks can't be carried to the sprint that is closing ({})",
                            repr_str(&id)
                        )));
                    }
                    target = Some(to);
                }
            }
            report.insert("to".into(), json!(target.clone().unwrap_or_else(|| "backlog".into())));
        }
        let stamp = now_iso();
        for i in &unfinished {
            move_task(&mut work.document, *i, target.as_deref(), actor.as_deref(), &stamp, true);
        }
        for i in &pending {
            if let Some(task) = work.document["tasks"][*i].as_object_mut() {
                task.insert("sprint_review".into(), json!("reviewed"));
                touch(task, actor.as_deref(), &stamp);
            }
        }
        // The new sprint, if any, came after the closing one: its position didn't change
        let stored = sprint_at(&mut work.document, at);
        let was_active = stored.get("state").and_then(Value::as_str) == Some("active");
        stored.insert("state".into(), json!("closed"));
        stored.insert("end".into(), json!(stamp));
        stored.insert("report".into(), Value::Object(report));
        let closed = Value::Object(stored.clone());
        Vocabulary::of_document(&work.document)?;
        outcome = Some(Closed { sprint: closed, created, target, was_active });
        Ok(())
    })?;
    outcome.ok_or_else(|| not_found(format!("no sprint {}", repr_str(name))))
}

/// What a sprint's deletion did: the sprint, its state, the tasks moved and where (`None`: the backlog).
#[derive(Debug, Clone)]
pub struct Deleted {
    /// The deleted sprint.
    pub sprint: Value,
    /// Its state when it was deleted.
    pub state: String,
    /// The tasks moved out of it.
    pub moved: Vec<String>,
    /// Where they went; none for the backlog.
    pub target: Option<String>,
}

/// `atm sprint delete` (T-339): a sprint created by mistake. An empty one (no task in it, none ever) goes at once,
/// whatever its state; an open one's unfinished tasks go to `move_to` (the backlog by default); one that tasks finished
/// in or moved on from needs `force` when open, and a closed one with any history stays (its record and report).
///
/// # Errors
/// An unknown sprint, a task in it in progress, a history without `force` (or a closed sprint's), a target that can't
/// take its tasks.
pub fn delete_sprint(
    project: &mut Project,
    name: &str,
    move_to: Option<&str>,
    force: bool,
) -> Result<Deleted> {
    let actor = project.actor().map(String::from);
    let mut outcome: Option<Deleted> = None;
    project.store_mut().change(&mut |work: &mut Working| {
        let vocab = Vocabulary::of_document(&work.document)?;
        let archived = work.archive()?["tasks"].as_array().cloned().unwrap_or_default();
        let at = stored_sprint(&work.document, &vocab, name)?;
        let sprint = sprint_at(&mut work.document, at).clone();
        let id = sprint_id(&sprint);
        let state = sprint.get("state").and_then(Value::as_str).unwrap_or("planned").to_string();
        let document = &work.document;
        let kind = |t: &Value| vocab.kind_of(t.get("status"));
        let task = |i: &usize| &document["tasks"][*i];
        let own = indices_in_sprint(document, &id);
        let claimed: Vec<usize> = own.iter().copied().filter(|i| kind(task(i)) == Some(Kind::Active)).collect();
        if !claimed.is_empty() {
            let n = claimed.len();
            return Err(invalid(format!(
                "{} {} in progress: finish or release {} before deleting the sprint",
                task_ids_of(&claimed, document).join(", "),
                one_or_many(n, "is", "are"),
                one_or_many(n, "it", "them")
            )));
        }
        let unfinished: Vec<usize> = own.iter().copied().filter(|i| !is_finished(kind(task(i)))).collect();
        // Its history: tasks that finished in it (archived ones too), or were in it and moved on
        let mut history: Vec<String> =
            task_ids_of(&own.iter().copied().filter(|i| !unfinished.contains(i)).collect::<Vec<_>>(), document);
        let id_text = |t: &Value| t.get("id").map_or_else(|| "None".into(), py_str);
        history.extend(archived.iter().filter(|t| t.is_object() && in_sprint(t, &id)).map(id_text));
        let all = document["tasks"].as_array().into_iter().flatten().chain(&archived);
        history.extend(
            all.filter(|t| t.is_object() && !in_sprint(t, &id) && !history_in(t, &id).is_empty())
                .map(id_text),
        );
        if !history.is_empty() && state == "closed" {
            return Err(invalid(format!(
                "the sprint {} is closed and had tasks ({}): its record and report stay; only an empty sprint can be deleted",
                repr_str(&id),
                history.join(", ")
            )));
        }
        if !history.is_empty() && !force {
            return Err(invalid(format!(
                "tasks finished in the sprint {} or moved on from it ({}): delete it with --force to drop that record (its tasks go to --move-to, the backlog by default)",
                repr_str(&id),
                history.join(", ")
            )));
        }
        let to = move_to.filter(|m| !m.is_empty()).unwrap_or("backlog");
        let target = if to == "backlog" { None } else { Some(open_sprint(&vocab, &json!(to))?) };
        if target.as_deref().is_some_and(|t| t.to_lowercase() == id.to_lowercase()) {
            return Err(invalid(format!(
                "the tasks can't move to the sprint being deleted ({})",
                repr_str(&id)
            )));
        }
        // With --force, the finished tasks still in it move too, so no task names a sprint that is gone
        let moving = if force { own.clone() } else { unfinished };
        let moved = task_ids_of(&moving, document);
        let stamp = now_iso();
        for i in &moving {
            move_task(&mut work.document, *i, target.as_deref(), actor.as_deref(), &stamp, true);
        }
        sprints_mut(&mut work.document).remove(at);
        Vocabulary::of_document(&work.document)?;
        outcome = Some(Deleted { sprint: Value::Object(sprint), state, moved, target });
        Ok(())
    })?;
    outcome.ok_or_else(|| not_found(format!("no sprint {}", repr_str(name))))
}

/// The sprint (`None`, the backlog) `atm sprint move --to` means for the task.
fn move_target(vocab: &Vocabulary, task: &Task, to: &str) -> Result<Option<String>> {
    match to {
        "backlog" => Ok(None),
        "active" => vocab
            .sprints
            .iter()
            .find(|s| s.get("state").and_then(Value::as_str) == Some("active"))
            .map(|s| Some(sprint_id(s)))
            .ok_or_else(|| invalid("no sprint is active (atm sprint start <name>)")),
        "next" => {
            // The first planned sprint after the task's own (for a task in the backlog or a closed sprint: the first
            // planned sprint)
            let current = vocab.find_sprint(task.get("sprint"));
            let start = current
                .and_then(|c| {
                    vocab
                        .sprints
                        .iter()
                        .position(|s| sprint_id(s) == sprint_id(c))
                })
                .map_or(0, |i| i + 1);
            vocab.sprints[start.min(vocab.sprints.len())..]
                .iter()
                .find(|s| s.get("state").and_then(Value::as_str) == Some("planned"))
                .map(|s| Some(sprint_id(s)))
                .ok_or_else(|| {
                    invalid(format!(
                        "{}: no planned sprint after {}; add one with atm sprint add [\"<name>\"]",
                        task.get("id").map(py_str).unwrap_or_default(),
                        current.map_or_else(|| "the active one".to_string(), sprint_id)
                    ))
                })
        }
        other => open_sprint(vocab, &json!(other)).map(Some),
    }
}

/// `atm sprint move`: to-do, blocked or in-review tasks into a sprint, the next planned one, the active one, or the
/// backlog (claimed and finished tasks stay where they are), all or none. Returns `[{id, from, to}]`.
///
/// # Errors
/// An unknown task, one in progress or finished, a target that can't take it.
pub fn move_tasks(project: &mut Project, ids: &[String], to: &str) -> Result<Vec<Value>> {
    let actor = project.actor().map(String::from);
    let mut moved = Vec::new();
    project.store_mut().change(&mut |work: &mut Working| {
        let vocab = Vocabulary::of_document(&work.document)?;
        let mut plans: Vec<(usize, Option<String>)> = Vec::new();
        for task_id in ids {
            let at = work.document["tasks"]
                .as_array()
                .and_then(|list| list.iter().position(|t| id_of(t) == Some(task_id.as_str())));
            let Some(at) = at else {
                let archived = work.archive()?["tasks"]
                    .as_array()
                    .is_some_and(|list| list.iter().any(|t| id_of(t) == Some(task_id.as_str())));
                let hint = if archived {
                    " (it is archived: atm db unarchive brings it back)"
                } else {
                    ""
                };
                return Err(Error::failed(format!("no task {task_id}{hint}")));
            };
            let task = work.document["tasks"][at]
                .as_object()
                .cloned()
                .unwrap_or_default();
            match vocab.kind_of(task.get("status")) {
                Some(Kind::Active) => {
                    return Err(invalid(format!(
                        "{task_id} is in progress (claimed): it stays where the work is happening"
                    )));
                }
                Some(Kind::Done | Kind::Skipped) => {
                    return Err(invalid(format!(
                        "{task_id} is {}: finished work stays in the sprint it was done in",
                        atm_common::py::py_str_opt(task.get("status"))
                    )));
                }
                _ => {}
            }
            plans.push((at, move_target(&vocab, &task, to)?));
        }
        let stamp = now_iso();
        moved = plans
            .into_iter()
            .map(|(at, target)| {
                let before = work.document["tasks"][at]
                    .get("sprint")
                    .cloned()
                    .unwrap_or(Value::Null);
                move_task(
                    &mut work.document,
                    at,
                    target.as_deref(),
                    actor.as_deref(),
                    &stamp,
                    false,
                );
                let id = work.document["tasks"][at]
                    .get("id")
                    .cloned()
                    .unwrap_or(Value::Null);
                json!({"id": id, "from": before, "to": target})
            })
            .collect();
        Ok(())
    })?;
    Ok(moved)
}

#[cfg(test)]
mod tests;
