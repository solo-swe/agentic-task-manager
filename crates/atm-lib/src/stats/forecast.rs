//! When the work will be done (docs/dashboard-v2-requirements.md §3, SR-7.4): the agents' pace from their recent work,
//! and each sprint's tasks counted, the work left accumulating in the loop's order (the active sprint, the planned
//! ones in order, the backlog), as `/atm-loop-tasks --all` takes it.

use std::collections::HashMap;

use atm_common::{Kind, Vocabulary};
use serde_json::{Value, json};

use super::Item;

/// A gap between completions longer than this is idle time, not work.
pub const GAP_CAP_MINUTES: f64 = 30.0;
/// The pace is the last this many timed completions'.
pub const HISTORY: usize = 10;
/// Below this many timed completions there is no pace.
pub const MIN_HISTORY: usize = 3;

const MINUTE: f64 = 60_000.0;

/// Minutes per task: the middle of the recent ones, and the quartiles around it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pace {
    pub likely: f64,
    pub low: f64,
    pub high: f64,
    pub based_on: usize,
}

/// A sprint's tasks (or the backlog's: `sprint` none), counted, and when they may be done.
#[derive(Debug, Clone, PartialEq)]
pub struct SprintRow {
    pub sprint: Option<String>,
    /// All but the skipped ones.
    pub total: usize,
    pub done: usize,
    pub skipped: usize,
    /// Agent work left on them (atm's `agent_work`).
    pub remaining: usize,
    pub blocked: usize,
    /// In review and waiting for the reviewer, not back with the agent.
    pub in_review: usize,
    /// `done` (every task finished), `in_progress` (something moved) or `not_started`.
    pub state: &'static str,
    /// The last completion, once every task is finished.
    pub completed_at: Option<i64>,
    /// Working minutes until it is done, and the time then, from now: (likely, low, high) each.
    pub estimate: Option<([f64; 3], [i64; 3])>,
}

/// The pace and the rows.
#[derive(Debug, Clone, PartialEq)]
pub struct Forecast {
    pub pace: Option<Pace>,
    pub rows: Vec<SprintRow>,
}

/// Each done task's minutes of work (atm's, every attempt and round), oldest first, as (task, completed at, minutes):
/// for one without work, the gap since the completion before it, capped; the first such has nothing to measure from.
pub(super) fn durations(history: &[&Item]) -> Vec<(String, i64, f64)> {
    let mut finished: Vec<(&Item, i64)> = history
        .iter()
        .filter(|i| i.kind == Some(Kind::Done))
        .filter_map(|i| i.completed_at().map(|end| (*i, end)))
        .collect();
    finished.sort_by_key(|(_, end)| *end);
    let mut out = Vec::new();
    for (n, (item, end)) in finished.iter().enumerate() {
        #[allow(clippy::cast_precision_loss)]
        // reason: milliseconds between two times, far below 2^52
        let minutes = item.work_minutes().or_else(|| {
            (n > 0).then(|| ((end - finished[n - 1].1) as f64 / MINUTE).min(GAP_CAP_MINUTES))
        });
        if let Some(minutes) = minutes {
            out.push((item.id.clone(), *end, minutes));
        }
    }
    out
}

/// Linear interpolation between the closest ranks (`p` in 0..1) of sorted values.
fn percentile(sorted: &[f64], p: f64) -> f64 {
    #[allow(clippy::cast_precision_loss)] // reason: a count of tasks
    let rank = p * (sorted.len() - 1) as f64;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // reason: 0 ≤ rank < len
    let (lo, hi) = (rank.floor() as usize, rank.ceil() as usize);
    #[allow(clippy::cast_precision_loss)] // reason: a small index
    let part = rank - lo as f64;
    sorted[lo] + (sorted[hi] - sorted[lo]) * part
}

/// The pace of the last [`HISTORY`] durations; none with fewer than [`MIN_HISTORY`].
#[must_use]
pub fn pace(minutes: &[f64]) -> Option<Pace> {
    let recent = &minutes[minutes.len().saturating_sub(HISTORY)..];
    if recent.len() < MIN_HISTORY {
        return None;
    }
    let mut sorted = recent.to_vec();
    sorted.sort_by(f64::total_cmp);
    Some(Pace {
        likely: percentile(&sorted, 0.5),
        low: percentile(&sorted, 0.25),
        high: percentile(&sorted, 0.75),
        based_on: recent.len(),
    })
}

/// Where the loop takes a row's work: the active sprint first, the planned ones, the backlog; a closed one none.
fn loop_rank(row: &SprintRow, state_of: &HashMap<String, String>) -> Option<u8> {
    match row.sprint.as_ref() {
        None => Some(2),
        Some(id) => match state_of.get(id).map(String::as_str) {
            Some("active") => Some(0),
            Some("closed") => None,
            _ => Some(1),
        },
    }
}

fn sprint_states(vocab: &Vocabulary) -> Vec<(String, String)> {
    vocab
        .sprints
        .iter()
        .filter_map(|s| {
            let id = s.get("id").and_then(Value::as_str)?.to_string();
            let state = s
                .get("state")
                .and_then(Value::as_str)
                .unwrap_or("planned")
                .to_string();
            Some((id, state))
        })
        .collect()
}

/// The rows of `tasks`, with estimates from `history`'s pace: the closed sprints in the project's order, then the
/// active one, the planned ones and the backlog, each only when it has tasks. Tasks done with the user leave the pace
/// out (their time includes waiting on them).
#[must_use]
pub(crate) fn forecast(
    tasks: &[&Item],
    history: &[&Item],
    vocab: &Vocabulary,
    now: i64,
) -> Forecast {
    let together: std::collections::HashSet<&str> = history
        .iter()
        .filter(|i| i.with_user)
        .map(|i| i.id.as_str())
        .collect();
    let minutes: Vec<f64> = durations(history)
        .into_iter()
        .filter(|(id, _, _)| !together.contains(id.as_str()))
        .map(|(_, _, m)| m)
        .collect();
    let pace = pace(&minutes);
    let states = sprint_states(vocab);
    let state_of: HashMap<String, String> = states.iter().cloned().collect();
    let mut groups: HashMap<Option<String>, Vec<&Item>> = HashMap::new();
    for item in tasks {
        let key = item.sprint.clone().filter(|s| state_of.contains_key(s));
        groups.entry(key).or_default().push(item);
    }
    let of_state = |wanted: &str| {
        states
            .iter()
            .filter(move |(_, state)| state == wanted)
            .map(|(id, _)| Some(id.clone()))
            .collect::<Vec<_>>()
    };
    let keys: Vec<Option<String>> = [
        of_state("closed"),
        of_state("active"),
        of_state("planned"),
        vec![None],
    ]
    .concat()
    .into_iter()
    .filter(|k| groups.contains_key(k))
    .collect();
    let mut rows: Vec<SprintRow> = keys
        .into_iter()
        .map(|key| row(key.clone(), &groups[&key]))
        .collect();
    if let Some(pace) = pace {
        let mut order: Vec<usize> = (0..rows.len())
            .filter(|&n| rows[n].state != "done" && loop_rank(&rows[n], &state_of).is_some())
            .collect();
        order.sort_by_key(|&n| loop_rank(&rows[n], &state_of));
        let mut cumulative = 0usize;
        for n in order {
            cumulative += rows[n].remaining;
            if rows[n].remaining == 0 {
                continue;
            }
            #[allow(clippy::cast_precision_loss)] // reason: a count of tasks
            let left = cumulative as f64;
            let minutes = [left * pace.likely, left * pace.low, left * pace.high];
            #[allow(clippy::cast_possible_truncation)]
            // reason: minutes of work as milliseconds, far below 2^63
            let finish = minutes.map(|m| now + (m * MINUTE) as i64);
            rows[n].estimate = Some((minutes, finish));
        }
    }
    Forecast { pace, rows }
}

fn row(sprint: Option<String>, own: &[&Item]) -> SprintRow {
    let count = |f: &dyn Fn(&Item) -> bool| own.iter().filter(|i| f(i)).count();
    let done = count(&|i| i.kind == Some(Kind::Done));
    let skipped = count(&|i| i.kind == Some(Kind::Skipped));
    let finished = own
        .iter()
        .all(|i| matches!(i.kind, Some(Kind::Done | Kind::Skipped)));
    // Started once something moved: done, active, in review or a status atm doesn't know. Blocked work hasn't begun
    let started = own
        .iter()
        .any(|i| !matches!(i.kind, Some(Kind::Todo | Kind::Skipped | Kind::Blocked)));
    SprintRow {
        sprint,
        total: own.len() - skipped,
        done,
        skipped,
        remaining: count(&|i| i.agent_work),
        blocked: count(&|i| i.kind == Some(Kind::Blocked)),
        in_review: count(&|i| i.kind == Some(Kind::Review) && !i.agent_work),
        state: if finished {
            "done"
        } else if started {
            "in_progress"
        } else {
            "not_started"
        },
        completed_at: finished
            .then(|| {
                own.iter()
                    .filter(|i| i.kind == Some(Kind::Done))
                    .filter_map(|i| i.completed_at())
                    .max()
            })
            .flatten(),
        estimate: None,
    }
}

impl Forecast {
    /// A slice's rows with the whole project's estimates (T-239): the loop works through whole sprints, so when one
    /// finishes doesn't depend on the slice. A sprint the slice has finished keeps none.
    #[must_use]
    pub fn with_estimates_of(mut self, whole: &Forecast) -> Forecast {
        for row in &mut self.rows {
            row.estimate = if row.state == "done" {
                None
            } else {
                whole
                    .rows
                    .iter()
                    .find(|w| w.sprint == row.sprint)
                    .and_then(|w| w.estimate)
            };
        }
        self.pace = whole.pace;
        self
    }

    /// The row the loop works on first: the first unfinished one in its order.
    #[must_use]
    pub fn next_in_loop(&self, vocab: &Vocabulary) -> Option<&SprintRow> {
        let state_of: HashMap<String, String> = sprint_states(vocab).into_iter().collect();
        self.rows
            .iter()
            .filter(|r| r.state != "done")
            .filter_map(|r| loop_rank(r, &state_of).map(|rank| (rank, r)))
            .min_by_key(|(rank, _)| *rank)
            .map(|(_, r)| r)
    }

    /// As the stats route gives it: `{pace, sprints: [{sprint, total, done, skipped, remaining, blocked, in_review,
    /// state, completed_at?, estimate?: {minutes, finish_at}}]}`, the backlog's `sprint` null.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let rows: Vec<Value> = self
            .rows
            .iter()
            .map(|r| {
                let mut row = json!({
                    "sprint": r.sprint, "total": r.total, "done": r.done, "skipped": r.skipped,
                    "remaining": r.remaining, "blocked": r.blocked, "in_review": r.in_review, "state": r.state,
                });
                if let Some(at) = r.completed_at {
                    row["completed_at"] = json!(at);
                }
                if let Some(([likely, low, high], [at_likely, at_low, at_high])) = r.estimate {
                    row["estimate"] = json!({
                        "minutes": {"likely": likely, "low": low, "high": high},
                        "finish_at": {"likely": at_likely, "low": at_low, "high": at_high},
                    });
                }
                row
            })
            .collect();
        json!({
            "pace": self.pace.map(|p| json!({"likely": p.likely, "low": p.low, "high": p.high, "based_on": p.based_on})),
            "sprints": rows,
        })
    }
}
