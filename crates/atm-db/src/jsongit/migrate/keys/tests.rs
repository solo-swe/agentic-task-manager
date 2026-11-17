use super::*;
use atm_common::ids::valid_task_uuid;
use atm_common::keys::{Via, resolve};
use serde_json::json;

fn legacy() -> (Value, Value, Map<String, Value>) {
    let document = json!({
        "meta": {
            "id": "3f2a9c1e",
            "project": "Agentic Task Manager",
            "next_id": 14,
            "sprints": [{"id": "One", "state": "closed", "report": {"done": ["T-001"], "carried": ["T-012"], "to": "Two"}}]
        },
        "tasks": [
            {"id": "T-012", "name": "twelve", "status": "todo", "dependencies": ["T-001", "T-404"], "related": ["T-P1-03"]},
            {"id": "T-P1-03", "name": "odd", "status": "todo"},
            {"id": "T-013", "name": "thirteen", "status": "skipped", "resolution": "duplicate", "duplicate_of": "T-012"}
        ]
    });
    let archive = json!({"meta": {}, "tasks": [{"id": "T-001", "name": "one", "status": "done"}]});
    let claims = json!({
        "claims": {"T-012": {"task": "T-012", "agent": "claude:a"}},
        "completed": {"T-001": {"task": "T-001"}},
        "waiting": {},
        "hints": {"archive": "2026-11-01"}
    })
    .as_object()
    .unwrap()
    .clone();
    (document, archive, claims)
}

fn migrated() -> (Value, Value, Map<String, Value>, KeysReport) {
    let (mut document, mut archive, mut claims) = legacy();
    let report = to_keys_format(&mut document, &mut archive, &mut claims).unwrap();
    (document, archive, claims, report)
}

fn uuid(old: &str) -> String {
    migrated_uuid("3f2a9c1e", old)
}

#[test]
fn every_task_gets_a_uuid_its_number_and_is_final() {
    let (document, archive, _, report) = migrated();
    assert_eq!(
        report,
        KeysReport {
            key: "ATM".into(),
            old_prefix: Some("T".into()),
            tasks: 4,
            archived: 1,
            renumbered: vec![("T-P1-03".into(), 14)],
        }
    );
    let t = &document["tasks"];
    assert_eq!(t[0]["id"], json!(uuid("T-012")));
    assert!(valid_task_uuid(t[0]["id"].as_str().unwrap()));
    assert_eq!(
        &t[0]["id"].as_str().unwrap()[14..15],
        "8",
        "version 8, from the project and old ID"
    );
    assert_eq!(
        (t[0]["number"].clone(), t[0]["key_final"].clone()),
        (json!(12), json!(true))
    );
    assert_eq!(
        t[0].get("aliases"),
        None,
        "T-012 still finds it through the old prefix"
    );
    assert_eq!(
        t[1]["number"],
        json!(14),
        "an ID off the prefix gets the next free number"
    );
    assert_eq!(t[1]["aliases"], json!(["T-P1-03"]));
    assert_eq!(archive["tasks"][0]["number"], json!(1));
    assert_eq!(archive["meta"]["format"], json!(2));
    let keys: Vec<&String> = t[0].as_object().unwrap().keys().take(4).collect();
    assert_eq!(
        keys,
        ["id", "number", "key_final", "name"],
        "the new fields first"
    );
}

#[test]
fn references_become_uuids_and_meta_says_the_format_key_and_next_number() {
    let (document, _, claims, _) = migrated();
    let t = &document["tasks"];
    assert_eq!(
        t[0]["dependencies"],
        json!([uuid("T-001"), "T-404"]),
        "an unknown one stays for atm doctor"
    );
    assert_eq!(t[0]["related"], json!([uuid("T-P1-03")]));
    assert_eq!(t[2]["duplicate_of"], json!(uuid("T-012")));
    let meta = &document["meta"];
    assert_eq!(meta["format"], json!(2));
    assert_eq!(meta["key"], json!("ATM"));
    assert_eq!(meta["key_history"], json!(["T"]));
    assert_eq!(meta["next_number"], json!(15));
    assert_eq!(meta.get("next_id"), None);
    let report = &meta["sprints"][0]["report"];
    assert_eq!(report["done"], json!([uuid("T-001")]));
    assert_eq!(report["carried"], json!([uuid("T-012")]));
    assert_eq!(report["to"], json!("Two"));
    assert_eq!(
        claims["claims"][uuid("T-012").as_str()]["task"],
        json!(uuid("T-012"))
    );
    assert_eq!(
        claims["claims"][uuid("T-012").as_str()]["agent"],
        json!("claude:a")
    );
    assert_eq!(
        claims["completed"][uuid("T-001").as_str()]["task"],
        json!(uuid("T-001"))
    );
    assert_eq!(
        claims["hints"],
        json!({"archive": "2026-11-01"}),
        "not a task's"
    );
}

#[test]
fn every_old_id_still_finds_its_task() {
    let (document, archive, _, _) = migrated();
    let archived: Vec<atm_common::Task> = archive["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t.as_object().unwrap().clone())
        .collect();
    for (old, number, via) in [
        ("T-012", 12, Via::EarlierPrefix),
        ("T-12", 12, Via::EarlierPrefix),
        ("ATM-12", 12, Via::Key),
        ("T-P1-03", 14, Via::Alias),
        ("T-001", 1, Via::EarlierPrefix),
    ] {
        let found = resolve(&document, &archived, old).unwrap();
        assert_eq!(
            (found.task["number"].as_u64(), found.via),
            (Some(number), via),
            "{old}"
        );
    }
}

#[test]
fn the_same_file_migrates_the_same_in_every_clone() {
    let (a, ..) = migrated();
    let (b, ..) = migrated();
    assert_eq!(a, b);
    assert_ne!(
        migrated_uuid("3f2a9c1e", "T-1"),
        migrated_uuid("deadbeef", "T-1"),
        "per project"
    );
}

#[test]
fn a_file_already_migrated_or_newer_is_left_alone() {
    let (mut document, mut archive, mut claims, _) = migrated();
    let before = document.clone();
    assert_eq!(
        to_keys_format(&mut document, &mut archive, &mut claims),
        None
    );
    assert_eq!(document, before);
}

#[test]
fn a_key_the_file_names_is_kept_and_the_suggestion_falls_back() {
    let (mut document, mut archive, mut claims) = legacy();
    document["meta"]["key"] = json!("TaskMa");
    let report = to_keys_format(&mut document, &mut archive, &mut claims).unwrap();
    assert_eq!(report.key, "TaskMa");
    assert_eq!(suggested_key("Agentic Task Manager", Some("T")), "ATM");
    assert_eq!(suggested_key("Fig", Some("FIG")), "FIG");
    assert_eq!(
        suggested_key("Notes", Some("NT")),
        "NOTES",
        "initials too short, prefix too"
    );
    assert_eq!(suggested_key("A b", Some("T")), "TASK");
    assert_eq!(suggested_key("Learn Forever Blog", None), "LFB");
}

#[test]
fn ids_without_a_dash_prefix_keep_their_old_ids_as_aliases() {
    let mut document = json!({"meta": {"project": "Fig"}, "tasks": [{"id": "F1"}, {"id": "F2", "dependencies": ["F1"]}]});
    let mut archive = json!({"tasks": []});
    let report = to_keys_format(&mut document, &mut archive, &mut Map::new()).unwrap();
    assert_eq!(report.old_prefix, None);
    assert_eq!(report.key, "FIG");
    assert_eq!(document["tasks"][0]["aliases"], json!(["F1"]));
    assert_eq!(document["tasks"][1]["number"], json!(2));
    assert_eq!(document["meta"].get("key_history"), None);
    assert_eq!(
        archive.get("meta"),
        None,
        "an empty archive stays as it was"
    );
}
