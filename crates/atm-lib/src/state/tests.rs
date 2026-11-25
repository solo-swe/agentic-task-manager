use serde_json::{Map, Value, json};

use super::*;

mod docs;

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
    let none = |_: &str| Ok(None);
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
const TABLE: [(Command, Kind, Kind); 20] = [
    (Command::Start, Kind::Todo, Kind::Active),
    (Command::Start, Kind::Review, Kind::Active),
    (Command::Review, Kind::Active, Kind::Review),
    // A round on a review sent back (ATM-713)
    (Command::Review, Kind::Review, Kind::Review),
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
                Command::Review => "changes_requested",
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

/// ATM-713 (docs/task-review-state-machine.md §3.2: changes requested → waiting, a round): a review sent back asks for
/// review again, its last round kept with its outcome; waiting or decided otherwise, it is the review's to decide
#[test]
fn a_review_sent_back_is_asked_for_again_and_only_then() {
    let mut sent_back = task(json!({
        "id": "T-1", "status": "in_review", "review_state": "changes_requested",
        "review_requested_at": "2026-11-01T10:00:00Z", "reviewed_at": "2026-11-01T11:00:00Z",
    }));
    let options = Options {
        note: Some("CR-1: round 2".into()),
        ..Options::default()
    };
    run(&mut sent_back, Command::Review, &options, Some("claude:1")).unwrap();
    assert_eq!(sent_back["status"], "in_review");
    assert_eq!(sent_back["review_state"], "waiting");
    assert!(sent_back.get("reviewed_at").is_none());
    assert_eq!(
        sent_back["reviews"],
        json!([{ "from": "2026-11-01T10:00:00Z", "to": "2026-11-01T11:00:00Z", "outcome": "changes_requested" }])
    );
    for state in ["waiting", "approved", "rejected"] {
        let mut t = task(json!({"id": "T-1", "status": "in_review", "review_state": state}));
        let error = run(&mut t, Command::Review, &options, Some("claude:1")).unwrap_err();
        assert!(
            error
                .to_string()
                .starts_with("can't ask for T-1's review again: it is in review"),
            "{state}: {error}"
        );
        assert_eq!(t["review_state"], state, "{state}: unchanged");
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
    let none = |_: &str| Ok(None);
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
    assert!(error.to_string().starts_with(
        "T-3 is reviewed (blocker): open its code review with atm review create --task T-3"
    ));
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

fn opts(f: impl FnOnce(&mut Options)) -> Options {
    let mut o = Options::default();
    f(&mut o);
    o
}

/// A project with two statuses of the review kind (and the others one each).
fn two_review_statuses() -> Vocabulary {
    let meta = json!({"statuses": [
        {"id": "todo", "kind": "todo"}, {"id": "doing", "kind": "active"},
        {"id": "qa", "kind": "review"}, {"id": "uat", "kind": "review"},
        {"id": "done", "kind": "done"}, {"id": "skipped", "kind": "skipped"},
        {"id": "blocked", "kind": "blocked"}
    ]});
    Vocabulary::of_meta(meta.as_object().unwrap()).unwrap()
}

fn run_in(
    vocab: &Vocabulary,
    t: &mut Task,
    command: Command,
    options: &Options,
    agent: Option<&str>,
) -> Result<Value> {
    let none = |_: &str| Ok(None);
    change_status(t, command, options, vocab, &[], agent, "S", &none)
}

#[test]
fn start_from_review_needs_its_decision_to_send_it_back_to_work() {
    for (state, allowed) in [
        ("waiting", false),
        ("rejected", false),
        ("approved", true),
        ("changes_requested", true),
    ] {
        let mut t = task(json!({
            "id": "T-1", "status": "in_review", "review_state": state,
            "review_requested_at": "R", "reviewed_at": "D"
        }));
        let result = run(&mut t, Command::Start, &Options::default(), Some("a"));
        if allowed {
            result.unwrap();
            assert_eq!(t["status"], json!("in_progress"), "{state}");
            // the round ends at the decision, with its outcome; a new round of work starts
            assert_eq!(
                t["reviews"],
                json!([{"from": "R", "to": "D", "outcome": state}])
            );
            for gone in ["review_state", "reviewed_at", "review_requested_at"] {
                assert!(t.get(gone).is_none(), "{state}: {gone}");
            }
        } else {
            assert_eq!(
                result.unwrap_err().to_string(),
                "can't start T-1: it waits for its review (atm task review approve or request-changes T-1 decides it)",
                "{state}"
            );
            assert_eq!(t["status"], json!("in_review"), "{state}: unchanged");
        }
    }
}

#[test]
fn a_review_round_without_a_decision_ends_when_the_task_leaves_review() {
    // skipped after a rejection with no reviewed_at: the round ends now
    let mut t = task(json!({
        "id": "T-1", "status": "in_review", "review_state": "rejected", "review_requested_at": "R"
    }));
    run(&mut t, Command::Skip, &Options::default(), None).unwrap();
    assert_eq!(
        t["reviews"],
        json!([{"from": "R", "to": "2026-01-01T00:00:00Z", "outcome": "rejected"}])
    );
}

#[test]
fn drop_and_skip_from_review_need_a_rejection() {
    for (command, way) in [
        (Command::Drop, "send back to to do"),
        (Command::Skip, "skip"),
    ] {
        for state in ["waiting", "approved", "changes_requested"] {
            let mut t = task(json!({"id": "T-1", "status": "in_review", "review_state": state}));
            assert_eq!(
                run(&mut t, command, &Options::default(), None)
                    .unwrap_err()
                    .to_string(),
                format!(
                    "can't {way} T-1: its review isn't rejected (atm task review reject T-1 --reason … decides it)"
                ),
                "{state}"
            );
        }
    }
}

#[test]
fn drop_undoes_the_rejected_work() {
    let mut t = task(json!({
        "id": "T-1", "status": "in_review", "review_state": "rejected", "commit": "abc",
        "started_by": "a", "review_requested_at": "R"
    }));
    run(&mut t, Command::Drop, &Options::default(), Some("u")).unwrap();
    assert_eq!(t["status"], json!("todo"));
    for gone in [
        "commit",
        "started_by",
        "review_requested_at",
        "review_state",
    ] {
        assert!(t.get(gone).is_none(), "{gone}");
    }
}

#[test]
fn the_review_decisions_record_what_was_decided() {
    let in_review = || task(json!({"id": "T-1", "status": "in_review", "review_state": "waiting"}));
    let mut t = in_review();
    run(
        &mut t,
        Command::Approve,
        &opts(|o| o.note = Some("fine".into())),
        Some("u"),
    )
    .unwrap();
    assert_eq!(
        (t["reviewed_at"].clone(), t["notes"][0]["text"].clone()),
        (json!("2026-01-01T00:00:00Z"), json!("Approved: fine"))
    );
    let mut t = in_review();
    run(&mut t, Command::Approve, &Options::default(), Some("u")).unwrap();
    assert_eq!(t["notes"][0]["text"], json!("Approved"));
    for (command, state) in [
        (Command::RequestChanges, "changes_requested"),
        (Command::Reject, "rejected"),
    ] {
        let mut t = in_review();
        assert_eq!(
            run(&mut t, command, &Options::default(), Some("u"))
                .unwrap_err()
                .to_string(),
            "a review that isn't approved says why: --reason"
        );
        run(
            &mut t,
            command,
            &opts(|o| o.reason = Some("no".into())),
            Some("u"),
        )
        .unwrap();
        assert_eq!(t["review_state"], json!(state));
        assert_eq!(
            (t["notes"][0]["kind"].clone(), t["notes"][0]["text"].clone()),
            (json!("rejection"), json!("no"))
        );
    }
}

#[test]
fn an_empty_reason_is_no_reason() {
    let mut t = task(json!({"id": "T-1", "status": "in_review", "review_state": "waiting"}));
    let error = run(
        &mut t,
        Command::Reject,
        &opts(|o| o.reason = Some(String::new())),
        None,
    )
    .unwrap_err();
    assert_eq!(error.code(), 2);
}

#[test]
fn a_decision_keeps_the_review_status_the_task_is_in() {
    let vocab = two_review_statuses();
    let mut t = task(json!({"id": "T-1", "status": "uat", "review_state": "waiting"}));
    run_in(&vocab, &mut t, Command::Approve, &Options::default(), None).unwrap();
    assert_eq!(t["status"], json!("uat"));
}

#[test]
fn status_chooses_among_the_destination_kinds_statuses() {
    let vocab = two_review_statuses();
    let mut t = task(json!({"id": "T-1", "status": "doing"}));
    run_in(&vocab, &mut t, Command::Review, &Options::default(), None).unwrap();
    assert_eq!(t["status"], json!("qa"), "the first of the kind by default");
    let mut t = task(json!({"id": "T-1", "status": "doing"}));
    run_in(
        &vocab,
        &mut t,
        Command::Review,
        &opts(|o| o.status = Some("uat".into())),
        None,
    )
    .unwrap();
    assert_eq!(t["status"], json!("uat"));
    for (chosen, why) in [
        ("done", "its kind is done"),
        ("nope", "the project has no such status"),
    ] {
        let mut t = task(json!({"id": "T-1", "status": "doing"}));
        let error = run_in(
            &vocab,
            &mut t,
            Command::Review,
            &opts(|o| o.status = Some(chosen.into())),
            None,
        )
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            format!("--status {chosen} is not one of the review statuses ({why}); use qa or uat")
        );
        assert_eq!(t["status"], json!("doing"), "unchanged");
    }
}

#[test]
fn a_refusal_names_every_status_the_command_applies_to() {
    let vocab = two_review_statuses();
    let mut t = task(json!({"id": "T-1", "status": "todo"}));
    assert_eq!(
        run_in(&vocab, &mut t, Command::Approve, &Options::default(), None)
            .unwrap_err()
            .to_string(),
        "can't approve T-1: it is todo, not qa or uat"
    );
}

#[test]
fn a_task_without_a_known_status_counts_as_to_do() {
    for status in [json!(null), json!("nonsense")] {
        let mut t = task(json!({"id": "T-1", "status": status}));
        run(&mut t, Command::Start, &Options::default(), None).unwrap();
        assert_eq!(t["status"], json!("in_progress"));
    }
}

#[test]
fn a_dependency_is_done_when_it_is_finished_or_archived() {
    let archived =
        |id: &str| Ok((id == "T-A").then(|| task(json!({"id": "T-A", "status": "done"}))));
    for (deps, waiting) in [
        (json!(["T-D"]), None),
        (json!(["T-A"]), None),
        (json!(["T-A", "T-W"]), Some("T-W (in_progress)")),
        // a skipped task isn't done: what depends on it waits
        (json!(["T-S"]), Some("T-S (skipped)")),
        (json!(["T-X"]), Some("T-X (missing)")),
    ] {
        let done = task(json!({"id": "T-D", "status": "done"}));
        let skipped = task(json!({"id": "T-S", "status": "skipped"}));
        let working = task(json!({"id": "T-W", "status": "in_progress"}));
        let mut t = task(json!({"id": "T-1", "status": "todo", "dependencies": deps}));
        let result = change_status(
            &mut t,
            Command::Start,
            &Options::default(),
            &vocab(),
            &[&done, &skipped, &working],
            None,
            "S",
            &archived,
        );
        match waiting {
            None => assert!(result.is_ok(), "{deps}: {result:?}"),
            Some(on) => assert_eq!(
                result.unwrap_err().to_string(),
                format!("can't start T-1: waiting on {on}")
            ),
        }
    }
}

#[test]
fn done_asks_an_agent_for_the_review_its_mode_needs() {
    for (mode, agent, outcome) in [
        // blocker and async: an agent asks for the review; a person may finish it
        ("blocker", Some("claude:1"), None),
        ("async", Some("claude:1"), None),
        ("blocker", Some("alice"), Some("outcome")),
        ("async", None, Some("outcome")),
        // batch: a person's done is an ordinary done
        ("batch", Some("alice"), Some("outcome")),
        ("none", Some("claude:1"), Some("outcome")),
    ] {
        let mut t = task(json!({"id": "T-1", "status": "in_progress", "review": mode}));
        let result = run(
            &mut t,
            Command::Done,
            &opts(|o| o.note = Some("n".into())),
            agent,
        );
        match outcome {
            None => assert_eq!(result.unwrap_err().code(), 2, "{mode} {agent:?}"),
            Some(kind) => {
                result.unwrap();
                assert_eq!(t["notes"][0]["kind"], json!(kind), "{mode} {agent:?}");
                assert!(t.get("sprint_review").is_none(), "{mode} {agent:?}");
            }
        }
    }
}

#[test]
fn done_from_an_approved_review_records_who_finished_it_and_no_sprint_review() {
    let mut t = task(json!({
        "id": "T-1", "status": "in_review", "review": "batch", "review_state": "approved"
    }));
    run(&mut t, Command::Done, &Options::default(), Some("claude:1")).unwrap();
    assert_eq!(t["completed_by"], json!("claude:1"));
    assert!(t.get("sprint_review").is_none());
}

#[test]
fn skip_without_a_resolution_wont_do_it_and_refuses_a_stray_duplicate_of() {
    let mut t = task(json!({"id": "T-1", "status": "todo"}));
    assert_eq!(
        run(
            &mut t,
            Command::Skip,
            &opts(|o| o.duplicate_of = Some("T-0".into())),
            None
        )
        .unwrap_err()
        .to_string(),
        "--duplicate-of goes with --resolution duplicate"
    );
    run(&mut t, Command::Skip, &Options::default(), None).unwrap();
    assert!(t.get("resolution").is_none(), "won't do is the default");
    assert_eq!(t["notes"][0]["text"], json!("Skipped: None"));
    assert_eq!(t["skipped_at"], json!("2026-01-01T00:00:00Z"));
}

#[test]
fn skip_checks_its_resolution_before_the_tasks_status() {
    let mut t = task(json!({"id": "T-1", "status": "done"}));
    let error = run(
        &mut t,
        Command::Skip,
        &opts(|o| o.resolution = Some("done".into())),
        None,
    )
    .unwrap_err();
    assert!(error.to_string().starts_with("--resolution must be one of"));
}

#[test]
fn skipping_a_blocked_task_ends_its_block() {
    let mut t = task(json!({
        "id": "T-1", "status": "blocked", "waiting_on": "agent", "questions": ["Q"], "blocked_at": "B"
    }));
    run(
        &mut t,
        Command::Skip,
        &opts(|o| o.resolution = Some("obsolete".into())),
        None,
    )
    .unwrap();
    assert_eq!(
        t["blocks"],
        json!([{"from": "B", "to": "2026-01-01T00:00:00Z", "on": "agent"}])
    );
    for gone in ["waiting_on", "questions", "blocked_at"] {
        assert!(t.get(gone).is_none(), "{gone}");
    }
    assert_eq!(t["resolution"], json!("obsolete"));
}

#[test]
fn block_with_a_reason_notes_it_and_keeps_who_started_the_work() {
    let mut t = task(json!({"id": "T-1", "status": "in_progress", "started_by": "a"}));
    run(
        &mut t,
        Command::Block,
        &opts(|o| {
            o.waiting_on = Some("agent".into());
            o.reason = Some("needs T-2".into());
        }),
        Some("a"),
    )
    .unwrap();
    assert_eq!(
        (
            t["started_by"].clone(),
            t["questions"].clone(),
            t["notes"][0]["text"].clone()
        ),
        (json!("a"), json!([]), json!("needs T-2"))
    );
    assert_eq!(t["blocked_at"], json!("2026-01-01T00:00:00Z"));
}

#[test]
fn unblock_notes_every_question_answered_or_not_and_nothing_when_there_was_none() {
    let mut t = task(json!({"id": "T-1", "status": "blocked", "questions": ["A?", "B?"]}));
    run(
        &mut t,
        Command::Unblock,
        &opts(|o| o.answers = vec!["yes".into()]),
        None,
    )
    .unwrap();
    assert_eq!(t["notes"][0]["text"], json!("A? → yes; B? → (no answer)"));
    let mut t = task(json!({"id": "T-1", "status": "blocked", "waiting_on": "user"}));
    run(&mut t, Command::Unblock, &Options::default(), None).unwrap();
    assert!(t.get("notes").is_none());
    assert!(t.get("blocks").is_none(), "no blocked_at, no interval");
}

#[test]
fn reopen_undoes_the_work_and_notes_why() {
    for status in ["done", "skipped"] {
        let mut t = task(json!({
            "id": "T-1", "status": status, "commit": "abc", "completed_at": "C", "resolution": "obsolete"
        }));
        run(
            &mut t,
            Command::Reopen,
            &opts(|o| o.reason = Some("again".into())),
            None,
        )
        .unwrap();
        assert_eq!(t["status"], json!("todo"));
        for gone in ["commit", "completed_at", "resolution"] {
            assert!(t.get(gone).is_none(), "{status}: {gone}");
        }
        assert_eq!(t["notes"][0]["text"], json!("Reopened: again"));
    }
}

#[test]
fn fail_notes_the_reason_given() {
    let mut t = task(json!({"id": "T-1", "status": "in_progress"}));
    run(
        &mut t,
        Command::Fail,
        &opts(|o| o.reason = Some("tests red".into())),
        None,
    )
    .unwrap();
    assert_eq!(
        (t["notes"][0]["kind"].clone(), t["notes"][0]["text"].clone()),
        (json!("failure"), json!("tests red"))
    );
}

#[test]
fn every_text_is_at_most_a_note_long_and_a_refusal_changes_nothing() {
    let long = "x".repeat(atm_common::edits::MAX_NOTE + 1);
    let at_limit = "x".repeat(atm_common::edits::MAX_NOTE);
    let cases: [(Command, &str, Options); 5] = [
        (
            Command::Release,
            "in_progress",
            opts(|o| o.note = Some(long.clone())),
        ),
        (
            Command::Fail,
            "in_progress",
            opts(|o| o.reason = Some(long.clone())),
        ),
        (
            Command::Block,
            "todo",
            opts(|o| {
                o.waiting_on = Some("user".into());
                o.questions = vec![long.clone()];
            }),
        ),
        (
            Command::Unblock,
            "blocked",
            opts(|o| o.answers = vec![long.clone()]),
        ),
        (
            Command::Review,
            "in_progress",
            opts(|o| o.note = Some(long.clone())),
        ),
    ];
    for (command, status, options) in cases {
        let mut t = task(json!({"id": "T-1", "status": status}));
        let before = t.clone();
        let error = run(&mut t, command, &options, None).unwrap_err();
        assert_eq!(error.code(), 2, "{}", command.name());
        assert!(
            error.to_string().starts_with("a note is at most 500"),
            "{error}"
        );
        assert_eq!(t, before, "{}", command.name());
    }
    let mut t = task(json!({"id": "T-1", "status": "in_progress"}));
    run(
        &mut t,
        Command::Release,
        &opts(|o| o.note = Some(at_limit)),
        None,
    )
    .unwrap();
}

#[test]
fn unblock_records_answers_longer_together_than_one_note() {
    let part = "x".repeat(300);
    let mut t = task(json!({"id": "T-1", "status": "blocked", "questions": [part.clone()]}));
    run(
        &mut t,
        Command::Unblock,
        &opts(|o| o.answers = vec![part.clone()]),
        None,
    )
    .unwrap();
    assert_eq!(t["notes"][0]["text"], json!(format!("{part} → {part}")));
}

#[test]
fn guards_are_checked_before_arguments() {
    // a reject without a reason on a task in progress: the kind is the refusal, not the reason
    let mut t = task(json!({"id": "T-1", "status": "in_progress"}));
    let error = run(&mut t, Command::Reject, &Options::default(), None).unwrap_err();
    assert!(error.to_string().starts_with("can't reject T-1: it is"));
    // a done review without approval and a too-long note: the guard first
    let mut t = task(json!({"id": "T-1", "status": "in_review", "review_state": "waiting"}));
    let error = run(
        &mut t,
        Command::Done,
        &opts(|o| o.note = Some("x".repeat(600))),
        None,
    )
    .unwrap_err();
    assert!(error.to_string().starts_with("can't finish T-1"));
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
