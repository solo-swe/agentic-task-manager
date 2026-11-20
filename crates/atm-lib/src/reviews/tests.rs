//! Unit tests of the reviews' private parts; the rest, through the real store and git, are the integration tests
//! in `tests/reviews/` (ATM-711).

use std::path::Path;

use serde_json::{Value, json};

use crate::project::Project;

struct Dir(std::path::PathBuf);
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn dir() -> Dir {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let count = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("atm-lib-reviews-{}-{count}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    // Without Windows' `\\?\` prefix, as atm and git name the folder
    Dir(atm_common::pathtext::canonical(&path).unwrap())
}
/// A project in git with tasks T-1 and T-2 (in progress, `blocker`), as `actor`.
fn project(root: &Path, actor: &str) -> Project {
    let tasks = json!({
        "meta": { "project": "Reviews", "review": "blocker" },
        "tasks": [
            { "id": "T-1", "name": "One", "type": "feature", "status": "in_progress", "started_by": "claude:1" },
            { "id": "T-2", "name": "Two", "type": "feature", "status": "in_progress" },
        ]
    });
    std::fs::create_dir_all(root.join(".agentic-task-manager")).unwrap();
    std::fs::write(
        root.join(".agentic-task-manager/tasks.json"),
        tasks.to_string(),
    )
    .unwrap();
    Project::open(root).unwrap().with_actor(Some(actor.into()))
}
fn reopen(root: &Path, actor: &str) -> Project {
    Project::open(root).unwrap().with_actor(Some(actor.into()))
}

/// ATM-670 (audit R7): a decision is checked against every task it changes before the review records it
#[test]
fn a_decision_is_checked_on_every_task_before_anything_is_written() {
    use crate::state::{Command, Options};
    let dir = dir();
    project(&dir.0, "Hakan");
    let file = dir.0.join(".agentic-task-manager/tasks.json");
    let mut tasks: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    tasks["tasks"][1]["status"] = json!("in_review");
    tasks["tasks"][1]["review_state"] = json!("waiting");
    std::fs::write(&file, tasks.to_string()).unwrap();
    let before = std::fs::read_to_string(&file).unwrap();
    let p = reopen(&dir.0, "Hakan");
    let ids = ["T-1".to_string(), "T-2".to_string()];
    // A request for changes without its reason: refused, naming the task in review it can't reach
    let refused =
        super::check_decision(&p, &ids, Command::RequestChanges, &Options::default()).unwrap_err();
    assert!(
        refused
            .to_string()
            .starts_with("the decision can't reach T-2, so it isn't recorded"),
        "{refused}"
    );
    // With it, fine; T-1, in progress, is left as it is
    let options = Options {
        reason: Some("CR-1 round 1: 1 open comment".into()),
        ..Options::default()
    };
    super::check_decision(&p, &ids, Command::RequestChanges, &options).unwrap();
    super::check_decision(&p, &ids, Command::Approve, &options).unwrap();
    // Checking writes nothing
    assert_eq!(std::fs::read_to_string(&file).unwrap(), before);
    // An unknown task is named
    let missing =
        super::check_decision(&p, &["T-9".to_string()], Command::Approve, &options).unwrap_err();
    assert_eq!(missing.to_string(), "no task T-9");
}
