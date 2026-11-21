use atm_db::MemoryStore;
use serde_json::{Value, json};

use super::*;

fn task(value: Value) -> Task {
    match value {
        Value::Object(map) => map,
        other => panic!("not a task: {other}"),
    }
}

fn project_with(meta: Value, tasks: Value, archive: Value) -> Project {
    let list = |v: Value| match v {
        Value::Array(items) => items.into_iter().map(task).collect(),
        other => panic!("not a list: {other}"),
    };
    Project::new(Box::new(
        MemoryStore::new(list(tasks), list(archive), &task(meta)).unwrap(),
    ))
}

fn project(tasks: Value) -> Project {
    project_with(json!({}), tasks, json!([]))
}

fn ids(p: &Project, filters: &Filters) -> Vec<String> {
    list_matching(p, filters)
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap().to_string())
        .collect()
}

fn filters(f: impl FnOnce(&mut Filters)) -> Filters {
    let mut out = Filters::default();
    f(&mut out);
    out
}

#[test]
fn a_search_finds_the_tasks_with_every_word_in_any_case() {
    let p = project_with(
        json!({}),
        json!([
            {"id": "T-1", "name": "Parse the config", "type": "feature", "labels": ["cli"]},
            {"id": "T-2", "name": "Docs", "description": "Explain the CONFIG file", "type": "docs"},
            {"id": "T-3", "name": "Other", "notes": [{"text": "config seen in review"}]},
            {"id": "T-4", "name": "Nothing here"},
            {"name": "no id, never found: config"}
        ]),
        json!([{"id": "T-0", "name": "archived config"}]),
    );
    let search = |q: &str, also: &str| search_tasks(&p, q, also).unwrap();
    assert_eq!(search("config", ""), ["T-1", "T-2", "T-3"]);
    assert_eq!(search("CONFIG cli", ""), ["T-1"]);
    assert_eq!(search("t-4", ""), ["T-4"], "by its ID");
    assert_eq!(search("docs", ""), ["T-2"], "by its type");
    assert_eq!(search("config missing", "").len(), 0);
    assert_eq!(search("   ", "").len(), 0, "no words, no tasks");
    // the project's name finds all of its tasks
    assert_eq!(search("Atm", "atm"), ["T-1", "T-2", "T-3", "T-4"]);
}

#[test]
fn claims_are_listed_by_task_and_entries_without_a_task_are_left_out() {
    let store = json!({"claims": {
        "b": {"task": "T-2", "agent": "a"},
        "a": {"task": "T-1", "agent": "b"},
        "c": {"agent": "no task"},
        "d": {"task": 7}
    }});
    let p = Project::new(Box::new(
        MemoryStore::new(Vec::new(), Vec::new(), &Map::new())
            .unwrap()
            .with_claims(store.as_object().unwrap().clone()),
    ));
    let tasks: Vec<Value> = list_claims(&p).iter().map(|c| c["task"].clone()).collect();
    assert_eq!(tasks, [json!("T-1"), json!("T-2")]);
    let empty = project(json!([]));
    assert_eq!(list_claims(&empty).len(), 0);
}

#[test]
fn a_done_tasks_stage_is_merged_in_pr_or_done() {
    let vocab = Vocabulary::of_meta(&Map::new()).unwrap();
    for (t, stage) in [
        (json!({"status": "done"}), Some("done")),
        (
            json!({"status": "done", "pr": {"number": 1}}),
            Some("in_pr"),
        ),
        (
            json!({"status": "done", "pr": {"number": 1}, "merged_at": "M"}),
            Some("merged"),
        ),
        (json!({"status": "done", "merged_at": "M"}), Some("merged")),
        (json!({"status": "done", "pr": "not a PR"}), Some("done")),
        (json!({"status": "todo", "pr": {"number": 1}}), None),
        (json!({"status": "skipped"}), None),
    ] {
        assert_eq!(task_stage(&vocab, &task(t.clone())), stage, "{t}");
    }
    assert!(in_open_pr(&task(json!({"pr": {}}))));
    assert!(!in_open_pr(&task(json!({"pr": {}, "merged_at": "M"}))));
    assert!(
        in_open_pr(&task(json!({"pr": {}, "merged_at": ""}))),
        "an empty time isn't a merge"
    );
    assert!(!in_open_pr(&task(json!({}))));
}

#[test]
fn several_statuses_or_sprints_keep_each_one_named() {
    let p = project_with(
        json!({"sprints": [{"id": "A"}, {"id": "B"}, {"id": "C"}]}),
        json!([
            {"id": "T-1", "status": "todo", "sprint": "A"},
            {"id": "T-2", "status": "blocked", "sprint": "B"},
            {"id": "T-3", "status": "done", "sprint": "C"}
        ]),
        json!([]),
    );
    assert_eq!(
        ids(
            &p,
            &filters(|f| f.status = vec!["todo".into(), "done".into()])
        ),
        ["T-1", "T-3"]
    );
    assert_eq!(
        ids(&p, &filters(|f| f.sprint = vec!["a".into(), "B".into()])),
        ["T-1", "T-2"]
    );
    // the sprint and the status together
    assert_eq!(
        ids(
            &p,
            &filters(|f| {
                f.sprint = vec!["a".into(), "B".into()];
                f.status = vec!["blocked".into()];
            })
        ),
        ["T-2"]
    );
}

#[test]
fn an_unknown_sprint_in_a_project_without_sprints_says_there_are_none() {
    let p = project(json!([]));
    assert_eq!(
        list_matching(&p, &filters(|f| f.sprint = vec!["X".into()]))
            .unwrap_err()
            .to_string(),
        "no sprint 'X'; the sprints: none"
    );
    // an empty release or component keeps everything, unchecked
    let p = project(json!([{"id": "T-1"}]));
    assert_eq!(
        ids(
            &p,
            &filters(|f| {
                f.release = Some(String::new());
                f.component = Some(String::new());
                f.priority = Some(String::new());
                f.agent = Some(String::new());
            })
        ),
        ["T-1"]
    );
}

#[test]
fn labels_given_as_a_text_are_its_characters() {
    let p = project(json!([
        {"id": "T-1", "labels": "ab"},
        {"id": "T-2", "labels": null},
        {"id": "T-3", "labels": ["a"]}
    ]));
    assert_eq!(
        ids(&p, &filters(|f| f.label = vec!["a".into()])),
        ["T-1", "T-3"]
    );
    assert_eq!(ids(&p, &filters(|f| f.label = vec!["b".into()])), ["T-1"]);
}

#[test]
fn the_effective_priority_is_filtered_on() {
    let p = project(json!([{"id": "T-1"}, {"id": "T-2", "priority": "P0"}]));
    // the default priority is P2: a task without one has it
    assert_eq!(
        ids(&p, &filters(|f| f.priority = Some("P2".into()))),
        ["T-1"]
    );
}

#[test]
fn views_carry_the_effective_release_and_say_when_they_are_the_archives() {
    let p = project_with(
        json!({"sprints": [{"id": "S", "release": "1.0"}], "releases": [{"id": "1.0"}]}),
        json!([{"id": "T-1", "status": "todo", "sprint": "S"}]),
        json!([{"id": "T-0", "status": "done", "pr": {"number": 1}, "merged_at": "M"}]),
    );
    let views = task_views(&p, &Filters::default()).unwrap();
    assert_eq!(views[0]["derived"]["effective_release"], json!("1.0"));
    let archived = task_views(&p, &filters(|f| f.archived = true)).unwrap();
    assert_eq!(archived.len(), 1);
    assert_eq!(archived[0]["derived"]["archived"], json!(true));
    assert_eq!(archived[0]["derived"]["stage"], json!("merged"));
    // an archived task is never ready: it isn't a working one
    assert_eq!(archived[0]["derived"]["ready"], json!(false));
}

#[test]
fn deps_of_a_task_in_both_files_are_its_working_copys() {
    let p = project_with(
        json!({}),
        json!([{"id": "T-1", "status": "todo", "dependencies": []}]),
        json!([
            {"id": "T-1", "status": "done", "dependencies": ["T-9"]},
            {"id": "T-8", "status": "done", "dependencies": ["T-1"]}
        ]),
    );
    let deps = task_dependencies(&p, "T-1").unwrap();
    assert_eq!(deps["waits_for"], json!([]));
    assert_eq!(
        deps["waited_on_by"],
        json!([{"id": "T-8", "status": "done, archived", "name": ""}])
    );
}

#[test]
fn ready_and_blocked_follow_the_dependencies() {
    let p = project(json!([
        {"id": "T-1", "status": "done"},
        {"id": "T-2", "status": "todo", "dependencies": ["T-1"]},
        {"id": "T-3", "status": "todo", "dependencies": ["T-2"]},
        {"id": "T-4", "status": "blocked"}
    ]));
    assert_eq!(
        ids(
            &p,
            &Filters {
                ready: true,
                ..Filters::default()
            }
        ),
        ["T-2"]
    );
    assert_eq!(
        ids(
            &p,
            &Filters {
                blocked: true,
                ..Filters::default()
            }
        ),
        ["T-3", "T-4"]
    );
}

#[test]
fn filters_combine_and_labels_need_all() {
    let p = project(json!([
        {"id": "T-1", "type": "test", "status": "done", "labels": ["api"]},
        {"id": "T-2", "type": "test", "status": "todo", "priority": "P0", "labels": ["api", "ux"]},
        {"id": "T-3", "type": "test", "status": "todo", "labels": ["api", "ux"]},
        {"id": "T-4", "type": "docs", "status": "todo", "labels": ["api", "ux"]}
    ]));
    let filters = Filters {
        label: vec!["api".into(), "ux".into()],
        status: vec!["todo".into()],
        priority: Some("P2".into()),
        type_: Some("test".into()),
        ..Filters::default()
    };
    assert_eq!(ids(&p, &filters), ["T-3"]);
    // An empty text keeps everything, as in Python
    assert_eq!(
        ids(
            &p,
            &Filters {
                type_: Some(String::new()),
                ..Filters::default()
            }
        )
        .len(),
        4
    );
}

#[test]
fn sprints_backlog_release_and_component() {
    let p = project_with(
        json!({"sprints": [{"id": "Alpha", "release": "1.0"}], "releases": [{"id": "1.0"}, {"id": "2.0"}]}),
        json!([
            {"id": "T-1", "sprint": "alpha", "components": ["api"]},
            {"id": "T-2", "release": "2.0"},
            {"id": "T-3", "sprint": "Gone"}
        ]),
        json!([]),
    );
    assert_eq!(
        ids(
            &p,
            &Filters {
                sprint: vec!["ALPHA".into()],
                ..Filters::default()
            }
        ),
        ["T-1"]
    );
    // A sprint the project doesn't have counts as the backlog
    assert_eq!(
        ids(
            &p,
            &Filters {
                backlog: true,
                ..Filters::default()
            }
        ),
        ["T-2", "T-3"]
    );
    assert_eq!(
        ids(
            &p,
            &Filters {
                release: Some("1.0".into()),
                ..Filters::default()
            }
        ),
        ["T-1"]
    );
    assert_eq!(
        ids(
            &p,
            &Filters {
                component: Some("api".into()),
                ..Filters::default()
            }
        ),
        ["T-1"]
    );
    let unknown = list_matching(
        &p,
        &Filters {
            sprint: vec!["Nope".into()],
            ..Filters::default()
        },
    )
    .unwrap_err();
    assert_eq!(
        (unknown.to_string(), unknown.code()),
        ("no sprint 'Nope'; the sprints: Alpha".into(), 2)
    );
    assert!(
        list_matching(
            &p,
            &Filters {
                release: Some("9".into()),
                ..Filters::default()
            }
        )
        .is_err()
    );
}

#[test]
fn agents_pull_requests_and_work_together() {
    let p = project(json!([
        {"id": "T-1", "status": "done", "started_by": "alice", "pr": {"number": 1}},
        {"id": "T-2", "status": "done", "agent": "alice", "pr": {"number": 1}, "merged_at": "x"},
        {"id": "T-3", "status": "todo", "with": "user"}
    ]));
    assert_eq!(
        ids(
            &p,
            &Filters {
                agent: Some("alice".into()),
                ..Filters::default()
            }
        ),
        ["T-1", "T-2"]
    );
    assert_eq!(
        ids(
            &p,
            &Filters {
                in_pr: true,
                ..Filters::default()
            }
        ),
        ["T-1"]
    );
    assert_eq!(
        ids(
            &p,
            &Filters {
                with_user: true,
                ..Filters::default()
            }
        ),
        ["T-3"]
    );
}

#[test]
fn the_archive_lists_instead_with_archived() {
    let p = project_with(
        json!({}),
        json!([{"id": "T-2"}]),
        json!([{"id": "T-1", "status": "done"}]),
    );
    assert_eq!(
        ids(
            &p,
            &Filters {
                archived: true,
                ..Filters::default()
            }
        ),
        ["T-1"]
    );
}

#[test]
fn views_derive_what_applies_and_leave_out_the_rest() {
    let p = project_with(
        json!({}),
        json!([
            {"id": "T-2", "status": "todo", "dependencies": ["T-1"]},
            {"id": "T-3", "status": "todo", "dependencies": ["T-2"], "priority": "P0"},
            {"id": "T-4", "status": "blocked", "waiting_on": "user"},
            {"id": "T-5", "status": "done", "pr": {"number": 2}}
        ]),
        json!([{"id": "T-1", "status": "done"}]),
    );
    let views = task_views(&p, &Filters::default()).unwrap();
    assert_eq!(
        views[0]["derived"],
        json!({
            "kind": "todo", "ready": true, "waiting_for": [], "priority": "P2", "effective_review": "none", "archived": false,
            "agent_work": true
        })
    );
    assert_eq!(views[1]["derived"]["waiting_for"], json!(["T-2"]));
    // Who each waits on, from atm's one rule (ATM-717): its dependency, the user (a need of theirs)
    assert_eq!(
        views[1]["derived"]["waiting"],
        json!({"on": "dependencies"})
    );
    assert_eq!(views[2]["derived"]["waiting_on"], json!("user"));
    assert_eq!(views[2]["derived"]["waiting"], json!({"on": "user"}));
    assert_eq!(views[2]["derived"]["needs_you"], json!("blocked"));
    assert_eq!(views[2]["derived"]["agent_work"], json!(false));
    assert_eq!(views[3]["derived"]["stage"], json!("in_pr"));
    assert_eq!(
        views[0]["derived"]
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>(),
        [
            "kind",
            "ready",
            "waiting_for",
            "priority",
            "effective_review",
            "archived",
            "agent_work"
        ]
    );
}

#[test]
fn deps_say_what_a_task_waits_for_and_what_waits_for_it_archived_ones_too() {
    let p = project_with(
        json!({}),
        json!([
            {"id": "T-2", "name": "Two", "status": "todo", "dependencies": ["T-1", "T-9"]},
            {"id": "T-5", "name": "Live", "status": "todo", "dependencies": ["T-2"]}
        ]),
        json!([
            {"id": "T-1", "name": "Old", "status": "done"},
            {"id": "T-3", "name": "Older", "status": "done", "dependencies": ["T-2"]}
        ]),
    );
    assert_eq!(
        task_dependencies(&p, "T-2").unwrap(),
        json!({
            "id": "T-2",
            "waits_for": [
                {"id": "T-1", "status": "done, archived", "name": "Old"},
                {"id": "T-9", "status": "missing", "name": ""}
            ],
            "waited_on_by": [
                {"id": "T-5", "status": "todo", "name": "Live"},
                {"id": "T-3", "status": "done, archived", "name": "Older"}
            ]
        })
    );
    assert_eq!(
        task_dependencies(&p, "T-1").unwrap()["waits_for"],
        json!([])
    );
    assert_eq!(
        task_dependencies(&p, "T-7").unwrap_err(),
        Error::NotFound("no task T-7".into())
    );
}
