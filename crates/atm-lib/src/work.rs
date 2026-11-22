//! A task's work by one rule (ATM-721, docs/dashboard-derived-state.md group D, decision 4): every attempt and every
//! round counts. Its work is its ended attempts (`attempts`) and its current one (from `started_at`), less the time it
//! waited: its review rounds (`reviews`, and the round open now) and its blocks (`blocks`, and the block now). The
//! task views carry it (`derived.work`), and the summary's agent time is made of it.

use atm_common::py::{py_str, truthy};
use atm_common::time::{format_iso, parse_iso};
use atm_common::{Kind, Task, Vocabulary};
use serde_json::{Value, json};

/// A stretch of work: (start, end) in seconds since 1970, and who did it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub from: i64,
    pub to: i64,
    pub by: Option<String>,
}

/// A task's work: its spans in time order, and whether it is going on now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Work {
    pub spans: Vec<Span>,
    /// An agent is at it now (the task is active): its last span runs to now.
    pub live: bool,
}

impl Work {
    /// The work's length in seconds.
    #[must_use]
    pub fn seconds(&self) -> i64 {
        self.spans.iter().map(|s| s.to - s.from).sum()
    }

    /// As the task views give it: `{minutes, live, spans: [{from, to, by?}]}`.
    #[must_use]
    pub fn to_json(&self) -> Value {
        #[allow(clippy::cast_precision_loss)] // reason: seconds of work, far below 2^52
        let minutes = self.seconds() as f64 / 60.0;
        let spans: Vec<Value> = self
            .spans
            .iter()
            .map(|s| {
                let mut span = json!({"from": format_iso(s.from), "to": format_iso(s.to)});
                if let Some(by) = &s.by {
                    span["by"] = json!(by);
                }
                span
            })
            .collect();
        json!({"minutes": minutes, "live": self.live, "spans": spans})
    }
}

fn time(value: Option<&Value>) -> Option<i64> {
    value.and_then(Value::as_str).and_then(parse_iso)
}

/// `(from, to)` intervals of the task's list `field` (`attempts`, `reviews`, `blocks`), with each one's `by`.
fn intervals(task: &Task, field: &str) -> Vec<(i64, i64, Option<String>)> {
    task.get(field)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|i| {
            let (from, to) = (time(i.get("from"))?, time(i.get("to"))?);
            let by = i.get("by").filter(|v| truthy(v)).map(py_str);
            (to >= from).then_some((from, to, by))
        })
        .collect()
}

/// Who started the current attempt: `started_by`, or `agent` in files written before v4 (DM-4.4).
fn starter(task: &Task) -> Option<String> {
    task.get("started_by")
        .or_else(|| task.get("agent"))
        .filter(|v| truthy(v))
        .map(py_str)
}

/// The current attempt's stretch, (start, end), by its kind: to now while active; to the review's request or the
/// block's start while it waits; to its completion once done (to its review's request in a file from before rounds
/// were kept); none while it isn't started (to do, skipped).
fn current(vocab: &Vocabulary, task: &Task, now: i64) -> Option<(i64, i64)> {
    let start = time(task.get("started_at"))?;
    let end = match vocab.kind_of(task.get("status"))? {
        Kind::Active => now,
        Kind::Review => {
            time(task.get("review_requested_at")).or_else(|| time(task.get("updated_at")))?
        }
        Kind::Blocked => time(task.get("blocked_at")).or_else(|| time(task.get("updated_at")))?,
        Kind::Done => {
            let rounds_kept = task.get("reviews").is_some_and(truthy);
            let requested = time(task.get("review_requested_at")).filter(|_| !rounds_kept);
            requested
                .or_else(|| time(task.get("completed_at")))
                .or_else(|| time(task.get("updated_at")))
                .unwrap_or(start)
        }
        Kind::Todo | Kind::Skipped => return None,
    };
    // An end before the start is a time that can't be: no work to count from it
    (end >= start).then_some((start, end))
}

/// `(from, to)` less every waiting interval in `waits` (sorted by start): the pieces left, in order.
fn less(from: i64, to: i64, waits: &[(i64, i64)]) -> Vec<(i64, i64)> {
    let mut pieces = Vec::new();
    let mut at = from;
    for &(w_from, w_to) in waits {
        if w_to <= at || w_from >= to {
            continue;
        }
        if w_from > at {
            pieces.push((at, w_from));
        }
        at = at.max(w_to);
        if at >= to {
            break;
        }
    }
    if at < to {
        pieces.push((at, to));
    }
    pieces
}

/// The task's work at `now`, or none when it holds none (never started, or started and gone back without a trace). A
/// finished task without a known end has a zero-length span at its start.
#[must_use]
pub fn work_of(vocab: &Vocabulary, task: &Task, now: i64) -> Option<Work> {
    let mut stretches: Vec<(i64, i64, Option<String>)> = intervals(task, "attempts");
    if let Some((from, to)) = current(vocab, task, now) {
        stretches.push((from, to, starter(task)));
    }
    if stretches.is_empty() {
        return None;
    }
    let mut waits: Vec<(i64, i64)> = intervals(task, "reviews")
        .into_iter()
        .chain(intervals(task, "blocks"))
        .map(|(f, t, _)| (f, t))
        .collect();
    waits.sort_unstable();
    stretches.sort_by_key(|(from, _, _)| *from);
    let mut spans = Vec::new();
    for (from, to, by) in stretches {
        let pieces = less(from, to, &waits);
        if pieces.is_empty() && from == to {
            // A finished attempt with no known length: a zero-length span marks when it was
            spans.push(Span {
                from,
                to,
                by: by.clone(),
            });
        }
        spans.extend(pieces.into_iter().map(|(f, t)| Span {
            from: f,
            to: t,
            by: by.clone(),
        }));
    }
    let live = vocab.kind_of(task.get("status")) == Some(Kind::Active);
    Some(Work { spans, live })
}

#[cfg(test)]
mod tests;
