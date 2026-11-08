//! Claims and their leases (docs/parallel-agents-design.md; the Python atm's `lib/loop.py`): when a lease runs out,
//! who holds a claim, an agent's own lease that just expired, and the records a change leaves in the claims store
//! (a claim, a completion, a task waiting in this working copy).

use atm_common::edits::{add_note, back_to_todo};
use atm_common::py::py_str_opt;
use atm_common::time::{format_iso, now, now_iso, parse_iso};
use atm_common::{Error, Kind, Result, Vocabulary};
use atm_db::working::Here;
use serde_json::{Map, Value, json};

use crate::loop_rules::lease_minutes;

/// When a lease taken now runs out.
#[must_use]
pub fn lease_until(document: &Value) -> String {
    #[allow(clippy::cast_possible_truncation)] // reason: a lease's seconds fit an i64
    let seconds = (lease_minutes(document) * 60.0) as i64;
    format_iso(now() + seconds)
}

/// `{:g}` of a number of minutes, as the Python atm prints it in messages: `120`, `0.5`.
fn minutes_text(minutes: f64) -> String {
    if minutes.fract() == 0.0 && minutes.abs() < 1e15 {
        #[allow(clippy::cast_possible_truncation)] // reason: a whole number below 1e15
        let whole = minutes as i64;
        whole.to_string()
    } else {
        atm_common::py::py_repr(&json!(minutes))
    }
}

fn task_at<'a>(document: &'a mut Value, task_id: &str) -> Option<&'a mut Map<String, Value>> {
    document["tasks"]
        .as_array_mut()?
        .iter_mut()
        .filter_map(Value::as_object_mut)
        .find(|t| t.get("id").and_then(Value::as_str) == Some(task_id))
}

/// Drop the claims whose lease ran out; their tasks in this working copy go back to to do, with a failure note.
/// Returns the expired claims.
pub fn drop_expired_claims(
    document: &mut Value,
    claims: &mut Map<String, Value>,
    vocab: &Vocabulary,
) -> Vec<Value> {
    let current = now();
    let ids: Vec<String> = claims.keys().cloned().collect();
    let mut expired = Vec::new();
    for task_id in ids {
        let alive = claims
            .get(&task_id)
            .and_then(|c| c.get("lease_expires_at"))
            .and_then(Value::as_str)
            .and_then(parse_iso)
            .is_some_and(|until| until > current);
        if alive {
            continue;
        }
        let Some(claim) = claims.shift_remove(&task_id) else {
            continue;
        };
        if let Some(task) = task_at(document, &task_id)
            && vocab.kind_of(task.get("status")) == Some(Kind::Active)
        {
            let stamp = now_iso();
            back_to_todo(task, vocab, None, &stamp, true);
            let text = format!("lease expired ({})", py_str_opt(claim.get("agent")));
            // A note built by atm, short by construction
            let _ = add_note(task, "failure", &text, None, &stamp, false);
        }
        expired.push(claim);
    }
    expired
}

/// Refuse a change by an agent other than the one holding the task's claim (exit code 4).
///
/// # Errors
/// When another agent holds the claim.
pub fn check_holder(
    claims: &Map<String, Value>,
    task_id: &str,
    agent: Option<&str>,
    command: &str,
) -> Result<()> {
    if let Some(claim) = claims.get(task_id).filter(|c| atm_common::py::truthy(c))
        && claim.get("agent").and_then(Value::as_str) != agent
    {
        return Err(Error::Failed {
            message: format!(
                "can't {command} {task_id}: it is claimed by {} until {}",
                py_str_opt(claim.get("agent")),
                py_str_opt(claim.get("lease_expires_at"))
            ),
            code: 4,
        });
    }
    Ok(())
}

/// When `agent`'s own claim on `task_id` is among the claims that just expired and the task went back to to do: an
/// error that says so and how to go on (`then`, the command to run after starting it again). Exit code 4.
#[must_use]
pub fn own_lease_expired(
    expired: &[Value],
    document: &Value,
    vocab: &Vocabulary,
    task_id: &str,
    agent: Option<&str>,
    then: Option<&str>,
) -> Option<Error> {
    let claim = expired.iter().find(|c| {
        c.get("task").and_then(Value::as_str) == Some(task_id)
            && c.get("agent").and_then(Value::as_str) == agent
    })?;
    let task = document["tasks"]
        .as_array()?
        .iter()
        .find(|t| t.get("id").and_then(Value::as_str) == Some(task_id))?;
    if vocab.kind_of(task.get("status")) != Some(Kind::Todo) {
        return None;
    }
    let until = claim
        .get("lease_expires_at")
        .map(atm_common::py::py_str)
        .unwrap_or_default();
    let at = if until.chars().count() >= 16 {
        format!("{} UTC", until.chars().skip(11).take(5).collect::<String>())
    } else {
        until
    };
    let idle = format!("{} min", minutes_text(lease_minutes(document)));
    let kept = if task.get("commit").is_some_and(atm_common::py::truthy) {
        " Your commit is kept."
    } else {
        ""
    };
    let next = match then {
        Some(then) => format!("Run `atm task state {task_id} in_progress`, then `{then}` again."),
        None => format!("Run `atm task state {task_id} in_progress` to claim it again."),
    };
    Some(Error::Failed {
        message: format!(
            "{task_id}: your lease expired at {at} (no heartbeat for {idle}), so it went back to todo.{kept} {next}"
        ),
        code: 4,
    })
}

/// A new claim on the task, from this working copy (its worktree and branch, T-202).
#[must_use]
pub fn claim_record(here: &Here, document: &Value, task_id: &str, agent: Option<&str>) -> Value {
    json!({
        "task": task_id,
        "agent": agent,
        "branch": here.branch,
        "worktree": here.worktree,
        "claimed_at": now_iso(),
        "lease_expires_at": lease_until(document),
    })
}

/// Done in this working copy: so `loop next` doesn't hand the task out again in another before the branches merge.
#[must_use]
pub fn completion_record(here: &Here, task_id: &str, agent: Option<&str>) -> Value {
    json!({"task": task_id, "agent": agent, "branch": here.branch, "worktree": here.worktree, "completed_at": now_iso()})
}

/// The shared record of a task blocked or sent for review in this working copy (T-204).
#[must_use]
pub fn waiting_entry(here: &Here, task: &Map<String, Value>) -> Value {
    json!({
        "task": task.get("id"),
        "status": task.get("status"),
        "waiting_on": task.get("waiting_on"),
        "questions": task.get("questions").and_then(Value::as_array).cloned().unwrap_or_default(),
        "review_state": task.get("review_state"),
        "worktree": here.worktree,
        "branch": here.branch,
        "at": now_iso(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(v: Value) -> Map<String, Value> {
        match v {
            Value::Object(m) => m,
            other => panic!("not an object: {other}"),
        }
    }

    #[test]
    fn an_expired_claim_sends_its_task_back_and_says_so_to_its_agent() {
        let vocab = Vocabulary::of_meta(&Map::new()).unwrap();
        let mut document =
            json!({"tasks": [{"id": "T-1", "status": "in_progress", "commit": "abc"}]});
        let mut claims = map(json!({
            "T-1": {"task": "T-1", "agent": "a", "lease_expires_at": "2020-01-01T10:30:00Z"},
            "T-2": {"task": "T-2", "agent": "b", "lease_expires_at": "2999-01-01T00:00:00Z"}
        }));
        let expired = drop_expired_claims(&mut document, &mut claims, &vocab);
        assert_eq!(expired.len(), 1);
        assert_eq!(claims.keys().collect::<Vec<_>>(), ["T-2"]);
        assert_eq!(document["tasks"][0]["status"], json!("todo"));
        assert_eq!(
            document["tasks"][0]["notes"][0]["text"],
            json!("lease expired (a)")
        );
        let own = own_lease_expired(
            &expired,
            &document,
            &vocab,
            "T-1",
            Some("a"),
            Some("atm task state T-1 done"),
        )
        .unwrap();
        assert_eq!(
            own.to_string(),
            "T-1: your lease expired at 10:30 UTC (no heartbeat for 120 min), so it went back to todo. Your commit is kept. Run `atm task state T-1 in_progress`, then `atm task state T-1 done` again."
        );
        assert!(own_lease_expired(&expired, &document, &vocab, "T-1", Some("b"), None).is_none());
    }

    #[test]
    fn another_agents_claim_is_refused_with_exit_code_4() {
        let claims = map(json!({"T-1": {"agent": "a", "lease_expires_at": "x"}}));
        assert!(check_holder(&claims, "T-1", Some("a"), "done").is_ok());
        let error = check_holder(&claims, "T-1", Some("b"), "done").unwrap_err();
        assert_eq!(
            (error.to_string(), error.code()),
            ("can't done T-1: it is claimed by a until x".into(), 4)
        );
    }

    #[test]
    fn minutes_print_as_python_g_does() {
        assert_eq!(minutes_text(120.0), "120");
        assert_eq!(minutes_text(0.5), "0.5");
    }
}
