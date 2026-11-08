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
pub mod py;
pub mod repository;
pub mod rules;
pub mod sha1;
pub mod sha256;
pub mod skills;
pub mod sprint_names;
pub mod time;
pub mod vocabulary;

pub use error::{Error, Result};
pub use ids::TaskId;
pub use vocabulary::{Kind, Vocabulary};

/// atm's version: the workspace's, which is the plugin's (`plugin/.claude-plugin/plugin.json`, checked by
/// `dashboard/scripts/check-versions.mjs`).
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
}
