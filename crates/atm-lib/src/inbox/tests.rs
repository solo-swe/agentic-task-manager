use atm_common::Task;
use atm_db::MemoryStore;
use serde_json::{Value, json};

use super::*;

const PERSON: &str = "Hakan Serce";
const AGENT: &str = "claude:ab12";

fn task(v: Value) -> Task {
    match v {
        Value::Object(m) => m,
        other => panic!("not a task: {other}"),
    }
}

/// A project of two tasks, its inbox `inbox` (and archive), worked on by `actor`
fn project_with(inbox: Value, archive: Value, actor: &str) -> Project {
    let store = MemoryStore::new(
        vec![
            task(json!({"id": "T-1", "name": "One"})),
            task(json!({"id": "T-2", "name": "Two"})),
        ],
        vec![task(json!({"id": "T-0", "name": "Old", "status": "done"}))],
        &serde_json::Map::new(),
    )
    .unwrap()
    .with_inbox(inbox, archive);
    Project::new(Box::new(store)).with_actor(Some(actor.to_string()))
}

fn empty(actor: &str) -> Project {
    project_with(
        atm_common::inbox::empty_inbox(),
        atm_common::inbox::empty_inbox(),
        actor,
    )
}

/// The same project, now worked on by `actor`
fn as_actor(project: Project, actor: &str) -> Project {
    project.with_actor(Some(actor.to_string()))
}

fn item(id: &str, state: &str, decided_at: Option<&str>) -> Value {
    let mut item = json!({
        "id": id, "text": format!("{id} text"), "state": state,
        "created_at": "2026-11-24T09:00:00Z", "created_by": PERSON, "updated_at": "2026-11-24T09:00:00Z",
    });
    if let Some(at) = decided_at {
        item["decided_at"] = json!(at);
        item["decided_by"] = json!(PERSON);
    }
    item
}

fn inbox(items: &[Value]) -> Value {
    json!({"version": 1, "next": items.len() + 1, "items": items})
}

fn ids(listed: &Listed) -> Vec<String> {
    listed
        .items
        .iter()
        .map(|i| i["id"].as_str().unwrap().to_string())
        .collect()
}

// Adding

#[test]
fn a_person_adds_an_item_numbered_after_the_last_open_with_who_and_when() {
    let mut p = empty(PERSON);
    let first = add(&mut p, "  An idea\nwith a second line  ").unwrap();
    assert_eq!(first["id"], "IN-1");
    assert_eq!(first["text"], "An idea\nwith a second line");
    assert_eq!(first["state"], "created");
    assert_eq!(first["created_by"], PERSON);
    assert!(first["created_at"].is_string() && first["updated_at"] == first["created_at"]);
    assert_eq!(add(&mut p, "Another").unwrap()["id"], "IN-2");
    let stored = p.store().inbox(false).unwrap();
    assert_eq!(stored["next"], 3);
    assert_eq!(stored["version"], 1);
}

#[test]
fn an_agent_can_t_add_to_the_inbox() {
    let mut p = empty(AGENT);
    let error = add(&mut p, "From an agent").unwrap_err();
    assert!(matches!(error, Error::Refused(_)), "{error}");
    assert!(error.to_string().contains(AGENT));
    assert_eq!(
        p.store().inbox(false).unwrap()["items"],
        json!([]),
        "nothing added"
    );
}

#[test]
fn an_item_s_text_is_not_empty_and_within_its_limit() {
    let mut p = empty(PERSON);
    assert!(matches!(add(&mut p, "  \n "), Err(Error::Invalid(_))));
    assert!(matches!(
        add(&mut p, &"x".repeat(20_001)),
        Err(Error::Invalid(_))
    ));
    assert!(add(&mut p, &"x".repeat(20_000)).is_ok());
}

#[test]
fn numbers_are_never_reused_and_follow_the_highest_ever_given() {
    // An archived IN-7 keeps its number: `next` past it
    let mut p = project_with(
        json!({"version": 1, "next": 8, "items": [item("IN-2", "created", None)]}),
        inbox(&[item("IN-7", "discarded", Some("2020-01-01T00:00:00Z"))]),
        PERSON,
    );
    assert_eq!(add(&mut p, "after").unwrap()["id"], "IN-8");
}

// Editing, discarding, restoring

#[test]
fn a_person_edits_an_open_item_and_no_one_edits_a_decided_one() {
    let mut p = project_with(
        inbox(&[
            item("IN-1", "created", None),
            item("IN-2", "discarded", Some("2026-11-24T10:00:00Z")),
        ]),
        atm_common::inbox::empty_inbox(),
        PERSON,
    );
    let edited = edit(&mut p, "in-1", "Better words").unwrap();
    assert_eq!(edited["text"], "Better words");
    assert_eq!(edited["updated_by"], PERSON);
    assert_ne!(edited["updated_at"], "2026-11-24T09:00:00Z");
    let refused = edit(&mut p, "IN-2", "x").unwrap_err();
    assert!(
        matches!(&refused, Error::Refused(m) if m.contains("IN-2 is discarded")),
        "{refused}"
    );
    let mut agent = as_actor(p, AGENT);
    assert!(matches!(
        edit(&mut agent, "IN-1", "x"),
        Err(Error::Refused(_))
    ));
}

#[test]
fn a_discard_needs_a_reason_and_an_open_item_by_anyone() {
    let mut p = project_with(
        inbox(&[item("IN-1", "created", None), item("IN-2", "created", None)]),
        atm_common::inbox::empty_inbox(),
        AGENT,
    );
    assert!(matches!(
        discard(&mut p, "IN-1", " "),
        Err(Error::Invalid(_))
    ));
    let discarded = discard(&mut p, "IN-1", "  A duplicate of ATM-12  ").unwrap();
    assert_eq!(discarded["state"], "discarded");
    assert_eq!(discarded["reason"], "A duplicate of ATM-12");
    assert_eq!(
        discarded["decided_by"], AGENT,
        "an agent, on the user's word in the triage"
    );
    assert!(discarded["decided_at"].is_string());
    let again = discard(&mut p, "IN-1", "again").unwrap_err();
    assert!(matches!(again, Error::Refused(_)));
}

#[test]
fn a_person_restores_a_discarded_item_which_forgets_its_reason() {
    let mut p = project_with(
        inbox(&[item("IN-1", "created", None)]),
        atm_common::inbox::empty_inbox(),
        PERSON,
    );
    discard(&mut p, "IN-1", "not now").unwrap();
    let mut agent = as_actor(p, AGENT);
    assert!(matches!(
        restore(&mut agent, "IN-1"),
        Err(Error::Refused(_))
    ));
    let mut p = as_actor(agent, PERSON);
    let back = restore(&mut p, "IN-1").unwrap();
    assert_eq!(back["state"], "created");
    for gone in ["reason", "decided_at", "decided_by"] {
        assert!(back.get(gone).is_none(), "{gone}");
    }
    // Only a discarded item comes back
    let refused = restore(&mut p, "IN-1").unwrap_err();
    assert!(
        matches!(&refused, Error::Refused(m) if m.contains("only a discarded item")),
        "{refused}"
    );
}

// Injecting

#[test]
fn an_injection_links_the_item_and_its_tasks_both_ways_and_is_final() {
    let mut p = project_with(
        inbox(&[item("IN-1", "created", None)]),
        atm_common::inbox::empty_inbox(),
        AGENT,
    );
    let injected = inject(&mut p, "IN-1", &["T-2".into(), "T-1".into(), "T-2".into()]).unwrap();
    assert_eq!(injected["state"], "injected");
    assert_eq!(
        injected["tasks"],
        json!(["T-2", "T-1"]),
        "each once, as named"
    );
    assert_eq!(injected["decided_by"], AGENT);
    for id in ["T-1", "T-2"] {
        assert_eq!(
            p.store().get_task(id).unwrap().unwrap()["from"],
            json!(["IN-1"]),
            "{id}"
        );
    }
    // Final: no other move
    for refused in [
        discard(&mut p, "IN-1", "x").unwrap_err(),
        inject(&mut p, "IN-1", &["T-1".into()]).unwrap_err(),
    ] {
        assert!(matches!(refused, Error::Refused(_)));
    }
    let mut person = as_actor(p, PERSON);
    assert!(matches!(
        restore(&mut person, "IN-1"),
        Err(Error::Refused(_))
    ));
    assert!(matches!(
        edit(&mut person, "IN-1", "x"),
        Err(Error::Refused(_))
    ));
}

#[test]
fn a_task_made_from_two_items_names_both() {
    let mut p = project_with(
        inbox(&[item("IN-1", "created", None), item("IN-2", "created", None)]),
        atm_common::inbox::empty_inbox(),
        PERSON,
    );
    inject(&mut p, "IN-1", &["T-1".into()]).unwrap();
    inject(&mut p, "IN-2", &["T-1".into()]).unwrap();
    assert_eq!(
        p.store().get_task("T-1").unwrap().unwrap()["from"],
        json!(["IN-1", "IN-2"])
    );
}

#[test]
fn an_injection_names_working_tasks_that_exist_and_at_least_one() {
    let mut p = project_with(
        inbox(&[item("IN-1", "created", None)]),
        atm_common::inbox::empty_inbox(),
        PERSON,
    );
    assert!(matches!(
        inject(&mut p, "IN-1", &[]),
        Err(Error::Invalid(_))
    ));
    assert!(
        inject(&mut p, "IN-1", &["T-9".into()]).is_err(),
        "no such task"
    );
    let archived = inject(&mut p, "IN-1", &["T-0".into()]).unwrap_err();
    assert!(archived.to_string().contains("archive"), "{archived}");
    // Nothing changed by the refusals
    assert_eq!(show(&p, "IN-1").unwrap()["state"], "created");
    assert!(
        p.store()
            .get_task("T-1")
            .unwrap()
            .unwrap()
            .get("from")
            .is_none()
    );
}

// Reading

#[test]
fn a_listing_gives_the_open_items_by_default_newest_first_with_the_counts_by_state() {
    let p = project_with(
        inbox(&[
            item("IN-1", "created", None),
            item("IN-2", "discarded", Some("2026-11-24T10:00:00Z")),
            item("IN-3", "created", None),
            item("IN-4", "injected", Some("2026-11-24T10:00:00Z")),
        ]),
        inbox(&[item("IN-9", "discarded", Some("2020-01-01T00:00:00Z"))]),
        PERSON,
    );
    let open = list(&p, Listing::Open, false).unwrap();
    assert_eq!(ids(&open), ["IN-3", "IN-1"]);
    assert_eq!((open.created, open.discarded, open.injected), (2, 1, 1));
    assert_eq!(
        ids(&list(&p, Listing::In(InboxState::Discarded), false).unwrap()),
        ["IN-2"]
    );
    assert_eq!(
        ids(&list(&p, Listing::All, false).unwrap()),
        ["IN-4", "IN-3", "IN-2", "IN-1"]
    );
    assert_eq!(ids(&list(&p, Listing::All, true).unwrap()), ["IN-9"]);
    assert_eq!(open_count(&p).unwrap(), 2);
}

#[test]
fn an_item_is_shown_from_the_inbox_or_else_its_archive() {
    let p = project_with(
        inbox(&[item("IN-1", "created", None)]),
        inbox(&[item("IN-9", "discarded", Some("2020-01-01T00:00:00Z"))]),
        PERSON,
    );
    assert_eq!(show(&p, "IN-1").unwrap()["state"], "created");
    assert_eq!(show(&p, "in-9").unwrap()["state"], "discarded");
    assert!(matches!(show(&p, "IN-5"), Err(Error::NotFound(_))));
    for bad in ["IN-0", "T-1", "", "IN-x"] {
        assert!(matches!(show(&p, bad), Err(Error::Invalid(_))), "{bad:?}");
    }
}

// Archiving

#[test]
fn archiving_moves_decided_items_older_than_the_days_kept_never_open_ones() {
    let mut p = project_with(
        inbox(&[
            item("IN-1", "created", None),
            item("IN-2", "discarded", Some("2020-01-01T00:00:00Z")),
            item("IN-3", "injected", Some("2020-01-02T00:00:00Z")),
            item("IN-4", "discarded", Some(&now_iso())),
        ]),
        atm_common::inbox::empty_inbox(),
        PERSON,
    );
    assert_eq!(archive(&mut p, Some(7.0)).unwrap(), ["IN-2", "IN-3"]);
    assert_eq!(
        ids(&list(&p, Listing::All, false).unwrap()),
        ["IN-4", "IN-1"]
    );
    assert_eq!(
        ids(&list(&p, Listing::All, true).unwrap()),
        ["IN-3", "IN-2"]
    );
    // Nothing left to move: nothing changes
    assert_eq!(archive(&mut p, Some(7.0)).unwrap(), Vec::<String>::new());
    // With no days kept, the recent decided one goes too; the open one never
    assert_eq!(archive(&mut p, Some(0.0)).unwrap(), ["IN-4"]);
    assert_eq!(ids(&list(&p, Listing::All, false).unwrap()), ["IN-1"]);
}

#[test]
fn unarchiving_brings_items_back_as_they_were_and_refuses_one_the_archive_doesn_t_hold() {
    let mut p = project_with(
        atm_common::inbox::empty_inbox(),
        inbox(&[
            item("IN-2", "discarded", Some("2020-01-01T00:00:00Z")),
            item("IN-3", "injected", Some("2020-01-01T00:00:00Z")),
        ]),
        PERSON,
    );
    assert!(matches!(
        unarchive(&mut p, &["IN-2".into(), "IN-5".into()]),
        Err(Error::NotFound(_))
    ));
    assert_eq!(
        ids(&list(&p, Listing::All, true).unwrap()),
        ["IN-3", "IN-2"],
        "nothing moved"
    );
    assert_eq!(unarchive(&mut p, &["in-2".into()]).unwrap(), ["IN-2"]);
    assert_eq!(show(&p, "IN-2").unwrap()["state"], "discarded");
    assert_eq!(ids(&list(&p, Listing::All, false).unwrap()), ["IN-2"]);
    // Past IN-3, still in the archive
    assert_eq!(p.store().inbox(false).unwrap()["next"], 4);
    // The next item is numbered past both
    assert_eq!(add(&mut p, "new").unwrap()["id"], "IN-4");
}

#[test]
fn a_new_item_never_takes_an_archived_item_s_number_whatever_the_inbox_s_next_says() {
    // An inbox whose `next` fell behind (edited by hand, or a merge): IN-1 to IN-3 are in the archive
    let mut p = project_with(
        json!({"version": 1, "next": 1, "items": []}),
        inbox(&[
            item("IN-1", "discarded", Some("2020-01-01T00:00:00Z")),
            item("IN-3", "injected", Some("2020-01-01T00:00:00Z")),
        ]),
        PERSON,
    );
    assert_eq!(add(&mut p, "new").unwrap()["id"], "IN-4");
}

#[test]
fn an_unknown_item_is_named_whatever_the_move() {
    let mut p = empty(PERSON);
    for error in [
        edit(&mut p, "IN-4", "x").unwrap_err(),
        discard(&mut p, "IN-4", "x").unwrap_err(),
        restore(&mut p, "IN-4").unwrap_err(),
        inject(&mut p, "IN-4", &["T-1".into()]).unwrap_err(),
    ] {
        assert!(
            matches!(&error, Error::NotFound(m) if m == "no inbox item IN-4"),
            "{error}"
        );
    }
}
