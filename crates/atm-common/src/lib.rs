//! The model (task, sprint, release, …), the vocabularies, errors and the JSON schema: what every layer shares. Depends
//! on nothing of atm.
//!
//! Part of atm's Rust port (T-415): docs/porting-to-rust.md, docs/layered-architecture.md, docs/guides/rust.md.

pub mod archive;
pub mod components;
pub mod dashboard;
pub mod edits;
pub mod error;
pub mod home;
pub mod ids;
pub mod inbox;
pub mod keys;
pub mod pathtext;
pub mod py;
pub mod repository;
pub mod rules;
pub mod secrets;
pub mod sha1;
pub mod sha256;
pub mod skills;
pub mod sprint_names;
pub mod subscription;
pub mod time;
pub mod vocabulary;

pub use error::{Error, Result};
pub use ids::TaskId;
pub use pathtext::Shown;
pub use vocabulary::{Kind, Vocabulary};

/// atm's version: the workspace's, its one source (the dashboard is stamped with it).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The project's task folder, at its root.
pub const TASK_DIR: &str = ".agentic-task-manager";
/// The task file, in the task folder.
pub const TASKS_FILE: &str = "tasks.json";
/// The archive of finished tasks, in the task folder.
pub const ARCHIVE_FILE: &str = "tasks-archive.json";

/// A task as the task file stores it: a JSON object, its fields in their order. atm reads the fields it knows and
/// keeps the others as they are.
pub type Task = serde_json::Map<String, serde_json::Value>;

/// Who started a task: `started_by`, or `agent` in files written before v4.
#[must_use]
pub fn started_by(task: &Task) -> Option<&serde_json::Value> {
    task.get("started_by")
        .filter(|v| py::truthy(v))
        .or_else(|| task.get("agent"))
}

/// A task's ID, when it has one as text (ATM-681: the one copy).
#[must_use]
pub fn task_id(task: &Task) -> Option<&str> {
    task.get("id").and_then(serde_json::Value::as_str)
}

/// Whether `label` is a label atm accepts: lowercase letters, digits, `-` and `_`, 1 to 32 characters.
#[must_use]
pub fn valid_label(label: &str) -> bool {
    (1..=32).contains(&label.len())
        && label
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn version_is_a_semantic_version() {
        let parts: Vec<&str> = VERSION.split('.').collect();
        assert_eq!(parts.len(), 3, "{VERSION}");
        assert!(parts.iter().all(|p| p.parse::<u32>().is_ok()), "{VERSION}");
    }

    #[test]
    fn started_by_falls_back_to_agent() {
        let task = |v: serde_json::Value| v.as_object().unwrap().clone();
        assert_eq!(
            started_by(&task(json!({"started_by": "a", "agent": "b"}))),
            Some(&json!("a"))
        );
        assert_eq!(
            started_by(&task(json!({"started_by": "", "agent": "b"}))),
            Some(&json!("b"))
        );
        assert_eq!(started_by(&task(json!({}))), None);
    }

    #[test]
    fn a_tasks_id_is_its_text_id_only() {
        let task = |v: serde_json::Value| v.as_object().unwrap().clone();
        assert_eq!(task_id(&task(json!({"id": "T-1"}))), Some("T-1"));
        assert_eq!(task_id(&task(json!({"id": 7}))), None);
        assert_eq!(task_id(&task(json!({}))), None);
    }

    #[test]
    fn a_label_is_lowercase_letters_digits_dash_and_underscore_up_to_32() {
        for (label, ok) in [
            ("ux", true),
            ("spike-2", true),
            ("a_b", true),
            (&"a".repeat(32), true),
            (&"a".repeat(33), false),
            ("", false),
            ("UX", false),
            ("a b", false),
            ("é", false),
        ] {
            assert_eq!(valid_label(label), ok, "{label:?}");
        }
    }
}
