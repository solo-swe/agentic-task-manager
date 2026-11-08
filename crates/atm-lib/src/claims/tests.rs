use atm_common::time::format_iso;
use atm_db::MemoryStore;
use serde_json::{Map, Value, json};

use super::*;

#[allow(clippy::needless_pass_by_value)] // reason: the tests build the claims inline, with json!
fn project(claims: Value, actor: &str) -> Project {
    let task = |v: Value| v.as_object().unwrap().clone();
    let store = MemoryStore::new(
        vec![task(json!({"id": "T-1", "status": "in_progress"}))],
        Vec::new(),
        &Map::new(),
    )
    .unwrap()
    .with_claims(task(json!({ "claims": claims })));
    Project::new(Box::new(store)).with_actor(Some(actor.into()))
}

fn claim(agent: &str, claimed: i64, until: i64) -> Value {
    json!({
        "task": "T-1", "agent": agent,
        "claimed_at": format_iso(now() + claimed), "lease_expires_at": format_iso(now() + until)
    })
}

#[test]
fn a_heartbeat_renews_the_holders_claim_and_refuses_anyone_elses() {
    let mut p = project(json!({"T-1": claim("alice", -60, 600)}), "alice");
    let renewed = renew_lease(&mut p, "T-1", true).unwrap();
    assert!(renewed.claim.get("renewed_at").is_some());
    assert!((renewed.minutes - 120.0).abs() < f64::EPSILON);
    let mut bob = project(json!({"T-1": claim("alice", -60, 600)}), "bob");
    assert_eq!(renew_lease(&mut bob, "T-1", false).unwrap_err().code(), 4);
    let mut none = project(json!({}), "alice");
    assert_eq!(
        renew_lease(&mut none, "T-1", false)
            .unwrap_err()
            .to_string(),
        "T-1 has no live claim"
    );
}

#[test]
fn only_claims_ten_minutes_old_are_renewed_by_the_auto_heartbeat() {
    let mut fresh = project(json!({"T-1": claim("alice", -60, 600)}), "alice");
    assert_eq!(renew_due_claims(&mut fresh).unwrap().len(), 0);
    let mut old = project(json!({"T-1": claim("alice", -700, 600)}), "alice");
    assert_eq!(renew_due_claims(&mut old).unwrap().len(), 1);
    let mut other = project(json!({"T-1": claim("alice", -700, 600)}), "bob");
    assert_eq!(renew_due_claims(&mut other).unwrap().len(), 0);
}

#[test]
fn an_expired_claim_is_dropped_and_its_task_goes_back_to_todo() {
    let mut p = project(json!({"T-1": claim("alice", -7000, -10)}), "bob");
    let live = expire_claims(&mut p).unwrap();
    assert_eq!(live.claims.len(), 0);
    assert_eq!(live.expired.len(), 1);
    let document = p.store().document().unwrap();
    assert_eq!(document["tasks"][0]["status"], json!("todo"));
}
