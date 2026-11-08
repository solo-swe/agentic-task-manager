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
    fn a_task_id_shows_as_its_text() {
        assert_eq!(TaskId::new("T-1").to_string(), "T-1");
        assert_eq!(TaskId::new("T-1").as_str(), "T-1");
    }
}
