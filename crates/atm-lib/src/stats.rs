//! A project's stats, progress and forecast (ATM-722, docs/dashboard-derived-state.md group F and decision 2): what the
//! Progress tab, the Progress widget and the Project tab show, by atm's rules, for the whole project or a slice of it
//! (the tasks a view's search and filters keep) and a period. The dashboard draws them; the CLI prints the forecast.
//!
//! Every figure reads what atm derives for a task ([`crate::query::task_views`]: its kind, sprint, whether agent work
//! is left, who it waits on) and its work by one rule ([`crate::work`]), working and archived tasks alike. Times are
//! milliseconds since 1970, as the page counts them.

use std::collections::BTreeSet;

use atm_common::{Kind, Result, Task, Vocabulary};
use serde_json::{Value, json};

use crate::project::Project;
use crate::query::{Filters, task_views};

mod forecast;
mod intervals;
mod people;
mod progress;
mod time;

pub use forecast::{Forecast, Pace, SprintRow};
pub use intervals::Interval;

/// What is asked: the slice (the tasks a view keeps; none, the whole project) and the period time use is over (none:
/// from the first work to now), at `now`.
#[derive(Debug, Clone, Default)]
pub struct StatsQuery {
    /// The IDs (or keys) of the slice's tasks.
    pub tasks: Option<BTreeSet<String>>,
    /// The period's start and end, in ms.
    pub from: Option<i64>,
    pub to: Option<i64>,
}

/// A task as the figures read it, once: what atm derives for it, and its work in milliseconds.
#[derive(Debug, Clone)]
pub(crate) struct Item {
    pub task: Task,
    pub id: String,
    pub key: Option<String>,
    pub kind: Option<Kind>,
    pub sprint: Option<String>,
    /// Agent work is left on it (atm's `agent_work`); a task atm says nothing of counts as work left.
    pub agent_work: bool,
    /// Who it waits on now (`waiting.on`): `reviewer`, `user`, `agent`, `dependencies`.
    pub waiting_on: Option<String>,
    /// Done together with the user (`with: user`): its time includes waiting on them.
    pub with_user: bool,
    /// From the archive.
    pub archived: bool,
    /// Its work's stretches, in ms, with who did each; while an agent is at it, the last runs to now.
    pub spans: Vec<(i64, i64, Option<String>)>,
}

impl Item {
    fn of(view: &Value, vocab: &Vocabulary, now: i64) -> Option<Self> {
        let task = view.get("task")?.as_object()?.clone();
        let derived = view.get("derived");
        let text = |key: &str| derived.and_then(|d| d.get(key)).and_then(Value::as_str);
        let id = task.get("id").and_then(Value::as_str)?.to_string();
        let work = crate::work::work_of(vocab, &task, now / 1000);
        Some(Self {
            key: task.get("key").and_then(Value::as_str).map(String::from),
            kind: vocab.kind_of(task.get("status")),
            sprint: text("sprint").map(String::from),
            agent_work: derived
                .and_then(|d| d.get("agent_work"))
                .and_then(Value::as_bool)
                .unwrap_or(true),
            waiting_on: derived
                .and_then(|d| d.get("waiting"))
                .and_then(|w| w.get("on"))
                .and_then(Value::as_str)
                .map(String::from),
            with_user: task.get("with").and_then(Value::as_str) == Some("user"),
            archived: derived
                .and_then(|d| d.get("archived"))
                .and_then(Value::as_bool)
                .unwrap_or(false),
            spans: work
                .map(|w| {
                    w.spans
                        .into_iter()
                        .map(|s| (s.from * 1000, s.to * 1000, s.by))
                        .collect()
                })
                .unwrap_or_default(),
            id,
            task,
        })
    }

    /// A time field of the task, in ms.
    pub fn time(&self, field: &str) -> Option<i64> {
        self.task
            .get(field)
            .and_then(Value::as_str)
            .and_then(atm_common::time::parse_iso)
            .map(|s| s * 1000)
    }

    /// When a done task was finished: `completed_at`, or `updated_at` in files from before it.
    pub fn completed_at(&self) -> Option<i64> {
        self.time("completed_at")
            .or_else(|| self.time("updated_at"))
    }

    /// Its work in minutes; none when it holds none.
    #[allow(clippy::cast_precision_loss)] // reason: milliseconds of work, far below 2^52
    pub fn work_minutes(&self) -> Option<f64> {
        (!self.spans.is_empty())
            .then(|| self.spans.iter().map(|s| s.1 - s.0).sum::<i64>() as f64 / 60_000.0)
    }

    /// Who started it: `started_by`, or `agent` in files from before v4.
    pub fn started_by(&self) -> Option<&str> {
        atm_common::started_by(&self.task).and_then(Value::as_str)
    }

    fn named(&self, slice: &BTreeSet<String>) -> bool {
        slice.contains(&self.id) || self.key.as_ref().is_some_and(|k| slice.contains(k))
    }
}

/// Every task of the project, working and archived, as the figures read them.
fn items(project: &Project, vocab: &Vocabulary, now: i64) -> Result<Vec<Item>> {
    let working = task_views(project, &Filters::default())?;
    let archived = task_views(
        project,
        &Filters {
            archived: true,
            ..Filters::default()
        },
    )?;
    Ok(working
        .iter()
        .chain(&archived)
        .filter_map(|v| Item::of(v, vocab, now))
        .collect())
}

const DAY_MS: i64 = 86_400_000;

/// What the overview's Progress tab shows of a project (ATM-722), in its summary, so the overview loads no project's
/// tasks for it: `{done, total}` (all but the skipped, the archive too), `open_releases` (`[{id, name?, done, total}]`),
/// `done_days` (`[[day, count]]`, days since 1970 in UTC, the days with completions), `burndown` (`[[day, scope,
/// remaining]]`, at the end of each day from the first task's to today) and `forecast` (`{minutes: {likely, low, high},
/// waiting}`: the agent work left at the agents' pace, and the open tasks waiting on someone; null without a pace).
///
/// # Errors
/// Task data that can't be read.
pub fn overview(project: &Project, now: i64) -> Result<Value> {
    let planned = crate::sprints::planned_vocabulary(project)?;
    let vocab = &planned;
    let all = items(project, vocab, now)?;
    let counted: Vec<&Item> = all
        .iter()
        .filter(|i| i.kind != Some(Kind::Skipped))
        .collect();
    let done = counted
        .iter()
        .filter(|i| i.kind == Some(Kind::Done))
        .count();
    let open_releases: Vec<Value> = crate::releases::release_views(project)?
        .into_iter()
        .filter(|r| r.get("state").and_then(Value::as_str) == Some("planned"))
        .map(|r| json!({"id": r.get("id"), "name": r.get("name"), "done": r.get("done"), "total": r.get("total")}))
        .collect();
    let finished: Vec<i64> = counted
        .iter()
        .filter(|i| i.kind == Some(Kind::Done))
        .filter_map(|i| i.completed_at())
        .collect();
    let mut by_day: std::collections::BTreeMap<i64, usize> = std::collections::BTreeMap::new();
    for at in &finished {
        *by_day.entry(at.div_euclid(DAY_MS)).or_default() += 1;
    }
    // Scope by creation (a task without its time counts from the first time known), less what was done by then
    let earliest = all
        .iter()
        .flat_map(|i| ["created_at", "updated_at", "started_at", "completed_at"].map(|f| i.time(f)))
        .flatten()
        .min();
    let burndown: Vec<Value> = earliest.map_or_else(Vec::new, |first| {
        let created: Vec<i64> = counted
            .iter()
            .map(|i| i.time("created_at").unwrap_or(first))
            .collect();
        (first.div_euclid(DAY_MS)..=now.div_euclid(DAY_MS))
            .map(|day| {
                let end = (day + 1) * DAY_MS;
                let scope = created.iter().filter(|c| **c < end).count();
                let done = finished.iter().filter(|f| **f < end).count();
                json!([day, scope, scope.saturating_sub(done)])
            })
            .collect()
    });
    let refs: Vec<&Item> = all.iter().collect();
    let whole = forecast::forecast(&refs, &refs, vocab, now);
    let waiting = counted
        .iter()
        .filter(|i| i.kind != Some(Kind::Done) && !i.agent_work)
        .count();
    let forecast = whole.pace.map(|pace| {
        let left = counted
            .iter()
            .filter(|i| i.kind != Some(Kind::Done) && i.agent_work)
            .count();
        #[allow(clippy::cast_precision_loss)] // reason: a count of tasks
        let left = left as f64;
        json!({
            "minutes": {"likely": left * pace.likely, "low": left * pace.low, "high": left * pace.high},
            "waiting": waiting,
        })
    });
    Ok(json!({
        "done": done,
        "total": counted.len(),
        "open_releases": open_releases,
        "done_days": by_day.into_iter().map(|(d, n)| json!([d, n])).collect::<Vec<_>>(),
        "burndown": burndown,
        "forecast": forecast,
    }))
}

/// The project's stats, progress and forecast, for `query`'s slice and period, at `now` (ms): `{progress, forecast,
/// time_use}` of the slice, and `{stats, histogram, actors}` of the whole project (they describe it, not the slice).
///
/// # Errors
/// Task data that can't be read.
pub fn project_stats(project: &Project, query: &StatsQuery, now: i64) -> Result<Value> {
    // An unmigrated project's phases are its sprints, as the sprint views count them
    let planned = crate::sprints::planned_vocabulary(project)?;
    let vocab = &planned;
    let all = items(project, vocab, now)?;
    let slice: Vec<&Item> = match &query.tasks {
        Some(named) => all.iter().filter(|i| i.named(named)).collect(),
        None => all.iter().collect(),
    };
    let history: Vec<&Item> = all.iter().collect();
    let period = match (query.from, query.to) {
        (None, None) => None,
        (from, to) => Some(Interval {
            start: from.unwrap_or(i64::MIN),
            end: to.unwrap_or(now),
        }),
    };
    let meta = project
        .store()
        .document()?
        .get("meta")
        .cloned()
        .unwrap_or(Value::Null);
    let whole = forecast::forecast(&history, &history, vocab, now);
    // The row the loop works on next, by the whole project's (`{sprint}`, its sprint or null for the backlog; none when
    // nothing is left)
    let next = whole
        .next_in_loop(vocab)
        .map_or(Value::Null, |r| json!({"sprint": r.sprint}));
    let forecast = if query.tasks.is_some() {
        forecast::forecast(&slice, &history, vocab, now).with_estimates_of(&whole)
    } else {
        whole
    };
    let mut served = forecast.to_json();
    served["next"] = next;
    Ok(json!({
        "progress": progress::progress(&slice, vocab),
        "forecast": served,
        "time_use": time::time_use(&slice, period, now).map(|u| u.to_json()),
        "stats": time::project_numbers(&history, &meta, vocab, now),
        "histogram": time::histogram(&history),
        "actors": people::actors(&history, now),
    }))
}

#[cfg(test)]
mod tests;
