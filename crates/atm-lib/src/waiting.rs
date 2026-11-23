//! Who a task waits on, and since when (ATM-717, docs/dashboard-derived-state.md §4.2): one rule, for the task views'
//! `derived` (`waiting`, `needs_you`, `agent_work`, `review`) and the project summary's needs-you count, so no front end
//! works it out from the task's fields.

use atm_common::{Kind, Task, Vocabulary};
use serde::Serialize;
use serde_json::{Map, Value};

/// Who a task waits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum On {
    /// Its review waits for a reviewer's decision.
    Reviewer,
    /// Its agent: approved or sent back (it finishes or reworks it), or blocked on a question for an agent.
    Agent,
    /// The user: blocked on a question for them, rejected (they send it back or skip it), or work done together.
    User,
    /// Its dependencies: to do, and one isn't done yet.
    Dependencies,
    /// Blocked by hand: nobody said who decides.
    Unknown,
}

/// Who a task waits on, since when (ISO 8601, when known), and its open questions (blocked).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Waiting {
    pub on: On,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub questions: Vec<String>,
}

/// Why a task needs the user (the dashboard's needs-you chips, the summary's counts).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NeedsYou {
    /// In review, waiting for the reviewer (or rejected, waiting for what the user does with it).
    Review,
    /// Blocked on a question for the user.
    Blocked,
    /// To do together with the user, now: ready, and in the active sprint (any, without one).
    Together,
}

/// What [`of`] works out for one task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    pub waiting: Option<Waiting>,
    pub needs_you: Option<NeedsYou>,
    /// Whether agent work is left on it: to do or active, sent back or approved but not finished, or a status atm
    /// doesn't know. Not when it waits on a reviewer, the user or a decision.
    pub agent_work: bool,
}

/// What the task's own fields give it, beside what atm-lib already knows of it.
#[derive(Debug, Clone, Copy)]
pub struct Context<'a> {
    /// To do, and every dependency done.
    pub ready: bool,
    /// A to-do task's unfinished dependencies.
    pub waiting_for: &'a [String],
    /// In the active sprint, or there is none (with-you work is due now only then).
    pub in_scope: bool,
}

fn text(task: &Task, field: &str) -> Option<String> {
    task.get(field)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(String::from)
}

fn questions(task: &Task) -> Vec<String> {
    task.get("questions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(String::from)
        .collect()
}

/// Who `task` waits on, whether it needs the user, and whether agent work is left: the one rule.
#[must_use]
pub fn of(vocab: &Vocabulary, task: &Task, context: Context<'_>) -> Answer {
    let kind = vocab.kind_of(task.get("status"));
    let state = task.get("review_state").and_then(Value::as_str);
    let with_user = task.get("with").and_then(Value::as_str) == Some("user");
    let waiting = |on, since: Option<String>| {
        Some(Waiting {
            on,
            since,
            questions: Vec::new(),
        })
    };
    let (waiting, needs_you, agent_work) = match kind {
        Some(Kind::Review) => match state {
            Some("approved" | "changes_requested") => (
                waiting(
                    On::Agent,
                    text(task, "reviewed_at").or_else(|| text(task, "updated_at")),
                ),
                None,
                true,
            ),
            Some("rejected") => (
                waiting(
                    On::User,
                    text(task, "reviewed_at").or_else(|| text(task, "updated_at")),
                ),
                Some(NeedsYou::Review),
                false,
            ),
            _ => (
                waiting(
                    On::Reviewer,
                    text(task, "review_requested_at").or_else(|| text(task, "updated_at")),
                ),
                Some(NeedsYou::Review),
                false,
            ),
        },
        Some(Kind::Blocked) => {
            let on = match task.get("waiting_on").and_then(Value::as_str) {
                Some("user") => On::User,
                Some("agent") => On::Agent,
                _ => On::Unknown,
            };
            let blocked = Waiting {
                on,
                since: text(task, "blocked_at"),
                questions: questions(task),
            };
            let needs = (on == On::User).then_some(NeedsYou::Blocked);
            (Some(blocked), needs, false)
        }
        Some(Kind::Todo) if !context.waiting_for.is_empty() => {
            (waiting(On::Dependencies, None), None, true)
        }
        Some(Kind::Todo) if with_user => {
            let now = context.ready && context.in_scope;
            (
                waiting(On::User, None),
                now.then_some(NeedsYou::Together),
                true,
            )
        }
        Some(Kind::Todo | Kind::Active) | None => (None, None, true),
        Some(Kind::Done | Kind::Skipped) => (None, None, false),
    };
    Answer {
        waiting,
        needs_you,
        agent_work,
    }
}

/// Whether `task` is in the active sprint, or there is none: when work to do together with the user is due.
#[must_use]
pub fn in_active_scope(vocab: &Vocabulary, task: &Task) -> bool {
    let active = vocab.active_sprint();
    active.is_none_or(|a| {
        vocab
            .find_sprint(task.get("sprint"))
            .is_some_and(|s| s.get("id") == a.get("id"))
    })
}

/// The code review `task` waits on, as `{id, ref, status}`: of its linked reviews (`code_reviews`, oldest first), the
/// latest one still open here (draft, waiting, live or sent back, by `statuses`, by ID), else the latest link; none
/// without one. Not simply the latest link: a task's newer review may be closed while an older one is open. A review
/// this working copy doesn't have (another branch's) has no `status`: the link is still the task's review.
#[must_use]
pub fn review_of(task: &Task, statuses: &Map<String, Value>) -> Option<Value> {
    let links: Vec<(&str, &str, Option<&str>)> = task
        .get("code_reviews")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|l| {
            let id = l.get("id").and_then(Value::as_str)?;
            let reference = l.get("ref").and_then(Value::as_str).unwrap_or_default();
            Some((id, reference, statuses.get(id).and_then(Value::as_str)))
        })
        .collect();
    let open =
        |s: Option<&str>| matches!(s, Some("draft" | "waiting" | "live" | "changes_requested"));
    let (id, reference, status) = links
        .iter()
        .rev()
        .find(|(_, _, s)| open(*s))
        .or_else(|| links.last())?;
    let mut review = serde_json::json!({"id": id, "ref": reference});
    if let Some(status) = status {
        review["status"] = serde_json::json!(status);
    }
    Some(review)
}

#[cfg(test)]
mod tests;
