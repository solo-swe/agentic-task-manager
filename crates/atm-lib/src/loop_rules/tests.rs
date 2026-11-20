use serde_json::{Value, json};

use super::*;

fn task(value: Value) -> Task {
    match value {
        Value::Object(map) => map,
        other => panic!("not a task: {other}"),
    }
}

/// The worktree the loop runs in, in these tests.
const HERE: Option<&str> = Some("/work/here");

struct Fixture {
    document: Value,
    tasks: Vec<Task>,
}

impl Fixture {
    fn new(meta: Value, tasks: Value) -> Self {
        let list: Vec<Task> = match tasks.clone() {
            Value::Array(items) => items.into_iter().map(task).collect(),
            other => panic!("not a list: {other}"),
        };
        let mut document = Map::new();
        document.insert("meta".into(), meta);
        document.insert("tasks".into(), tasks);
        Self {
            document: Value::Object(document),
            tasks: list,
        }
    }

    fn next(
        &self,
        sel: &Selection,
        store: Value,
        with_user: bool,
    ) -> (Option<String>, Option<String>) {
        self.next_for(sel, store, None, with_user)
    }

    /// The order the loop takes the ready tasks in, by ID.
    fn order(&self, sel: &Selection) -> Vec<String> {
        let vocab = Vocabulary::of_document(&self.document).unwrap();
        let tasks: Vec<&Task> = self.tasks.iter().collect();
        let store = Map::new();
        let order = loop_order(
            &vocab,
            &self.document,
            &tasks,
            &store,
            HERE,
            sel,
            None,
            false,
            &|_| Ok(false),
        )
        .unwrap();
        order
            .ready
            .iter()
            .map(|t| t["id"].as_str().unwrap().to_string())
            .collect()
    }

    fn refusal(&self, sel: &Selection) -> Error {
        let vocab = Vocabulary::of_document(&self.document).unwrap();
        let tasks: Vec<&Task> = self.tasks.iter().collect();
        let store = Map::new();
        next_task(
            &vocab,
            &self.document,
            &tasks,
            &store,
            HERE,
            sel,
            None,
            false,
            &|_| Ok(false),
        )
        .unwrap_err()
    }

    fn next_for(
        &self,
        sel: &Selection,
        store: Value,
        agent: Option<&str>,
        with_user: bool,
    ) -> (Option<String>, Option<String>) {
        let vocab = Vocabulary::of_document(&self.document).unwrap();
        let tasks: Vec<&Task> = self.tasks.iter().collect();
        let store = match store {
            Value::Object(map) => map,
            other => panic!("not a store: {other}"),
        };
        let archived = |_: &str| Ok(false);
        let (t, reason) = next_task(
            &vocab,
            &self.document,
            &tasks,
            &store,
            HERE,
            sel,
            agent,
            with_user,
            &archived,
        )
        .unwrap();
        (t.and_then(|t| t["id"].as_str().map(String::from)), reason)
    }
}

fn reason(f: &Fixture, sel: &Selection, store: Value) -> String {
    let (task, reason) = f.next(sel, store, false);
    assert_eq!(task, None);
    reason.expect("a reason")
}

fn sprints_meta() -> Value {
    json!({"sprints": [
        {"id": "Old", "state": "closed"},
        {"id": "Next", "state": "planned"},
        {"id": "Now", "state": "active"},
        {"id": "Later", "state": "planned"}
    ]})
}

fn sel_all() -> Selection {
    Selection {
        all: true,
        ..Selection::default()
    }
}

fn sel_sprints(names: &[&str]) -> Selection {
    Selection {
        sprints: names.iter().map(|n| (*n).to_string()).collect(),
        ..Selection::default()
    }
}

#[test]
fn all_takes_the_active_sprint_then_the_planned_in_order_then_leftovers_then_the_backlog() {
    let f = Fixture::new(
        sprints_meta(),
        json!([
            {"id": "T-1", "status": "todo"},
            {"id": "T-2", "status": "todo", "sprint": "Old"},
            {"id": "T-3", "status": "todo", "sprint": "Later", "priority": "P0"},
            {"id": "T-4", "status": "todo", "sprint": "Next"},
            {"id": "T-5", "status": "todo", "sprint": "Now"}
        ]),
    );
    assert_eq!(f.order(&sel_all()), ["T-5", "T-4", "T-3", "T-2", "T-1"]);
}

#[test]
fn the_backlog_is_the_tasks_in_no_sprint() {
    let f = Fixture::new(
        sprints_meta(),
        json!([
            {"id": "T-1", "status": "todo", "sprint": "Now"},
            {"id": "T-2", "status": "todo", "sprint": "Gone"},
            {"id": "T-3", "status": "todo"}
        ]),
    );
    let backlog = Selection {
        backlog: true,
        ..Selection::default()
    };
    // a sprint the project doesn't list is no sprint
    assert_eq!(f.order(&backlog), ["T-2", "T-3"]);
    let empty = Fixture::new(
        sprints_meta(),
        json!([{"id": "T-1", "status": "todo", "sprint": "Now"}]),
    );
    assert_eq!(
        reason(&empty, &backlog, empty_store()),
        "the backlog have no tasks"
    );
}

#[test]
fn named_sprints_are_found_by_id_and_unknown_or_closed_ones_are_refused() {
    let f = Fixture::new(
        sprints_meta(),
        json!([
            {"id": "T-1", "status": "todo", "sprint": "Next"},
            {"id": "T-2", "status": "todo", "sprint": "Later"},
            {"id": "T-3", "status": "todo", "sprint": "Now"}
        ]),
    );
    assert_eq!(f.order(&sel_sprints(&["Next", "Later"])), ["T-1", "T-2"]);
    let error = f.refusal(&sel_sprints(&["Nope"]));
    assert_eq!(
        (error.to_string(), error.code()),
        (
            "no sprint 'Nope'; the sprints: Old, Next, Now, Later".into(),
            2
        )
    );
    let error = f.refusal(&sel_sprints(&["Old"]));
    assert_eq!(
        (error.to_string(), error.code()),
        ("the sprint 'Old' is closed".into(), 2)
    );
    let none = Fixture::new(json!({}), json!([]));
    assert_eq!(
        none.refusal(&sel_sprints(&["X"])).to_string(),
        "no sprint 'X'; the sprints: none"
    );
}

#[test]
fn the_reasons_name_the_selection_in_words() {
    let f = Fixture::new(sprints_meta(), json!([]));
    assert_eq!(
        reason(&f, &sel_sprints(&["Next", "Later"]), empty_store()),
        "the sprint “Next” and “Later” has no tasks"
    );
    assert_eq!(
        reason(&f, &Selection::default(), empty_store()),
        "the sprint “Now” has no tasks"
    );
    assert_eq!(
        reason(&f, &sel_all(), empty_store()),
        "all tasks have no tasks"
    );
}

#[test]
fn a_finished_selection_counts_its_done_and_skipped_tasks() {
    let f = Fixture::new(
        sprints_meta(),
        json!([
            {"id": "T-1", "status": "done", "sprint": "Now"},
            {"id": "T-2", "status": "skipped", "sprint": "Now"},
            {"id": "T-3", "status": "done", "sprint": "Now"}
        ]),
    );
    assert_eq!(
        reason(&f, &Selection::default(), empty_store()),
        "the sprint “Now” is finished: 2 done, 1 skipped"
    );
    // without sprints, finished is nothing left to do
    let plain = Fixture::new(json!({}), json!([{"id": "T-1", "status": "done"}]));
    assert_eq!(
        reason(&plain, &Selection::default(), empty_store()),
        "nothing left to do"
    );
}

#[test]
fn waiting_work_is_counted_by_kind() {
    let f = Fixture::new(
        json!({}),
        json!([
            {"id": "T-1", "status": "in_progress"},
            {"id": "T-2", "status": "in_review", "review_state": "waiting"},
            {"id": "T-3", "status": "blocked"},
            {"id": "T-4", "status": "blocked"},
            {"id": "T-5", "status": "done"}
        ]),
    );
    assert_eq!(
        reason(&f, &Selection::default(), empty_store()),
        "nothing ready in the tasks: the rest is waiting (1 in progress, 1 in review, 2 blocked)"
    );
}

#[test]
fn work_elsewhere_is_not_ready_and_the_reason_names_where_it_is() {
    let f = Fixture::new(
        json!({}),
        json!([
            {"id": "T-1", "status": "todo"},
            {"id": "T-2", "status": "todo"},
            {"id": "T-3", "status": "todo"},
            {"id": "T-4", "status": "todo", "dependencies": ["T-9"]}
        ]),
    );
    let store = json!({"claims": {"T-1": {}}, "completed": {"T-2": {}}, "waiting": {"T-3": {}}});
    assert_eq!(
        reason(&f, &Selection::default(), store),
        "4 todo task(s) waiting on dependencies or claimed by other agents (T-1); done in another worktree, not merged yet: T-2; blocked or in review in another worktree: T-3"
    );
}

#[test]
fn a_wait_recorded_in_this_worktree_doesnt_hold_a_task_back_one_in_another_does() {
    // T-1 and T-2 back from review here (a decision records a wait, ATM-657), T-3 to do here but in review elsewhere
    let f = Fixture::new(
        json!({}),
        json!([
            {"id": "T-1", "status": "in_review", "review_state": "changes_requested"},
            {"id": "T-2", "status": "in_review", "review_state": "approved"},
            {"id": "T-3", "status": "todo"},
            {"id": "T-4", "status": "todo"}
        ]),
    );
    let store = json!({"waiting": {
        "T-1": {"task": "T-1", "worktree": "/work/here", "status": "in_review"},
        "T-2": {"task": "T-2", "worktree": "/work/here", "status": "in_review"},
        "T-3": {"task": "T-3", "worktree": "/work/other", "status": "in_review"},
        "T-4": {"task": "T-4", "status": "blocked"}
    }});
    let vocab = Vocabulary::of_document(&f.document).unwrap();
    let tasks: Vec<&Task> = f.tasks.iter().collect();
    let Value::Object(store) = store else {
        unreachable!()
    };
    let ready = |here: Option<&str>| -> Vec<String> {
        loop_order(
            &vocab,
            &f.document,
            &tasks,
            &store,
            here,
            &Selection::default(),
            None,
            false,
            &|_| Ok(false),
        )
        .unwrap()
        .ready
        .iter()
        .map(|t| t["id"].as_str().unwrap().to_string())
        .collect()
    };
    // Here: its own waits don't count; a record without a worktree (made outside git) is from somewhere else
    assert_eq!(ready(HERE), ["T-1", "T-2"]);
    // Outside git: only the record without a worktree is its own
    assert_eq!(ready(None), ["T-4"]);
    // Nothing ready elsewhere names only the tasks waiting in another worktree
    let others = Fixture::new(
        json!({}),
        json!([{"id": "T-3", "status": "todo"}, {"id": "T-5", "status": "todo"}]),
    );
    let mine = json!({"waiting": {
        "T-3": {"task": "T-3", "worktree": "/work/other", "status": "in_review"},
        "T-5": {"task": "T-5", "worktree": "/work/here", "status": "blocked"}
    }});
    assert_eq!(
        others.next(&Selection::default(), mine, false).0.as_deref(),
        Some("T-5")
    );
}

#[test]
fn only_a_decided_review_sends_a_task_back_to_the_loop() {
    for (state, ready) in [
        ("waiting", false),
        ("rejected", false),
        ("approved", true),
        ("changes_requested", true),
    ] {
        let f = Fixture::new(
            json!({}),
            json!([{"id": "T-1", "status": "in_review", "review_state": state}]),
        );
        assert_eq!(
            f.order(&Selection::default()).len(),
            usize::from(ready),
            "{state}"
        );
    }
}

#[test]
fn a_task_back_from_review_waits_a_lease_for_the_agent_that_worked_on_it() {
    let back = |reviewed: &str| {
        Fixture::new(
            json!({"lease_minutes": 30}),
            json!([{
                "id": "T-1", "status": "in_review", "review_state": "changes_requested",
                "started_by": "claude:a", "reviewed_at": reviewed
            }]),
        )
    };
    let recent = back("2999-01-01T00:00:00Z");
    let long_ago = back("2020-01-01T00:00:00Z");
    let unknown = back("not a time");
    for (f, agent, offered) in [
        (&recent, Some("claude:a"), true),
        (&recent, Some("claude:b"), false),
        (&recent, None, true),
        (&recent, Some(""), true),
        (&long_ago, Some("claude:b"), true),
        (&unknown, Some("claude:b"), true),
    ] {
        let (task, _) = f.next_for(&Selection::default(), empty_store(), agent, false);
        assert_eq!(task.is_some(), offered, "{agent:?}");
    }
}

#[test]
fn the_type_narrows_the_selection() {
    let f = Fixture::new(
        json!({}),
        json!([
            {"id": "T-1", "status": "todo", "type": "bug", "priority": "P0"},
            {"id": "T-2", "status": "todo", "type": "feature"},
            {"id": "T-3", "status": "todo"}
        ]),
    );
    let features = Selection {
        task_type: Some("feature".into()),
        ..Selection::default()
    };
    assert_eq!(f.order(&features), ["T-2"]);
}

#[test]
fn a_task_listed_twice_takes_its_last_place() {
    let f = Fixture::new(
        json!({}),
        json!([
            {"id": "T-1", "status": "todo"},
            {"id": "T-2", "status": "todo"},
            {"id": "T-3", "status": "done"}
        ]),
    );
    let sel = Selection {
        tasks: vec!["T-1".into(), "T-2".into(), "T-1".into(), "T-3".into()],
        ..Selection::default()
    };
    assert_eq!(f.order(&sel), ["T-2", "T-1"]);
}

#[test]
fn an_archived_dependency_is_done() {
    let f = Fixture::new(
        json!({}),
        json!([{"id": "T-1", "status": "todo", "dependencies": ["T-0"]}]),
    );
    let vocab = Vocabulary::of_document(&f.document).unwrap();
    let tasks: Vec<&Task> = f.tasks.iter().collect();
    let store = Map::new();
    let (task, _) = next_task(
        &vocab,
        &f.document,
        &tasks,
        &store,
        HERE,
        &Selection::default(),
        None,
        false,
        &|id| Ok(id == "T-0"),
    )
    .unwrap();
    assert!(task.is_some());
}

#[test]
fn an_archive_that_cant_be_read_is_an_error() {
    let f = Fixture::new(
        json!({}),
        json!([{"id": "T-1", "status": "todo", "dependencies": ["T-0"]}]),
    );
    let vocab = Vocabulary::of_document(&f.document).unwrap();
    let tasks: Vec<&Task> = f.tasks.iter().collect();
    let store = Map::new();
    let result = next_task(
        &vocab,
        &f.document,
        &tasks,
        &store,
        HERE,
        &Selection::default(),
        None,
        false,
        &|_| Err(Error::failed("archive unreadable")),
    );
    assert_eq!(result.unwrap_err().to_string(), "archive unreadable");
}

#[test]
fn a_lease_that_isnt_a_positive_number_is_the_default() {
    for given in [
        json!(0),
        json!("0"),
        json!("soon"),
        json!(null),
        json!([30]),
        json!(false),
    ] {
        let minutes = lease_minutes(&json!({"meta": {"lease_minutes": given}}));
        assert!(
            (minutes - DEFAULT_LEASE_MINUTES).abs() < f64::EPSILON,
            "{given}"
        );
    }
    assert!(
        (lease_minutes(&json!({"meta": {"lease_minutes": " 45 "}})) - 45.0).abs() < f64::EPSILON
    );
    assert!((lease_minutes(&json!({"meta": {"lease_minutes": 0.5}})) - 0.5).abs() < f64::EPSILON);
}

fn empty_store() -> Value {
    json!({"claims": {}, "completed": {}, "waiting": {}, "resolved": {}, "hints": {}})
}

#[test]
fn the_next_task_is_by_priority_then_natural_id() {
    let f = Fixture::new(
        json!({}),
        json!([
            {"id": "T-10", "status": "todo"},
            {"id": "T-2", "status": "todo"},
            {"id": "T-3", "status": "todo", "priority": "P0"}
        ]),
    );
    assert_eq!(
        f.next(&Selection::default(), empty_store(), false)
            .0
            .as_deref(),
        Some("T-3")
    );
    let f = Fixture::new(
        json!({}),
        json!([{"id": "T-10", "status": "todo"}, {"id": "T-2", "status": "todo"}]),
    );
    assert_eq!(
        f.next(&Selection::default(), empty_store(), false)
            .0
            .as_deref(),
        Some("T-2")
    );
}

#[test]
fn work_back_from_review_comes_first_and_claimed_work_is_skipped() {
    let f = Fixture::new(
        json!({}),
        json!([
            {"id": "T-1", "status": "todo", "priority": "P0"},
            {"id": "T-2", "status": "in_review", "review_state": "changes_requested"}
        ]),
    );
    assert_eq!(
        f.next(&Selection::default(), empty_store(), false)
            .0
            .as_deref(),
        Some("T-2")
    );
    let store = json!({"claims": {"T-2": {}}, "completed": {}, "waiting": {}});
    assert_eq!(
        f.next(&Selection::default(), store, false).0.as_deref(),
        Some("T-1")
    );
}

#[test]
fn the_active_sprint_is_the_default_and_reasons_say_why_nothing_is_ready() {
    let meta = json!({"sprints": [{"id": "Alpha", "state": "active"}, {"id": "Beta"}]});
    let f = Fixture::new(
        meta.clone(),
        json!([
            {"id": "T-1", "status": "done", "sprint": "Alpha"},
            {"id": "T-2", "status": "todo", "sprint": "Beta"}
        ]),
    );
    assert_eq!(
        f.next(&Selection::default(), empty_store(), false),
        (None, Some("the sprint “Alpha” is finished: 1 done".into()))
    );
    let sel = Selection {
        sprints: vec!["beta".into()],
        ..Selection::default()
    };
    assert_eq!(f.next(&sel, empty_store(), false).0.as_deref(), Some("T-2"));
    let all = Selection {
        all: true,
        ..Selection::default()
    };
    assert_eq!(f.next(&all, empty_store(), false).0.as_deref(), Some("T-2"));

    let waiting = Fixture::new(
        meta,
        json!([{"id": "T-1", "status": "in_progress", "sprint": "Alpha"}]),
    );
    assert_eq!(
        waiting
            .next(&Selection::default(), empty_store(), false)
            .1
            .as_deref(),
        Some("nothing ready in the sprint “Alpha”: the rest is waiting (1 in progress)")
    );
    let none_active = Fixture::new(json!({"sprints": [{"id": "Beta"}]}), json!([]));
    assert_eq!(
        none_active
            .next(&Selection::default(), empty_store(), false)
            .1
            .as_deref(),
        Some("no active sprint: plan one with /atm-plan-sprint, or run with --all")
    );
}

#[test]
fn work_together_waits_for_the_user_and_dependencies_are_named() {
    let f = Fixture::new(
        json!({}),
        json!([{"id": "T-1", "status": "todo", "with": "user"}]),
    );
    assert_eq!(
        f.next(&Selection::default(), empty_store(), false)
            .1
            .as_deref(),
        Some(
            "only work done together with the user is left in the tasks: T-1 (--with-user takes it)"
        )
    );
    assert_eq!(
        f.next(&Selection::default(), empty_store(), true)
            .0
            .as_deref(),
        Some("T-1")
    );
    let deps = Fixture::new(
        json!({}),
        json!([{"id": "T-1", "status": "todo", "dependencies": ["T-9"]}]),
    );
    assert_eq!(
        deps.next(&Selection::default(), empty_store(), false)
            .1
            .as_deref(),
        Some("1 todo task(s) waiting on dependencies")
    );
    let empty = Fixture::new(json!({}), json!([]));
    assert_eq!(
        empty
            .next(&Selection::default(), empty_store(), false)
            .1
            .as_deref(),
        Some("nothing left to do")
    );
}

#[test]
fn listed_tasks_go_in_their_order_and_unknown_ones_are_refused() {
    let f = Fixture::new(
        json!({}),
        json!([{"id": "T-1", "status": "todo"}, {"id": "T-2", "status": "todo"}]),
    );
    let sel = Selection {
        tasks: vec!["T-2".into(), "T-1".into()],
        ..Selection::default()
    };
    assert_eq!(f.next(&sel, empty_store(), false).0.as_deref(), Some("T-2"));
    let vocab = Vocabulary::of_document(&f.document).unwrap();
    let tasks: Vec<&Task> = f.tasks.iter().collect();
    let bad = Selection {
        tasks: vec!["T-9".into()],
        ..Selection::default()
    };
    let store = Map::new();
    let error = next_task(
        &vocab,
        &f.document,
        &tasks,
        &store,
        HERE,
        &bad,
        None,
        false,
        &|_| Ok(false),
    )
    .unwrap_err();
    assert_eq!(
        (error.to_string(), error.code()),
        ("--tasks: no task T-9".into(), 2)
    );
}

#[test]
fn the_lease_is_the_projects_when_it_is_a_positive_number() {
    assert!((lease_minutes(&json!({})) - 120.0).abs() < f64::EPSILON);
    assert!((lease_minutes(&json!({"meta": {"lease_minutes": 30}})) - 30.0).abs() < f64::EPSILON);
    assert!((lease_minutes(&json!({"meta": {"lease_minutes": "15"}})) - 15.0).abs() < f64::EPSILON);
    assert!((lease_minutes(&json!({"meta": {"lease_minutes": -1}})) - 120.0).abs() < f64::EPSILON);
}
