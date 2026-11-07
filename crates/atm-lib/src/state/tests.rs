use serde_json::{Map, Value, json};

use super::*;

fn task(v: Value) -> Task {
    match v {
        Value::Object(m) => m,
        other => panic!("not a task: {other}"),
    }
}

fn vocab() -> Vocabulary {
    Vocabulary::of_meta(&Map::new()).unwrap()
}

fn run(t: &mut Task, command: Command, options: &Options, agent: Option<&str>) -> Result<Value> {
    let none = |_: &str| Ok(false);
    change_status(
        t,
        command,
        options,
        &vocab(),
        &[],
        agent,
        "2026-01-01T00:00:00Z",
        &none,
    )
}

/// The status of each kind in the default vocabulary.
fn status_of(kind: Kind) -> &'static str {
    match kind {
        Kind::Todo => "todo",
        Kind::Active => "in_progress",
        Kind::Review => "in_review",
        Kind::Blocked => "blocked",
        Kind::Done => "done",
        Kind::Skipped => "skipped",
    }
}

/// Options that pass every command's argument checks.
fn full_options() -> Options {
    Options {
        reason: Some("why".into()),
        waiting_on: Some("user".into()),
        questions: vec!["Q?".into()],
        ..Options::default()
    }
}

/// docs/task-states.md's transitions: (command, from, to). Every other pair is refused.
const TABLE: [(Command, Kind, Kind); 19] = [
    (Command::Start, Kind::Todo, Kind::Active),
    (Command::Start, Kind::Review, Kind::Active),
    (Command::Review, Kind::Active, Kind::Review),
    (Command::Done, Kind::Active, Kind::Done),
    (Command::Done, Kind::Review, Kind::Done),
    (Command::Fail, Kind::Active, Kind::Todo),
    (Command::Release, Kind::Active, Kind::Todo),
    (Command::Approve, Kind::Review, Kind::Review),
    (Command::RequestChanges, Kind::Review, Kind::Review),
    (Command::Reject, Kind::Review, Kind::Review),
    (Command::Drop, Kind::Review, Kind::Todo),
    (Command::Block, Kind::Todo, Kind::Blocked),
    (Command::Block, Kind::Active, Kind::Blocked),
    (Command::Unblock, Kind::Blocked, Kind::Todo),
    (Command::Reopen, Kind::Done, Kind::Todo),
    (Command::Reopen, Kind::Skipped, Kind::Todo),
    (Command::Skip, Kind::Todo, Kind::Skipped),
    (Command::Skip, Kind::Active, Kind::Skipped),
    (Command::Skip, Kind::Blocked, Kind::Skipped),
];

#[test]
fn every_kind_and_command_follows_the_table() {
    for command in Command::ALL {
        for kind in Kind::ALL {
            // In review, the review's decision opens the way out (start and done need approval or changes asked
            // for, drop and skip a rejection): give it the one that opens this command's way
            let review_state = match command {
                Command::Drop | Command::Skip => "rejected",
                _ => "approved",
            };
            let mut t =
                task(json!({"id": "T-1", "status": status_of(kind), "review_state": review_state}));
            let expected = TABLE
                .iter()
                .find(|(c, from, _)| *c == command && *from == kind)
                .map(|(_, _, to)| *to);
            let expected = expected
                .or((command == Command::Skip && kind == Kind::Review).then_some(Kind::Skipped));
            let result = run(&mut t, command, &full_options(), Some("alice"));
            if let Some(to) = expected {
                assert!(
                    result.is_ok(),
                    "{} from {kind:?}: {result:?}",
                    command.name()
                );
                assert_eq!(
                    vocab().kind_of(t.get("status")),
                    Some(to),
                    "{} from {kind:?}",
                    command.name()
                );
            } else {
                let error = result.expect_err("refused");
                assert!(
                    error
                        .to_string()
                        .starts_with(&format!("can't {} T-1: it is ", command.name())),
                    "{error}"
                );
            }
        }
    }
}

#[test]
fn the_destinations_agree_with_the_table() {
    for (command, _, to) in TABLE {
        assert_eq!(command.destination(), to, "{}", command.name());
    }
}

#[test]
fn start_needs_done_dependencies_and_records_who() {
    let none = |_: &str| Ok(false);
    let dep = task(json!({"id": "T-0", "status": "todo"}));
    let mut t = task(json!({"id": "T-1", "status": "todo", "dependencies": ["T-0"]}));
    let error = change_status(
        &mut t,
        Command::Start,
        &Options::default(),
        &vocab(),
        &[&dep],
        None,
        "x",
        &none,
    )
    .unwrap_err();
    assert_eq!(error.to_string(), "can't start T-1: waiting on T-0 (todo)");
    let mut t = task(json!({"id": "T-1", "status": "todo", "agent": "old", "completed_at": "x"}));
    assert_eq!(
        run(&mut t, Command::Start, &Options::default(), Some("bob")).unwrap(),
        json!("todo")
    );
    assert_eq!(
        Value::Object(t),
        json!({
            "id": "T-1", "status": "in_progress", "updated_at": "2026-01-01T00:00:00Z", "updated_by": "bob",
            "started_at": "2026-01-01T00:00:00Z", "started_by": "bob"
        })
    );
}

#[test]
fn done_records_the_outcome_and_a_batch_task_waits_for_the_sprint_review() {
    let mut t = task(json!({"id": "T-1", "status": "in_progress"}));
    let options = Options {
        note: Some("Shipped".into()),
        commit: Some("abc".into()),
        ..Options::default()
    };
    run(&mut t, Command::Done, &options, Some("bob")).unwrap();
    assert_eq!(t["commit"], json!("abc"));
    assert_eq!(t["notes"][0]["kind"], json!("outcome"));
    let mut b = task(json!({"id": "T-2", "status": "in_progress", "review": "batch"}));
    run(&mut b, Command::Done, &options, Some("claude:1")).unwrap();
    assert_eq!(
        (b["sprint_review"].clone(), b["notes"][0]["kind"].clone()),
        (json!("pending"), json!("review"))
    );
    let mut r = task(json!({"id": "T-3", "status": "in_progress", "review": "blocker"}));
    let error = run(&mut r, Command::Done, &options, Some("claude:1")).unwrap_err();
    assert_eq!(error.code(), 2);
    assert!(
        error
            .to_string()
            .starts_with("T-3 is reviewed (blocker): ask for it with atm task state T-3 in_review")
    );
}

#[test]
fn a_review_keeps_its_rounds_and_its_decision_opens_one_way_out() {
    let mut t = task(json!({"id": "T-1", "status": "in_progress"}));
    run(
        &mut t,
        Command::Review,
        &Options {
            note: Some("Check".into()),
            ..Options::default()
        },
        Some("a"),
    )
    .unwrap();
    assert_eq!(t["review_state"], json!("waiting"));
    let error = run(&mut t, Command::Done, &Options::default(), Some("a")).unwrap_err();
    assert_eq!(
        error.to_string(),
        "can't finish T-1: its review isn't approved (atm task review approve T-1 decides it)"
    );
    let error = run(&mut t, Command::Reject, &Options::default(), Some("a")).unwrap_err();
    assert_eq!(
        error.to_string(),
        "a review that isn't approved says why: --reason"
    );
    run(&mut t, Command::Approve, &Options::default(), Some("u")).unwrap();
    assert_eq!(
        (t["status"].clone(), t["review_state"].clone()),
        (json!("in_review"), json!("approved"))
    );
    run(&mut t, Command::Done, &Options::default(), Some("a")).unwrap();
    assert_eq!(
        t["reviews"],
        json!([{"from": "2026-01-01T00:00:00Z", "to": "2026-01-01T00:00:00Z", "outcome": "approved"}])
    );
    assert!(t.get("review_state").is_none());
}

#[test]
fn block_says_what_it_waits_for_and_unblock_turns_the_answers_into_a_note() {
    let mut t = task(json!({"id": "T-1", "status": "todo"}));
    let error = run(
        &mut t,
        Command::Block,
        &Options {
            waiting_on: Some("x".into()),
            ..Options::default()
        },
        None,
    )
    .unwrap_err();
    assert_eq!(error.to_string(), "--on must be user or agent");
    let error = run(
        &mut t,
        Command::Block,
        &Options {
            waiting_on: Some("user".into()),
            ..Options::default()
        },
        None,
    )
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        "say what it waits for: --question (repeatable) or --reason"
    );
    let block = Options {
        waiting_on: Some("user".into()),
        questions: vec!["Which?".into()],
        ..Options::default()
    };
    run(&mut t, Command::Block, &block, None).unwrap();
    assert_eq!(
        (t["waiting_on"].clone(), t["questions"].clone()),
        (json!("user"), json!(["Which?"]))
    );
    run(
        &mut t,
        Command::Unblock,
        &Options {
            answers: vec!["A".into()],
            ..Options::default()
        },
        Some("u"),
    )
    .unwrap();
    assert_eq!(t["status"], json!("todo"));
    assert_eq!(
        t["blocks"],
        json!([{"from": "2026-01-01T00:00:00Z", "to": "2026-01-01T00:00:00Z", "on": "user"}])
    );
    assert_eq!(
        t["notes"][0],
        json!({"at": "2026-01-01T00:00:00Z", "kind": "answer", "text": "Which? → A", "by": "u"})
    );
}

#[test]
fn skip_records_its_resolution_and_refuses_what_doesnt_fit() {
    let mut t = task(json!({"id": "T-1", "status": "todo"}));
    let bad = Options {
        reason: Some("r".into()),
        resolution: Some("done".into()),
        ..Options::default()
    };
    assert_eq!(
        run(&mut t, Command::Skip, &bad, None)
            .unwrap_err()
            .to_string(),
        "--resolution must be one of wont_do, obsolete, duplicate"
    );
    let itself = Options {
        reason: Some("r".into()),
        resolution: Some("duplicate".into()),
        duplicate_of: Some("T-1".into()),
        ..Options::default()
    };
    assert_eq!(
        run(&mut t, Command::Skip, &itself, None)
            .unwrap_err()
            .to_string(),
        "T-1 can't be a duplicate of itself"
    );
    let dup = Options {
        reason: Some("same".into()),
        resolution: Some("duplicate".into()),
        duplicate_of: Some("T-0".into()),
        ..Options::default()
    };
    run(&mut t, Command::Skip, &dup, None).unwrap();
    assert_eq!(
        (t["resolution"].clone(), t["duplicate_of"].clone()),
        (json!("duplicate"), json!("T-0"))
    );
    assert_eq!(t["notes"][0]["text"], json!("Skipped: same"));
}

#[test]
fn going_back_to_todo_keeps_or_drops_the_commit() {
    let mut t = task(json!({"id": "T-1", "status": "in_progress", "commit": "abc"}));
    run(
        &mut t,
        Command::Release,
        &Options {
            note: Some("half".into()),
            ..Options::default()
        },
        None,
    )
    .unwrap();
    assert_eq!(
        (t["commit"].clone(), t["notes"][0]["kind"].clone()),
        (json!("abc"), json!("handoff"))
    );
    let mut f = task(json!({"id": "T-2", "status": "in_progress", "commit": "abc"}));
    run(&mut f, Command::Fail, &Options::default(), None).unwrap();
    assert!(f.get("commit").is_none());
    assert_eq!(f["notes"][0]["text"], json!("verification failed"));
}
