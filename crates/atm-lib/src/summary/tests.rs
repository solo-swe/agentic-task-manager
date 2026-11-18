use atm_db::MemoryStore;
use serde_json::{Value, json};

use super::*;

fn project(tasks: &Value, meta: &Value) -> Project {
    let list: Vec<Task> = tasks
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t.as_object().unwrap().clone())
        .collect();
    Project::new(Box::new(
        MemoryStore::new(list, Vec::new(), meta.as_object().unwrap()).unwrap(),
    ))
}

#[test]
fn counts_are_by_kind_and_skipped_tasks_count_as_neither() {
    let p = project(
        &json!([
            {"id": "T-1", "status": "done", "completed_at": "2026-01-01T00:00:00Z"},
            {"id": "T-2", "status": "skipped"},
            {"id": "T-3", "status": "todo", "name": "next one"},
            {"id": "T-4", "status": "blocked", "waiting_on": "user", "questions": ["q"]}
        ]),
        &json!({}),
    );
    let s = project_summary(&p).unwrap();
    assert_eq!(
        (s["total"].clone(), s["done"].clone(), s["skipped"].clone()),
        (json!(4), json!(1), json!(1))
    );
    assert_eq!(s["percent"], json!(33));
    assert_eq!(s["next"], json!([{"id": "T-3", "name": "next one"}]));
    assert_eq!(s["needs_you"]["blocked_on_user"], json!(1));
    assert_eq!(s["board"]["blocked"]["cards"][0]["questions"], json!(1));
    assert_eq!(s["recent_sprints"], Value::Null);
    assert_eq!(s["stats"]["tasks"], json!(4));
}

#[test]
fn a_keys_format_project_gives_its_tasks_keys_and_a_legacy_one_does_not() {
    const SETUP: &str = "0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8001";
    const LOGIN: &str = "0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8002";
    const DRAFT: &str = "0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8003";
    let keyed = project(
        &json!([
            {"id": SETUP, "number": 2, "key_final": true, "status": "done", "completed_at": "2026-01-01T00:00:00Z"},
            {"id": LOGIN, "number": 10, "key_final": true, "status": "todo"},
            {"id": DRAFT, "number": 11, "key_final": false, "status": "todo", "dependencies": [LOGIN]},
            {"id": "0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8004", "number": 12, "status": "in_progress"},
            {"id": "0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8005", "number": 13, "key_final": true, "status": "in_review"}
        ]),
        &json!({"format": 2, "key": "ATM", "next_number": 14}),
    );
    let s = project_summary(&keyed).unwrap();
    let key_of = |v: &Value| (v["key"].clone(), v["key_final"].clone());
    assert_eq!(key_of(&s["last_done"]), (json!("ATM-2"), json!(true)));
    assert_eq!(key_of(&s["next"][0]), (json!("ATM-10"), json!(true)));
    assert_eq!(
        key_of(&s["needs_you_tasks"][0]),
        (json!("ATM-13"), json!(true))
    );
    let todo = &s["board"]["todo"]["cards"];
    let draft = todo
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == DRAFT)
        .unwrap();
    assert_eq!(
        key_of(draft),
        (json!("ATM-11"), json!(false)),
        "provisional"
    );
    assert_eq!(draft["waiting_for"], json!([LOGIN]));
    assert_eq!(draft["waiting_for_labels"], json!(["ATM-10"]));
    assert_eq!(
        key_of(&s["board"]["active"]["cards"][0]),
        (json!("ATM-12"), json!(false)),
        "no key_final: provisional"
    );

    let legacy = project(
        &json!([
            {"id": "T-1", "status": "done", "completed_at": "2026-01-01T00:00:00Z"},
            {"id": "T-2", "status": "todo", "dependencies": ["T-3"]},
            {"id": "T-3", "status": "todo"}
        ]),
        &json!({}),
    );
    let s = project_summary(&legacy).unwrap();
    for entry in [
        &s["last_done"],
        &s["next"][0],
        &s["board"]["todo"]["cards"][1],
    ] {
        assert!(
            entry.get("key").is_none() && entry.get("key_final").is_none(),
            "{entry}"
        );
    }
    assert!(
        s["board"]["todo"]["cards"][1]
            .get("waiting_for_labels")
            .is_none()
    );
    assert_eq!(
        s["board"]["todo"]["cards"][1]["waiting_for"],
        json!(["T-3"])
    );
}

fn vocab(meta: &Value) -> Vocabulary {
    Vocabulary::of_meta(meta.as_object().unwrap()).unwrap()
}

fn tasks(list: &Value) -> Vec<Task> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|t| t.as_object().unwrap().clone())
        .collect()
}

const H: i64 = 3600;

/// An ISO time `hours` after 2026-01-01T00:00:00Z.
fn at(hours: i64) -> String {
    format_iso(parse_iso("2026-01-01T00:00:00Z").unwrap() + hours * H)
}

#[test]
fn the_overall_percent_counts_done_of_the_unskipped_and_an_all_skipped_project_is_done() {
    for (list, expected) in [
        (json!([]), 0),
        (json!([{"id": "T-1", "status": "skipped"}]), 100),
        (
            json!([{"id": "T-1", "status": "done"}, {"id": "T-2", "status": "todo"}]),
            50,
        ),
        (
            json!([{"id": "T-1", "status": "done"}, {"id": "T-2", "status": "skipped"}]),
            100,
        ),
    ] {
        let s = project_summary(&project(&list, &json!({}))).unwrap();
        assert_eq!(s["percent"], json!(expected), "{list}");
    }
}

#[test]
fn a_sprint_without_counted_tasks_is_complete() {
    let sprint = json!({"id": "S", "state": "active"});
    let numbers = sprint_numbers(sprint.as_object().unwrap(), &[Some(Kind::Skipped)]);
    assert_eq!(
        (
            numbers["done"].clone(),
            numbers["total"].clone(),
            numbers["percent"].clone()
        ),
        (json!(0), json!(0), json!(100))
    );
    let numbers = sprint_numbers(
        sprint.as_object().unwrap(),
        &[Some(Kind::Done), Some(Kind::Todo), None],
    );
    assert_eq!(numbers["percent"], json!(33));
}

#[test]
fn the_recent_sprints_are_the_open_ones_with_tasks_then_the_latest_closed_up_to_four() {
    let meta = json!({"sprints": [
        {"id": "C1", "state": "closed", "end": "2026-01-01"},
        {"id": "C2", "state": "closed", "end": "2026-03-01"},
        {"id": "C3", "state": "closed", "end": "2026-02-01"},
        {"id": "Empty"},
        {"id": "Now", "state": "active"},
        {"id": "P1"}
    ]});
    let v = vocab(&meta);
    let list = tasks(&json!([
        {"sprint": "C1", "status": "done"}, {"sprint": "C2", "status": "done"},
        {"sprint": "C3", "status": "done"}, {"sprint": "Now", "status": "todo"},
        {"sprint": "P1", "status": "todo"}
    ]));
    let refs: Vec<&Task> = list.iter().collect();
    let kinds: Vec<Option<Kind>> = refs.iter().map(|t| v.kind_of(t.get("status"))).collect();
    let ids: Vec<Value> = recent_sprints(&v, &refs, &kinds)
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["id"].clone())
        .collect();
    // in the project's order; an empty sprint is left out; C1 ended first and has no room
    assert_eq!(ids, [json!("C2"), json!("C3"), json!("Now"), json!("P1")]);
}

#[test]
fn a_tasks_work_runs_from_its_start_to_its_review_or_its_end() {
    let v = vocab(&json!({}));
    let now = parse_iso(&at(10)).unwrap();
    let span = |t: Value| work_span(&v, &tasks(&json!([t]))[0], now);
    let s = |h: i64| parse_iso(&at(h)).unwrap();
    assert_eq!(span(json!({"status": "todo", "started_at": at(1)})), None);
    assert_eq!(span(json!({"status": "done"})), None, "never started");
    assert_eq!(
        span(json!({"status": "in_progress", "started_at": at(1)})),
        Some((s(1), s(10)))
    );
    assert_eq!(
        span(json!({"status": "done", "started_at": at(1), "completed_at": at(3)})),
        Some((s(1), s(3)))
    );
    assert_eq!(
        span(
            json!({"status": "done", "started_at": at(1), "review_requested_at": at(2), "completed_at": at(5)})
        ),
        Some((s(1), s(2))),
        "the review isn't agent work"
    );
    assert_eq!(
        span(json!({"status": "in_review", "started_at": at(1), "updated_at": at(4)})),
        Some((s(1), s(4)))
    );
    // an end before the start: no time
    assert_eq!(
        span(json!({"status": "done", "started_at": at(3), "completed_at": at(1)})),
        Some((s(3), s(3)))
    );
    // started after now while active: none yet
    assert_eq!(
        span(json!({"status": "in_progress", "started_at": at(20)})),
        Some((s(20), s(20)))
    );
}

#[test]
fn agent_time_merges_an_agents_overlapping_work_and_adds_up_parallel_agents() {
    let v = vocab(&json!({}));
    let done = |by: Option<&str>, from: i64, to: i64| {
        let mut t = json!({"status": "done", "started_at": at(from), "completed_at": at(to)});
        if let Some(by) = by {
            t["started_by"] = json!(by);
        }
        t
    };
    let minutes = |list: Value| {
        let list = tasks(&list);
        let refs: Vec<&Task> = list.iter().collect();
        agent_minutes(&v, &refs, 0)
    };
    // one agent on two overlapping tasks: 0-3h once
    assert_eq!(
        minutes(json!([
            done(Some("claude:a"), 0, 2),
            done(Some("claude:a"), 1, 3)
        ])),
        180
    );
    // one inside the other
    assert_eq!(
        minutes(json!([
            done(Some("claude:a"), 0, 4),
            done(Some("claude:a"), 1, 2)
        ])),
        240
    );
    // two agents at once add up
    assert_eq!(
        minutes(json!([
            done(Some("claude:a"), 0, 2),
            done(Some("claude:b"), 0, 2)
        ])),
        240
    );
    // unattributed work (before v4) is each its own; a person's isn't agent time
    assert_eq!(
        minutes(json!([
            done(None, 0, 1),
            done(None, 0, 1),
            done(Some("Hakan"), 0, 5)
        ])),
        120
    );
    assert_eq!(minutes(json!([])), 0);
}

#[test]
fn the_git_line_is_the_highest_pr_with_a_web_address_and_merged_once_its_tasks_are() {
    let p = project(
        &json!([
            {"id": "T-1", "status": "done", "pr": {"number": 3, "url": "https://x/3"}, "merged_at": "M"},
            {"id": "T-2", "status": "done", "pr": {"number": 7, "url": "javascript:alert(1)"}},
            {"id": "T-3", "status": "done", "pr": {"number": 7, "url": "https://x/7"}},
            {"id": "T-4", "status": "done", "pr": {"number": "9"}},
            {"id": "T-5", "status": "done", "pr": "not one"}
        ]),
        &json!({}),
    );
    let s = project_summary(&p).unwrap();
    assert_eq!(
        s["git"]["pr"],
        json!({"number": 7, "url": "https://x/7", "state": "open"})
    );
    let none = project_summary(&project(&json!([]), &json!({}))).unwrap();
    assert_eq!(none["git"]["pr"], Value::Null);
}

#[test]
fn what_needs_the_user_is_a_review_waiting_a_block_on_them_or_ready_work_together() {
    let p = project(
        &json!([
            {"id": "T-1", "status": "in_review", "review_state": "waiting"},
            {"id": "T-2", "status": "in_review", "review_state": "changes_requested"},
            {"id": "T-3", "status": "in_review"},
            {"id": "T-4", "status": "blocked", "waiting_on": "user"},
            {"id": "T-5", "status": "blocked", "waiting_on": "agent"},
            {"id": "T-6", "status": "todo", "with": "user", "sprint": "Now"},
            {"id": "T-7", "status": "todo", "with": "user", "sprint": "Later"},
            {"id": "T-8", "status": "todo", "with": "user", "sprint": "Now", "dependencies": ["T-9"]}
        ]),
        &json!({"sprints": [{"id": "Now", "state": "active"}, {"id": "Later"}]}),
    );
    let s = project_summary(&p).unwrap();
    let ids: Vec<(Value, Value)> = s["needs_you_tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| (n["id"].clone(), n["reason"].clone()))
        .collect();
    assert_eq!(
        ids,
        [
            (json!("T-1"), json!("review")),
            (json!("T-3"), json!("review")),
            (json!("T-4"), json!("blocked")),
            (json!("T-6"), json!("together"))
        ]
    );
    assert_eq!(
        s["needs_you"],
        // No code reviews in a project without a folder (ATM-606: reviews.rs's tests count them)
        json!({"review": 2, "blocked_on_user": 1, "together": 1, "code_reviews": 0})
    );
}

#[test]
fn only_live_claims_are_work_in_progress_oldest_first() {
    let claims = json!({"claims": {
        "a": {"task": "T-1", "agent": "claude:a", "claimed_at": "2026-01-02", "lease_expires_at": "2999-01-01T00:00:00Z"},
        "b": {"task": "T-2", "agent": "claude:b", "claimed_at": "2026-01-01", "lease_expires_at": "2999-01-01T00:00:00Z"},
        "c": {"task": "T-3", "agent": "claude:c", "lease_expires_at": "2020-01-01T00:00:00Z"},
        "e": {"task": "T-3", "agent": "claude:e", "claimed_at": "2026-01-01"},
        "d": "not a claim"
    }});
    let store = MemoryStore::new(
        tasks(&json!([
            {"id": "T-1", "name": "One", "status": "in_progress"},
            {"id": "T-2", "name": "Two", "status": "in_progress", "started_by": "claude:z"}
        ])),
        Vec::new(),
        &Map::new(),
    )
    .unwrap()
    .with_claims(claims.as_object().unwrap().clone());
    let s = project_summary(&Project::new(Box::new(store))).unwrap();
    assert_eq!(s["agents_working"], json!(2));
    assert_eq!(
        s["working"],
        json!([
            {"task": "T-2", "name": "Two", "agent": "claude:b", "branch": null},
            {"task": "T-1", "name": "One", "agent": "claude:a", "branch": null}
        ])
    );
    // the board's card says who holds it: the claim's agent before who started it
    let by: Vec<Value> = s["board"]["active"]["cards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["by"].clone())
        .collect();
    assert_eq!(by, [json!("claude:a"), json!("claude:b")]);
}

#[test]
fn the_board_orders_each_column_and_caps_the_to_dos() {
    let mut list = vec![
        json!({"id": "T-1", "status": "blocked", "waiting_on": "agent", "blocked_at": at(1)}),
        json!({"id": "T-2", "status": "blocked", "waiting_on": "user", "blocked_at": at(5)}),
        json!({"id": "T-3", "status": "in_review", "review_requested_at": at(5)}),
        json!({"id": "T-4", "status": "in_review", "updated_at": at(2)}),
        json!({"id": "T-5", "status": "done", "completed_at": "2020-01-01T00:00:00Z"}),
    ];
    for i in 0..12 {
        list.push(json!({"id": format!("T-{}", 100 + i), "status": "todo"}));
    }
    let s = project_summary(&project(&Value::Array(list), &json!({}))).unwrap();
    let ids = |column: &str| -> Vec<Value> {
        s["board"][column]["cards"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["id"].clone())
            .collect()
    };
    assert_eq!(
        ids("blocked"),
        [json!("T-2"), json!("T-1")],
        "on the user first"
    );
    assert_eq!(
        ids("review"),
        [json!("T-4"), json!("T-3")],
        "longest waiting first"
    );
    assert!(ids("done").is_empty(), "only the last 24 hours");
    assert_eq!(s["board"]["todo"]["count"], json!(12));
    assert_eq!(ids("todo").len(), 10);
    assert_eq!(s["board_active_sprint"], Value::Null, "no active sprint");
}

#[test]
fn the_to_do_column_puts_the_loops_ready_tasks_first_then_work_together_then_waiting() {
    let p = project(
        &json!([
            {"id": "T-1", "status": "todo", "dependencies": ["T-9"]},
            {"id": "T-2", "status": "todo", "with": "user"},
            {"id": "T-3", "status": "todo"},
            {"id": "T-4", "status": "todo", "priority": "P0"}
        ]),
        &json!({}),
    );
    let s = project_summary(&p).unwrap();
    let cards = s["board"]["todo"]["cards"].as_array().unwrap();
    let ids: Vec<&Value> = cards.iter().map(|c| &c["id"]).collect();
    assert_eq!(
        ids,
        [&json!("T-4"), &json!("T-3"), &json!("T-2"), &json!("T-1")]
    );
    assert_eq!(cards[2]["with_user"], json!(true));
    assert_eq!(cards[3]["waiting_for"], json!(["T-9"]));
}

#[test]
fn the_active_sprints_board_shows_all_its_finished_work() {
    let p = project(
        &json!([
            {"id": "T-1", "status": "done", "sprint": "S", "completed_at": "2020-01-01T00:00:00Z"},
            {"id": "T-2", "status": "done", "completed_at": "2020-01-01T00:00:00Z"}
        ]),
        &json!({"sprints": [{"id": "S", "state": "active", "goal": "G"}]}),
    );
    let s = project_summary(&p).unwrap();
    assert_eq!(s["board_active_sprint"]["done"]["count"], json!(1));
    assert_eq!(s["active_sprint"]["goal"], json!("G"));
    assert_eq!(s["active_sprint"]["percent"], json!(100));
}

#[test]
fn completions_are_counted_by_day_for_two_weeks_and_the_last_activity_is_the_latest_time() {
    let today = format_iso(now());
    let p = project(
        &json!([
            {"id": "T-1", "status": "done", "completed_at": today},
            {"id": "T-2", "status": "done", "completed_at": "2020-01-01T00:00:00Z",
             "notes": [{"at": "2999-01-01T00:00:00Z"}]},
            {"id": "T-3", "status": "skipped", "completed_at": today},
            {"id": "T-4", "status": "done", "completed_at": "not a time"}
        ]),
        &json!({}),
    );
    let s = project_summary(&p).unwrap();
    let days = s["completions_14d"].as_array().unwrap();
    assert_eq!(days.len(), 14);
    assert_eq!(days[13], json!(1), "today is the last");
    assert_eq!(days.iter().filter_map(Value::as_u64).sum::<u64>(), 1);
    assert_eq!(
        s["last_activity"],
        json!("2999-01-01T00:00:00Z"),
        "a note counts"
    );
    assert_eq!(s["last_done"]["id"], json!("T-1"));
    let quiet = project_summary(&project(&json!([{"id": "T-1"}]), &json!({}))).unwrap();
    assert_eq!(
        (quiet["last_activity"].clone(), quiet["last_done"].clone()),
        (Value::Null, Value::Null)
    );
}

#[test]
fn the_stats_start_with_the_first_task_or_the_project_and_count_the_archive_once() {
    let store = MemoryStore::new(
        tasks(&json!([{"id": "T-2", "created_at": "2026-02-01T00:00:00Z"}])),
        tasks(&json!([
            {"id": "T-1", "created_at": "2026-01-01T00:00:00Z"},
            {"id": "T-2", "created_at": "2025-01-01T00:00:00Z"}
        ])),
        &Map::new(),
    )
    .unwrap();
    let s = project_summary(&Project::new(Box::new(store))).unwrap();
    // the archive's copy of a working task isn't counted, nor its time
    assert_eq!(
        s["stats"],
        json!({"started": "2026-01-01T00:00:00Z", "tasks": 2, "agent_minutes": 0})
    );
    let fresh = project(&json!([]), &json!({"created": "2026-05-05T00:00:00Z"}));
    assert_eq!(
        project_summary(&fresh).unwrap()["stats"]["started"],
        json!("2026-05-05T00:00:00Z")
    );
}

#[test]
fn names_and_descriptions_are_capped() {
    assert_eq!(capped(Some(&json!("x".repeat(100)))).chars().count(), 80);
    assert_eq!(capped(Some(&json!(3))), "");
    assert_eq!(
        description(Some(&json!({"description": "  hi  "}))),
        json!("hi")
    );
    assert_eq!(description(Some(&json!({}))), Value::Null);
    assert!(
        web_address("HTTPS://x/pull/1")
            && !web_address("javascript:x")
            && !web_address("https://a b")
    );
    assert_eq!(percent(1, 8), 12, "12.5 rounds to even");
    assert_eq!(percent(3, 8), 38, "37.5 rounds to even");
    // at the cap, kept whole; past it, cut with an ellipsis
    let at_cap = "x".repeat(80);
    assert_eq!(capped(Some(&json!(at_cap))), at_cap);
    assert!(capped(Some(&json!("x".repeat(81)))).ends_with('…'));
    assert_eq!(capped(None), "");
    let long = "y".repeat(201);
    let cut = description(Some(&json!({"description": long})));
    assert_eq!(cut.as_str().unwrap().chars().count(), 200);
    assert_eq!(
        description(Some(&json!({"description": "y".repeat(200)}))),
        json!("y".repeat(200))
    );
    assert_eq!(
        description(Some(&json!({"description": "   "}))),
        Value::Null
    );
    assert_eq!(description(None), Value::Null);
    for url in ["http://x", "https://x/y?z=1"] {
        assert!(web_address(url), "{url}");
    }
    for url in ["https://", "ftp://x", "x", ""] {
        assert!(!web_address(url), "{url}");
    }
    assert_eq!(day_of(-1), -1, "before 1970: the day before");
    assert_eq!(day_of(86_399), 0);
}
