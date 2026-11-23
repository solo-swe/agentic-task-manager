//! How far along the tasks are: overall, by status, by type and by sprint. Archived tasks count as what they are (an
//! archived skipped task is skipped, not done: the dashboard's old count took every archived task for done).

use std::collections::BTreeMap;

use atm_common::{Kind, Vocabulary};
use serde_json::{Value, json};

use super::Item;

/// The status split's order: by kind, finished first; statuses without a kind last.
const KIND_ORDER: [Kind; 6] = [
    Kind::Done,
    Kind::Active,
    Kind::Review,
    Kind::Todo,
    Kind::Blocked,
    Kind::Skipped,
];

/// A group's completion, as atm counts it: done of all but the skipped; a group of only skipped tasks is finished.
fn share(key: &Value, label: &str, items: &[&Item]) -> Value {
    let done = items.iter().filter(|i| i.kind == Some(Kind::Done)).count();
    let skipped = items
        .iter()
        .filter(|i| i.kind == Some(Kind::Skipped))
        .count();
    let total = items.len() - skipped;
    #[allow(clippy::cast_precision_loss)] // reason: counts of tasks
    let ratio = if total > 0 {
        done as f64 / total as f64
    } else if skipped > 0 {
        1.0
    } else {
        0.0
    };
    json!({"key": key, "label": label, "done": done, "total": total, "skipped": skipped, "ratio": ratio})
}

/// `{overall, archived, by_status: [{status, count}], by_type: [share], by_sprint: [share]}`: the sprints in the
/// project's order, then the backlog (`key` null), each only with tasks.
pub(super) fn progress(items: &[&Item], vocab: &Vocabulary) -> Value {
    let archived = items.iter().filter(|i| i.archived).count();
    let mut by_status: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_type: BTreeMap<String, Vec<&Item>> = BTreeMap::new();
    for item in items {
        let status = item
            .task
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or_default();
        *by_status.entry(status.to_string()).or_default() += 1;
        let kind = item
            .task
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        by_type.entry(kind.to_string()).or_default().push(item);
    }
    let rank = |status: &str| {
        vocab
            .kind_of(Some(&json!(status)))
            .and_then(|k| KIND_ORDER.iter().position(|o| *o == k))
            .unwrap_or(KIND_ORDER.len())
    };
    let mut statuses: Vec<(String, usize)> = by_status.into_iter().collect();
    statuses.sort_by(|a, b| rank(&a.0).cmp(&rank(&b.0)).then_with(|| a.0.cmp(&b.0)));
    let sprint_ids: Vec<String> = vocab
        .sprints
        .iter()
        .filter_map(|s| s.get("id").and_then(Value::as_str).map(String::from))
        .collect();
    let mut by_sprint: Vec<Value> = sprint_ids
        .iter()
        .filter_map(|id| {
            let own: Vec<&Item> = items
                .iter()
                .copied()
                .filter(|i| i.sprint.as_deref() == Some(id.as_str()))
                .collect();
            (!own.is_empty()).then(|| share(&json!(id), id, &own))
        })
        .collect();
    let backlog: Vec<&Item> = items
        .iter()
        .copied()
        .filter(|i| i.sprint.as_ref().is_none_or(|s| !sprint_ids.contains(s)))
        .collect();
    if !backlog.is_empty() {
        by_sprint.push(share(&Value::Null, "Backlog", &backlog));
    }
    json!({
        "overall": share(&json!("all"), "All tasks", items),
        "archived": archived,
        "by_status": statuses.into_iter().map(|(status, count)| json!({"status": status, "count": count})).collect::<Vec<_>>(),
        "by_type": by_type.iter().map(|(t, own)| share(&json!(t), t, own)).collect::<Vec<_>>(),
        "by_sprint": by_sprint,
    })
}
