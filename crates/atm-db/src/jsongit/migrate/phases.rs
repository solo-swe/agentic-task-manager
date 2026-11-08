//! Phases become sprints (SR-8.1): each phase a sprint named after it, closed when all its tasks are finished,
//! planned otherwise, and every task with a phase put in its sprint.

use atm_common::py::{strip, truthy};
use atm_common::vocabulary::MAX_SPRINT_NAME;
use atm_common::{Kind, Vocabulary};
use serde_json::{Map, Value, json};

/// A task's phase (a number, not a boolean), or none: a task without one goes to the backlog.
fn task_phase(phase: Option<&Value>) -> Option<f64> {
    match phase? {
        Value::Number(n) => n.as_f64(),
        _ => None,
    }
}

/// A phase as Python prints it: `2` for a whole number, `1.5` otherwise.
fn phase_text(phase: f64) -> String {
    if phase.fract() == 0.0 && phase.abs() < 1e15 {
        #[allow(clippy::cast_possible_truncation)] // reason: a whole number well inside i64
        let whole = phase as i64;
        whole.to_string()
    } else {
        format!("{phase}")
    }
}

/// The first `n` characters of `text`.
fn chars(text: &str, n: usize) -> String {
    text.chars().take(n).collect()
}

/// `(phase, sprint name)` for every phase with tasks or a name, in phase order: its name, or "Phase n" when unnamed,
/// with " (phase n)" added to a name already taken (the names the dashboard shows for unmigrated projects).
fn phase_sprint_names(
    meta: &Map<String, Value>,
    tasks: &[&Map<String, Value>],
) -> Vec<(f64, String)> {
    let mut named: Vec<(f64, String)> = Vec::new();
    for p in meta
        .get("phases")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
    {
        let Some(phase) = task_phase(p.get("id")) else {
            continue;
        };
        let name = p
            .get("name")
            .and_then(Value::as_str)
            .map(|n| chars(strip(n), MAX_SPRINT_NAME).trim_end().to_string())
            .unwrap_or_default();
        if !name.is_empty() {
            named.retain(|(q, _)| !same(*q, phase));
            named.push((phase, name));
        }
    }
    let mut phases: Vec<f64> = named.iter().map(|(p, _)| *p).collect();
    phases.extend(tasks.iter().filter_map(|t| task_phase(t.get("phase"))));
    phases.sort_by(f64::total_cmp);
    phases.dedup();
    let mut result = Vec::new();
    let mut taken: Vec<String> = Vec::new();
    for phase in phases {
        let mut name = named.iter().find(|(p, _)| same(*p, phase)).map_or_else(
            || format!("Phase {}", phase_text(phase)),
            |(_, n)| n.clone(),
        );
        if taken.contains(&name.to_lowercase()) {
            let suffix = format!(" (phase {})", phase_text(phase));
            let room = MAX_SPRINT_NAME.saturating_sub(suffix.chars().count());
            name = chars(&name, room).trim_end().to_string() + &suffix;
        }
        taken.push(name.to_lowercase());
        result.push((phase, name));
    }
    result
}

/// Whether two phases are the same phase (`1` and `1.0` are).
fn same(a: f64, b: f64) -> bool {
    a.total_cmp(&b).is_eq()
}

fn finished(vocab: &Vocabulary, task: &Map<String, Value>) -> bool {
    matches!(
        vocab.kind_of(task.get("status")),
        Some(Kind::Done | Kind::Skipped)
    )
}

/// The sprint a phase becomes: closed when all its tasks are finished (start, end and report from them), else
/// planned (SR-8.1).
fn sprint_from_phase(name: &str, tasks: &[&Map<String, Value>], vocab: &Vocabulary) -> Value {
    if tasks.is_empty() || !tasks.iter().all(|t| finished(vocab, t)) {
        return json!({"id": name, "state": "planned"});
    }
    let mut sprint = Map::new();
    sprint.insert("id".into(), json!(name));
    sprint.insert("state".into(), json!("closed"));
    let starts: Vec<&str> = tasks
        .iter()
        .filter_map(|t| t.get("started_at").and_then(Value::as_str))
        .collect();
    let ends: Vec<&str> = tasks
        .iter()
        .filter_map(|t| {
            ["completed_at", "skipped_at", "updated_at"]
                .iter()
                .find_map(|k| t.get(*k).filter(|v| truthy(v)))
                .and_then(Value::as_str)
        })
        .collect();
    if let Some(start) = starts.iter().min() {
        sprint.insert("start".into(), json!(start));
    }
    if let Some(end) = ends.iter().max() {
        sprint.insert("end".into(), json!(end));
    }
    let done: Vec<Value> = tasks
        .iter()
        .filter(|t| vocab.kind_of(t.get("status")) == Some(Kind::Done))
        .filter_map(|t| t.get("id").cloned())
        .collect();
    sprint.insert("report".into(), json!({"done": done, "carried": []}));
    Value::Object(sprint)
}

/// Turn phases into sprints in `tasks.json`'s document and the archive's tasks, in place (SR-8.1). A project that
/// already has `meta.sprints` only gets its leftover phases converted (a task merged in from an unmigrated branch,
/// SR-8.5). The sprints it added, or none when there was nothing to convert.
pub fn phases_to_sprints(
    document: &mut Value,
    archived: &mut [Value],
    vocab: &Vocabulary,
    stamp: &str,
) -> Vec<Value> {
    if !document.get("meta").is_some_and(Value::is_object) {
        document["meta"] = json!({});
    }
    let names = {
        let meta = document["meta"].as_object().cloned().unwrap_or_default();
        let live = document["tasks"].as_array().cloned().unwrap_or_default();
        let all: Vec<&Map<String, Value>> = live
            .iter()
            .chain(archived.iter())
            .filter_map(Value::as_object)
            .collect();
        if !meta.get("phases").is_some_and(truthy) && !all.iter().any(|t| t.contains_key("phase")) {
            return Vec::new();
        }
        let names = phase_sprint_names(&meta, &all);
        let mut sprints: Vec<Value> = meta
            .get("sprints")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let id_of = |s: &Value| s.get("id").and_then(Value::as_str).map(str::to_lowercase);
        let existing: Vec<String> = sprints.iter().filter_map(id_of).collect();
        let mut added = Vec::new();
        for (phase, name) in &names {
            if existing.contains(&name.to_lowercase()) {
                continue;
            }
            let of_phase: Vec<&Map<String, Value>> = all
                .iter()
                .copied()
                .filter(|t| task_phase(t.get("phase")).is_some_and(|p| same(p, *phase)))
                .collect();
            let sprint = sprint_from_phase(name, &of_phase, vocab);
            sprints.push(sprint.clone());
            added.push(sprint);
        }
        let by_name: Vec<(String, String)> = sprints
            .iter()
            .filter_map(|s| s.get("id").and_then(Value::as_str))
            .map(|id| (id.to_lowercase(), id.to_string()))
            .collect();
        if let Some(meta) = document["meta"].as_object_mut() {
            meta.insert("sprints".into(), Value::Array(sprints));
            meta.shift_remove("phases");
        }
        (names, by_name, added)
    };
    let (names, by_name, added) = names;
    let sprint_of = |phase: f64| -> Option<String> {
        let name = &names.iter().find(|(p, _)| same(*p, phase))?.1;
        by_name
            .iter()
            .find(|(lower, _)| *lower == name.to_lowercase())
            .map(|(_, id)| id.clone())
    };
    let mut move_task = |task: &mut Value| {
        let Some(task) = task.as_object_mut() else {
            return;
        };
        let phase = task_phase(task.get("phase"));
        task.shift_remove("phase");
        let Some(phase) = phase else { return };
        if task.get("sprint").is_some_and(truthy) {
            return;
        }
        let Some(sprint) = sprint_of(phase) else {
            return;
        };
        let from = task
            .get("created_at")
            .and_then(Value::as_str)
            .unwrap_or(stamp)
            .to_string();
        task.insert("sprint".into(), json!(sprint));
        let history = task.entry("sprint_history").or_insert_with(|| json!([]));
        if let Some(history) = history.as_array_mut() {
            history.push(json!({"sprint": sprint, "from": from}));
        }
    };
    if let Some(tasks) = document["tasks"].as_array_mut() {
        tasks.iter_mut().for_each(&mut move_task);
    }
    archived.iter_mut().for_each(&mut move_task);
    added
}

/// What the phase migration did, said once (it doesn't run again).
#[must_use]
pub fn describe_sprints_from_phases(sprints: &[Value]) -> Vec<String> {
    if sprints.is_empty() {
        return vec!["Moved the tasks' phases to their sprints (staged, not committed).".into()];
    }
    let ids = |state: &str| -> Vec<String> {
        sprints
            .iter()
            .filter(|s| s["state"] == state)
            .map(|s| atm_common::py::py_str(&s["id"]))
            .collect()
    };
    let (closed, planned) = (ids("closed"), ids("planned"));
    let count = if sprints.len() == 1 {
        "1 phase".to_string()
    } else {
        format!("{} phases", sprints.len())
    };
    let mut lines = vec![format!(
        "Phases are now sprints: turned {count} into sprints (staged, not committed)."
    )];
    if !closed.is_empty() {
        lines.push(format!(
            "Closed (all their tasks finished): {}.",
            closed.join(", ")
        ));
    }
    if !planned.is_empty() {
        lines.push(format!("Planned: {}.", planned.join(", ")));
        lines.push("None is active yet: plan one with /atm:plan-sprint.".into());
    }
    lines
}
