use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::json;

use super::*;

struct Dir(PathBuf);
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A project with one task, claimed by `claude:a` until `until`
fn project(name: &str, until: &str) -> Dir {
    let root = std::env::temp_dir().join(format!("atm-lib-shown-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join(".agentic-task-manager")).unwrap();
    std::fs::write(
        root.join(".agentic-task-manager/tasks.json"),
        json!({"tasks": [{"id": "T-1", "name": "One", "status": "in_progress"}]}).to_string(),
    )
    .unwrap();
    claims(&root, "claude:a", until);
    Dir(root)
}

fn claims(root: &Path, agent: &str, until: &str) {
    std::fs::write(
        root.join(".agentic-task-manager/claims-store.json"),
        json!({"claims": {"T-1": {
            "task": "T-1", "agent": agent, "claimed_at": "2026-11-24T08:00:00Z",
            "lease_expires_at": until, "renewed_at": until,
        }}})
        .to_string(),
    )
    .unwrap();
}

/// An `OnChange` counting its calls, and the count
fn counted() -> (OnChange, Arc<AtomicUsize>) {
    let count = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&count);
    (
        Arc::new(move || {
            seen.fetch_add(1, Ordering::SeqCst);
        }),
        count,
    )
}

#[test]
fn the_claims_the_page_sees_have_no_lease_s_end_or_renewal() {
    let store = json!({
        "claims": {"T-1": {"task": "T-1", "agent": "a", "lease_expires_at": "x", "renewed_at": "y"}},
        "waiting": {"T-2": {"lease_expires_at": "kept: not a claim"}},
    });
    assert_eq!(
        shown_claims(store.as_object().unwrap()),
        json!({
            "claims": {"T-1": {"task": "T-1", "agent": "a"}},
            "waiting": {"T-2": {"lease_expires_at": "kept: not a claim"}},
        })
    );
    assert_eq!(shown_claims(&Map::new()), json!({}));
}

#[test]
fn a_lease_renewed_tells_nothing_a_new_holder_or_a_task_changed_tells_once_each() {
    let dir = project("renewal", "2026-11-24T10:00:00Z");
    let (on_change, count) = counted();
    let shown = on_shown_change(dir.0.clone(), on_change);
    // A renewal: the claims store rewritten, only the lease moved
    claims(&dir.0, "claude:a", "2026-11-24T10:05:00Z");
    shown();
    assert_eq!(count.load(Ordering::SeqCst), 0);
    // Told twice of the same change: once
    claims(&dir.0, "claude:b", "2026-11-24T10:05:00Z");
    shown();
    shown();
    assert_eq!(count.load(Ordering::SeqCst), 1);
    // The tasks changed
    std::fs::write(
        dir.0.join(".agentic-task-manager/tasks.json"),
        json!({"tasks": [{"id": "T-1", "name": "One", "status": "in_review"}, {"id": "T-2", "name": "Two"}]})
            .to_string(),
    )
    .unwrap();
    shown();
    assert_eq!(count.load(Ordering::SeqCst), 2);
}

#[test]
fn a_project_whose_files_appear_is_a_change() {
    let root = std::env::temp_dir().join(format!("atm-lib-shown-none-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (on_change, count) = counted();
    let shown = on_shown_change(root.clone(), on_change);
    let dir = project("none", "2026-11-24T10:00:00Z");
    std::fs::rename(&dir.0, &root).unwrap();
    shown();
    assert_eq!(count.load(Ordering::SeqCst), 1);
    let _ = std::fs::remove_dir_all(&root);
}
