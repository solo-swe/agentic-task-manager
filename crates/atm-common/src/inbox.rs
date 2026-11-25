//! The inbox's data (docs/inbox-design.md §2, ATM-759): a project's raw input, `IN-<n>` items in
//! `.agentic-task-manager/inbox.json`, each `created`, `discarded` (with a reason) or `injected` (with the tasks made
//! from it). What no store or rule needs: the files' names, the shape of an ID, the limits, the states. The rules
//! (who may move an item, from which state) are atm-lib's `inbox`; the files, atm-db's.

use serde_json::{Value, json};

use crate::{Error, Result};

/// The inbox's file, in the task folder.
pub const INBOX_FILE: &str = "inbox.json";
/// Its archive: the decided items moved out of it (docs/inbox-design.md §4).
pub const INBOX_ARCHIVE_FILE: &str = "inbox-archive.json";
/// The file format's version.
pub const INBOX_VERSION: u64 = 1;
/// An item's text, at most (characters).
pub const MAX_TEXT: usize = 20_000;
/// A discard's reason, at most (characters), as a note is.
pub const MAX_REASON: usize = 500;
/// An item's ID prefix: `IN-12`.
const PREFIX: &str = "IN-";

/// Where an item is (docs/inbox-design.md §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InboxState {
    /// Waiting to be looked at.
    Created,
    /// Not to be done, with a reason.
    Discarded,
    /// Turned into tasks, which it links: final.
    Injected,
}

impl InboxState {
    /// Its name in the file and the commands.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Discarded => "discarded",
            Self::Injected => "injected",
        }
    }

    /// The state named `name`, or none.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "created" => Some(Self::Created),
            "discarded" => Some(Self::Discarded),
            "injected" => Some(Self::Injected),
            _ => None,
        }
    }

    /// Whether it is decided (discarded or injected): only decided items are archived.
    #[must_use]
    pub fn decided(self) -> bool {
        self != Self::Created
    }
}

/// An item's state, read from it; an unknown one is none.
#[must_use]
pub fn state_of(item: &Value) -> Option<InboxState> {
    item.get("state")
        .and_then(Value::as_str)
        .and_then(InboxState::parse)
}

/// The ID of item number `n`: `IN-12`.
#[must_use]
pub fn item_id(n: u64) -> String {
    format!("{PREFIX}{n}")
}

/// The number an item ID names (`IN-12`, `in-12`), or none for anything else (`IN-0`, `IN-x`, `T-12`).
#[must_use]
pub fn item_number(id: &str) -> Option<u64> {
    let rest = id
        .get(..PREFIX.len())?
        .eq_ignore_ascii_case(PREFIX)
        .then(|| &id[PREFIX.len()..])?;
    if rest.is_empty() || !rest.bytes().all(|b| b.is_ascii_digit()) || rest.starts_with('0') {
        return None;
    }
    rest.parse().ok()
}

/// An inbox with nothing in it, as a file that isn't there reads.
#[must_use]
pub fn empty_inbox() -> Value {
    json!({"version": INBOX_VERSION, "next": 1, "items": []})
}

/// The inbox document `value` is, or why not: an object with an `items` list, each item an object with an `IN-<n>`
/// `id` and a known `state`, no ID twice, and a `version` atm knows (or none, the first).
///
/// # Errors
/// `Invalid`, naming what is wrong.
pub fn check_inbox(value: &Value) -> Result<()> {
    let bad = |what: &str| Err(Error::Invalid(format!("the inbox: {what}")));
    let Some(doc) = value.as_object() else {
        return bad("not an object");
    };
    if let Some(version) = doc.get("version")
        && version.as_u64() != Some(INBOX_VERSION)
    {
        return bad(&format!("version {version}, which this atm doesn't know"));
    }
    let Some(items) = doc.get("items").and_then(Value::as_array) else {
        return bad("no list of items");
    };
    let mut seen = std::collections::HashSet::new();
    for item in items {
        let Some(id) = item.get("id").and_then(Value::as_str) else {
            return bad("an item without an ID");
        };
        if item_number(id).is_none() {
            return bad(&format!("{id:?} is no item ID (IN-<n>)"));
        }
        if !seen.insert(id.to_ascii_uppercase()) {
            return bad(&format!("{id} twice"));
        }
        if state_of(item).is_none() {
            return bad(&format!(
                "{id}'s state isn't created, discarded or injected"
            ));
        }
    }
    Ok(())
}

/// The next free number: the file's `next`, or past every item's (an inbox merged or edited by hand).
#[must_use]
pub fn next_number(doc: &Value) -> u64 {
    let stored = doc.get("next").and_then(Value::as_u64).unwrap_or(1);
    let past = doc
        .get("items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|i| i.get("id").and_then(Value::as_str).and_then(item_number))
        .max()
        .map_or(1, |n| n + 1);
    stored.max(past).max(1)
}

/// An item's text, trimmed, or why it can't be one: empty, or longer than [`MAX_TEXT`] characters.
///
/// # Errors
/// `Invalid`.
pub fn checked_text(text: &str) -> Result<String> {
    let text = text.trim();
    if text.is_empty() {
        return Err(Error::Invalid("an inbox item needs some text".into()));
    }
    let length = text.chars().count();
    if length > MAX_TEXT {
        return Err(Error::Invalid(format!(
            "an inbox item's text is at most {MAX_TEXT} characters ({length} given)"
        )));
    }
    Ok(text.to_string())
}

/// A discard's reason, trimmed, or why it can't be one: empty, or longer than [`MAX_REASON`] characters.
///
/// # Errors
/// `Invalid`.
pub fn checked_reason(reason: &str) -> Result<String> {
    let reason = reason.trim();
    if reason.is_empty() {
        return Err(Error::Invalid("a discard needs a reason".into()));
    }
    let length = reason.chars().count();
    if length > MAX_REASON {
        return Err(Error::Invalid(format!(
            "a discard's reason is at most {MAX_REASON} characters ({length} given)"
        )));
    }
    Ok(reason.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_item_id_is_in_and_a_number_from_one_in_any_case() {
        assert_eq!(item_id(12), "IN-12");
        for (id, n) in [("IN-12", Some(12)), ("in-3", Some(3)), ("In-1", Some(1))] {
            assert_eq!(item_number(id), n, "{id}");
        }
        for id in [
            "IN-0", "IN-012", "IN-", "IN-x", "IN-1a", "T-12", "IN12", "", "I", "IN--1", "ÍN-1",
        ] {
            assert_eq!(item_number(id), None, "{id}");
        }
    }

    #[test]
    fn states_by_name_and_which_are_decided() {
        for state in [
            InboxState::Created,
            InboxState::Discarded,
            InboxState::Injected,
        ] {
            assert_eq!(InboxState::parse(state.as_str()), Some(state));
        }
        assert_eq!(InboxState::parse("done"), None);
        assert!(!InboxState::Created.decided());
        assert!(InboxState::Discarded.decided() && InboxState::Injected.decided());
        assert_eq!(
            state_of(&json!({"state": "injected"})),
            Some(InboxState::Injected)
        );
        assert_eq!(state_of(&json!({"state": "x"})), None);
        assert_eq!(state_of(&json!({})), None);
    }

    #[test]
    fn an_inbox_document_is_checked() {
        assert!(check_inbox(&empty_inbox()).is_ok());
        assert!(check_inbox(&json!({"items": [{"id": "IN-1", "state": "created"}]})).is_ok());
        for (doc, says) in [
            (json!([]), "not an object"),
            (json!({"version": 2, "items": []}), "version 2"),
            (json!({"next": 1}), "no list of items"),
            (json!({"items": [{"state": "created"}]}), "without an ID"),
            (
                json!({"items": [{"id": "T-1", "state": "created"}]}),
                "no item ID",
            ),
            (
                json!({"items": [{"id": "IN-1", "state": "created"}, {"id": "in-1", "state": "created"}]}),
                "twice",
            ),
            (json!({"items": [{"id": "IN-1", "state": "done"}]}), "state"),
        ] {
            let error = check_inbox(&doc).unwrap_err().to_string();
            assert!(error.contains(says), "{doc}: {error}");
        }
    }

    #[test]
    fn the_next_number_is_past_every_item_and_the_files_own() {
        assert_eq!(next_number(&empty_inbox()), 1);
        assert_eq!(
            next_number(&json!({"next": 7, "items": [{"id": "IN-3"}]})),
            7
        );
        assert_eq!(
            next_number(&json!({"next": 2, "items": [{"id": "IN-9"}]})),
            10
        );
        assert_eq!(next_number(&json!({"items": []})), 1);
        assert_eq!(next_number(&json!({"next": 0})), 1);
    }

    #[test]
    fn text_and_reason_are_trimmed_not_empty_and_within_their_limits() {
        assert_eq!(checked_text("  an idea \n").unwrap(), "an idea");
        assert!(checked_text(" \n ").is_err());
        assert!(
            checked_text(&"é".repeat(MAX_TEXT)).is_ok(),
            "the limit counts characters"
        );
        assert!(
            checked_text(&"x".repeat(MAX_TEXT + 1))
                .unwrap_err()
                .to_string()
                .contains("at most 20000")
        );
        assert_eq!(checked_reason(" a duplicate ").unwrap(), "a duplicate");
        assert!(checked_reason("").is_err());
        assert!(checked_reason(&"x".repeat(MAX_REASON)).is_ok());
        assert!(checked_reason(&"x".repeat(MAX_REASON + 1)).is_err());
    }
}
