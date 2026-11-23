use std::collections::BTreeSet;

use atm_db::MemoryStore;
use serde_json::{Value, json};

use super::*;

/// 2026-11-01T00:00:00Z in ms, and n minutes after it as ISO text
const T0: i64 = 1_793_491_200_000;
fn at(minutes: i64) -> String {
    atm_common::time::format_iso((T0 + minutes * 60_000) / 1000)
}

fn task(v: &Value) -> atm_common::Task {
    v.as_object().unwrap().clone()
}

fn project(meta: &Value, tasks: &[Value], archive: &[Value]) -> Project {
    Project::new(Box::new(
        MemoryStore::new(
            tasks.iter().map(task).collect(),
            archive.iter().map(task).collect(),
            meta.as_object().unwrap(),
        )
        .unwrap(),
    ))
}

/// A done task with `minutes` of work, finished at minute `end`, by `by`
fn done(id: &str, end: i64, minutes: i64, by: &str) -> Value {
    json!({"id": id, "name": id, "type": "feature", "status": "done", "started_by": by,
           "started_at": at(end - minutes), "completed_at": at(end), "completed_by": by})
}

fn stats(p: &Project, query: &StatsQuery, now_minutes: i64) -> Value {
    project_stats(p, query, T0 + now_minutes * 60_000).unwrap()
}

const SPRINTS: &str = r#"{"sprints": [
    {"id": "Old", "state": "closed"},
    {"id": "Later", "state": "planned"},
    {"id": "Now", "state": "active"}
]}"#;

/// The overview's Progress tab from the summary: done of all but the skipped (the archive too), the open releases,
/// the days with completions, a daily burndown and the work left at the agents' pace
#[test]
fn the_overview_figures_count_the_archive_and_give_a_daily_burndown_and_the_work_left() {
    let day = T0 / 86_400_000;
    let meta = json!({"releases": [{"id": "1.0", "state": "planned"}, {"id": "0.9", "state": "released"}]});
    let mut tasks = vec![
        json!({"id": "T-1", "name": "1", "type": "x", "status": "todo", "release": "1.0", "created_at": at(0)}),
        json!({"id": "T-2", "name": "2", "type": "x", "status": "blocked", "waiting_on": "user", "created_at": at(0)}),
        json!({"id": "T-3", "name": "3", "type": "x", "status": "skipped", "created_at": at(0)}),
    ];
    for (id, end) in [("D-1", 60), ("D-2", 1500), ("D-3", 1600)] {
        let mut d = done(id, end, 10, "claude:a");
        d["created_at"] = json!(at(0));
        tasks.push(d);
    }
    let mut archived = done("A-1", 30, 10, "claude:a");
    archived["created_at"] = json!(at(0));
    let p = project(&meta, &tasks, &[archived]);
    let o = overview(&p, T0 + 1700 * 60_000).unwrap();
    assert_eq!(
        (o["done"].clone(), o["total"].clone()),
        (json!(4), json!(6))
    );
    assert_eq!(
        o["open_releases"],
        json!([{"id": "1.0", "name": null, "done": 0, "total": 1}])
    );
    // A-1 and D-1 on the first day, D-2 and D-3 on the second (minutes 1500 and 1600)
    assert_eq!(o["done_days"], json!([[day, 2], [day + 1, 2]]));
    assert_eq!(o["burndown"], json!([[day, 6, 4], [day + 1, 6, 2]]));
    // T-1 is agent work left (10 minutes each at this pace); T-2 waits on the user
    assert_eq!(
        o["forecast"],
        json!({"minutes": {"likely": 10.0, "low": 10.0, "high": 10.0}, "waiting": 1})
    );
    // No pace yet: no forecast
    let new = project(
        &json!({}),
        &[json!({"id": "T-1", "name": "1", "type": "x", "status": "todo"})],
        &[],
    );
    let fresh = overview(&new, T0).unwrap();
    assert_eq!(fresh["forecast"], Value::Null);
    assert_eq!(
        fresh["burndown"],
        json!([]),
        "no task with a time: nothing to draw"
    );
}

/// Who worked on it (T-252, the dashboard's cases): people by what they created, started and completed; agents by
/// session, a task from before v4 (`agent`) or with nobody recorded counting as agent work
#[test]
fn people_and_agents_count_what_each_did_older_files_too() {
    let tasks = vec![
        json!({"id": "T-1", "name": "1", "type": "x", "status": "done", "created_by": "Hakan",
               "started_by": "claude:a", "started_at": at(0), "completed_by": "claude:a", "completed_at": at(30)}),
        json!({"id": "T-2", "name": "2", "type": "x", "status": "done", "created_by": "Hakan",
               "started_by": "claude:b", "started_at": at(10), "completed_by": "Hakan", "completed_at": at(60)}),
        json!({"id": "T-3", "name": "3", "type": "x", "status": "in_progress", "created_by": "Hakan",
               "agent": "claude:c", "started_at": at(60)}),
        json!({"id": "T-4", "name": "4", "type": "x", "status": "done", "created_by": "Hakan",
               "started_at": at(100), "completed_at": at(110)}),
        json!({"id": "T-5", "name": "5", "type": "x", "status": "in_progress", "created_by": "claude:a",
               "started_by": "Ana", "started_at": at(0)}),
    ];
    let p = project(&json!({}), &tasks, &[]);
    let actors = &stats(&p, &StatsQuery::default(), 120)["actors"];
    assert_eq!(
        actors["people"],
        json!([
            {"name": "Hakan", "created": 4, "started": 0, "completed": 1},
            {"name": "Ana", "created": 0, "started": 1, "completed": 0}
        ])
    );
    // Sessions a, b, c; T-4 (nobody recorded) is agent work with no session. Completed: T-1 by a, T-4 finished with
    // no completed_by; T-2 was Hakan's
    assert_eq!(actors["agents"]["sessions"], json!(3));
    assert_eq!(
        (
            actors["agents"]["started"].clone(),
            actors["agents"]["completed"].clone()
        ),
        (json!(4), json!(2))
    );
    // T-1 30 m, T-2 50 m, T-3 60 m (to now), T-4 10 m; Ana's T-5 isn't agent time
    assert_eq!(actors["agents"]["agent_ms"], json!(150 * 60_000));
    // No tasks: nobody, and no agent time
    let empty = project(&json!({}), &[], &[]);
    assert_eq!(
        stats(&empty, &StatsQuery::default(), 0)["actors"],
        json!({"people": [], "agents": {"sessions": 0, "started": 0, "completed": 0, "agent_ms": 0}})
    );
}

#[test]
fn the_pace_is_the_middle_of_the_last_ten_with_quartiles_and_none_under_three() {
    assert_eq!(forecast::pace(&[10.0, 20.0]), None);
    let p = forecast::pace(&[10.0, 20.0, 30.0, 40.0]).unwrap();
    assert!((p.likely - 25.0).abs() < 1e-9, "{p:?}");
    assert!((p.low - 17.5).abs() < 1e-9, "{p:?}");
    assert!((p.high - 32.5).abs() < 1e-9, "{p:?}");
    assert_eq!(p.based_on, 4);
    // Only the last ten count
    let many: Vec<f64> = (1..=12).map(f64::from).collect();
    assert_eq!(forecast::pace(&many).unwrap().based_on, 10);
}

/// The rows in the loop's order (closed, active, planned, backlog), the work left accumulating from the active sprint
/// on; the pace leaves the tasks done with the user out
#[test]
fn the_forecast_counts_each_sprint_and_estimates_in_the_loops_order() {
    let meta: Value = serde_json::from_str(SPRINTS).unwrap();
    let mut tasks = vec![
        json!({"id": "T-1", "name": "1", "type": "feature", "status": "todo", "sprint": "Now"}),
        json!({"id": "T-2", "name": "2", "type": "feature", "status": "todo", "sprint": "Later"}),
        json!({"id": "T-3", "name": "3", "type": "feature", "status": "todo"}),
        json!({"id": "T-4", "name": "4", "type": "feature", "status": "blocked", "sprint": "Now", "waiting_on": "user"}),
        json!({"id": "T-5", "name": "5", "type": "feature", "status": "skipped", "sprint": "Now"}),
    ];
    // The pace: three tasks of 10, 20 and 30 minutes, and one with the user of 600 (left out)
    for (id, end, minutes) in [("D-1", 100, 10), ("D-2", 200, 20), ("D-3", 300, 30)] {
        let mut d = done(id, end, minutes, "claude:a");
        d["sprint"] = json!("Old");
        tasks.push(d);
    }
    let mut together = done("D-4", 1000, 600, "claude:a");
    together["with"] = json!("user");
    together["sprint"] = json!("Old");
    tasks.push(together);
    let p = project(&meta, &tasks, &[]);
    let f = &stats(&p, &StatsQuery::default(), 2000)["forecast"];
    assert_eq!(
        f["pace"]["likely"],
        json!(20.0),
        "the with-user task is left out"
    );
    let rows: Vec<(Value, Value, Value, Value)> = f["sprints"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["sprint"].clone(),
                r["total"].clone(),
                r["remaining"].clone(),
                r["state"].clone(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            (json!("Old"), json!(4), json!(0), json!("done")),
            (json!("Now"), json!(2), json!(1), json!("not_started")),
            (json!("Later"), json!(1), json!(1), json!("not_started")),
            (Value::Null, json!(1), json!(1), json!("not_started")),
        ]
    );
    let sprints = f["sprints"].as_array().unwrap();
    let likely = |n: usize| sprints[n]["estimate"]["minutes"]["likely"].clone();
    // Done: none; then 1, 2 and 3 tasks' work at 20 minutes each, in the loop's order
    assert_eq!(sprints[0].get("estimate"), None);
    assert_eq!(
        (likely(1), likely(2), likely(3)),
        (json!(20.0), json!(40.0), json!(60.0))
    );
    assert_eq!(sprints[0]["completed_at"], json!(T0 + 1000 * 60_000));
    assert_eq!(sprints[1]["blocked"], json!(1));
    assert_eq!(sprints[1]["skipped"], json!(1));
    // The loop works on the active sprint next
    assert_eq!(f["next"], json!({"sprint": "Now"}));
}

/// A slice keeps its own counts, with the whole project's estimates (the loop works through whole sprints)
#[test]
fn a_slice_counts_its_own_tasks_with_the_whole_projects_estimates() {
    let meta: Value = serde_json::from_str(SPRINTS).unwrap();
    let mut tasks = vec![
        json!({"id": "T-1", "name": "1", "type": "bugfix", "status": "todo", "sprint": "Now"}),
        json!({"id": "T-2", "name": "2", "type": "feature", "status": "todo", "sprint": "Now"}),
    ];
    for (id, end) in [("D-1", 100), ("D-2", 200), ("D-3", 300)] {
        tasks.push(done(id, end, 10, "claude:a"));
    }
    let p = project(&meta, &tasks, &[]);
    let query = StatsQuery {
        tasks: Some(BTreeSet::from(["T-1".to_string()])),
        ..StatsQuery::default()
    };
    let s = stats(&p, &query, 2000);
    let now = &s["forecast"]["sprints"][0];
    assert_eq!(
        (now["sprint"].clone(), now["total"].clone()),
        (json!("Now"), json!(1))
    );
    // Two tasks left in the whole sprint: 20 minutes, not the slice's one
    assert_eq!(now["estimate"]["minutes"]["likely"], json!(20.0));
    assert_eq!(s["progress"]["overall"]["total"], json!(1));
    // The project's own numbers stay the whole project's
    assert_eq!(s["stats"]["tasks"]["total"], json!(5));
}

/// Archived tasks count as what they are: an archived skipped task is skipped, not done (the dashboard's old count)
#[test]
fn progress_counts_archived_tasks_as_they_are_by_status_type_and_sprint() {
    let meta: Value = serde_json::from_str(SPRINTS).unwrap();
    let tasks = vec![
        json!({"id": "T-1", "name": "1", "type": "bugfix", "status": "todo", "sprint": "Now"}),
        json!({"id": "T-2", "name": "2", "type": "feature", "status": "in_progress"}),
        json!({"id": "T-3", "name": "3", "type": "feature", "status": "made_up"}),
    ];
    let archive = vec![
        done("A-1", 10, 5, "claude:a"),
        json!({"id": "A-2", "name": "2", "type": "feature", "status": "skipped", "sprint": "Old"}),
    ];
    let p = project(&meta, &tasks, &archive);
    let progress = &stats(&p, &StatsQuery::default(), 100)["progress"];
    assert_eq!(
        progress["overall"],
        json!({"key": "all", "label": "All tasks", "done": 1, "total": 4, "skipped": 1, "ratio": 0.25})
    );
    assert_eq!(progress["archived"], json!(2));
    // By kind, finished first; a status the vocabulary lacks last
    let statuses: Vec<Value> = progress["by_status"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["status"].clone())
        .collect();
    assert_eq!(
        statuses,
        [
            json!("done"),
            json!("in_progress"),
            json!("todo"),
            json!("skipped"),
            json!("made_up")
        ]
    );
    let types: Vec<Value> = progress["by_type"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["key"].clone())
        .collect();
    assert_eq!(types, [json!("bugfix"), json!("feature")]);
    // The sprints in the project's order, then the backlog; a closed sprint of only skipped tasks is finished
    let sprints: Vec<(Value, Value)> = progress["by_sprint"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| (s["key"].clone(), s["ratio"].clone()))
        .collect();
    assert_eq!(
        sprints,
        [
            (json!("Old"), json!(1.0)),
            (json!("Now"), json!(0.0)),
            (Value::Null, json!(1.0 / 3.0))
        ]
    );
}

/// A project not migrated to sprints: its phases are the sprints `atm db migrate` would make, as the sprint views count
/// them, in the forecast and the progress alike
#[test]
fn an_unmigrated_projects_phases_are_its_sprints() {
    let mut first = done("T-1", 10, 5, "claude:a");
    first["phase"] = json!(0);
    let tasks = vec![
        first,
        json!({"id": "T-2", "name": "2", "type": "feature", "status": "todo", "phase": 1}),
        json!({"id": "T-3", "name": "3", "type": "feature", "status": "todo"}),
    ];
    let p = project(&json!({}), &tasks, &[]);
    let s = stats(&p, &StatsQuery::default(), 100);
    let rows: Vec<Value> = s["forecast"]["sprints"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["sprint"].clone())
        .collect();
    assert_eq!(rows, [json!("Phase 0"), json!("Phase 1"), Value::Null]);
    let by_sprint: Vec<Value> = s["progress"]["by_sprint"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["key"].clone())
        .collect();
    assert_eq!(by_sprint, [json!("Phase 0"), json!("Phase 1"), Value::Null]);
}

#[test]
fn time_is_split_into_working_waiting_on_you_and_idle_and_parallel_agents_add_up() {
    // Two agents at once for 30 minutes (0–30, 10–40), a review on the user from 40 to 60, then idle to 120
    let tasks = vec![
        done("D-1", 30, 30, "claude:a"),
        done("D-2", 40, 30, "claude:b"),
        json!({"id": "R-1", "name": "r", "type": "feature", "status": "done", "started_by": "claude:a",
               "started_at": at(40), "review_requested_at": at(40), "completed_at": at(60),
               "reviews": [{"from": at(40), "to": at(60)}]}),
    ];
    let p = project(&json!({}), &tasks, &[]);
    let s = stats(&p, &StatsQuery::default(), 120);
    let u = &s["time_use"];
    assert_eq!(u["from"], json!(T0));
    assert_eq!(u["working_ms"], json!(40 * 60_000));
    assert_eq!(u["waiting_ms"], json!(20 * 60_000));
    assert_eq!(u["idle_ms"], json!(60 * 60_000));
    assert_eq!(u["agent_ms"], json!(60 * 60_000));
    assert_eq!(u["max_agents"], json!(2));
    // A period inside it: cut to it
    let period = StatsQuery {
        from: Some(T0 + 20 * 60_000),
        to: Some(T0 + 30 * 60_000),
        ..StatsQuery::default()
    };
    let cut = &stats(&p, &period, 120)["time_use"];
    assert_eq!(
        (cut["working_ms"].clone(), cut["agent_ms"].clone()),
        (json!(600_000), json!(1_200_000))
    );
    // No work at all: none
    let empty = project(
        &json!({}),
        &[json!({"id": "T-1", "name": "1", "type": "x", "status": "todo"})],
        &[],
    );
    assert_eq!(
        stats(&empty, &StatsQuery::default(), 10)["time_use"],
        Value::Null
    );
}

#[test]
fn the_projects_numbers_people_and_agents_and_the_time_per_task() {
    let mut with_you = done("D-2", 200, 90, "claude:b");
    with_you["with"] = json!("user");
    with_you["pr"] = json!({"number": 7});
    with_you["commit"] = json!("abc");
    let tasks = vec![
        done("D-1", 100, 3, "claude:a"),
        with_you,
        json!({"id": "T-1", "name": "1", "type": "feature", "status": "todo", "created_by": "Hakan Serce",
               "created_at": at(-60)}),
        done("D-3", 300, 300, "Hakan Serce"),
    ];
    let p = project(&json!({"workflow": {"type": "direct"}}), &tasks, &[]);
    let s = stats(&p, &StatsQuery::default(), 400);
    let numbers = &s["stats"];
    assert_eq!(numbers["started"], json!(T0 - 60 * 60_000));
    assert_eq!(numbers["duration"]["ongoing"], json!(true));
    assert_eq!(
        numbers["tasks"],
        json!({"total": 4, "done": 3, "open": 1, "skipped": 0})
    );
    assert_eq!(
        (
            numbers["task_commits"].clone(),
            numbers["pull_requests"].clone()
        ),
        (json!(1), json!(1))
    );
    // Agents' time: a's 3 minutes alone, b's 90 with you; the person's isn't the agents'
    assert_eq!(numbers["agent_ms"], json!(93 * 60_000));
    assert_eq!(numbers["alone_ms"], json!(3 * 60_000));
    assert_eq!(numbers["with_you_ms"], json!(90 * 60_000));
    let actors = &s["actors"];
    assert_eq!(
        actors["people"],
        json!([{"name": "Hakan Serce", "created": 1, "started": 1, "completed": 1}])
    );
    assert_eq!(actors["agents"]["sessions"], json!(2));
    assert_eq!(actors["agents"]["completed"], json!(2));
    // The time per task: 3 m in the first bin, 90 m with you, 300 m in the last (4 h+)
    let h = &s["histogram"];
    let in_bin = |n: usize, side: &str| h["bins"][n][side].as_array().unwrap().len();
    assert_eq!(
        (
            in_bin(0, "alone"),
            in_bin(7, "with_you"),
            in_bin(10, "alone")
        ),
        (1, 1, 1)
    );
    assert_eq!(h["bins"][10]["label"], json!("4 h+"));
    assert_eq!(h["bins"][0]["label"], json!("0 m–5 m"));
    assert_eq!(h["slowest"][0]["id"], json!("D-3"));
    assert_eq!(h["median_with_you"], json!(90.0));
    // A direct project with no PR recorded has none to count
    let plain = project(
        &json!({"workflow": {"type": "direct"}}),
        &[done("D-1", 10, 5, "claude:a")],
        &[],
    );
    assert_eq!(
        stats(&plain, &StatsQuery::default(), 20)["stats"]["pull_requests"],
        Value::Null
    );
}
