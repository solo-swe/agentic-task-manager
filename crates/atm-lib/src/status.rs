//! `atm project status`: the overview (the Python atm's `lib/query.py` `status_report`): the active sprint, the counts,
//! what is in progress, in review, blocked or waiting, the next task by the loop's rule, the files' sizes and hints.

use atm_common::archive::{archive_hint, open_in_closed_sprints};
use atm_common::py::{dumps, py_str, py_str_opt, truthy};
use atm_common::rules::unfinished_dependencies;
use atm_common::{Kind, Result, Task, Vocabulary};
use serde_json::{Map, Value, json};

use crate::loop_rules::{Selection, next_task};
use crate::project::Project;
use crate::query::{Filters, Scope};

/// What `project status` reports: `report` is what `--json` prints; `has_sprints` says whether the project has any.
#[derive(Debug, Clone, PartialEq)]
pub struct StatusReport {
    /// The report.
    pub report: Value,
    /// Whether the project has sprints (the text then says so, and counts by sprint).
    pub has_sprints: bool,
}

fn id_of(task: &Task) -> Option<&str> {
    task.get("id").and_then(Value::as_str)
}

fn name_of(task: &Task) -> Value {
    task.get("name").cloned().unwrap_or_else(|| json!(""))
}

/// Counts of each value a task has for `key`, by `str(value)`, in the order they come.
fn counts(tasks: &[&Task], key: impl Fn(&Task) -> Vec<String>) -> Map<String, Value> {
    let mut found: Map<String, Value> = Map::new();
    for t in tasks {
        for value in key(t) {
            let n = found.get(&value).and_then(Value::as_u64).unwrap_or(0);
            found.insert(value, json!(n + 1));
        }
    }
    found
}

fn active_sprint(vocab: &Vocabulary, tasks: &[&Task]) -> Value {
    let Some(active) = vocab
        .sprints
        .iter()
        .find(|s| s.get("state").and_then(Value::as_str) == Some("active"))
    else {
        return Value::Null;
    };
    let id = active.get("id").and_then(Value::as_str);
    let in_active: Vec<&&Task> = tasks
        .iter()
        .filter(|t| {
            vocab
                .find_sprint(t.get("sprint"))
                .and_then(|s| s.get("id"))
                .and_then(Value::as_str)
                == id
        })
        .collect();
    let mut sprint = Map::new();
    for key in ["id", "goal", "due", "release", "start"] {
        if let Some(value) = active.get(key).filter(|v| truthy(v)) {
            sprint.insert(key.into(), value.clone());
        }
    }
    let kind = |t: &Task| vocab.kind_of(t.get("status"));
    sprint.insert(
        "done".into(),
        json!(
            in_active
                .iter()
                .filter(|t| kind(t) == Some(Kind::Done))
                .count()
        ),
    );
    sprint.insert(
        "tasks".into(),
        json!(
            in_active
                .iter()
                .filter(|t| kind(t) != Some(Kind::Skipped))
                .count()
        ),
    );
    // Batch tasks waiting for the end-of-sprint review: they wait on the user (SR-4.4)
    let pending: Vec<Value> = in_active
        .iter()
        .filter(|t| t.get("sprint_review").and_then(Value::as_str) == Some("pending"))
        .map(|t| t.get("id").cloned().unwrap_or(Value::Null))
        .collect();
    sprint.insert("review_pending".into(), Value::Array(pending));
    Value::Object(sprint)
}

/// A list field's entries as Python iterates `field or []`: a list's elements, a text's characters.
fn entries(field: Option<&Value>) -> Vec<String> {
    match field.filter(|v| truthy(v)) {
        Some(Value::Array(items)) => items.iter().map(py_str).collect(),
        Some(Value::String(text)) => text.chars().map(String::from).collect(),
        _ => Vec::new(),
    }
}

/// `atm project status`: the overview of the tasks in the scope (`Filters` with sprint, backlog, release, component).
///
/// # Errors
/// An unknown sprint or release in the scope (exit code 2); task data that can't be read.
#[allow(clippy::too_many_lines)] // reason: one block per part of the report, as the Python atm's status_report has them
pub fn status_report(project: &Project, scope: &Filters) -> Result<StatusReport> {
    let store = project.store();
    let vocab = store.vocabulary()?;
    let document = store.document()?;
    let all = store.list_tasks()?;
    let claims = store.read_claims();
    let (file_bytes, archive_bytes) = store.sizes();
    let archived = |id: &str| {
        store
            .get_archived_task(id)
            .map(|t| t.is_some_and(|t| !t.is_empty()))
    };
    let in_scope = Scope::new(vocab, scope)?;
    let tasks: Vec<&Task> = all
        .iter()
        .copied()
        .filter(|t| in_scope.keeps(vocab, t))
        .collect();
    let kind = |t: &Task| vocab.kind_of(t.get("status"));

    let found = counts(&tasks, |t| vec![py_str_opt(t.get("status"))]);
    let mut by_status = Map::new();
    for s in &vocab.statuses {
        if let Some(n) = found.get(&s.id) {
            by_status.insert(s.id.clone(), n.clone());
        }
    }
    for (s, n) in &found {
        if !by_status.contains_key(s) {
            by_status.insert(s.clone(), n.clone());
        }
    }

    let claim_of = |t: &Task| {
        claims
            .get("claims")
            .and_then(|c| id_of(t).and_then(|id| c.get(id)))
    };
    let in_progress: Vec<Value> = tasks
        .iter()
        .filter(|t| kind(t) == Some(Kind::Active))
        .map(|t| {
            let by = claim_of(t)
                .and_then(|c| c.get("agent"))
                .filter(|a| truthy(a))
                .or_else(|| atm_common::started_by(t))
                .cloned()
                .unwrap_or(Value::Null);
            json!({"id": t.get("id"), "name": name_of(t), "by": by})
        })
        .collect();
    let mut waiting = Vec::new();
    for t in tasks.iter().filter(|t| kind(t) == Some(Kind::Todo)) {
        let on = unfinished_dependencies(vocab, &all, t, &archived)?;
        if !on.is_empty() {
            let ids: Vec<String> = on.into_iter().map(|(d, _)| d).collect();
            waiting.push(json!({"id": t.get("id"), "name": name_of(t), "on": ids}));
        }
    }
    let mut blocked: Vec<Value> = tasks
        .iter()
        .filter(|t| kind(t) == Some(Kind::Blocked))
        .map(|t| {
            json!({
                "id": t.get("id"),
                "name": name_of(t),
                "waiting_on": t.get("waiting_on"),
                "questions": t.get("questions").cloned().unwrap_or_else(|| json!([])),
            })
        })
        .collect();
    let mut in_review: Vec<Value> = tasks
        .iter()
        .filter(|t| kind(t) == Some(Kind::Review))
        .map(|t| json!({"id": t.get("id"), "name": name_of(t), "since": t.get("review_requested_at")}))
        .collect();

    // Blocked or in review in another worktree (T-204): this worktree's copy doesn't say so until the branches merge
    let listed: Vec<Value> = blocked
        .iter()
        .chain(&in_review)
        .map(|i| i["id"].clone())
        .collect();
    let here = store
        .working_copy()
        .map_or(Value::Null, |p| Value::String(p.display().to_string()));
    for record in claims
        .get("waiting")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|w| w.values())
    {
        let Some(r) = record.as_object() else {
            continue;
        };
        let task = r.get("task").cloned().unwrap_or(Value::Null);
        if r.get("worktree").unwrap_or(&Value::Null) == &here || listed.contains(&task) {
            continue;
        }
        let name = task
            .as_str()
            .and_then(|id| tasks.iter().rev().find(|t| id_of(t) == Some(id)))
            .map_or_else(|| json!(""), |t| name_of(t));
        let mut item = Map::new();
        item.insert("id".into(), task);
        item.insert("name".into(), name);
        item.insert(
            "branch".into(),
            r.get("branch").cloned().unwrap_or(Value::Null),
        );
        if vocab.kind_of(r.get("status")) == Some(Kind::Review) {
            item.insert("since".into(), r.get("at").cloned().unwrap_or(Value::Null));
            in_review.push(Value::Object(item));
        } else {
            item.insert(
                "waiting_on".into(),
                r.get("waiting_on").cloned().unwrap_or(Value::Null),
            );
            let questions = r
                .get("questions")
                .filter(|q| truthy(q))
                .cloned()
                .unwrap_or_else(|| json!([]));
            item.insert("questions".into(), questions);
            blocked.push(Value::Object(item));
        }
    }

    let (next, reason) = next_task(
        vocab,
        document,
        &all,
        &claims,
        &Selection::default(),
        None,
        false,
        &archived,
    )?;
    // The largest first, by their size as one line of JSON, then by ID (both from the largest down)
    let mut largest: Vec<(usize, String, Value)> = tasks
        .iter()
        .map(|t| {
            let size = dumps(&Value::Object((*t).clone())).chars().count();
            (
                size,
                py_str_opt(t.get("id")),
                t.get("id").cloned().unwrap_or(Value::Null),
            )
        })
        .collect();
    largest.sort_by(|a, b| (b.0, &b.1).cmp(&(a.0, &a.1)));
    let largest: Vec<Value> = largest
        .into_iter()
        .take(3)
        .map(|(bytes, _, id)| json!({"id": id, "bytes": bytes}))
        .collect();

    let sprint_of = |t: &Task| {
        vec![
            vocab
                .find_sprint(t.get("sprint"))
                .and_then(|s| s.get("id"))
                .filter(|v| truthy(v))
                .map_or_else(|| "backlog".into(), py_str),
        ]
    };
    let mut report = json!({
        "sprint": active_sprint(vocab, &all),
        "tasks": tasks.len(),
        "by_status": by_status,
        "by_sprint": counts(&tasks, sprint_of),
        "by_type": counts(&tasks, |t| vec![py_str_opt(t.get("type"))]),
        "by_label": counts(&tasks, |t| entries(t.get("labels"))),
        "in_progress": in_progress,
        "in_review": in_review,
        "blocked": blocked,
        "waiting_on_dependencies": waiting,
        "next": next.map_or(Value::Null, |t| json!({"id": t.get("id"), "name": name_of(t)})),
        "next_reason": reason,
        "file_bytes": file_bytes,
        "largest_tasks": largest,
        "archived": store.list_archived()?.len(),
        "archive_bytes": archive_bytes,
    });
    // Open tasks left in closed sprints (T-330), whatever the scope: the loop's default selection never takes them
    let stranded = open_in_closed_sprints(vocab, document);
    if !stranded.is_empty() {
        report["open_in_closed_sprints"] = Value::Array(stranded);
    }
    if let Some(hint) = archive_hint(document, vocab, file_bytes) {
        report["suggest"] = json!({"archive": hint});
    }
    Ok(StatusReport {
        report,
        has_sprints: !vocab.sprints.is_empty(),
    })
}

#[cfg(test)]
mod tests;
