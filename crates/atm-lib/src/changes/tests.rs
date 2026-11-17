use atm_db::MemoryStore;
use serde_json::{Map, Value, json};

use super::*;

fn task(v: Value) -> Task {
    match v {
        Value::Object(m) => m,
        other => panic!("not a task: {other}"),
    }
}

fn project(tasks: Value, actor: &str) -> Project {
    let list: Vec<Task> = match tasks {
        Value::Array(items) => items.into_iter().map(task).collect(),
        other => panic!("not a list: {other}"),
    };
    Project::new(Box::new(
        MemoryStore::new(list, Vec::new(), &Map::new()).unwrap(),
    ))
    .with_actor(Some(actor.into()))
}

#[test]
fn start_claims_and_done_drops_the_claim_and_says_what_is_ready() {
    let mut p = project(
        json!([{"id": "T-1", "status": "todo"}, {"id": "T-2", "status": "todo", "dependencies": ["T-1"]}]),
        "claude:a",
    );
    let started = change_task_status(&mut p, Command::Start, "T-1", &Options::default()).unwrap();
    assert_eq!(started.previous, json!("todo"));
    assert_eq!(started.claim["agent"], json!("claude:a"));
    assert_eq!(started.task["started_by"], json!("claude:a"));
    let claims = p.store().read_claims();
    assert!(claims["claims"].get("T-1").is_some());

    let done = change_task_status(
        &mut p,
        Command::Done,
        "T-1",
        &Options {
            note: Some("ok".into()),
            ..Options::default()
        },
    )
    .unwrap();
    assert_eq!(done.claim, Value::Null);
    assert_eq!(done.unblocked, ["T-2"]);
    let claims = p.store().read_claims();
    assert!(claims["claims"].get("T-1").is_none());
    assert!(claims["completed"].get("T-1").is_some());
}

#[test]
fn another_agents_claim_and_an_unknown_task_are_refused() {
    let mut p = project(json!([{"id": "T-1", "status": "todo"}]), "claude:a");
    change_task_status(&mut p, Command::Start, "T-1", &Options::default()).unwrap();
    let mut other = p.with_actor(Some("claude:b".into()));
    let error =
        change_task_status(&mut other, Command::Done, "T-1", &Options::default()).unwrap_err();
    assert_eq!(error.code(), 4);
    assert!(
        error
            .to_string()
            .starts_with("can't done T-1: it is claimed by claude:a until ")
    );
    let error =
        change_task_status(&mut other, Command::Start, "T-9", &Options::default()).unwrap_err();
    assert_eq!(error.to_string(), "no task T-9");
}

// The claim rules (ATM-580): who may take a task, from which working copy, and when a claim ends

/// A project in the worktree `/wt/<branch>` on `branch`, sharing `claims` (the clone's claims store) with the others.
fn project_in(tasks: Value, actor: &str, branch: &str, claims: Value) -> Project {
    let list: Vec<Task> = match tasks {
        Value::Array(items) => items.into_iter().map(task).collect(),
        other => panic!("not a list: {other}"),
    };
    let store = MemoryStore::new(list, Vec::new(), &Map::new())
        .unwrap()
        .with_claims(task(json!({
            "claims": claims, "completed": {}, "waiting": {}, "resolved": {}, "hints": {}
        })))
        .with_here(&format!("/wt/{branch}"), branch);
    Project::new(Box::new(store)).with_actor(Some(actor.into()))
}

/// A claim on `task_id` by `agent` in the worktree of `branch`, its lease running out `until` seconds from now.
fn held(task_id: &str, agent: &str, branch: &str, until: i64) -> Value {
    let at = |s: i64| atm_common::time::format_iso(atm_common::time::now() + s);
    json!({
        "task": task_id, "agent": agent, "branch": branch, "worktree": format!("/wt/{branch}"),
        "claimed_at": at(-60), "lease_expires_at": at(until)
    })
}

fn claims_of(p: &Project) -> Map<String, Value> {
    p.store().read_claims()["claims"]
        .as_object()
        .cloned()
        .unwrap_or_default()
}

#[test]
fn a_free_task_is_claimed_with_this_working_copys_branch_and_worktree_and_a_lease() {
    let mut p = project_in(
        json!([{"id": "T-1", "status": "todo"}]),
        "alice",
        "main",
        json!({}),
    );
    let started = change_task_status(&mut p, Command::Start, "T-1", &Options::default()).unwrap();
    let claim = &claims_of(&p)["T-1"];
    assert_eq!(&started.claim, claim);
    assert_eq!(
        (&claim["agent"], &claim["branch"], &claim["worktree"]),
        (&json!("alice"), &json!("main"), &json!("/wt/main"))
    );
    let until = atm_common::time::parse_iso(claim["lease_expires_at"].as_str().unwrap()).unwrap();
    let minutes = (until - atm_common::time::now()) / 60;
    assert!(
        (119..=120).contains(&minutes),
        "the default lease is two hours: {minutes} min"
    );
}

#[test]
fn a_task_another_agent_holds_is_refused_to_every_command_that_changes_it() {
    let tasks = json!([{"id": "T-1", "status": "in_progress"}]);
    let claims = json!({"T-1": held("T-1", "alice", "main", 600)});
    let block = Options {
        waiting_on: Some("user".into()),
        questions: vec!["Which?".into()],
        ..Options::default()
    };
    for (command, options) in [
        (Command::Start, Options::default()),
        (Command::Done, Options::default()),
        (Command::Release, Options::default()),
        (
            Command::Fail,
            Options {
                reason: Some("no".into()),
                ..Options::default()
            },
        ),
        (Command::Block, block),
    ] {
        let mut bob = project_in(tasks.clone(), "bob", "main", claims.clone());
        let error = change_task_status(&mut bob, command, "T-1", &options).unwrap_err();
        assert_eq!(error.code(), 4, "{command:?}");
        assert!(
            error.to_string().contains("it is claimed by alice until "),
            "{command:?}: {error}"
        );
        // Nothing changed: the task is still alice's
        assert_eq!(claims_of(&bob)["T-1"]["agent"], json!("alice"));
        assert_eq!(
            bob.store().document().unwrap()["tasks"][0]["status"],
            json!("in_progress")
        );
    }
}

#[test]
fn a_task_claimed_in_another_worktree_is_refused_there_though_it_is_to_do_here() {
    // alice claimed T-1 in the main worktree; this worktree's copy still has it to do
    let tasks = json!([{"id": "T-1", "status": "todo"}, {"id": "T-2", "status": "todo"}]);
    let claims = json!({"T-1": held("T-1", "alice", "main", 600)});
    let mut bob = project_in(tasks.clone(), "bob", "agent-2", claims.clone());
    let error =
        change_task_status(&mut bob, Command::Start, "T-1", &Options::default()).unwrap_err();
    assert_eq!(error.code(), 4);
    assert!(
        error
            .to_string()
            .starts_with("can't start T-1: it is claimed by alice until ")
    );
    // The loop doesn't hand it out here either: it takes the next free one
    let next = next_task_for(&mut bob, &Selection::default(), false, true, None).unwrap();
    assert_eq!(next.task.unwrap()["id"], json!("T-2"));
    assert_eq!(next.claim.unwrap()["branch"], json!("agent-2"));
    let held_now = claims_of(&bob);
    assert_eq!(held_now["T-1"]["worktree"], json!("/wt/main"));
    assert_eq!(held_now["T-2"]["worktree"], json!("/wt/agent-2"));
}

#[test]
fn a_task_done_in_another_worktree_is_not_handed_out_here_before_the_branches_merge() {
    let store = MemoryStore::new(vec![task(json!({"id": "T-1", "status": "todo"}))], Vec::new(), &Map::new())
        .unwrap()
        .with_claims(task(json!({
            "claims": {}, "waiting": {}, "resolved": {}, "hints": {},
            "completed": {"T-1": {"task": "T-1", "agent": "alice", "branch": "main", "worktree": "/wt/main",
                                  "completed_at": "2026-11-17T10:00:00Z"}}
        })))
        .with_here("/wt/agent-2", "agent-2");
    let mut bob = Project::new(Box::new(store)).with_actor(Some("bob".into()));
    let next = next_task_for(&mut bob, &Selection::default(), false, true, None).unwrap();
    assert!(next.task.is_none());
    assert!(
        next.reason
            .unwrap()
            .contains("done in another worktree, not merged yet: T-1")
    );
}

#[test]
fn the_holder_starting_its_own_task_again_is_refused_and_keeps_its_claim() {
    let mut p = project_in(
        json!([{"id": "T-1", "status": "todo"}]),
        "alice",
        "main",
        json!({}),
    );
    change_task_status(&mut p, Command::Start, "T-1", &Options::default()).unwrap();
    let before = claims_of(&p)["T-1"].clone();
    let error = change_task_status(&mut p, Command::Start, "T-1", &Options::default()).unwrap_err();
    // Its own: the state machine's refusal (exit code 1), not another agent's claim (4)
    assert_eq!(
        (error.to_string(), error.code()),
        (
            "can't start T-1: it is in_progress, not todo or in_review".into(),
            1
        )
    );
    assert_eq!(claims_of(&p)["T-1"], before);
}

#[test]
fn a_released_task_is_free_again_and_another_agent_may_claim_it() {
    let tasks = json!([{"id": "T-1", "status": "in_progress"}]);
    let mut alice = project_in(
        tasks,
        "alice",
        "main",
        json!({"T-1": held("T-1", "alice", "main", 600)}),
    );
    let note = Options {
        note: Some("half done".into()),
        ..Options::default()
    };
    let released = change_task_status(&mut alice, Command::Release, "T-1", &note).unwrap();
    assert_eq!(released.claim, Value::Null);
    assert!(claims_of(&alice).is_empty());
    assert_eq!(released.task["status"], json!("todo"));
    let mut bob = alice.with_actor(Some("bob".into()));
    let started = change_task_status(&mut bob, Command::Start, "T-1", &Options::default()).unwrap();
    assert_eq!(started.claim["agent"], json!("bob"));
}

#[test]
fn an_expired_lease_is_taken_over_by_the_next_agent_and_its_holder_is_told() {
    let tasks = json!([{"id": "T-1", "status": "in_progress"}]);
    let claims = json!({"T-1": held("T-1", "alice", "main", -10)});
    // bob takes it over, by name or from the loop
    let mut bob = project_in(tasks.clone(), "bob", "main", claims.clone());
    let started = change_task_status(&mut bob, Command::Start, "T-1", &Options::default()).unwrap();
    assert_eq!(started.expired.len(), 1);
    assert_eq!(started.claim["agent"], json!("bob"));
    let mut carol = project_in(tasks.clone(), "carol", "main", claims.clone());
    let next = next_task_for(&mut carol, &Selection::default(), false, true, None).unwrap();
    assert_eq!(next.task.unwrap()["id"], json!("T-1"));
    assert_eq!(next.claim.unwrap()["agent"], json!("carol"));
    // alice, coming back to finish it, is told her lease ran out and how to go on (exit code 4)
    let mut alice = project_in(tasks, "alice", "main", claims);
    let error =
        change_task_status(&mut alice, Command::Done, "T-1", &Options::default()).unwrap_err();
    assert_eq!(error.code(), 4);
    assert!(
        error.to_string().starts_with("T-1: your lease expired at "),
        "{error}"
    );
    assert_eq!(
        alice.store().document().unwrap()["tasks"][0]["status"],
        json!("todo")
    );
}

#[test]
fn a_block_is_recorded_for_every_worktree_and_unblocking_ends_it() {
    let mut p = project(json!([{"id": "T-1", "status": "todo"}]), "alice");
    let block = Options {
        waiting_on: Some("user".into()),
        questions: vec!["Which?".into()],
        ..Options::default()
    };
    change_task_status(&mut p, Command::Block, "T-1", &block).unwrap();
    assert_eq!(
        p.store().read_claims()["waiting"]["T-1"]["waiting_on"],
        json!("user")
    );
    change_task_status(
        &mut p,
        Command::Unblock,
        "T-1",
        &Options {
            answers: vec!["A".into()],
            ..Options::default()
        },
    )
    .unwrap();
    assert!(p.store().read_claims()["waiting"].get("T-1").is_none());
}

#[test]
fn back_to_todo_is_decided_by_the_kind_the_task_is_in() {
    let p = project(
        json!([
            {"id": "T-1", "status": "in_progress"},
            {"id": "T-2", "status": "blocked"},
            {"id": "T-3", "status": "done"},
            {"id": "T-4", "status": "in_review"},
            {"id": "T-5", "status": "todo"}
        ]),
        "alice",
    );
    assert_eq!(todo_command(&p, "T-1", false).unwrap(), Command::Release);
    assert_eq!(todo_command(&p, "T-1", true).unwrap(), Command::Fail);
    assert_eq!(todo_command(&p, "T-2", false).unwrap(), Command::Unblock);
    assert_eq!(todo_command(&p, "T-3", false).unwrap(), Command::Reopen);
    assert_eq!(todo_command(&p, "T-4", false).unwrap(), Command::Drop);
    assert_eq!(
        todo_command(&p, "T-5", false).unwrap_err().to_string(),
        "T-5 is already todo"
    );
    assert_eq!(
        todo_command(&p, "T-9", false).unwrap_err().to_string(),
        "no task T-9"
    );
}

#[test]
fn next_shows_or_claims_the_loops_next_task_and_says_why_there_is_none() {
    let mut p = project(
        json!([
            {"id": "T-1", "status": "todo", "dependencies": ["T-2"]},
            {"id": "T-2", "status": "todo"}
        ]),
        "claude:a",
    );
    let sel = Selection::default();
    let shown = next_task_for(&mut p, &sel, false, false, None).unwrap();
    assert_eq!(shown.task.unwrap()["id"], json!("T-2"));
    assert!(shown.claim.is_none());
    let claimed = next_task_for(&mut p, &sel, false, true, None).unwrap();
    assert_eq!(
        claimed.task.as_ref().unwrap()["status"],
        json!("in_progress")
    );
    assert_eq!(claimed.claim.unwrap()["agent"], json!("claude:a"));
    assert!(p.store().read_claims()["claims"].get("T-2").is_some());
    let none = next_task_for(&mut p, &sel, false, false, None).unwrap();
    assert!(none.task.is_none());
    assert!(none.reason.is_some());
}
