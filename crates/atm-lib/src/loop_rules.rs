//! The loop's rules (docs/parallel-agents-design.md; docs/sprints-and-releases-design.md §3; the Python atm's
//! `lib/loop.py`): which tasks a selection covers, the order the loop takes them in, and the next one, with why there
//! is none (`project status` shows it; `loop next` hands it out, with its slice). The same order as the dashboard's
//! Next up.

use std::collections::HashSet;

use atm_common::py::repr_str;
use atm_common::rules::{natural_cmp, unfinished_dependencies};
use atm_common::time::{now, parse_iso};
use atm_common::{Error, Kind, Result, Task, Vocabulary};
use serde_json::{Map, Value};

/// How long a claim lasts without a renewal, in minutes (`meta.lease_minutes` sets a project's own): two hours
/// (T-439), as every atm command an agent runs renews its claims.
pub const DEFAULT_LEASE_MINUTES: f64 = 120.0;

/// Review states that make a task in review work for an agent again (SR-3.9).
const REWORK_STATES: [&str; 2] = ["changes_requested", "approved"];

/// The project's lease length in minutes: `meta.lease_minutes` when it is a positive number (or a text of one).
#[must_use]
pub fn lease_minutes(document: &Value) -> f64 {
    let given = document.get("meta").and_then(|m| m.get("lease_minutes"));
    let minutes = match given {
        None => Some(DEFAULT_LEASE_MINUTES),
        Some(Value::Number(n)) => n.as_f64(),
        Some(Value::String(s)) => s.trim().parse().ok(),
        Some(Value::Bool(b)) => Some(f64::from(u8::from(*b))),
        Some(_) => None,
    };
    minutes
        .filter(|m| *m > 0.0)
        .unwrap_or(DEFAULT_LEASE_MINUTES)
}

/// What the loop works on (SR-3.2): the active sprint (the default, in a project with sprints), named sprints, a list
/// of tasks, all tasks, or the backlog; `task_type` narrows it.
#[derive(Debug, Clone, Default)]
pub struct Selection {
    /// Named sprints.
    pub sprints: Vec<String>,
    /// These tasks, in this order.
    pub tasks: Vec<String>,
    /// Every task: the active sprint, then the planned ones in order, then the backlog.
    pub all: bool,
    /// The tasks in no sprint.
    pub backlog: bool,
    /// Only tasks of this type.
    pub task_type: Option<String>,
}

fn id_of(task: &Task) -> Option<&str> {
    task.get("id").and_then(Value::as_str)
}

fn sprint_id<'v>(vocab: &'v Vocabulary, task: &Task) -> Option<&'v str> {
    vocab
        .find_sprint(task.get("sprint"))
        .and_then(|s| s.get("id"))
        .and_then(Value::as_str)
}

/// The tasks a selection covers, in any status, with each one's rank in the selection's own order and the words for
/// it; or why the selection can't be made (no active sprint).
struct Selected<'t> {
    tasks: Vec<&'t Task>,
    rank: Vec<usize>,
    scope: String,
    problem: Option<String>,
}

/// Whether a selection keeps a task, and its rank in the selection's own order.
type KeepAndRank<'v> = Box<dyn Fn(&Task) -> Option<usize> + 'v>;

#[allow(clippy::too_many_lines)] // reason: one case per kind of selection, as the Python atm's `select` has them
fn select<'t>(vocab: &Vocabulary, tasks: &[&'t Task], sel: &Selection) -> Result<Selected<'t>> {
    let sprints = &vocab.sprints;
    let names_of = |s: &Map<String, Value>| {
        s.get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let mut scope = "the tasks".to_string();
    let keep_and_rank: KeepAndRank = if !sel.tasks.is_empty() {
        let missing: Vec<&str> = sel
            .tasks
            .iter()
            .map(String::as_str)
            .filter(|i| !tasks.iter().any(|t| id_of(t) == Some(i)))
            .collect();
        if !missing.is_empty() {
            return Err(Error::Failed {
                message: format!("--tasks: no task {}", missing.join(", ")),
                code: 2,
            });
        }
        scope = "the listed tasks".into();
        let order: Vec<String> = sel.tasks.clone();
        // The last place an ID is listed, as a dict of the IDs by place has it
        Box::new(move |t| id_of(t).and_then(|id| order.iter().rposition(|o| o == id)))
    } else if sel.all {
        // The active sprint, then the planned ones in order, then the backlog (closed sprints' leftovers before it)
        let n = sprints.len();
        let place: Vec<(String, usize)> = sprints
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let at = match s.get("state").and_then(Value::as_str) {
                    Some("active") => 0,
                    Some("planned") => 1 + i,
                    _ => n + 1,
                };
                (names_of(s), at)
            })
            .collect();
        scope = "all tasks".into();
        let found = move |t: &Task| -> usize {
            sprint_id(vocab, t)
                .and_then(|id| place.iter().find(|(name, _)| name == id).map(|(_, at)| *at))
                .unwrap_or(n + 2)
        };
        Box::new(move |t| Some(found(t)))
    } else if sel.backlog {
        scope = "the backlog".into();
        Box::new(move |t| vocab.find_sprint(t.get("sprint")).is_none().then_some(0))
    } else if !sel.sprints.is_empty() {
        let mut named = Vec::new();
        for name in &sel.sprints {
            let sprint = vocab
                .find_sprint(Some(&Value::String(name.clone())))
                .ok_or_else(|| {
                    let ids: Vec<String> = sprints.iter().map(names_of).collect();
                    let known = if ids.is_empty() {
                        "none".to_string()
                    } else {
                        ids.join(", ")
                    };
                    Error::Failed {
                        message: format!("no sprint {}; the sprints: {known}", repr_str(name)),
                        code: 2,
                    }
                })?;
            if sprint.get("state").and_then(Value::as_str) == Some("closed") {
                return Err(Error::Failed {
                    message: format!("the sprint {} is closed", repr_str(&names_of(sprint))),
                    code: 2,
                });
            }
            named.push(names_of(sprint));
        }
        scope = format!(
            "the sprint {}",
            named
                .iter()
                .map(|n| format!("“{n}”"))
                .collect::<Vec<_>>()
                .join(" and ")
        );
        Box::new(move |t| {
            sprint_id(vocab, t)
                .is_some_and(|id| named.iter().any(|n| n == id))
                .then_some(0)
        })
    } else if !sprints.is_empty() {
        let active: Vec<String> = sprints
            .iter()
            .filter(|s| s.get("state").and_then(Value::as_str) == Some("active"))
            .map(names_of)
            .collect();
        let Some(first) = active.first() else {
            return Ok(Selected {
                tasks: Vec::new(),
                rank: Vec::new(),
                scope: "the active sprint".into(),
                problem: Some(
                    "no active sprint: plan one with /atm:plan-sprint, or run with --all".into(),
                ),
            });
        };
        scope = format!("the sprint “{first}”");
        Box::new(move |t| {
            sprint_id(vocab, t)
                .is_some_and(|id| active.iter().any(|a| a == id))
                .then_some(0)
        })
    } else {
        // A project without sprints: every task, as before sprints
        Box::new(|_| Some(0))
    };
    let mut kept = Vec::new();
    let mut rank = Vec::new();
    for t in tasks {
        let type_ok = sel
            .task_type
            .as_ref()
            .is_none_or(|ty| t.get("type").and_then(Value::as_str) == Some(ty.as_str()));
        if let (Some(r), true) = (keep_and_rank(t), type_ok) {
            kept.push(*t);
            rank.push(r);
        }
    }
    Ok(Selected {
        tasks: kept,
        rank,
        scope,
        problem: None,
    })
}

/// Whether a task back from review may go to `agent`: always to the agent that worked on it; to others once one lease
/// length has passed since the review ended (SR-3.9).
fn offered_to(document: &Value, task: &Task, agent: Option<&str>) -> bool {
    let Some(agent) = agent.filter(|a| !a.is_empty()) else {
        return true;
    };
    if task.get("started_by").and_then(Value::as_str) == Some(agent) {
        return true;
    }
    let Some(reviewed) = task
        .get("reviewed_at")
        .and_then(Value::as_str)
        .and_then(parse_iso)
    else {
        return true;
    };
    #[allow(clippy::cast_possible_truncation)] // reason: minutes of a lease in seconds fit an i64
    let lease = (lease_minutes(document) * 60.0) as i64;
    now() >= reviewed + lease
}

/// What the loop sees for a selection: the tasks it could take now in its order, the selection's tasks, the
/// candidates (to do, or back from review), the work done together with the user, the words for the selection, and
/// why the selection can't be made.
pub struct LoopOrder<'t> {
    /// The tasks the loop could take now, in the order it takes them.
    pub ready: Vec<&'t Task>,
    /// Every task of the selection.
    pub selected: Vec<&'t Task>,
    /// The selection's to-do tasks and those back from review (without work done with the user, unless asked).
    pub candidates: Vec<&'t Task>,
    /// The selection's candidates done together with the user.
    pub together: Vec<&'t Task>,
    /// The selection in words ("the sprint “Alpha”").
    pub scope: String,
    /// Why the selection itself is refused (no active sprint).
    pub problem: Option<String>,
}

/// Every task the loop could take now, in the order it takes them (tasks back from review first, then the
/// selection's own order, the most urgent priority and the lowest ID): `loop next` hands out the first.
///
/// # Errors
/// A selection naming a task or sprint the project doesn't have (exit code 2); an archive that can't be read.
#[allow(clippy::too_many_arguments)] // reason: the loop's inputs, as the Python atm's loop_order takes them
pub fn loop_order<'t>(
    vocab: &Vocabulary,
    document: &Value,
    tasks: &[&'t Task],
    store: &Map<String, Value>,
    sel: &Selection,
    agent: Option<&str>,
    with_user: bool,
    archived: &dyn Fn(&str) -> Result<bool>,
) -> Result<LoopOrder<'t>> {
    let selected = select(vocab, tasks, sel)?;
    if let Some(problem) = selected.problem {
        return Ok(LoopOrder {
            ready: Vec::new(),
            selected: selected.tasks,
            candidates: Vec::new(),
            together: Vec::new(),
            scope: selected.scope,
            problem: Some(problem),
        });
    }
    // Tasks back from review come first (SR-3.9): the agent that worked on them gets them back for a lease length
    let kind = |t: &Task| vocab.kind_of(t.get("status"));
    let mut candidates: Vec<(&Task, bool, usize)> = Vec::new();
    for (t, r) in selected.tasks.iter().zip(&selected.rank) {
        let rework = kind(t) == Some(Kind::Review)
            && t.get("review_state")
                .and_then(Value::as_str)
                .is_some_and(|s| REWORK_STATES.contains(&s))
            && offered_to(document, t, agent);
        if rework {
            candidates.push((t, true, *r));
        }
    }
    for (t, r) in selected.tasks.iter().zip(&selected.rank) {
        if kind(t) == Some(Kind::Todo) {
            candidates.push((t, false, *r));
        }
    }
    // Work done together with the user waits for them, unless they're watching (SR-3.5)
    let is_together = |t: &Task| t.get("with").and_then(Value::as_str) == Some("user");
    let together: Vec<&Task> = candidates
        .iter()
        .map(|c| c.0)
        .filter(|t| is_together(t))
        .collect();
    if !with_user {
        candidates.retain(|c| !is_together(c.0));
    }
    let section = |key: &str| {
        store
            .get(key)
            .and_then(Value::as_object)
            .map(|m| m.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default()
    };
    let taken: HashSet<String> = [section("claims"), section("completed"), section("waiting")]
        .concat()
        .into_iter()
        .collect();
    let mut ready = Vec::new();
    for c in &candidates {
        if id_of(c.0).is_some_and(|id| taken.contains(id))
            || !unfinished_dependencies(vocab, tasks, c.0, archived)?.is_empty()
        {
            continue;
        }
        ready.push(*c);
    }
    ready.sort_by(|a, b| {
        (!a.1).cmp(&!b.1).then(a.2.cmp(&b.2)).then_with(|| {
            vocab
                .priority_rank(a.0.get("priority"))
                .cmp(&vocab.priority_rank(b.0.get("priority")))
                .then_with(|| {
                    natural_cmp(
                        id_of(a.0).unwrap_or_default(),
                        id_of(b.0).unwrap_or_default(),
                    )
                })
        })
    });
    Ok(LoopOrder {
        ready: ready.into_iter().map(|c| c.0).collect(),
        selected: selected.tasks,
        candidates: candidates.into_iter().map(|c| c.0).collect(),
        together,
        scope: selected.scope,
        problem: None,
    })
}

/// The loop's rule (the dashboard's "Next up"): the next task the selection's loop takes, or why there is none: the
/// selection is finished, the rest of it waits, or it is empty (SR-3.6).
///
/// # Errors
/// As [`loop_order`].
#[allow(clippy::too_many_arguments, clippy::too_many_lines)] // reason: the loop's inputs and each reason it gives, as the Python atm's next_task has them
pub fn next_task<'t>(
    vocab: &Vocabulary,
    document: &Value,
    tasks: &[&'t Task],
    store: &Map<String, Value>,
    sel: &Selection,
    agent: Option<&str>,
    with_user: bool,
    archived: &dyn Fn(&str) -> Result<bool>,
) -> Result<(Option<&'t Task>, Option<String>)> {
    let order = loop_order(
        vocab, document, tasks, store, sel, agent, with_user, archived,
    )?;
    if let Some(problem) = order.problem {
        return Ok((None, Some(problem)));
    }
    if let Some(first) = order.ready.first() {
        return Ok((Some(first), None));
    }
    let ids = |list: &[&Task]| {
        list.iter()
            .filter_map(|t| id_of(t))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let scope = &order.scope;
    if !order.together.is_empty() && !with_user && order.candidates.is_empty() {
        return Ok((
            None,
            Some(format!(
                "only work done together with the user is left in {scope}: {} (--with-user takes it)",
                ids(&order.together)
            )),
        ));
    }
    if order.candidates.is_empty() {
        if order.selected.is_empty() {
            if vocab.sprints.is_empty() {
                return Ok((None, Some("nothing left to do".into())));
            }
            let verb = if scope.starts_with("the sprint") {
                "has"
            } else {
                "have"
            };
            return Ok((None, Some(format!("{scope} {verb} no tasks"))));
        }
        let kinds: Vec<Option<Kind>> = order
            .selected
            .iter()
            .map(|t| vocab.kind_of(t.get("status")))
            .collect();
        let count = |k: Kind| kinds.iter().filter(|x| **x == Some(k)).count();
        let open: Vec<String> = [
            (Kind::Active, "in progress"),
            (Kind::Review, "in review"),
            (Kind::Blocked, "blocked"),
        ]
        .into_iter()
        .filter(|(k, _)| count(*k) > 0)
        .map(|(k, word)| format!("{} {word}", count(k)))
        .collect();
        if !open.is_empty() {
            return Ok((
                None,
                Some(format!(
                    "nothing ready in {scope}: the rest is waiting ({})",
                    open.join(", ")
                )),
            ));
        }
        if !vocab.sprints.is_empty() {
            let skipped = count(Kind::Skipped);
            let tail = if skipped > 0 {
                format!(", {skipped} skipped")
            } else {
                String::new()
            };
            return Ok((
                None,
                Some(format!(
                    "{scope} is finished: {} done{tail}",
                    count(Kind::Done)
                )),
            ));
        }
        return Ok((None, Some("nothing left to do".into())));
    }
    let in_section = |key: &str, t: &Task| {
        store
            .get(key)
            .and_then(Value::as_object)
            .is_some_and(|m| id_of(t).is_some_and(|id| m.contains_key(id)))
    };
    let pick = |key: &str| {
        order
            .candidates
            .iter()
            .filter(|t| in_section(key, t))
            .copied()
            .collect::<Vec<_>>()
    };
    let (claimed, done_elsewhere, waiting_elsewhere) =
        (pick("claims"), pick("completed"), pick("waiting"));
    let mut parts = vec![format!(
        "{} todo task(s) waiting on dependencies",
        order.candidates.len()
    )];
    if !claimed.is_empty() {
        parts.push(format!(" or claimed by other agents ({})", ids(&claimed)));
    }
    if !done_elsewhere.is_empty() {
        parts.push(format!(
            "; done in another worktree, not merged yet: {}",
            ids(&done_elsewhere)
        ));
    }
    if !waiting_elsewhere.is_empty() {
        parts.push(format!(
            "; blocked or in review in another worktree: {}",
            ids(&waiting_elsewhere)
        ));
    }
    Ok((None, Some(parts.concat())))
}

#[cfg(test)]
mod tests;
