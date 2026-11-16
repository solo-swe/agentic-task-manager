//! IDs and the names agents put into shell commands (docs/security-review.md F5): kept to characters a shell doesn't
//! treat specially, as the Python atm's `common/model.py` checks them.

use std::fmt;

/// A task's ID (`T-001`). Any text a caller asks for: [`valid_task_id`] says whether a task file may hold it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TaskId(String);

impl TaskId {
    /// The ID as given.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// The ID's text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// `[A-Za-z0-9][A-Za-z0-9._-]{0,63}`: a letter or digit, then up to 63 of letters, digits, `.`, `_` and `-`.
#[must_use]
pub fn valid_short_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 64
        && bytes[0].is_ascii_alphanumeric()
        && bytes
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

/// Whether a task file may hold this task ID.
#[must_use]
pub fn valid_task_id(value: &str) -> bool {
    valid_short_id(value)
}

/// The task file's format (ADR-007 §6): `meta.format`, absent in a file from before task keys (format 1).
pub const LEGACY_FORMAT: u64 = 1;

/// The newest task-file format this atm reads and writes: tasks stored by UUID, shown by key (ADR-007).
pub const KEYS_FORMAT: u64 = 2;

/// The atm version that first wrote [`KEYS_FORMAT`], for the message an older atm can't give (it can only say "newer").
pub const KEYS_FORMAT_SINCE: &str = "0.38.0";

/// A file's format: `meta.format`, or [`LEGACY_FORMAT`] when it has none. None when `meta.format` isn't a whole number
/// of at least 1.
#[must_use]
pub fn file_format(document: &serde_json::Value) -> Option<u64> {
    match document.get("meta").and_then(|m| m.get("format")) {
        None => Some(LEGACY_FORMAT),
        Some(v) => v.as_u64().filter(|n| *n >= 1),
    }
}

/// A new task's UUID: version 7 (its first bits the time, so they sort by creation), in its hyphenated lower-case form.
#[must_use]
pub fn new_task_uuid() -> String {
    uuid::Uuid::now_v7().hyphenated().to_string()
}

/// Whether a value is a task's UUID as atm writes it: hyphenated, lower case (`0193…-…`), any version.
#[must_use]
pub fn valid_task_uuid(value: &str) -> bool {
    value.len() == 36
        && uuid::Uuid::try_parse(value).is_ok_and(|u| u.hyphenated().to_string() == value)
}

/// A project's short name, its tasks' key prefix (`meta.key`, ADR-007 §2): 3 to 6 ASCII letters and digits.
#[must_use]
pub fn valid_project_key(value: &str) -> bool {
    (3..=6).contains(&value.len()) && value.bytes().all(|b| b.is_ascii_alphanumeric())
}

/// A task's key, as people read it: the project's short name and the task's number, `ATM-527` (no padding).
#[must_use]
pub fn task_key(prefix: &str, number: u64) -> String {
    format!("{prefix}-{number}")
}

/// A branch name atm accepts (`meta.workflow.base`): `[A-Za-z0-9][A-Za-z0-9._/-]{0,199}`, without `..`.
#[must_use]
pub fn valid_branch(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 200
        && bytes[0].is_ascii_alphanumeric()
        && bytes
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'/' | b'-'))
        && !value.contains("..")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_ids_are_letters_digits_dot_underscore_dash() {
        for ok in ["T-001", "a", "T.1_x", &"a".repeat(64)] {
            assert!(valid_task_id(ok), "{ok}");
        }
        for bad in ["", "-T", "T 1", "T;1", "é", &"a".repeat(65), "$(x)"] {
            assert!(!valid_task_id(bad), "{bad}");
        }
    }

    #[test]
    fn branches_may_have_slashes_but_no_double_dot() {
        assert!(valid_branch("dev/0.32.0"));
        assert!(!valid_branch("a..b"));
        assert!(!valid_branch("-x"));
        assert!(!valid_branch("a b"));
        assert!(!valid_branch(&"a".repeat(201)));
    }

    #[test]
    fn task_uuids_are_version_7_hyphenated_lower_case_and_sort_by_creation() {
        let first = new_task_uuid();
        let second = new_task_uuid();
        assert!(valid_task_uuid(&first) && valid_task_uuid(&second));
        assert_eq!(&first[14..15], "7", "version 7");
        assert!(first < second, "{first} sorts before {second}");
        let upper = first.to_uppercase();
        for bad in [
            upper.as_str(),
            &first.replace('-', ""),
            "T-1",
            "",
            &format!("{{{first}}}"),
        ] {
            assert!(!valid_task_uuid(bad), "{bad}");
        }
    }

    #[test]
    fn a_project_key_is_3_to_6_letters_and_digits() {
        for ok in ["ATM", "TaskMa", "lfb", "A1B"] {
            assert!(valid_project_key(ok), "{ok}");
        }
        for bad in ["AT", "TaskMan", "AT-M", "", "ÄTM", "A M"] {
            assert!(!valid_project_key(bad), "{bad}");
        }
        assert_eq!(task_key("ATM", 527), "ATM-527");
        assert_eq!(task_key("ATM", 1), "ATM-1", "no padding");
    }

    #[test]
    fn the_format_is_meta_format_or_1_without_one() {
        use serde_json::json;
        assert_eq!(file_format(&json!({"tasks": []})), Some(LEGACY_FORMAT));
        assert_eq!(
            file_format(&json!({"meta": {"format": 2}})),
            Some(KEYS_FORMAT)
        );
        for bad in [json!(0), json!("2"), json!(1.5), json!(-1)] {
            assert_eq!(
                file_format(&json!({"meta": {"format": bad}})),
                None,
                "{bad}"
            );
        }
    }

    #[test]
    fn a_task_id_shows_as_its_text() {
        assert_eq!(TaskId::new("T-1").to_string(), "T-1");
        assert_eq!(TaskId::new("T-1").as_str(), "T-1");
    }
}
