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
    assert_eq!(claimed.task.as_ref().unwrap()["status"], json!("in_progress"));
    assert_eq!(claimed.claim.unwrap()["agent"], json!("claude:a"));
    assert!(p.store().read_claims()["claims"].get("T-2").is_some());
    let none = next_task_for(&mut p, &sel, false, false, None).unwrap();
    assert!(none.task.is_none());
    assert!(none.reason.is_some());
}
