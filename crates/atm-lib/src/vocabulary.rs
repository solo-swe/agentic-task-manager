//! A project's vocabulary as atm resolved it, for the dashboard (ATM-718): what `meta` defines with the defaults for
//! the rest, each list as atm uses it, so the dashboard never parses `meta` again. A tolerant read has left out what
//! doesn't work already, and says so in its problems.

use atm_common::vocabulary::{Term, Vocabulary, workflow_of};
use serde_json::{Map, Value, json};

/// `{statuses, types, priorities, default_priority, labels, components, workflow, review, phases}`:
/// - `statuses`: `{id, label, kind}` in order; `types` and `priorities` (most urgent first): `{id, label}`;
/// - `labels`: `meta.labels` as `{id, label}` (the label its text, else the ID), in their order;
/// - `components`: `{id, label, paths, review?}` in order (the label its ID when it has none);
/// - `workflow`: the project's workflow with its defaults filled in, or null; `review`: the project's review mode;
/// - `phases`: the named phases, `{id, name}`, of a project from before sprints.
#[must_use]
pub fn served(vocab: &Vocabulary, meta: &Map<String, Value>) -> Value {
    let terms = |list: &[Term]| -> Vec<Value> {
        list.iter()
            .map(|t| json!({"id": t.id, "label": t.label}))
            .collect()
    };
    let statuses: Vec<Value> = vocab
        .statuses
        .iter()
        .map(|s| json!({"id": s.id, "label": s.label, "kind": s.kind.as_str()}))
        .collect();
    json!({
        "statuses": statuses,
        "types": terms(&vocab.types),
        "priorities": terms(&vocab.priorities),
        "default_priority": vocab.default_priority,
        "labels": labels(meta),
        "components": components(&vocab.components),
        "workflow": workflow_of(meta).ok().flatten(),
        "review": vocab.review,
        "phases": phases(meta),
    })
}

/// The phases' names (`meta.phases`, `{id, name}` with a number and a text), from before phases were sprints: what an
/// unmigrated project calls them.
fn phases(meta: &Map<String, Value>) -> Vec<Value> {
    meta.get("phases")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|p| {
            let id = p.get("id").and_then(Value::as_i64)?;
            let name = p
                .get("name")
                .and_then(Value::as_str)
                .filter(|n| !n.is_empty())?;
            Some(json!({"id": id, "name": name}))
        })
        .collect()
}

/// `meta.labels` as `{id, label}`: an entry is the label's ID, or `{id, label?}`; one without an ID is left out.
fn labels(meta: &Map<String, Value>) -> Vec<Value> {
    meta.get("labels")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let id = entry
                .as_str()
                .or_else(|| entry.get("id").and_then(Value::as_str))
                .filter(|id| !id.is_empty())?;
            let label = entry
                .get("label")
                .and_then(Value::as_str)
                .filter(|l| !l.is_empty())
                .unwrap_or(id);
            Some(json!({"id": id, "label": label}))
        })
        .collect()
}

/// The components as the dashboard shows them: the label their ID when they have none.
fn components(list: &[Map<String, Value>]) -> Vec<Value> {
    list.iter()
        .filter_map(|c| {
            let id = c.get("id").and_then(Value::as_str).filter(|id| !id.is_empty())?;
            let label = c
                .get("label")
                .and_then(Value::as_str)
                .filter(|l| !l.is_empty())
                .unwrap_or(id);
            let mut out = json!({"id": id, "label": label, "paths": c.get("paths").cloned().unwrap_or(json!([]))});
            if let Some(review) = c.get("review").filter(|r| r.is_string()) {
                out["review"] = review.clone();
            }
            Some(out)
        })
        .collect()
}

#[cfg(test)]
mod tests;
