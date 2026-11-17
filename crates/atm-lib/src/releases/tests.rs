use atm_db::MemoryStore;
use serde_json::{Value, json};

use super::*;

fn task(v: Value) -> Task {
    match v {
        Value::Object(m) => m,
        other => panic!("not a task: {other}"),
    }
}

fn list(tasks: Value) -> Vec<Task> {
    match tasks {
        Value::Array(items) => items.into_iter().map(task).collect(),
        other => panic!("not a list: {other}"),
    }
}

fn project(tasks: Value, meta: Value) -> Project {
    project_with_archive(tasks, json!([]), meta)
}

fn project_with_archive(tasks: Value, archive: Value, meta: Value) -> Project {
    Project::new(Box::new(
        MemoryStore::new(list(tasks), list(archive), &task(meta)).unwrap(),
    ))
}

fn document(p: &Project) -> Value {
    p.store().document().unwrap().clone()
}

/// The refusal's message, with the store left as it was.
fn refused<T: std::fmt::Debug>(
    p: &mut Project,
    change: impl FnOnce(&mut Project) -> Result<T>,
) -> String {
    let before = document(p);
    let error = change(p).unwrap_err();
    assert_eq!(document(p), before, "a refusal changes nothing");
    assert_eq!(error.code(), 2, "{error}");
    error.to_string()
}

#[test]
fn a_release_id_is_a_short_id() {
    let mut p = project(json!([]), json!({}));
    for bad in ["", "1 0", "a/b", &"x".repeat(65)] {
        assert!(
            refused(&mut p, |p| add_release(p, bad, None, None, &[], None))
                .ends_with(": a release ID is letters, digits, '.', '-' and '_' (at most 64)"),
            "{bad}"
        );
    }
    add_release(&mut p, &"x".repeat(64), None, None, &[], None).unwrap();
    add_release(&mut p, "v1.0_rc-1", None, None, &[], None).unwrap();
}

#[test]
fn a_new_release_checks_its_pr_numbers_time_and_target() {
    let mut p = project(json!([]), json!({}));
    for n in [0, -3] {
        assert_eq!(
            refused(&mut p, |p| add_release(p, "1.0", None, None, &[n], None)),
            "--pr needs a pull request number"
        );
    }
    assert_eq!(
        refused(&mut p, |p| add_release(
            p,
            "1.0",
            None,
            None,
            &[],
            Some("yesterday")
        )),
        "--released-at needs a time like 2026-10-24T18:00:00Z, not 'yesterday'"
    );
    assert_eq!(
        refused(&mut p, |p| add_release(
            p,
            "1.0",
            None,
            Some("soon"),
            &[],
            None
        )),
        "--target 'soon': use a date, YYYY-MM-DD"
    );
    let added = add_release(
        &mut p,
        "1.0",
        None,
        Some("2026-12-01"),
        &[],
        Some("2026-10-24T18:00:00Z"),
    )
    .unwrap();
    assert_eq!(
        added.release,
        json!({"id": "1.0", "state": "released", "target": "2026-12-01", "released_at": "2026-10-24T18:00:00Z"})
    );
    assert_eq!(added.tasks.len(), 0);
}

#[test]
fn a_release_from_pull_requests_takes_their_tasks_in_both_files() {
    let mut p = project_with_archive(
        json!([
            {"id": "T-1", "status": "done", "pr": {"number": 7, "url": "https://x/7"}},
            {"id": "T-2", "status": "done", "pr": {"number": 8}},
            {"id": "T-3", "status": "done", "pr": {"number": 9}}
        ]),
        json!([{"id": "T-0", "status": "done", "pr": {"number": 7}}]),
        json!({}),
    );
    assert_eq!(
        refused(&mut p, |p| add_release(p, "1.0", None, None, &[7, 5], None)),
        "no task is in pull request #5"
    );
    let added = add_release(&mut p, "1.0", None, None, &[7, 8], None).unwrap();
    assert_eq!(added.tasks, ["T-1", "T-0", "T-2"]);
    // the PR recorded is the last one with an address
    assert_eq!(
        added.release["pr"],
        json!({"number": 7, "url": "https://x/7"})
    );
    let doc = document(&p);
    assert_eq!(doc["tasks"][1]["release"], json!("1.0"));
    assert!(doc["tasks"][2].get("release").is_none());
    assert_eq!(
        p.store().list_archived().unwrap()[0]["release"],
        json!("1.0")
    );
}

#[test]
fn the_release_list_counts_each_tasks_effective_release_by_kind_and_stage() {
    let p = project_with_archive(
        json!([
            {"id": "T-1", "status": "done", "release": "1.0"},
            {"id": "T-2", "status": "done", "release": "1.0", "pr": {"number": 1}},
            {"id": "T-3", "status": "done", "release": "1.0", "pr": {"number": 1}, "merged_at": "M"},
            {"id": "T-4", "status": "todo", "sprint": "S"},
            {"id": "T-5", "status": "todo", "sprint": "S", "release": "2.0"},
            {"id": "T-6", "status": "done", "release": "1.0", "merged_at": "M"}
        ]),
        json!([{"id": "T-0", "status": "done", "release": "1.0"}]),
        json!({
            "releases": [{"id": "1.0"}, {"id": "2.0"}, {"id": "0.9", "state": "archived"}],
            "sprints": [{"id": "S", "release": "1.0"}]
        }),
    );
    let (listed, any) = list_releases(&p, false).unwrap();
    assert!(any);
    assert_eq!(listed.len(), 2, "the archived release is hidden");
    let one = &listed[0];
    assert_eq!(one["tasks"], json!(6));
    assert_eq!(one["by_kind"], json!({"todo": 1, "done": 5}));
    // merged needs a PR: a merged_at without one is just done
    assert_eq!(one["by_stage"], json!({"done": 3, "in_pr": 1, "merged": 1}));
    assert_eq!(listed[1]["by_kind"], json!({"todo": 1}));
    assert_eq!(list_releases(&p, true).unwrap().0.len(), 3);
    let none = project(json!([]), json!({}));
    assert_eq!(list_releases(&none, true).unwrap(), (Vec::new(), false));
}

#[test]
fn editing_a_release_leaves_what_isnt_given_and_refuses_an_unknown_one() {
    let mut p = project(
        json!([]),
        json!({"releases": [{"id": "1.0", "name": "One"}]}),
    );
    let (release, changed) = edit_release(&mut p, "1.0", Some("One"), None).unwrap();
    assert_eq!(changed.len(), 0);
    assert_eq!(release, json!({"id": "1.0", "name": "One"}));
    assert_eq!(
        refused(&mut p, |p| edit_release(p, "2.0", Some("x"), None)),
        "no release '2.0'; the releases: 1.0"
    );
    assert_eq!(
        refused(&mut p, |p| edit_release(p, "1.0", None, Some("2026/12/01"))),
        "--target '2026/12/01': use a date, YYYY-MM-DD"
    );
    let mut empty = project(json!([]), json!({}));
    assert_eq!(
        refused(&mut empty, |p| edit_release(p, "1.0", None, None)),
        "no release '1.0'; the releases: none yet: atm release add <R>"
    );
}

#[test]
fn renaming_a_release_renames_it_on_its_tasks_and_sprints() {
    let mut p = project_with_archive(
        json!([
            {"id": "T-1", "status": "todo", "release": "1.0", "updated_at": "U"},
            {"id": "T-2", "status": "todo", "release": "2.0"},
            {"id": "T-3", "status": "todo", "sprint": "S"}
        ]),
        json!([{"id": "T-0", "status": "done", "release": "1.0"}]),
        json!({
            "releases": [{"id": "1.0", "name": "One", "state": "released"}, {"id": "2.0"}],
            "sprints": [{"id": "S", "release": "1.0"}]
        }),
    );
    let renamed = rename_release(&mut p, "1.0", "1.1").unwrap();
    assert_eq!(
        renamed,
        RenamedRelease {
            from: "1.0".into(),
            to: "1.1".into(),
            tasks: 1,
            archived: 1,
            sprints: 1
        }
    );
    let doc = document(&p);
    assert_eq!(
        doc["meta"]["releases"][0],
        json!({"id": "1.1", "name": "One", "state": "released"})
    );
    assert_eq!(doc["tasks"][0]["release"], json!("1.1"));
    assert_eq!(doc["tasks"][0]["updated_at"], json!("U"));
    assert_eq!(doc["meta"]["sprints"][0]["release"], json!("1.1"));
    assert_eq!(
        p.store().list_archived().unwrap()[0]["release"],
        json!("1.1")
    );
}

#[test]
fn a_release_is_renamed_only_to_a_free_valid_id() {
    let mut p = project(
        json!([]),
        json!({"releases": [{"id": "1.0"}, {"id": "2.0"}]}),
    );
    assert!(refused(&mut p, |p| rename_release(p, "1.0", "a b")).ends_with("(at most 64)"));
    assert_eq!(
        refused(&mut p, |p| rename_release(p, "1.0", "1.0")),
        "the release is already '1.0'"
    );
    assert_eq!(
        refused(&mut p, |p| rename_release(p, "1.0", "2.0")),
        "there is already a release '2.0'"
    );
    assert_eq!(
        refused(&mut p, |p| rename_release(p, "3.0", "4.0")),
        "no release '3.0'; the releases: 1.0, 2.0"
    );
}

#[test]
fn marking_a_release_counts_its_sprints_and_archived_tasks_and_dates_it_now_without_a_date() {
    let mut p = project_with_archive(
        json!([
            {"id": "T-1", "status": "todo", "sprint": "S"},
            {"id": "T-2", "status": "in_progress", "release": "1.0"}
        ]),
        json!([{"id": "T-0", "status": "done", "release": "1.0"}]),
        json!({"releases": [{"id": "1.0"}], "sprints": [{"id": "S", "release": "1.0"}]}),
    );
    assert_eq!(
        refused(&mut p, |p| mark_release(
            p,
            "1.0",
            &Mark::Released(None),
            false
        )),
        "T-1, T-2 are still open in the release '1.0': finish them, move them to another release, or mark it with --force"
    );
    let marked = mark_release(&mut p, "1.0", &Mark::Released(None), true).unwrap();
    assert_eq!(marked.unmerged, ["T-0"]);
    let at = marked.release["released_at"].as_str().unwrap();
    assert!(parse_iso(at).is_some(), "{at}");
}

#[test]
fn an_archived_release_isnt_archived_again_and_an_unknown_one_is_refused() {
    let mut p = project(json!([]), json!({"releases": [{"id": "1.0"}]}));
    assert_eq!(
        archive_release(&mut p, "1.0").unwrap(),
        json!({"id": "1.0", "state": "archived"})
    );
    assert_eq!(
        refused(&mut p, |p| archive_release(p, "1.0")),
        "the release '1.0' is already archived"
    );
    assert_eq!(
        refused(&mut p, |p| archive_release(p, "2.0")),
        "no release '2.0'; the releases: 1.0"
    );
}

#[test]
fn release_notes_group_the_done_tasks_by_type_in_the_projects_order() {
    let p = project_with_archive(
        json!([
            {"id": "T-1", "status": "done", "type": "bugfix", "name": "Fix", "release": "1.0",
             "notes": [{"kind": "outcome", "text": "first"}, {"kind": "note", "text": "n"},
                       {"kind": "outcome", "text": "last"}]},
            {"id": "T-2", "status": "done", "type": "feature", "name": "Feat", "sprint": "S"},
            {"id": "T-3", "status": "done", "type": "odd", "name": "Odd", "release": "1.0"},
            {"id": "T-4", "status": "blocked", "name": "Wait", "release": "1.0"},
            {"id": "T-5", "status": "skipped", "release": "1.0"},
            {"id": "T-6", "status": "done", "type": "feature", "release": "2.0"}
        ]),
        json!([{"id": "T-0", "status": "done", "type": "feature", "name": "Old", "release": "1.0"}]),
        json!({
            "releases": [{"id": "1.0"}, {"id": "2.0"}],
            "sprints": [{"id": "S", "release": "1.0"}]
        }),
    );
    let notes = release_notes(&p, "1.0").unwrap();
    assert_eq!(notes["release"]["id"], json!("1.0"));
    let groups: Vec<(Value, Vec<Value>)> = notes["groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| {
            (
                g["type"].clone(),
                g["tasks"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|t| t["id"].clone())
                    .collect(),
            )
        })
        .collect();
    let vocab = p.store().vocabulary().unwrap();
    let place = |ty: &str| vocab.types.iter().position(|t| t.id == ty).unwrap();
    assert!(
        place("feature") < place("bugfix"),
        "the fixture assumes the default order"
    );
    assert_eq!(
        groups,
        [
            (json!("feature"), vec![json!("T-2"), json!("T-0")]),
            (json!("bugfix"), vec![json!("T-1")]),
            // a type the project doesn't list comes last, under its own name
            (json!("odd"), vec![json!("T-3")]),
        ]
    );
    assert_eq!(notes["groups"][1]["tasks"][0]["outcome"], json!("last"));
    assert_eq!(notes["groups"][0]["tasks"][0]["outcome"], json!(null));
    assert_eq!(notes["groups"][2]["label"], json!("odd"));
    assert_eq!(
        notes["open"],
        json!([{"id": "T-4", "name": "Wait", "status": "blocked"}])
    );
}

#[test]
fn a_release_takes_the_tasks_of_its_pull_requests() {
    let mut p = project(
        json!([
            {"id": "T-1", "status": "done", "type": "feature", "pr": {"number": 7, "url": "https://x/7"}},
            {"id": "T-2", "status": "todo", "type": "bugfix"}
        ]),
        json!({}),
    );
    assert!(add_release(&mut p, "1 0", None, None, &[], None).is_err());
    assert!(add_release(&mut p, "1.0", None, None, &[9], None).is_err());
    let added = add_release(&mut p, "1.0", Some("One"), None, &[7], None).unwrap();
    assert_eq!(added.tasks, ["T-1"]);
    assert_eq!(
        added.release["pr"],
        json!({"number": 7, "url": "https://x/7"})
    );
    assert!(add_release(&mut p, "1.0", None, None, &[], None).is_err());
    let (listed, any) = list_releases(&p, false).unwrap();
    assert!(any);
    assert_eq!(listed[0]["by_stage"], json!({"in_pr": 1}));
    let (edited, changed) = edit_release(&mut p, "1.0", Some(""), Some("2026-12-01")).unwrap();
    assert_eq!(changed, ["name", "target"]);
    assert!(edited.get("name").is_none());
    let notes = release_notes(&p, "1.0").unwrap();
    assert_eq!(notes["groups"][0]["type"], json!("feature"));
    archive_release(&mut p, "1.0").unwrap();
    assert!(archive_release(&mut p, "1.0").is_err());
    assert_eq!(list_releases(&p, false).unwrap().0.len(), 0);
    assert_eq!(
        release_notes(&p, "2.0").unwrap_err().to_string(),
        "no release '2.0'; the releases: 1.0"
    );
}

/// T-527: a release marked by hand. Open tasks refuse it unless forced, done but unmerged ones are only named, a
/// skipped one counts as neither; planned again drops its date
#[test]
fn a_release_is_marked_released_by_hand_and_open_tasks_refuse_it() {
    let mut p = project(
        json!([
            {"id": "T-1", "status": "done", "release": "1.0", "pr": {"number": 7}, "merged_at": "2026-11-13T03:00:00Z"},
            {"id": "T-2", "status": "done", "release": "1.0"},
            {"id": "T-3", "status": "skipped", "release": "1.0"},
            {"id": "T-4", "status": "blocked", "release": "1.0"}
        ]),
        json!({"releases": [{"id": "1.0"}]}),
    );
    let refused =
        mark_release(&mut p, "1.0", &Mark::Released(Some("2026-11-13")), false).unwrap_err();
    assert!(
        refused
            .to_string()
            .starts_with("T-4 is still open in the release '1.0'"),
        "{refused}"
    );
    assert!(mark_release(&mut p, "1.0", &Mark::Released(Some("13/11/2026")), true).is_err());
    let marked = mark_release(&mut p, "1.0", &Mark::Released(Some("2026-11-13")), true).unwrap();
    assert_eq!(marked.open, ["T-4"]);
    assert_eq!(marked.unmerged, ["T-2"]);
    assert_eq!(marked.release["state"], json!("released"));
    assert_eq!(marked.release["released_at"], json!("2026-11-13T00:00:00Z"));
    let planned = mark_release(&mut p, "1.0", &Mark::Planned, false).unwrap();
    assert_eq!(planned.release, json!({"id": "1.0", "state": "planned"}));
    assert!(mark_release(&mut p, "2.0", &Mark::Planned, false).is_err());
}
