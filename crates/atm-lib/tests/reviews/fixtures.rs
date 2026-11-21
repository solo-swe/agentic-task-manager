//! What the review tests share: a scratch folder, git, and projects of a few tasks.

use std::path::Path;

use serde_json::{Value, json};

use atm_lib::Project;
use atm_lib::reviews::{NewReview, NewRound, Patch, ReviewRef, ReviewStore, RoundKind, Status};

pub const LATER: &str = "2999-01-01T00:00:00Z";

pub const LAPSED: &str = "2000-01-01T00:00:00Z";

pub const PROVISIONAL: &str = "0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8012";

pub struct Dir(pub std::path::PathBuf);
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub fn dir() -> Dir {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let count = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("atm-lib-reviews-{}-{count}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    // Without Windows' `\\?\` prefix, as atm and git name the folder
    Dir(atm_common::pathtext::canonical(&path).unwrap())
}

pub fn git(args: &[&str], cwd: &Path) {
    let mut all = vec!["-c", "user.name=t", "-c", "user.email=t@example.com"];
    all.extend_from_slice(args);
    assert!(atm_db::git::git(&all, cwd).is_some(), "git {args:?}");
}

/// A project in git with tasks T-1 and T-2 (in progress, `blocker`), as `actor`.
pub fn project(root: &Path, actor: &str) -> Project {
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

pub fn reopen(root: &Path, actor: &str) -> Project {
    Project::open(root).unwrap().with_actor(Some(actor.into()))
}

pub fn round() -> NewRound {
    NewRound {
        // Commits as atm records them, full IDs (ATM-659)
        kind: RoundKind::Range {
            base: "a".repeat(40),
            head: "b".repeat(40),
        },
        submodule: None,
        files: Vec::new(),
        patch: Patch(String::new()),
        by: "claude:1".into(),
    }
}

pub fn task(project: &Project, id: &str) -> Value {
    Value::Object(project.store().get_task(id).unwrap().unwrap().clone())
}

/// A project of `tasks`, its claims `claims` (by task ID), as `actor`.
pub fn project_of(root: &Path, actor: &str, tasks: &Value, claims: &Value) -> Project {
    let folder = root.join(".agentic-task-manager");
    std::fs::create_dir_all(&folder).unwrap();
    let document = json!({ "meta": { "project": "Reviews", "review": "blocker" }, "tasks": tasks });
    std::fs::write(folder.join("tasks.json"), document.to_string()).unwrap();
    std::fs::write(
        folder.join("claims-store.json"),
        json!({ "claims": claims }).to_string(),
    )
    .unwrap();
    reopen(root, actor)
}

/// The claim on task `id` runs out (§3.3 of docs/task-review-state-machine.md): its lease is put in the past, as two
/// hours with no renewal leave it, and the next atm command applies the lapse (here `atm claim list`'s): the task back
/// in to do, with its "lease expired" note.
pub fn lapse(project: &mut Project, id: &str) {
    project
        .store_mut()
        .change(&mut |work| {
            if let Some(claim) = work
                .claims_store
                .get_mut("claims")
                .and_then(|claims| claims.get_mut(id))
            {
                claim["lease_expires_at"] = LAPSED.into();
            }
            Ok(())
        })
        .unwrap();
    atm_lib::claims::expire_claims(project).unwrap();
}

/// Task `id`'s number moved to `to` at a merge (ADR-009): its old key kept as an alias, as the merge driver leaves it.
pub fn renumber(project: &mut Project, id: &str, to: u64) {
    let key = project.task_key(project.store().get_task(id).unwrap().unwrap());
    project
        .store_mut()
        .change(&mut |work| {
            for task in work.tasks() {
                if task.get("id").and_then(Value::as_str) == Some(id) {
                    task["number"] = to.into();
                    task["aliases"] = json!([key]);
                }
            }
            Ok(())
        })
        .unwrap();
}

/// Task `id` gone from the project's task file, as another branch's merge or a hand edit can leave it.
pub fn drop_task(project: &mut Project, id: &str) {
    project
        .store_mut()
        .change(&mut |work| {
            work.tasks()
                .retain(|t| t.get("id").and_then(Value::as_str) != Some(id));
            Ok(())
        })
        .unwrap();
}

pub fn claim(task: &str, agent: &str, until: &str) -> Value {
    json!({ "task": task, "agent": agent, "lease_expires_at": until, "claimed_at": "2026-11-19T10:00:00Z" })
}

pub fn waiting(
    project: &mut Project,
    tasks: &[&str],
) -> atm_common::Result<atm_lib::reviews::ReviewHeader> {
    atm_lib::reviews::create(
        project,
        NewReview {
            title: "The change".into(),
            summary: String::new(),
            tasks: tasks.iter().map(|t| (*t).to_string()).collect(),
            author: Some("claude:1".into()),
            reviewers: vec!["Hakan".into()],
            status: Status::Waiting,
            first: round(),
        },
    )
}

pub fn no_review(project: &Project) -> bool {
    atm_lib::reviews::store(project)
        .unwrap()
        .get_review(&ReviewRef("CR-1".into()))
        .is_err()
}

pub fn task_of(project: &Project, id: &str) -> Value {
    task(&reopen(project.root().unwrap(), "claude:1"), id)
}

pub fn open_review(project: &mut Project, tasks: &[&str]) -> atm_lib::reviews::ReviewHeader {
    atm_lib::reviews::create(
        project,
        NewReview {
            title: "Both".into(),
            summary: String::new(),
            tasks: tasks.iter().map(|t| (*t).to_string()).collect(),
            author: Some("claude:1".into()),
            reviewers: vec!["Hakan".into()],
            status: Status::Waiting,
            first: round(),
        },
    )
    .unwrap()
}

/// A live review opened by Hakan on the agent's change to `a.txt` (one line added), committed `one`
pub fn live(dir: &Path) -> (Project, ReviewRef) {
    git(&["init", "-q"], dir);
    std::fs::write(dir.join("a.txt"), "one\n").unwrap();
    let project = project(dir, "claude:1");
    git(&["add", "-A"], dir);
    git(&["commit", "-q", "-m", "base"], dir);
    std::fs::write(dir.join("a.txt"), "one\ntwo\n").unwrap();
    let first =
        atm_lib::reviews::rounds::from_working(&project.vcs().unwrap(), "HEAD", &"Hakan".into())
            .unwrap();
    let mut hakan = reopen(dir, "Hakan");
    let header = atm_lib::reviews::create(
        &mut hakan,
        NewReview {
            title: "Live".into(),
            summary: String::new(),
            tasks: vec!["T-1".into()],
            author: Some("Hakan".into()),
            reviewers: Vec::new(),
            status: Status::Live,
            first,
        },
    )
    .unwrap();
    (hakan, ReviewRef(header.reference.0))
}

/// A project in task keys' format: ATM-11 final, ATM-12 provisional, both in progress (`blocker`), as `actor`.
pub fn keyed(root: &Path, actor: &str) -> Project {
    let tasks = json!({
        "meta": { "project": "Keyed", "format": 2, "key": "ATM", "next_number": 13, "review": "blocker" },
        "tasks": [
            { "id": "0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8011", "number": 11, "key_final": true, "name": "Final",
              "type": "feature", "status": "in_progress", "started_by": "claude:1" },
            { "id": PROVISIONAL, "number": 12, "key_final": false, "name": "Provisional",
              "type": "feature", "status": "in_progress", "started_by": "claude:1" },
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
