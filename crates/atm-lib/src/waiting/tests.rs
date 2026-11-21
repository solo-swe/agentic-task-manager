use serde_json::{Map, Value, json};

use super::*;

fn task(fields: Value) -> Task {
    let Value::Object(mut task) = fields else {
        panic!("a task is an object");
    };
    task.entry("id").or_insert(json!("T-1"));
    task
}

const AT: &str = "2026-11-21T10:00:00Z";
const DECIDED: &str = "2026-11-21T11:00:00Z";

/// One row: the task's fields and context, and the answer: who it waits on (none: `-`), since which of its fields,
/// whether it needs the user and why, and whether agent work is left.
#[allow(clippy::too_many_lines)] // reason: one table, a row per case
#[test]
fn who_a_task_waits_on_for_every_kind_review_state_blocker_and_readiness() {
    let vocab = Vocabulary::of_meta(&Map::new()).unwrap();
    let ready = Context {
        ready: true,
        waiting_for: &[],
        in_scope: true,
    };
    let deps = ["T-2".to_string()];
    let not_ready = Context {
        ready: false,
        waiting_for: &deps,
        in_scope: true,
    };
    let out_of_scope = Context {
        in_scope: false,
        ..ready
    };
    let reviewed = |state: &str| json!({"status": "in_review", "review_state": state, "review_requested_at": AT, "reviewed_at": DECIDED});
    #[allow(clippy::type_complexity)] // reason: a table's row
    let rows: Vec<(
        &str,
        Value,
        Context,
        Option<On>,
        Option<&str>,
        Option<NeedsYou>,
        bool,
    )> = vec![
        // In review
        (
            "waiting for its review",
            json!({"status": "in_review", "review_requested_at": AT}),
            ready,
            Some(On::Reviewer),
            Some(AT),
            Some(NeedsYou::Review),
            false,
        ),
        (
            "review state waiting",
            reviewed("waiting"),
            ready,
            Some(On::Reviewer),
            Some(AT),
            Some(NeedsYou::Review),
            false,
        ),
        (
            "approved (ATM-711's card)",
            reviewed("approved"),
            ready,
            Some(On::Agent),
            Some(DECIDED),
            None,
            true,
        ),
        (
            "sent back",
            reviewed("changes_requested"),
            ready,
            Some(On::Agent),
            Some(DECIDED),
            None,
            true,
        ),
        (
            "rejected: the user sends it back or skips it",
            reviewed("rejected"),
            ready,
            Some(On::User),
            Some(DECIDED),
            Some(NeedsYou::Review),
            false,
        ),
        (
            "in review by hand, no request time: since its last update",
            json!({"status": "in_review", "updated_at": AT}),
            ready,
            Some(On::Reviewer),
            Some(AT),
            Some(NeedsYou::Review),
            false,
        ),
        // Blocked
        (
            "blocked on the user",
            json!({"status": "blocked", "waiting_on": "user", "blocked_at": AT, "questions": ["Which?"]}),
            ready,
            Some(On::User),
            Some(AT),
            Some(NeedsYou::Blocked),
            false,
        ),
        (
            "blocked on an agent",
            json!({"status": "blocked", "waiting_on": "agent", "blocked_at": AT}),
            ready,
            Some(On::Agent),
            Some(AT),
            None,
            false,
        ),
        (
            "blocked by hand",
            json!({"status": "blocked"}),
            ready,
            Some(On::Unknown),
            None,
            None,
            false,
        ),
        // To do
        (
            "to do and ready",
            json!({"status": "todo"}),
            ready,
            None,
            None,
            None,
            true,
        ),
        (
            "to do, a dependency open",
            json!({"status": "todo"}),
            not_ready,
            Some(On::Dependencies),
            None,
            None,
            true,
        ),
        (
            "with you, ready, in the active sprint",
            json!({"status": "todo", "with": "user"}),
            ready,
            Some(On::User),
            None,
            Some(NeedsYou::Together),
            true,
        ),
        (
            "with you, outside the active sprint",
            json!({"status": "todo", "with": "user"}),
            out_of_scope,
            Some(On::User),
            None,
            None,
            true,
        ),
        (
            "with you, a dependency open: its dependencies first",
            json!({"status": "todo", "with": "user"}),
            not_ready,
            Some(On::Dependencies),
            None,
            None,
            true,
        ),
        // Active, finished, unknown
        (
            "active",
            json!({"status": "in_progress"}),
            ready,
            None,
            None,
            None,
            true,
        ),
        (
            "done",
            json!({"status": "done"}),
            ready,
            None,
            None,
            None,
            false,
        ),
        (
            "done, an old review state left on it",
            json!({"status": "done", "review_state": "approved"}),
            ready,
            None,
            None,
            None,
            false,
        ),
        (
            "skipped",
            json!({"status": "skipped"}),
            ready,
            None,
            None,
            None,
            false,
        ),
        (
            "a status atm doesn't know",
            json!({"status": "parked"}),
            ready,
            None,
            None,
            None,
            true,
        ),
    ];
    let mut wrong = Vec::new();
    for (name, fields, context, on, since, needs, work) in rows {
        let answer = of(&vocab, &task(fields), context);
        let got = (
            answer.waiting.as_ref().map(|w| w.on),
            answer.waiting.as_ref().and_then(|w| w.since.clone()),
            answer.needs_you,
            answer.agent_work,
        );
        let wanted = (on, since.map(String::from), needs, work);
        if got != wanted {
            wrong.push(format!("{name}: got {got:?}, wanted {wanted:?}"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn a_blocked_tasks_questions_come_with_it() {
    let vocab = Vocabulary::of_meta(&Map::new()).unwrap();
    let answer = of(
        &vocab,
        &task(json!({"status": "blocked", "waiting_on": "user", "questions": ["One?", 2, "Two?"]})),
        Context {
            ready: false,
            waiting_for: &[],
            in_scope: true,
        },
    );
    assert_eq!(answer.waiting.unwrap().questions, ["One?", "Two?"]);
}

#[test]
fn the_review_a_task_waits_on_is_its_latest_open_one_else_its_latest() {
    let links = json!({"code_reviews": [
        {"id": "a", "ref": "CR-1"}, {"id": "b", "ref": "CR-2"}, {"id": "c", "ref": "CR-3"}
    ]});
    let t = task(links);
    let statuses = |pairs: &[(&str, &str)]| -> Map<String, Value> {
        pairs
            .iter()
            .map(|(id, s)| ((*id).to_string(), json!(s)))
            .collect()
    };
    // An older one still open, the newest closed: the open one
    assert_eq!(
        review_of(
            &t,
            &statuses(&[("a", "approved"), ("b", "waiting"), ("c", "closed")])
        ),
        Some(json!({"id": "b", "ref": "CR-2", "status": "waiting"}))
    );
    // None open: the latest
    assert_eq!(
        review_of(
            &t,
            &statuses(&[("a", "approved"), ("b", "closed"), ("c", "approved")])
        ),
        Some(json!({"id": "c", "ref": "CR-3", "status": "approved"}))
    );
    // Ones this working copy doesn't have aren't open here: the open one it has
    assert_eq!(
        review_of(&t, &statuses(&[("a", "changes_requested")])),
        Some(json!({"id": "a", "ref": "CR-1", "status": "changes_requested"}))
    );
    // None it has (another branch's): the latest link, without a status
    assert_eq!(
        review_of(&t, &Map::new()),
        Some(json!({"id": "c", "ref": "CR-3"}))
    );
    // No links: none
    assert_eq!(
        review_of(&task(json!({})), &statuses(&[("a", "waiting")])),
        None
    );
}

#[test]
fn work_with_you_is_due_in_the_active_sprint_or_when_none_is_active() {
    let meta = |state: &str| {
        let m =
            json!({"sprints": [{"id": "S1", "state": state}, {"id": "S2", "state": "planned"}]});
        Vocabulary::of_meta(m.as_object().unwrap()).unwrap()
    };
    let in_s1 = task(json!({"sprint": "s1"}));
    let in_s2 = task(json!({"sprint": "S2"}));
    let in_none = task(json!({}));
    assert!(
        in_active_scope(&meta("active"), &in_s1),
        "sprint names in any case"
    );
    assert!(!in_active_scope(&meta("active"), &in_s2));
    assert!(!in_active_scope(&meta("active"), &in_none));
    // No active sprint: all of them
    for t in [&in_s1, &in_s2, &in_none] {
        assert!(in_active_scope(&meta("closed"), t));
    }
}
