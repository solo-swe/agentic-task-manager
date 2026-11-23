//! Where the time went (docs/activity-popup-review.md §3.1, §5), the project's numbers (docs/project-stats-proposal.md
//! §4) and the time per task. Work is atm's ([`crate::work`]); agent time adds up parallel agents, each agent's
//! overlapping stretches merged first.

use std::collections::HashMap;

use atm_common::edits::is_agent;
use atm_common::{Kind, Vocabulary};
use serde_json::{Value, json};

use super::Item;
use super::intervals::{Interval, clip, covered, gaps, median, merge, subtract};

/// Shorter pauses (an agent picking up the next task) aren't idle.
pub const MIN_IDLE_MS: i64 = 10 * 60_000;

/// Time split three ways from the first work on: agents working (at least one at work, and the short pauses between
/// tasks), waiting on you (no agent working while a task is in review or blocked on the user), idle (the rest, in
/// stretches of at least [`MIN_IDLE_MS`]).
#[derive(Debug, Clone, PartialEq)]
pub struct TimeUse {
    pub from: i64,
    pub to: i64,
    pub working_ms: i64,
    pub waiting_ms: i64,
    pub idle_ms: i64,
    /// Work time with parallel agents added up.
    pub agent_ms: i64,
    /// The most agents working at the same moment.
    pub max_agents: usize,
    pub waiting: Vec<Interval>,
    pub idle: Vec<Interval>,
}

impl TimeUse {
    /// As the stats route gives it; `waiting` and `idle` as `[start, end]` pairs, for a chart's buckets.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let pairs = |list: &[Interval]| {
            list.iter()
                .map(|i| json!([i.start, i.end]))
                .collect::<Vec<_>>()
        };
        json!({
            "from": self.from, "to": self.to,
            "working_ms": self.working_ms, "waiting_ms": self.waiting_ms, "idle_ms": self.idle_ms,
            "agent_ms": self.agent_ms, "max_agents": self.max_agents,
            "waiting": pairs(&self.waiting), "idle": pairs(&self.idle),
        })
    }
}

/// When the task waited on you: each review round (`reviews`; the one now while atm says it waits on its reviewer or
/// you) and each block on the user (`blocks`; the one now); a done task's review from files before `reviews`.
fn waiting_intervals(item: &Item, now: i64) -> Vec<Interval> {
    let mut out = Vec::new();
    let mut add = |from: Option<i64>, to: Option<i64>| {
        if let (Some(start), Some(end)) = (from, to)
            && end > start
        {
            out.push(Interval { start, end });
        }
    };
    let ms = |v: Option<&Value>| {
        v.and_then(Value::as_str)
            .and_then(atm_common::time::parse_iso)
            .map(|s| s * 1000)
    };
    let list = |field: &str| {
        item.task
            .get(field)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };
    let reviews = list("reviews");
    for r in &reviews {
        add(ms(r.get("from")), ms(r.get("to")));
    }
    for b in list("blocks") {
        if b.get("on").and_then(Value::as_str) == Some("user") {
            add(ms(b.get("from")), ms(b.get("to")));
        }
    }
    let requested = item.task.get("review_requested_at");
    let recorded = reviews.iter().any(|r| r.get("from") == requested);
    let on_you = matches!(item.waiting_on.as_deref(), Some("reviewer" | "user"));
    match item.kind {
        Some(Kind::Review) if on_you => add(ms(requested), Some(now)),
        Some(Kind::Done) if !recorded => add(ms(requested), item.time("completed_at")),
        Some(Kind::Blocked)
            if item.task.get("waiting_on").and_then(Value::as_str) == Some("user") =>
        {
            add(item.time("blocked_at"), Some(now));
        }
        _ => {}
    }
    out
}

/// The work of `items` as (who did it, the stretch): the stretch's own `by`, else who started the task, else nobody.
fn work_of(items: &[&Item]) -> Vec<(String, Interval)> {
    items
        .iter()
        .flat_map(|item| {
            item.spans
                .iter()
                .filter(|s| s.1 > s.0)
                .map(move |(start, end, by)| {
                    let who = by
                        .clone()
                        .or_else(|| item.started_by().map(String::from))
                        .unwrap_or_default();
                    (
                        who,
                        Interval {
                            start: *start,
                            end: *end,
                        },
                    )
                })
        })
        .collect()
}

/// Each agent's stretches merged (an agent on two tasks counts once; a stretch with nobody is its own), and the most at
/// once.
fn agents_at_work(work: &[(String, Interval)]) -> (Vec<Interval>, usize) {
    let mut per_agent: HashMap<String, Vec<Interval>> = HashMap::new();
    for (n, (who, iv)) in work.iter().enumerate() {
        let key = if who.is_empty() {
            format!("\u{0}{n}")
        } else {
            who.clone()
        };
        per_agent.entry(key).or_default().push(*iv);
    }
    let spans: Vec<Interval> = per_agent.values().flat_map(|l| merge(l)).collect();
    let mut edges: Vec<(i64, i32)> = spans
        .iter()
        .flat_map(|s| [(s.start, 1), (s.end, -1)])
        .collect();
    edges.sort_unstable();
    let (mut current, mut most) = (0i32, 0i32);
    for (_, d) in edges {
        current += d;
        most = most.max(current);
    }
    (spans, usize::try_from(most).unwrap_or(0))
}

/// Where the time went in `period` (none: from the first work to now), or none when no work falls in it.
#[must_use]
pub(super) fn time_use(items: &[&Item], period: Option<Interval>, now: i64) -> Option<TimeUse> {
    let work = work_of(items);
    let first = work.iter().map(|(_, iv)| iv.start).min()?;
    let from = period.map_or(first, |p| p.start.max(first));
    let to = period.map_or(now, |p| p.end.min(now));
    if to <= from {
        return None;
    }
    let stretches: Vec<Interval> = work.iter().map(|(_, iv)| *iv).collect();
    let working = merge(&clip(&stretches, from, to));
    if working.is_empty() {
        return None;
    }
    let all_waiting: Vec<Interval> = items
        .iter()
        .flat_map(|i| waiting_intervals(i, now))
        .collect();
    let waiting = subtract(&merge(&clip(&merge(&all_waiting), from, to)), &working);
    let busy = merge(&[working.clone(), waiting.clone()].concat());
    let idle = gaps(&busy, from, to, MIN_IDLE_MS);
    let clipped: Vec<(String, Interval)> = work
        .iter()
        .flat_map(|(who, iv)| {
            clip(&[*iv], from, to)
                .into_iter()
                .map(move |c| (who.clone(), c))
        })
        .collect();
    let (agent_spans, max_agents) = agents_at_work(&clipped);
    let waiting_ms = covered(&waiting, from, to);
    let idle_ms = covered(&idle, from, to);
    Some(TimeUse {
        from,
        to,
        working_ms: (to - from - waiting_ms - idle_ms).max(0),
        waiting_ms,
        idle_ms,
        agent_ms: agent_spans.iter().map(|s| s.length()).sum(),
        max_agents,
        waiting,
        idle,
    })
}

/// Tasks an agent started, or nobody recorded (before v4): agent work, as the Information's Agents line counts it.
fn agent_tasks<'a>(items: &[&'a Item]) -> Vec<&'a Item> {
    items
        .iter()
        .copied()
        .filter(|i| i.started_by().is_none_or(|by| is_agent(Some(by))))
        .collect()
}

/// The project's numbers over its whole history (T-253): when it started, how long it ran, its tasks, sprints, commits
/// and pull requests, and its agents' time, alone and with you, at most at once and in parallel.
pub(super) fn project_numbers(
    items: &[&Item],
    meta: &Value,
    vocab: &Vocabulary,
    now: i64,
) -> Value {
    let min = |values: Vec<Option<i64>>| values.into_iter().flatten().min();
    let created = min(items.iter().map(|i| i.time("created_at")).collect());
    let meta_created = meta
        .get("created")
        .and_then(Value::as_str)
        .and_then(atm_common::time::parse_iso)
        .map(|s| s * 1000);
    let first_start = min(items.iter().map(|i| i.time("started_at")).collect());
    let finished = |i: &&Item| matches!(i.kind, Some(Kind::Done | Kind::Skipped));
    let open = items.iter().filter(|i| !finished(i)).count();
    let last_done = items
        .iter()
        .filter(|i| i.kind == Some(Kind::Done))
        .filter_map(|i| i.completed_at())
        .max();
    let duration = first_start.map(|from| {
        if open > 0 {
            json!({"from": from, "to": now, "ongoing": true})
        } else {
            json!({"from": from, "to": from.max(last_done.unwrap_or(from)), "ongoing": false})
        }
    });
    let prs: std::collections::HashSet<i64> = items
        .iter()
        .filter_map(|i| {
            i.task
                .get("pr")
                .and_then(|p| p.get("number"))
                .and_then(Value::as_i64)
        })
        .collect();
    let direct = meta
        .get("workflow")
        .and_then(|w| w.get("type"))
        .and_then(Value::as_str)
        == Some("direct");
    let agents = agent_tasks(items);
    let (together, alone): (Vec<&Item>, Vec<&Item>) = agents.iter().partition(|i| i.with_user);
    let use_of = |list: &[&Item]| time_use(list, None, now);
    let all = use_of(&agents);
    let stretches: Vec<Interval> = work_of(&agents).into_iter().map(|(_, iv)| iv).collect();
    let working: i64 = merge(&stretches).iter().map(|i| i.length()).sum();
    let agent_ms = all.as_ref().map_or(0, |u| u.agent_ms);
    #[allow(clippy::cast_precision_loss)] // reason: milliseconds, far below 2^52
    let parallelism = (all.is_some() && working > 0).then(|| agent_ms as f64 / working as f64);
    let count = |kind: Kind| items.iter().filter(|i| i.kind == Some(kind)).count();
    json!({
        "started": created.or(meta_created),
        "duration": duration,
        "tasks": {"total": items.len(), "done": count(Kind::Done), "open": open, "skipped": count(Kind::Skipped)},
        "sprints": {
            "total": vocab.sprints.len(),
            "closed": vocab.sprints.iter().filter(|s| s.get("state").and_then(Value::as_str) == Some("closed")).count(),
        },
        "task_commits": items.iter().filter(|i| i.task.get("commit").and_then(Value::as_str).is_some_and(|c| !c.is_empty())).count(),
        "pull_requests": if prs.is_empty() && direct { Value::Null } else { json!(prs.len()) },
        "agent_ms": agent_ms,
        "alone_ms": use_of(&alone).map_or(0, |u| u.agent_ms),
        "with_you_ms": use_of(&together).map_or(0, |u| u.agent_ms),
        "max_agents": all.as_ref().map_or(0, |u| u.max_agents),
        "parallelism": parallelism,
    })
}

/// Bins that widen with time, so short and long tasks both read: 0–5 m … 3–4 h, 4 h+.
pub const TIME_BINS: [u32; 11] = [0, 5, 10, 20, 30, 45, 60, 90, 120, 180, 240];

fn bin_label(m: u32) -> String {
    if m < 60 {
        format!("{m} m")
    } else {
        format!("{} h", f64::from(m) / 60.0)
    }
}

/// Every done task's work time, binned (T-254), alone and with you apart; tasks with no work are left out.
pub(super) fn histogram(items: &[&Item]) -> Value {
    let timed: Vec<(&Item, f64)> = items
        .iter()
        .filter(|i| i.kind == Some(Kind::Done))
        .filter_map(|i| i.work_minutes().map(|m| (*i, m)))
        .collect();
    let task = |(i, m): &(&Item, f64)| json!({"id": i.id, "key": i.key, "name": i.task.get("name"), "minutes": m, "with_you": i.with_user});
    let bins: Vec<Value> = TIME_BINS
        .iter()
        .enumerate()
        .map(|(n, &from)| {
            let to = TIME_BINS.get(n + 1).copied();
            let inside = |m: f64| m >= f64::from(from) && to.is_none_or(|t| m < f64::from(t));
            let own: Vec<&(&Item, f64)> = timed.iter().filter(|(_, m)| inside(*m)).collect();
            let label = to.map_or_else(|| format!("{}+", bin_label(from)), |t| format!("{}–{}", bin_label(from), bin_label(t)));
            json!({
                "from": from, "to": to, "label": label,
                "alone": own.iter().filter(|(i, _)| !i.with_user).map(|t| task(t)).collect::<Vec<_>>(),
                "with_you": own.iter().filter(|(i, _)| i.with_user).map(|t| task(t)).collect::<Vec<_>>(),
            })
        })
        .collect();
    let minutes: Vec<f64> = timed.iter().map(|(_, m)| *m).collect();
    let of = |with: bool| -> Vec<f64> {
        timed
            .iter()
            .filter(|(i, _)| i.with_user == with)
            .map(|(_, m)| *m)
            .collect()
    };
    let mut slowest: Vec<&(&Item, f64)> = timed.iter().collect();
    slowest.sort_by(|a, b| b.1.total_cmp(&a.1));
    #[allow(clippy::cast_precision_loss)] // reason: a count of tasks
    let average = (!minutes.is_empty()).then(|| minutes.iter().sum::<f64>() / minutes.len() as f64);
    json!({
        "bins": bins,
        "median_alone": median(&of(false)),
        "median_with_you": median(&of(true)),
        "min": minutes.iter().copied().reduce(f64::min),
        "max": minutes.iter().copied().reduce(f64::max),
        "average": average,
        "slowest": slowest.into_iter().take(5).map(task).collect::<Vec<_>>(),
    })
}
