//! What a listener tells: a comment heard, a decision.

use super::*;

#[test]
fn a_decision_is_told_by_the_review_its_task_and_the_status_it_has_now() {
    let mut header = review(Status::ChangesRequested, &["ATM-1"]).header;
    header.updated = "2026-11-18T10:00:00Z".into();
    assert_eq!(
        decision(&header, Some("ATM-1")),
        json!({
            "review": "CR-1",
            "task": "ATM-1",
            "decision": "changes_requested",
            "at": "2026-11-18T10:00:00Z",
            "title": "T",
        })
    );
}

#[test]
fn a_decision_is_approved_changes_requested_or_closed() {
    for (status, expected) in [
        (Status::Draft, false),
        (Status::Waiting, false),
        (Status::Live, false),
        (Status::ChangesRequested, true),
        (Status::Approved, true),
        (Status::Closed, true),
    ] {
        assert_eq!(decided(status), expected, "{status:?}");
    }
}

#[test]
fn every_comment_comes_with_the_one_it_answers_depth_first() {
    let deep = comment("c3", "a", "", Vec::new());
    let c = chain(
        None,
        vec![
            comment("c1", "a", "", vec![comment("c2", "b", "", vec![deep])]),
            comment("c4", "a", "", Vec::new()),
        ],
    );
    let pairs: Vec<(&str, Option<&str>)> = with_parents(&c)
        .into_iter()
        .map(|(c, p)| (c.id.0.as_str(), p.map(|p| p.id.0.as_str())))
        .collect();
    assert_eq!(
        pairs,
        [
            ("c1", None),
            ("c2", Some("c1")),
            ("c3", Some("c2")),
            ("c4", None)
        ]
    );
    assert_eq!(with_parents(&chain(None, Vec::new())).len(), 0);
}

#[test]
fn a_comment_heard_names_its_review_chain_place_and_text() {
    let r = review(Status::Waiting, &["ATM-1"]);
    let parent = comment("c1", "claude:me", "Done", Vec::new());
    let reply = comment("c2", "Hakan", "Why?", Vec::new());
    let c = chain(Some("src/a.rs"), Vec::new());
    assert_eq!(
        heard(&r.header, &c, &reply, Some(&parent), Some("ATM-1")),
        json!({
            "review": "CR-1", "task": "ATM-1", "chain": "t1", "comment": "c2", "kind": "comment",
            "reply_to": "c1", "author": "Hakan", "at": "2026-11-17T10:00:00Z", "path": "src/a.rs",
            "lines": [3, 4], "body": "Why?"
        })
    );
    // ATM-611: a poke is heard as one
    let mut poke = comment("c3", "Hakan", "Where are we?", Vec::new());
    poke.poke = true;
    assert_eq!(
        heard(&r.header, &whole_review(), &poke, None, Some("ATM-1"))["kind"],
        "poke"
    );
    let whole = chain(None, Vec::new());
    let v = heard(&r.header, &whole, &reply, None, None);
    assert_eq!(
        (
            v["path"].clone(),
            v["lines"].clone(),
            v["reply_to"].clone(),
            v["task"].clone()
        ),
        (Value::Null, Value::Null, Value::Null, Value::Null)
    );
}
