//! Claims (the Python atm's `lib/claims.py`): a claim's renewal (`claim heartbeat`, `claim hold`'s watcher), the
//! renewal every command an agent runs makes of its own claims once they are ten minutes old (T-439), and the live
//! claims. Each runs as one change of the store; the loop's next task is [`crate::changes::next_task_for`].

use atm_common::time::{now, now_iso, parse_iso};
use atm_common::{Error, Result, Vocabulary};
use atm_db::working::Working;
use serde_json::{Map, Value, json};

use crate::leases::{check_holder, drop_expired_claims, lease_until, own_lease_expired};
use crate::loop_rules::lease_minutes;
use crate::project::Project;

/// `heartbeat --auto`, and every command, renew a claim once its last renewal is this old (CLI-6.2).
pub const AUTO_RENEW_AFTER_SECONDS: i64 = 10 * 60;

/// A renewed claim: the claim, the lease's length in minutes, and the claims that expired on the way.
#[derive(Debug, Clone)]
pub struct Renewed {
    /// The claim, with its new lease.
    pub claim: Value,
    /// The project's lease length, in minutes.
    pub minutes: f64,
    /// The claims whose lease ran out on the way.
    pub expired: Vec<Value>,
}

/// The claims section of the claims store, made a map when it isn't one.
fn claims_of(store: &mut Map<String, Value>) -> &mut Map<String, Value> {
    let entry = store.entry("claims").or_insert_with(|| json!({}));
    if !entry.is_object() {
        *entry = json!({});
    }
    entry
        .as_object_mut()
        .expect("the section was made an object just above")
}

fn no_live_claim(task_id: &str, gone: bool) -> Error {
    let why = if gone { " (its lease expired)" } else { "" };
    Error::Failed {
        message: format!("{task_id} has no live claim{why}"),
        code: 4,
    }
}

fn renew(claim: &mut Value, document: &Value) {
    claim["lease_expires_at"] = json!(lease_until(document));
    claim["renewed_at"] = json!(now_iso());
}

/// Extend the actor's lease on a task (`atm claim heartbeat`; `claim hold`'s watcher with `expire` false: it renews a
/// live claim and lets nothing else expire).
///
/// # Errors
/// Exit code 4 when the task has no live claim (its lease expired: the agent's own says how to go on) or another agent
/// holds it; a store error.
pub fn renew_lease(project: &mut Project, task_id: &str, expire: bool) -> Result<Renewed> {
    let agent = project.actor().map(String::from);
    let agent = agent.as_deref();
    let mut outcome: Option<Renewed> = None;
    project.store_mut().change(&mut |work: &mut Working| {
        let document = work.document.clone();
        if !expire {
            let claims = claims_of(&mut work.claims_store);
            let alive = claims
                .get(task_id)
                .and_then(|c| c.get("lease_expires_at"))
                .and_then(Value::as_str)
                .and_then(parse_iso)
                .is_some_and(|until| until > now());
            if !alive {
                return Err(no_live_claim(task_id, false));
            }
            check_holder(claims, task_id, agent, "renew the claim on")?;
            let claim = claims
                .get_mut(task_id)
                .ok_or_else(|| no_live_claim(task_id, false))?;
            renew(claim, &document);
            outcome = Some(Renewed {
                claim: claim.clone(),
                minutes: lease_minutes(&document),
                expired: Vec::new(),
            });
            return Ok(());
        }
        let vocab = Vocabulary::of_document(&work.document)?;
        let mut claims = claims_of(&mut work.claims_store).clone();
        let expired = drop_expired_claims(&mut work.document, &mut claims, &vocab);
        *claims_of(&mut work.claims_store) = claims;
        let claims = claims_of(&mut work.claims_store);
        if !claims.get(task_id).is_some_and(atm_common::py::truthy) {
            let own = own_lease_expired(&expired, &work.document, &vocab, task_id, agent, None);
            let gone = expired
                .iter()
                .any(|c| c.get("task").and_then(Value::as_str) == Some(task_id));
            // The expiry is written first, so `atm task details` agrees with the error
            work.defer(own.unwrap_or_else(|| no_live_claim(task_id, gone)));
            return Ok(());
        }
        check_holder(claims, task_id, agent, "heartbeat")?;
        let document = work.document.clone();
        let claim = claims_of(&mut work.claims_store)
            .get_mut(task_id)
            .ok_or_else(|| no_live_claim(task_id, false))?;
        renew(claim, &document);
        outcome = Some(Renewed {
            claim: claim.clone(),
            minutes: lease_minutes(&document),
            expired,
        });
        Ok(())
    })?;
    outcome.ok_or_else(|| no_live_claim(task_id, false))
}

/// The actor's live claims whose last renewal (else their start) is ten minutes old or more, by task.
fn due(claims: &Map<String, Value>, agent: Option<&str>) -> Vec<String> {
    let current = now();
    let when = |c: &Value, key: &str| c.get(key).and_then(Value::as_str).and_then(parse_iso);
    claims
        .iter()
        .filter(|(_, c)| {
            c.is_object()
                && c.get("agent").and_then(Value::as_str) == agent
                && when(c, "lease_expires_at").is_some_and(|until| until > current)
                && c.get("renewed_at")
                    .filter(|r| atm_common::py::truthy(r))
                    .map_or_else(|| when(c, "claimed_at"), |r| r.as_str().and_then(parse_iso))
                    .is_none_or(|last| current - last >= AUTO_RENEW_AFTER_SECONDS)
        })
        .map(|(id, _)| id.clone())
        .collect()
}

/// `heartbeat --auto`, and every command an agent runs: renew the actor's live claims whose last renewal is ten minutes
/// old. Most calls only read the claims (no change, nothing written). Returns the claims renewed.
///
/// # Errors
/// A store error.
pub fn renew_due_claims(project: &mut Project) -> Result<Vec<Value>> {
    let agent = project.actor().map(String::from);
    let agent = agent.as_deref();
    let read = project.store().read_claims();
    let empty = Map::new();
    let claims = read
        .get("claims")
        .and_then(Value::as_object)
        .unwrap_or(&empty);
    if due(claims, agent).is_empty() {
        return Ok(Vec::new());
    }
    let mut renewed = Vec::new();
    project.store_mut().change(&mut |work: &mut Working| {
        let document = work.document.clone();
        let claims = claims_of(&mut work.claims_store);
        renewed = due(claims, agent)
            .into_iter()
            .filter_map(|id| {
                let claim = claims.get_mut(&id)?;
                renew(claim, &document);
                Some(claim.clone())
            })
            .collect();
        Ok(())
    })?;
    Ok(renewed)
}

/// The live claims, by task, after dropping those whose lease ran out (their tasks back to to do).
#[derive(Debug, Clone)]
pub struct LiveClaims {
    /// The live claims, ordered by task.
    pub claims: Vec<Value>,
    /// The claims that expired.
    pub expired: Vec<Value>,
}

/// `atm claim list`: drop the claims whose lease ran out, and give the live ones, by task.
///
/// # Errors
/// A store error.
pub fn expire_claims(project: &mut Project) -> Result<LiveClaims> {
    let mut outcome = LiveClaims {
        claims: Vec::new(),
        expired: Vec::new(),
    };
    project.store_mut().change(&mut |work: &mut Working| {
        let vocab = Vocabulary::of_document(&work.document)?;
        let mut claims = claims_of(&mut work.claims_store).clone();
        let expired = drop_expired_claims(&mut work.document, &mut claims, &vocab);
        let mut live: Vec<Value> = claims.values().cloned().collect();
        live.sort_by(|a, b| {
            atm_common::py::py_str_opt(a.get("task"))
                .cmp(&atm_common::py::py_str_opt(b.get("task")))
        });
        *claims_of(&mut work.claims_store) = claims;
        outcome = LiveClaims {
            claims: live,
            expired,
        };
        Ok(())
    })?;
    Ok(outcome)
}

#[cfg(test)]
mod tests;
