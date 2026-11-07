//! A project's summary for the dashboard's overview, project selector and All projects board (the Python atm's
//! `lib/query.py` `project_summary`; docs/multi-project-api.md §3.2): counts by status kind, the active and recent
//! sprints, who works on what, what the loop takes next, what needs the user, the git line, the board's columns and
//! the project's stats. Read-only.

use std::collections::HashMap;

use atm_common::py::{py_str, py_str_opt, strip, truthy};
use atm_common::rules::{natural_cmp, unfinished_dependencies};
use atm_common::time::{format_iso, now, parse_iso};
use atm_common::{Kind, Result, Task, Vocabulary, started_by};
use serde_json::{Map, Value, json};

use crate::loop_rules::{Selection, loop_order};
use crate::project::Project;

const ACTIVITY_FIELDS: [&str; 6] = [
    "updated_at",
    "started_at",
    "completed_at",
    "review_requested_at",
    "blocked_at",
    "skipped_at",
];
const SUMMARY_DAYS: usize = 14;
const RECENT_SPRINTS: usize = 4;
const NEXT_TASKS: usize = 2;
/// Task names in the summary, so `/api/projects` stays small with many projects.
const NAME_CAP: usize = 80;
const DESCRIPTION_CAP: usize = 200;
/// Cards per column of the All projects board.
const BOARD_CAP: usize = 30;
/// To-do tasks per project, so one project's new batch doesn't fill the To Do column.
const BOARD_READY_CAP: usize = 10;
const BOARD_DONE_HOURS: i64 = 24;
/// The review states in which a task in review waits for its agent, not the reviewer.
const REWORK_STATES: [&str; 2] = ["changes_requested", "approved"];

fn time_of(value: Option<&Value>) -> Option<i64> {
    value.and_then(Value::as_str).and_then(parse_iso)
}

/// A name for the summary: at most 80 characters, the last one an ellipsis when cut.
fn capped(name: Option<&Value>) -> String {
    let name = name.and_then(Value::as_str).unwrap_or_default();
    if name.chars().count() <= NAME_CAP {
        return name.to_string();
    }
    name.chars().take(NAME_CAP - 1).collect::<String>() + "…"
}

/// `round(100 * part / whole)` as Python rounds it (half to even).
fn percent(part: usize, whole: usize) -> i64 {
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    // reason: task counts, far below 2^52
    let value = (100.0 * part as f64 / whole as f64).round_ties_even() as i64;
    value
}

fn count(kinds: &[Option<Kind>], kind: Kind) -> usize {
    kinds.iter().filter(|k| **k == Some(kind)).count()
}

/// A sprint as the overview shows it: its name, state, due date and release, and done of its tasks but the skipped.
fn sprint_numbers(sprint: &Map<String, Value>, own: &[Option<Kind>]) -> Map<String, Value> {
    let done = count(own, Kind::Done);
    let total = own.len() - count(own, Kind::Skipped);
    let field = |k: &str| sprint.get(k).cloned().unwrap_or(Value::Null);
    json!({
        "id": field("id"),
        "state": field("state"),
        "due": field("due"),
        "release": field("release"),
        "done": done,
        "total": total,
        "percent": if total > 0 { percent(done, total) } else { 100 },
    })
    .as_object()
    .cloned()
    .unwrap_or_default()
}

fn sprint_of<'v>(vocab: &'v Vocabulary, task: &Task) -> Option<&'v Map<String, Value>> {
    vocab.find_sprint(task.get("sprint"))
}

fn sprint_key(sprint: &Map<String, Value>) -> String {
    sprint.get("id").map(py_str).unwrap_or_default()
}

/// Each sprint's tasks' kinds, by sprint ID.
fn kinds_by_sprint(
    vocab: &Vocabulary,
    tasks: &[&Task],
    kinds: &[Option<Kind>],
) -> HashMap<String, Vec<Option<Kind>>> {
    let mut own: HashMap<String, Vec<Option<Kind>>> = HashMap::new();
    for (t, k) in tasks.iter().zip(kinds) {
        if let Some(sprint) = sprint_of(vocab, t) {
            own.entry(sprint_key(sprint)).or_default().push(*k);
        }
    }
    own
}

/// Up to four sprints with tasks for the overview's rings (SR-7.7), in the project's order: the active one and the
/// planned ones first, then the most recently closed (by their end) to fill up. None without sprints.
fn recent_sprints(vocab: &Vocabulary, tasks: &[&Task], kinds: &[Option<Kind>]) -> Value {
    if vocab.sprints.is_empty() {
        return Value::Null;
    }
    let own = kinds_by_sprint(vocab, tasks, kinds);
    let with_tasks: Vec<&Map<String, Value>> = vocab
        .sprints
        .iter()
        .filter(|s| own.contains_key(&sprint_key(s)))
        .collect();
    let closed = |s: &Map<String, Value>| s.get("state").and_then(Value::as_str) == Some("closed");
    let mut chosen: Vec<String> = with_tasks
        .iter()
        .filter(|s| !closed(s))
        .take(RECENT_SPRINTS)
        .map(|s| sprint_key(s))
        .collect();
    let mut ended: Vec<&Map<String, Value>> =
        with_tasks.iter().copied().filter(|s| closed(s)).collect();
    let end = |s: &Map<String, Value>| {
        s.get("end")
            .filter(|e| truthy(e))
            .map(py_str)
            .unwrap_or_default()
    };
    // Newest end first; a stable sort, as Python's sorted(reverse=True) keeps equal ones in order
    ended.sort_by_key(|s| std::cmp::Reverse(end(s)));
    let room = RECENT_SPRINTS - chosen.len();
    chosen.extend(ended.iter().take(room).map(|s| sprint_key(s)));
    let list: Vec<Value> = with_tasks
        .iter()
        .filter(|s| chosen.contains(&sprint_key(s)))
        .map(|s| Value::Object(sprint_numbers(s, &own[&sprint_key(s)])))
        .collect();
    Value::Array(list)
}

fn active_sprint(vocab: &Vocabulary) -> Option<&Map<String, Value>> {
    vocab
        .sprints
        .iter()
        .find(|s| s.get("state").and_then(Value::as_str) == Some("active"))
}

fn in_sprint(vocab: &Vocabulary, task: &Task, sprint: &Map<String, Value>) -> bool {
    sprint_of(vocab, task).is_some_and(|s| s.get("id") == sprint.get("id"))
}

/// The active sprint for the overview's cards (SR-7.7), with its progress and goal; none when none is active.
fn active_sprint_numbers(vocab: &Vocabulary, tasks: &[&Task], kinds: &[Option<Kind>]) -> Value {
    let Some(active) = active_sprint(vocab) else {
        return Value::Null;
    };
    let own: Vec<Option<Kind>> = tasks
        .iter()
        .zip(kinds)
        .filter(|(t, _)| in_sprint(vocab, t, active))
        .map(|(_, k)| *k)
        .collect();
    let mut numbers = sprint_numbers(active, &own);
    numbers.insert(
        "goal".into(),
        active.get("goal").cloned().unwrap_or(Value::Null),
    );
    Value::Object(numbers)
}

/// `meta.description`, capped at 200 characters for the overview, where it is the project name's tooltip; none
/// without one.
fn description(meta: Option<&Value>) -> Value {
    let Some(text) = meta
        .and_then(|m| m.get("description"))
        .and_then(Value::as_str)
        .map(strip)
        .filter(|t| !t.is_empty())
    else {
        return Value::Null;
    };
    if text.chars().count() <= DESCRIPTION_CAP {
        return json!(text);
    }
    json!(text.chars().take(DESCRIPTION_CAP - 1).collect::<String>() + "…")
}

/// Only a web address is linked (docs/security-review.md F9): `http://` or `https://`, then no white space.
fn web_address(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    let rest = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"));
    rest.is_some_and(|r| !r.is_empty() && !r.chars().any(char::is_whitespace))
}

/// The project's git line for its card: the working copy's branch, and the latest PR recorded on its tasks (the
/// highest number), `merged` once its tasks have `merged_at`. No forge call and no CI state.
fn git_summary(project: &Project, tasks: &[&Task]) -> Value {
    let mut prs: Vec<(i64, Map<String, Value>)> = Vec::new();
    for t in tasks {
        let Some(pr) = t.get("pr").and_then(Value::as_object) else {
            continue;
        };
        let Some(number) = pr
            .get("number")
            .filter(|n| n.is_i64() || n.is_u64())
            .and_then(Value::as_i64)
        else {
            continue;
        };
        let at = prs
            .iter()
            .position(|(n, _)| *n == number)
            .unwrap_or_else(|| {
                let entry = json!({"number": number, "url": null, "state": "open"});
                prs.push((number, entry.as_object().cloned().unwrap_or_default()));
                prs.len() - 1
            });
        let entry = &mut prs[at].1;
        if let Some(url) = pr
            .get("url")
            .and_then(Value::as_str)
            .filter(|u| web_address(u))
        {
            entry.insert("url".into(), json!(url));
        }
        if t.get("merged_at").is_some_and(truthy) {
            entry.insert("state".into(), json!("merged"));
        }
    }
    let latest = prs
        .into_iter()
        .max_by_key(|(n, _)| *n)
        .map_or(Value::Null, |(_, e)| Value::Object(e));
    json!({"branch": project.store().branch(), "pr": latest})
}

/// What a project's Kanban card shows, nothing more, and `extra`.
fn board_card(vocab: &Vocabulary, task: &Task, extra: &[(&str, Value)]) -> Value {
    let labels: Vec<Value> = task
        .get("labels")
        .and_then(Value::as_array)
        .map(|l| l.iter().filter(|x| x.is_string()).cloned().collect())
        .unwrap_or_default();
    let mut card = json!({
        "id": task.get("id"),
        "name": capped(task.get("name")),
        "type": task.get("type"),
        "sprint": task.get("sprint").filter(|s| s.is_string()),
        "priority": vocab.priority_of(task),
        "labels": labels,
    });
    for (key, value) in extra {
        card[*key] = value.clone();
    }
    card
}

/// A column: its first `cap` cards, and how many there are in all (for "+ N more").
fn board_column(cards: Vec<Value>, cap: usize) -> Value {
    let count = cards.len();
    json!({"count": count, "cards": cards.into_iter().take(cap).collect::<Vec<_>>()})
}

/// A to-do task for the board, with the dependencies it waits for.
struct Todo<'t> {
    task: &'t Task,
    waits: Vec<String>,
}

/// Every to-do task, for the board's To Do column (T-349): first the ready ones the loop takes, in its order, then the
/// other ready ones, then the ready ones done with the user, then those waiting on dependencies (each by priority,
/// then ID).
fn todo_order<'t>(
    vocab: &Vocabulary,
    all: &[&'t Task],
    kinds: &[Option<Kind>],
    loop_ready: &[&Task],
    archived: &dyn Fn(&str) -> Result<bool>,
) -> Result<Vec<Todo<'t>>> {
    let rank = |t: &Task| {
        let id = t.get("id");
        loop_ready.iter().position(|r| r.get("id") == id)
    };
    let mut entries = Vec::new();
    for (t, k) in all.iter().zip(kinds) {
        if *k != Some(Kind::Todo) {
            continue;
        }
        let waits: Vec<String> = unfinished_dependencies(vocab, all, t, archived)?
            .into_iter()
            .map(|(d, _)| d)
            .collect();
        entries.push(Todo { task: t, waits });
    }
    let key = |e: &Todo| {
        let with_user = e.task.get("with").and_then(Value::as_str) == Some("user");
        let r = rank(e.task);
        (
            !e.waits.is_empty(),
            with_user,
            r.is_none(),
            r.unwrap_or(0),
            vocab.priority_rank(e.task.get("priority")),
        )
    };
    entries.sort_by(|a, b| {
        key(a)
            .cmp(&key(b))
            .then_with(|| natural_cmp(&py_str_opt(a.task.get("id")), &py_str_opt(b.task.get("id"))))
    });
    Ok(entries)
}

/// The text a value sorts by, as Python's `str(value or "")`.
fn sort_text(value: Option<&Value>) -> String {
    value.filter(|v| truthy(v)).map(py_str).unwrap_or_default()
}

/// A field, else `updated_at` (as Python's `t.get(field) or t.get("updated_at")`).
fn since<'t>(task: &'t Task, field: &str) -> Option<&'t Value> {
    task.get(field)
        .filter(|v| truthy(v))
        .or_else(|| task.get("updated_at"))
}

fn or_null(value: Option<&Value>) -> Value {
    value.cloned().unwrap_or(Value::Null)
}

/// The project's cards for the All projects board (T-257), one rule per column: every to-do task (`todo`, in the
/// board's order); everything in progress (longest running first), in review (longest waiting first) and blocked (on
/// the user first, then longest waiting); what was finished, newest first: in the last 24 hours, or all of it
/// without `recent_done` (a sprint's board shows all its tasks).
#[allow(clippy::too_many_lines)] // reason: one block per column, as the Python atm's board has them
fn board(
    vocab: &Vocabulary,
    tasks: &[&Task],
    kinds: &[Option<Kind>],
    todo: &[&Todo],
    current: i64,
    live: &[&Value],
    recent_done: bool,
) -> Value {
    let of_kind = |kind: Kind| -> Vec<&Task> {
        tasks
            .iter()
            .zip(kinds)
            .filter(|(_, k)| **k == Some(kind))
            .map(|(t, _)| *t)
            .collect()
    };
    let claimed_by = |t: &Task| {
        live.iter()
            .rev()
            .find(|c| c.get("task") == t.get("id"))
            .and_then(|c| c.get("agent"))
            .filter(|a| truthy(a))
    };
    let mut active = of_kind(Kind::Active);
    active.sort_by_key(|t| sort_text(t.get("started_at")));
    let mut review = of_kind(Kind::Review);
    review.sort_by_key(|t| sort_text(since(t, "review_requested_at")));
    let mut blocked = of_kind(Kind::Blocked);
    blocked.sort_by_key(|t| {
        (
            t.get("waiting_on").and_then(Value::as_str) != Some("user"),
            sort_text(since(t, "blocked_at")),
        )
    });
    let cutoff = current - BOARD_DONE_HOURS * 3600;
    let mut done: Vec<&Task> = of_kind(Kind::Done)
        .into_iter()
        .filter(|t| !recent_done || time_of(t.get("completed_at")).is_some_and(|f| f > cutoff))
        .collect();
    let completed = |t: &Task| {
        t.get("completed_at")
            .map_or_else(|| "None".to_string(), py_str)
    };
    done.sort_by_key(|t| std::cmp::Reverse(completed(t)));
    let todo_cards = todo
        .iter()
        .map(|e| {
            let mut extra = Vec::new();
            if e.task.get("with").and_then(Value::as_str) == Some("user") {
                extra.push(("with_user", json!(true)));
            }
            if !e.waits.is_empty() {
                extra.push(("waiting_for", json!(e.waits)));
            }
            board_card(vocab, e.task, &extra)
        })
        .collect();
    let active_cards = active
        .iter()
        .map(|t| {
            let by = claimed_by(t).or_else(|| started_by(t));
            board_card(
                vocab,
                t,
                &[
                    ("by", or_null(by)),
                    ("since", or_null(since(t, "started_at"))),
                ],
            )
        })
        .collect();
    let review_cards = review
        .iter()
        .map(|t| {
            board_card(
                vocab,
                t,
                &[("since", or_null(since(t, "review_requested_at")))],
            )
        })
        .collect();
    let blocked_cards = blocked
        .iter()
        .map(|t| {
            let questions = t
                .get("questions")
                .filter(|q| truthy(q))
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            board_card(
                vocab,
                t,
                &[
                    ("since", or_null(since(t, "blocked_at"))),
                    ("waiting_on", or_null(t.get("waiting_on"))),
                    ("questions", json!(questions)),
                ],
            )
        })
        .collect();
    let done_cards = done
        .iter()
        .map(|t| {
            board_card(
                vocab,
                t,
                &[("completed_at", or_null(t.get("completed_at")))],
            )
        })
        .collect();
    json!({
        "todo": board_column(todo_cards, BOARD_READY_CAP),
        "active": board_column(active_cards, BOARD_CAP),
        "review": board_column(review_cards, BOARD_CAP),
        "blocked": board_column(blocked_cards, BOARD_CAP),
        "done": board_column(done_cards, BOARD_CAP),
    })
}

/// A task's work as (start, end), as the dashboard counts it: from `started_at` until it asked for a review or was
/// done, or until now while active; none for a task that holds none.
fn work_span(vocab: &Vocabulary, task: &Task, current: i64) -> Option<(i64, i64)> {
    let start = time_of(task.get("started_at"))?;
    let end = match vocab.kind_of(task.get("status")) {
        Some(Kind::Active) => current.max(start),
        Some(kind @ (Kind::Done | Kind::Review)) => {
            let finished = if kind == Kind::Done {
                task.get("completed_at")
            } else {
                task.get("updated_at")
            };
            time_of(task.get("review_requested_at"))
                .or_else(|| time_of(finished))
                .unwrap_or(start)
        }
        _ => return None,
    };
    Some((start, end.max(start)))
}

/// Agent time (T-255): the work of tasks an agent started, or nobody recorded (before v4), each agent's spans merged
/// so an agent on two tasks counts once; parallel agents add up. In minutes.
fn agent_minutes(vocab: &Vocabulary, tasks: &[&Task], current: i64) -> i64 {
    let mut spans: Vec<(String, Vec<(i64, i64)>)> = Vec::new();
    for (i, t) in tasks.iter().enumerate() {
        let starter = started_by(t).filter(|s| truthy(s)).map(py_str);
        if starter.as_ref().is_some_and(|s| !s.starts_with("claude:")) {
            continue;
        }
        let Some(span) = work_span(vocab, t, current) else {
            continue;
        };
        // Unattributed spans are each their own
        let key = starter.unwrap_or_else(|| format!("\u{0}{i}"));
        match spans.iter_mut().find(|(k, _)| *k == key) {
            Some((_, list)) => list.push(span),
            None => spans.push((key, vec![span])),
        }
    }
    let mut total = 0i64;
    for (_, mut list) in spans {
        list.sort_unstable();
        let mut end: Option<i64> = None;
        for (s, e) in list {
            match end {
                Some(last) if s <= last => {
                    if e > last {
                        total += e - last;
                        end = Some(e);
                    }
                }
                _ => {
                    total += e - s;
                    end = Some(e);
                }
            }
        }
    }
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    // reason: seconds of work, far below 2^52
    let minutes = (total as f64 / 60.0).round_ties_even() as i64;
    minutes
}

/// The overview card's line (T-255): when the project started (its first task's `created_at`, else `meta.created`),
/// how many tasks it has, and the agent time, archived tasks included; an unreadable archive is left out.
fn project_stats(
    project: &Project,
    document: &Value,
    vocab: &Vocabulary,
    tasks: &[&Task],
    current: i64,
) -> Value {
    let archived = project.store().list_archived().unwrap_or_default();
    let ids: Vec<Option<&Value>> = tasks.iter().map(|t| t.get("id")).collect();
    let mut every: Vec<&Task> = tasks.to_vec();
    every.extend(archived.into_iter().filter(|t| !ids.contains(&t.get("id"))));
    let mut created: Vec<i64> = every
        .iter()
        .filter_map(|t| time_of(t.get("created_at")))
        .collect();
    if created.is_empty() {
        created.extend(time_of(document.get("meta").and_then(|m| m.get("created"))));
    }
    json!({
        "started": created.iter().min().map(|s| format_iso(*s)),
        "tasks": every.len(),
        "agent_minutes": agent_minutes(vocab, &every, current),
    })
}

/// The day a time falls on (days since 1970), in UTC.
fn day_of(seconds: i64) -> i64 {
    seconds.div_euclid(86_400)
}

/// A project's numbers for the dashboard's overview and project selector (docs/multi-project-api.md §3.2), by status
/// kind as atm counts them everywhere else: done / (total - skipped), the active and recent sprints, live claims,
/// what needs the user, the latest activity and the completions of the last 14 days, the board and the stats.
///
/// # Errors
/// Task data that can't be read.
#[allow(clippy::too_many_lines)] // reason: one block per part of the summary, as the Python atm's project_summary has them
pub fn project_summary(project: &Project) -> Result<Value> {
    let store = project.store();
    let document = store.document()?;
    let vocab = store.vocabulary()?;
    let tasks = store.list_tasks()?;
    let archived = |id: &str| {
        store
            .get_archived_task(id)
            .map(|t| t.is_some_and(|t| !t.is_empty()))
    };
    let kinds: Vec<Option<Kind>> = tasks
        .iter()
        .map(|t| vocab.kind_of(t.get("status")))
        .collect();
    let (done, skipped) = (count(&kinds, Kind::Done), count(&kinds, Kind::Skipped));
    let counted = tasks.len() - skipped;
    let overall = if counted > 0 {
        percent(done, counted)
    } else if skipped > 0 {
        100
    } else {
        0
    };

    let current = now();
    let claims = store.read_claims();
    let mut live: Vec<&Value> = claims
        .get("claims")
        .and_then(Value::as_object)
        .map(|c| {
            c.values()
                .filter(|c| c.is_object())
                .filter(|c| time_of(c.get("lease_expires_at")).unwrap_or(current) > current)
                .collect()
        })
        .unwrap_or_default();
    live.sort_by_key(|c| sort_text(c.get("claimed_at")));
    // The last task with an ID, as a dict of them by ID has it
    let by_id = |id: Option<&Value>| tasks.iter().rev().find(|t| t.get("id") == id).copied();
    let working: Vec<Value> = live
        .iter()
        .map(|c| {
            json!({
                "task": c.get("task"),
                "name": capped(by_id(c.get("task")).and_then(|t| t.get("name"))),
                "agent": c.get("agent"),
                "branch": c.get("branch"),
            })
        })
        .collect();
    // What the loop takes next with its default selection, by the very rule `atm loop next` follows
    let order = loop_order(
        vocab,
        document,
        &tasks,
        &claims,
        &Selection::default(),
        None,
        false,
        &archived,
    )?;
    let next_up: Vec<Value> = order
        .ready
        .iter()
        .take(NEXT_TASKS)
        .map(|t| json!({"id": t.get("id"), "name": capped(t.get("name"))}))
        .collect();
    let todo = todo_order(vocab, &tasks, &kinds, &order.ready, &archived)?;
    let mut last_task: Option<&Task> = None;
    for (t, k) in tasks.iter().zip(&kinds) {
        if *k != Some(Kind::Done) || time_of(t.get("completed_at")).is_none() {
            continue;
        }
        let at = py_str_opt(t.get("completed_at"));
        if last_task.is_none_or(|l| at > py_str_opt(l.get("completed_at"))) {
            last_task = Some(t);
        }
    }
    let last_done = last_task.map_or(Value::Null, |t| {
        json!({"id": t.get("id"), "name": capped(t.get("name")), "completed_at": t.get("completed_at")})
    });

    let mut last: Option<i64> = None;
    for t in &tasks {
        let notes = t
            .get("notes")
            .filter(|n| truthy(n))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|n| time_of(n.get("at")));
        let times = ACTIVITY_FIELDS
            .iter()
            .filter_map(|f| time_of(t.get(*f)))
            .chain(notes);
        for time in times {
            last = Some(last.map_or(time, |l| l.max(time)));
        }
    }

    let mut completions = [0u64; SUMMARY_DAYS];
    let today = day_of(current);
    for (t, k) in tasks.iter().zip(&kinds) {
        if *k != Some(Kind::Done) {
            continue;
        }
        if let Some(finished) = time_of(t.get("completed_at")) {
            let age = today - day_of(finished);
            if let Ok(age) = usize::try_from(age)
                && age < SUMMARY_DAYS
            {
                completions[SUMMARY_DAYS - 1 - age] += 1;
            }
        }
    }

    // The tasks that need the user, as the dashboard's needsYouReason decides it: in review and waiting for the
    // reviewer, blocked on the user, or to do together with the user now: in the active sprint (any, without one) and
    // ready, its dependencies done (T-347)
    let active = active_sprint(vocab);
    let mut needs_you_tasks = Vec::new();
    for (t, k) in tasks.iter().zip(&kinds) {
        let reason = match k {
            Some(Kind::Review) => {
                let state = t.get("review_state").and_then(Value::as_str);
                (!state.is_some_and(|s| REWORK_STATES.contains(&s))).then_some("review")
            }
            Some(Kind::Blocked) if t.get("waiting_on").and_then(Value::as_str) == Some("user") => {
                Some("blocked")
            }
            Some(Kind::Todo) if t.get("with").and_then(Value::as_str) == Some("user") => {
                let in_scope = active.is_none_or(|a| in_sprint(vocab, t, a));
                (in_scope && unfinished_dependencies(vocab, &tasks, t, &archived)?.is_empty())
                    .then_some("together")
            }
            _ => None,
        };
        if let Some(reason) = reason {
            needs_you_tasks
                .push(json!({"id": t.get("id"), "name": capped(t.get("name")), "reason": reason}));
        }
    }
    let reasons = |r: &str| needs_you_tasks.iter().filter(|n| n["reason"] == r).count();

    let todo_refs: Vec<&Todo> = todo.iter().collect();
    let sprint_board = active.map_or(Value::Null, |a| {
        let pairs: Vec<(&Task, Option<Kind>)> = tasks
            .iter()
            .zip(&kinds)
            .filter(|(t, _)| in_sprint(vocab, t, a))
            .map(|(t, k)| (*t, *k))
            .collect();
        let (in_tasks, in_kinds): (Vec<&Task>, Vec<Option<Kind>>) = pairs.into_iter().unzip();
        let in_todo: Vec<&Todo> = todo
            .iter()
            .filter(|e| in_sprint(vocab, e.task, a))
            .collect();
        board(vocab, &in_tasks, &in_kinds, &in_todo, current, &live, false)
    });

    Ok(json!({
        "total": tasks.len(),
        "done": done,
        "skipped": skipped,
        "percent": overall,
        "active_sprint": active_sprint_numbers(vocab, &tasks, &kinds),
        "recent_sprints": recent_sprints(vocab, &tasks, &kinds),
        "agents_working": live.len(),
        "working": working,
        "next": next_up,
        "last_done": last_done,
        "needs_you": {
            "review": reasons("review"),
            "blocked_on_user": reasons("blocked"),
            "together": reasons("together"),
        },
        "needs_you_tasks": needs_you_tasks,
        "git": git_summary(project, &tasks),
        "description": description(document.get("meta")),
        "last_activity": last.map(format_iso),
        "completions_14d": completions,
        "board": board(vocab, &tasks, &kinds, &todo_refs, current, &live, true),
        "board_active_sprint": sprint_board,
        "stats": project_stats(project, document, vocab, &tasks, current),
    }))
}

#[cfg(test)]
mod tests;
