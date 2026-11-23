use atm_common::Vocabulary;
use atm_common::time::parse_iso;
use serde_json::{Map, Value, json};

use super::work_of;

/// Minutes from 08:00 on 2026-05-01, as atm writes a time
fn at(minutes: i64) -> String {
    atm_common::time::format_iso(parse_iso("2026-05-01T08:00:00Z").unwrap() + minutes * 60)
}

fn task(v: &Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}

/// A stretch of work: from, to (minutes from 08:00) and who
type Piece = (i64, i64, Option<String>);
/// A task's work as the table writes it: (minutes, live, its pieces), or none
type Seen = Option<(i64, bool, Vec<Piece>)>;
/// A case of the table: its name, the task, the time now (minutes from 08:00) and its work
type Case = (&'static str, Value, i64, Seen);

/// The task's work as (minutes, live, [(from, to, by)] in minutes from 08:00)
fn work(t: &Value, now: i64) -> Seen {
    let vocab = Vocabulary::of_meta(&Map::new()).unwrap();
    let base = parse_iso("2026-05-01T08:00:00Z").unwrap();
    let w = work_of(&vocab, &task(t), base + now * 60)?;
    let spans = w
        .spans
        .iter()
        .map(|s| ((s.from - base) / 60, (s.to - base) / 60, s.by.clone()))
        .collect();
    Some((w.seconds() / 60, w.live, spans))
}

#[test]
fn every_attempt_and_round_counts_and_the_waiting_doesnt() {
    let by = |who: &str| Some(who.to_string());
    let cases: Vec<Case> = vec![
        (
            "one attempt, done",
            json!({"status": "done", "started_at": at(0), "started_by": "claude:a", "completed_at": at(30)}),
            100,
            Some((30, false, vec![(0, 30, by("claude:a"))])),
        ),
        (
            "active now: to now, live",
            json!({"status": "in_progress", "started_at": at(10), "started_by": "claude:a"}),
            40,
            Some((30, true, vec![(10, 40, by("claude:a"))])),
        ),
        (
            "taken back after a lapse, then done by another agent: both attempts",
            json!({"status": "done", "started_at": at(60), "started_by": "claude:b", "completed_at": at(80),
                   "attempts": [{"from": at(0), "to": at(20), "by": "claude:a", "outcome": "lapsed"}]}),
            100,
            Some((
                40,
                false,
                vec![(0, 20, by("claude:a")), (60, 80, by("claude:b"))],
            )),
        ),
        (
            "two review rounds: the waiting for each left out",
            json!({"status": "done", "started_at": at(0), "started_by": "claude:a", "completed_at": at(100),
                   "review_requested_at": at(70),
                   "reviews": [{"from": at(20), "to": at(40), "outcome": "changes_requested"},
                               {"from": at(70), "to": at(90), "outcome": "approved"}]}),
            200,
            Some((
                60,
                false,
                vec![
                    (0, 20, by("claude:a")),
                    (40, 70, by("claude:a")),
                    (90, 100, by("claude:a")),
                ],
            )),
        ),
        (
            "blocked between: the block left out",
            json!({"status": "in_progress", "started_at": at(0), "started_by": "claude:a",
                   "blocks": [{"from": at(10), "to": at(25), "on": "user"}]}),
            40,
            Some((
                25,
                true,
                vec![(0, 10, by("claude:a")), (25, 40, by("claude:a"))],
            )),
        ),
    ];
    for (name, t, now, expected) in cases {
        assert_eq!(work(&t, now), expected, "{name}");
    }
}

#[test]
fn the_current_attempt_ends_where_it_stands_and_a_time_that_cant_be_is_no_work() {
    let by = |who: &str| Some(who.to_string());
    let cases: Vec<Case> = vec![
        (
            "in review now: to the request",
            json!({"status": "in_review", "started_at": at(0), "started_by": "claude:a", "review_requested_at": at(15)}),
            90,
            Some((15, false, vec![(0, 15, by("claude:a"))])),
        ),
        (
            "blocked now: to the block",
            json!({"status": "blocked", "started_at": at(0), "agent": "claude:old", "blocked_at": at(12)}),
            90,
            Some((12, false, vec![(0, 12, by("claude:old"))])),
        ),
        (
            "a file from before rounds were kept: done, to its review's request",
            json!({"status": "done", "started_at": at(0), "review_requested_at": at(20), "completed_at": at(300)}),
            400,
            Some((20, false, vec![(0, 20, None)])),
        ),
        (
            "done without a known end: a zero-length span at its start",
            json!({"status": "done", "started_at": at(5)}),
            50,
            Some((0, false, vec![(5, 5, None)])),
        ),
        (
            "to do, never started: none",
            json!({"status": "todo"}),
            50,
            None,
        ),
        (
            "finished before it started (a time that can't be): none",
            json!({"status": "done", "started_at": at(30), "completed_at": at(10)}),
            50,
            None,
        ),
        (
            "to do again, its earlier attempt kept",
            json!({"status": "todo", "attempts": [{"from": at(0), "to": at(10), "by": "alice"}]}),
            50,
            Some((10, false, vec![(0, 10, by("alice"))])),
        ),
    ];
    for (name, t, now, expected) in cases {
        assert_eq!(work(&t, now), expected, "{name}");
    }
}

#[test]
fn as_the_views_give_it() {
    let vocab = Vocabulary::of_meta(&Map::new()).unwrap();
    let t = task(
        &json!({"status": "done", "started_at": at(0), "started_by": "claude:a", "completed_at": at(90)}),
    );
    let w = work_of(&vocab, &t, 0).unwrap();
    assert_eq!(
        w.to_json(),
        json!({"minutes": 90.0, "live": false, "spans": [{"from": at(0), "to": at(90), "by": "claude:a"}]})
    );
}
