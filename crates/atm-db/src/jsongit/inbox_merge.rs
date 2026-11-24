//! Merging the inbox (docs/inbox-design.md §8, ATM-759): `inbox.json` or its archive, three versions git hands atm's
//! merge driver, merged item by item instead of line by line.
//!
//! - An item both branches have is one item when it was made once (the same `created_at` and `created_by`): the
//!   version further on wins (injected is final; else the one changed last, `updated_at`, ours on a tie), and two
//!   injections keep every task they link.
//! - An item one branch dropped (archived it, or unarchived it from the archive) and the other left as the base had it
//!   stays dropped; one the other branch changed meanwhile is kept.
//! - The same number given to two different items (each branch added `IN-12`): ours keeps it, theirs takes the next
//!   free one and keeps the old number as `renumbered_from`, so the user can tell (a task made from it on that branch
//!   still names the old number). As the tasks' renumbering, it is no conflict: the merge goes on.
//! - `next` is past every item kept.

use atm_common::inbox::{INBOX_VERSION, InboxState, item_id, item_number, next_number, state_of};
use serde_json::{Map, Value, json};

fn items(doc: &Value) -> Vec<Map<String, Value>> {
    doc.get("items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
        .cloned()
        .collect()
}

fn id(item: &Map<String, Value>) -> String {
    item.get("id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_uppercase()
}

fn find<'a>(list: &'a [Map<String, Value>], key: &str) -> Option<&'a Map<String, Value>> {
    list.iter().find(|i| id(i) == key)
}

/// Whether two items with one number are one item: made at the same time by the same person
fn same_item(a: &Map<String, Value>, b: &Map<String, Value>) -> bool {
    a.get("created_at") == b.get("created_at") && a.get("created_by") == b.get("created_by")
}

fn text(item: &Map<String, Value>, key: &str) -> String {
    item.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// One item as both branches changed it
fn combined(ours: &Map<String, Value>, theirs: &Map<String, Value>) -> Map<String, Value> {
    if ours == theirs {
        return ours.clone();
    }
    let injected =
        |i: &Map<String, Value>| state_of(&Value::Object(i.clone())) == Some(InboxState::Injected);
    match (injected(ours), injected(theirs)) {
        (true, true) => {
            let mut merged = if text(theirs, "decided_at") > text(ours, "decided_at") {
                theirs.clone()
            } else {
                ours.clone()
            };
            let mut tasks: Vec<Value> = Vec::new();
            for task in [ours, theirs].iter().flat_map(|i| {
                i.get("tasks")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default()
            }) {
                if !tasks.contains(&task) {
                    tasks.push(task);
                }
            }
            merged.insert("tasks".into(), Value::Array(tasks));
            merged
        }
        (true, false) => ours.clone(),
        (false, true) => theirs.clone(),
        (false, false) => {
            if text(theirs, "updated_at") > text(ours, "updated_at") {
                theirs.clone()
            } else {
                ours.clone()
            }
        }
    }
}

/// The three versions of an inbox document merged item by item, and what the merge decided that the user may want to
/// know (a number given again), in words: none of it a conflict.
#[must_use]
pub fn merge_inbox(base: &Value, ours: &Value, theirs: &Value) -> (Value, Vec<String>) {
    let (base_items, our_items, their_items) = (items(base), items(ours), items(theirs));
    let mut merged: Vec<Map<String, Value>> = Vec::new();
    let mut moved: Vec<Map<String, Value>> = Vec::new();
    let mut notes = Vec::new();
    for item in &our_items {
        let key = id(item);
        match find(&their_items, &key) {
            Some(their) if same_item(item, their) => merged.push(combined(item, their)),
            Some(their) => {
                merged.push(item.clone());
                moved.push(their.clone());
            }
            // Dropped on theirs: stays dropped unless ours changed it meanwhile
            None => {
                if find(&base_items, &key) != Some(item) {
                    merged.push(item.clone());
                }
            }
        }
    }
    for item in &their_items {
        let key = id(item);
        if find(&our_items, &key).is_some() {
            continue;
        }
        if find(&base_items, &key) != Some(item) {
            merged.push(item.clone());
        }
    }
    // The numbers given again: each its next free one, past everything both branches and the base hold
    let mut next = [base, ours, theirs]
        .iter()
        .map(|d| next_number(d))
        .max()
        .unwrap_or(1);
    for mut item in moved {
        let old = text(&item, "id");
        let new = item_id(next);
        next += 1;
        notes.push(format!(
            "{old} was added on both branches: theirs is now {new}"
        ));
        item.insert("renumbered_from".into(), Value::String(old));
        item.insert("id".into(), Value::String(new));
        merged.push(item);
    }
    merged.sort_by_key(|i| item_number(&id(i)).unwrap_or(u64::MAX));
    let mut doc = json!({"version": INBOX_VERSION, "items": merged});
    doc["next"] = json!(next_number(&doc).max(next));
    (doc, notes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, state: &str, updated: &str) -> Value {
        json!({
            "id": id, "text": format!("{id} text"), "state": state,
            "created_at": "2026-11-24T09:00:00Z", "created_by": "Hakan", "updated_at": updated,
        })
    }
    fn doc(items: &[Value]) -> Value {
        json!({"version": 1, "next": 1, "items": items})
    }
    fn ids(doc: &Value) -> Vec<String> {
        items(doc).iter().map(|i| text(i, "id")).collect()
    }

    #[test]
    fn items_added_on_either_branch_are_both_kept_and_next_is_past_them() {
        let base = doc(&[item("IN-1", "created", "a")]);
        let ours = doc(&[item("IN-1", "created", "a"), item("IN-2", "created", "a")]);
        let theirs = doc(&[item("IN-1", "created", "a")]);
        let (merged, notes) = merge_inbox(&base, &ours, &theirs);
        assert_eq!(ids(&merged), ["IN-1", "IN-2"]);
        assert_eq!(merged["next"], 3);
        assert_eq!(notes, Vec::<String>::new());
    }

    #[test]
    fn one_number_for_two_items_ours_keeps_it_and_theirs_takes_the_next_free_one() {
        let base = doc(&[]);
        let ours = doc(&[item("IN-1", "created", "a")]);
        let mut other = item("IN-1", "created", "a");
        other["created_by"] = json!("Ana");
        let theirs = doc(&[other]);
        let (merged, notes) = merge_inbox(&base, &ours, &theirs);
        assert_eq!(ids(&merged), ["IN-1", "IN-2"]);
        assert_eq!(merged["items"][1]["created_by"], "Ana");
        assert_eq!(merged["items"][1]["renumbered_from"], "IN-1");
        assert!(
            merged["items"][0].get("renumbered_from").is_none(),
            "ours kept its number"
        );
        assert_eq!(merged["next"], 3);
        assert_eq!(
            notes,
            ["IN-1 was added on both branches: theirs is now IN-2"]
        );
    }

    #[test]
    fn one_item_changed_on_both_the_later_change_wins_and_an_injection_is_final() {
        let base = doc(&[item("IN-1", "created", "a")]);
        let (merged, _) = merge_inbox(
            &base,
            &doc(&[item("IN-1", "discarded", "b")]),
            &doc(&[item("IN-1", "created", "c")]),
        );
        assert_eq!(merged["items"][0]["state"], "created", "changed later");
        let mut injected = item("IN-1", "injected", "a");
        injected["tasks"] = json!(["ATM-1"]);
        let (merged, _) = merge_inbox(
            &base,
            &doc(&[item("IN-1", "discarded", "z")]),
            &doc(&[injected]),
        );
        assert_eq!(
            merged["items"][0]["state"], "injected",
            "final, whatever came later"
        );
    }

    #[test]
    fn injected_on_both_keeps_every_task_it_was_made_into() {
        let mut ours = item("IN-1", "injected", "a");
        ours["tasks"] = json!(["ATM-1", "ATM-2"]);
        ours["decided_at"] = json!("2026-11-24T10:00:00Z");
        let mut theirs = item("IN-1", "injected", "a");
        theirs["tasks"] = json!(["ATM-3", "ATM-1"]);
        theirs["decided_at"] = json!("2026-11-24T11:00:00Z");
        let (merged, _) = merge_inbox(&doc(&[]), &doc(&[ours]), &doc(&[theirs]));
        assert_eq!(
            merged["items"][0]["tasks"],
            json!(["ATM-1", "ATM-2", "ATM-3"])
        );
        assert_eq!(merged["items"][0]["decided_at"], "2026-11-24T11:00:00Z");
    }

    #[test]
    fn an_item_dropped_on_one_branch_stays_dropped_unless_the_other_changed_it() {
        let base = doc(&[
            item("IN-1", "discarded", "a"),
            item("IN-2", "discarded", "a"),
        ]);
        // Theirs archived both; ours restored IN-2 meanwhile
        let ours = doc(&[item("IN-1", "discarded", "a"), item("IN-2", "created", "b")]);
        let theirs = doc(&[]);
        let (merged, _) = merge_inbox(&base, &ours, &theirs);
        assert_eq!(ids(&merged), ["IN-2"]);
        // The other way around
        let (merged, _) = merge_inbox(&base, &theirs, &ours);
        assert_eq!(ids(&merged), ["IN-2"]);
    }

    #[test]
    fn the_same_change_on_both_is_one_change_and_ids_compare_in_any_case() {
        let base = doc(&[item("IN-1", "created", "a")]);
        let mut lower = item("in-1", "discarded", "b");
        lower["id"] = json!("in-1");
        let (merged, notes) = merge_inbox(
            &base,
            &doc(&[item("IN-1", "discarded", "b")]),
            &doc(&[lower]),
        );
        assert_eq!(merged["items"].as_array().unwrap().len(), 1);
        assert_eq!(notes, Vec::<String>::new());
    }
}
