//! Which task each key a review stores means (ATM-630). A review names its tasks by the keys they had when it was
//! written. When a merge moves a task's number (ADR-009 §3), the old key is that task's alias, and another task holds
//! it now, the one that kept it. That key is ambiguous: it names the task holding it now (ADR-007 §5), but the review
//! may be about the one that moved. The review's own commit tells them apart. `tasks.json` at the commit its first
//! round was made at (a range's head, a working round's base), or else at the commit that added its `review.md`, says
//! which task held that number then. Nothing is rewritten, and a key that isn't ambiguous costs no git read.

use std::path::Path;

use atm_common::keys::{self, Via};
use atm_common::{Result, TASK_DIR, Task};
use atm_review::folder::FileStore;
use atm_review::{ReviewHeader, RoundKind};
use serde_json::Value;

use crate::project::Project;

/// The task (its ID) the review `header` means by `key`, read from `reviews` (the store of the working copy it was
/// found in; none to resolve the key as every command does).
///
/// # Errors
/// The project has no task by that key, or its tasks can't be read.
pub fn task_of(
    project: &Project,
    reviews: Option<&FileStore>,
    header: &ReviewHeader,
    key: &str,
) -> Result<String> {
    let id = project.resolve_task(key)?;
    let Some(reviews) = reviews else {
        return Ok(id);
    };
    let document = project.store().document()?;
    let archive: Vec<Task> = project
        .store()
        .list_archived()?
        .into_iter()
        .cloned()
        .collect();
    // Only a key some task holds now can be ambiguous: an alias or a UUID names one task
    if !matches!(
        keys::resolve(document, &archive, key).map(|f| f.via),
        Ok(Via::Key | Via::EarlierPrefix)
    ) {
        return Ok(id);
    }
    let moved: Vec<String> = keys::alias_holders(document, &archive, key)
        .iter()
        .filter_map(|t| t.get("id").and_then(Value::as_str))
        .filter(|other| *other != id)
        .map(str::to_string)
        .collect();
    if moved.is_empty() {
        return Ok(id);
    }
    Ok(match holder_then(reviews, header, key) {
        Some(then) if then == id || moved.contains(&then) => then,
        _ => id,
    })
}

/// The task (its ID) that held `key` when the review was made: in `tasks.json` at its first round's commit, then at
/// the commit that added its `review.md`; none when neither can be read (a commit gone after a squash, a review
/// never committed, no git).
fn holder_then(reviews: &FileStore, header: &ReviewHeader, key: &str) -> Option<String> {
    let working_copy = reviews.folder().parent()?.parent()?;
    let vcs = atm_vcs::GitVcs::new(working_copy);
    let mut commits = Vec::new();
    // A submodule's round is of the submodule's commits, which have no tasks.json of the project's
    if let Some(round) = reviews
        .first_round(header)
        .filter(|r| r.submodule.is_none())
    {
        match round.kind {
            RoundKind::Range { head, .. } => commits.push(head),
            RoundKind::Working { base } => commits.push(base),
            RoundKind::Files => {}
        }
    }
    if let Some(review_md) = relative(working_copy, &reviews.folder_of(header).join("review.md"))
        && let Some(added) = vcs.first_commit_adding(&review_md)
    {
        commits.push(added);
    }
    let tasks = format!("{TASK_DIR}/tasks.json");
    // A round's commit is what its file says, and anyone who can change the review wrote it: only a full SHA is read,
    // never text git could take for an option or a revision expression
    let sha = |c: &&String| matches!(c.len(), 40 | 64) && c.bytes().all(|b| b.is_ascii_hexdigit());
    commits.iter().filter(sha).find_map(|commit| {
        let (_, blob) = vcs.blob_in(commit, &tasks)?;
        let document: Value = serde_json::from_slice(&vcs.blob_bytes(&blob).ok()?).ok()?;
        let found = keys::resolve(&document, &[], key).ok()?;
        found.task.get("id")?.as_str().map(str::to_string)
    })
}

/// `path` from `root`, with `/` between its parts, as git names files; none outside `root`.
fn relative(root: &Path, path: &Path) -> Option<String> {
    let parts: Vec<String> = path
        .strip_prefix(root)
        .ok()?
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    (!parts.is_empty()).then(|| parts.join("/"))
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use atm_review::{Actor, NewReview, NewRound, Patch, ReviewFilter, ReviewStore, Status};
    use serde_json::json;

    use super::*;

    const A: &str = "0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8001";
    const B: &str = "0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8002";

    struct Dir(PathBuf);
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn dir() -> Dir {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let count = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "atm-lib-review-keys-{}-{count}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(path.join(TASK_DIR)).unwrap();
        Dir(std::fs::canonicalize(&path).unwrap())
    }
    fn git(args: &[&str], cwd: &Path) {
        let mut all = vec!["-c", "user.name=t", "-c", "user.email=t@example.com"];
        all.extend_from_slice(args);
        assert!(atm_db::git::git(&all, cwd).is_some(), "git {args:?}");
    }
    fn write_tasks(root: &Path, tasks: &Value) {
        let document = json!({ "meta": { "project": "Keys", "format": 2, "key": "ATM", "next_number": 14 }, "tasks": tasks });
        std::fs::write(root.join(TASK_DIR).join("tasks.json"), document.to_string()).unwrap();
    }
    fn task(id: &str, number: u64, aliases: &[&str]) -> Value {
        json!({ "id": id, "number": number, "aliases": aliases, "name": id, "type": "feature", "status": "todo" })
    }
    /// A review of files (no commit of its own) naming ATM-12
    fn review_of_files(store: &mut FileStore) -> ReviewHeader {
        review(store, RoundKind::Files)
    }
    /// A review naming ATM-12, its first round `kind`
    fn review(store: &mut FileStore, kind: RoundKind) -> ReviewHeader {
        store
            .create_review(NewReview {
                title: "Files".into(),
                summary: String::new(),
                tasks: vec!["ATM-12".into()],
                author: Some(Actor("claude:a".into())),
                reviewers: Vec::new(),
                status: Status::Waiting,
                first: NewRound {
                    kind,
                    submodule: None,
                    files: Vec::new(),
                    patch: Patch::default(),
                    by: Actor("claude:a".into()),
                },
            })
            .unwrap()
    }

    /// A clash moved A off 12 (ATM-13 now, ATM-12 its alias); B holds 12. A review committed while A held 12 means A
    /// by ATM-12, read at the commit that added its review.md; one never committed can't tell, and keeps today's rule
    #[test]
    fn an_old_key_another_task_holds_now_is_the_task_that_held_it_at_the_reviews_commit() {
        let dir = dir();
        let root = &dir.0;
        git(&["init", "-q", "-b", "main"], root);
        write_tasks(root, &json!([task(A, 12, &[])]));
        let mut store = FileStore::open(&root.join(TASK_DIR)).unwrap();
        let committed = review_of_files(&mut store);
        git(&["add", "-A"], root);
        git(&["commit", "-q", "-m", "A and its review"], root);
        // The merge: A moved to 13, B holds 12 (uncommitted, as a merge leaves it before its commit)
        write_tasks(root, &json!([task(A, 13, &["ATM-12"]), task(B, 12, &[])]));
        let never_committed = review_of_files(&mut store);
        let project = Project::open(root).unwrap();
        let of = |header: &ReviewHeader, key: &str| {
            task_of(&project, Some(&store), header, key).unwrap()
        };
        assert_eq!(of(&committed, "ATM-12"), A, "the task that held 12 then");
        assert_eq!(of(&committed, "atm-12"), A, "any case");
        assert_eq!(of(&committed, "~ATM-12"), A, "an old review's mark");
        assert_eq!(
            of(&never_committed, "ATM-12"),
            B,
            "nothing to read: the holder now"
        );
        // Without the review's folder, as every command resolves it
        assert_eq!(task_of(&project, None, &committed, "ATM-12").unwrap(), B);
        // Not ambiguous: no git read needed, the same answer either way
        assert_eq!(of(&committed, "ATM-13"), A);
        assert_eq!(of(&committed, B), B);
        assert!(task_of(&project, Some(&store), &committed, "ATM-99").is_err());
        assert_eq!(
            store.list_reviews(&ReviewFilter::default()).unwrap().len(),
            2,
            "nothing rewritten"
        );
    }

    /// A round's commit is read only when its file names a full SHA: `HEAD` (or an option) written there is never given
    /// to git, so a review not committed yet still can't tell, and keeps today's rule
    #[test]
    fn a_rounds_commit_is_read_only_when_it_is_a_full_sha() {
        let dir = dir();
        let root = &dir.0;
        git(&["init", "-q", "-b", "main"], root);
        write_tasks(root, &json!([task(A, 12, &[])]));
        git(&["add", "-A"], root);
        git(&["commit", "-q", "-m", "A held 12"], root);
        write_tasks(root, &json!([task(A, 13, &["ATM-12"]), task(B, 12, &[])]));
        let mut store = FileStore::open(&root.join(TASK_DIR)).unwrap();
        let project = Project::open(root).unwrap();
        for head in ["HEAD", "main", "--output=x", "HEAD~0"] {
            let header = review(
                &mut store,
                RoundKind::Range {
                    base: head.into(),
                    head: head.into(),
                },
            );
            assert_eq!(
                task_of(&project, Some(&store), &header, "ATM-12").unwrap(),
                B,
                "{head}"
            );
        }
        // The same commit by its SHA is read: A held 12 there
        let sha = atm_db::git::git(&["rev-parse", "HEAD"], root).unwrap();
        let header = review(
            &mut store,
            RoundKind::Range {
                base: sha.trim().into(),
                head: sha.trim().into(),
            },
        );
        assert_eq!(task_of(&project, Some(&store), &header, "ATM-12").unwrap(), A);
    }

    /// At the review's commit a third task held the number (neither the holder now nor one that moved off it): the
    /// commit doesn't help, and the key means the holder now
    #[test]
    fn a_commit_naming_neither_task_leaves_the_holder_now() {
        let dir = dir();
        let root = &dir.0;
        let other = "0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8003";
        git(&["init", "-q", "-b", "main"], root);
        write_tasks(root, &json!([task(other, 12, &[])]));
        let mut store = FileStore::open(&root.join(TASK_DIR)).unwrap();
        let header = review_of_files(&mut store);
        git(&["add", "-A"], root);
        git(&["commit", "-q", "-m", "another task held 12"], root);
        write_tasks(root, &json!([task(A, 13, &["ATM-12"]), task(B, 12, &[])]));
        let project = Project::open(root).unwrap();
        assert_eq!(
            task_of(&project, Some(&store), &header, "ATM-12").unwrap(),
            B
        );
    }

    #[test]
    fn a_path_from_the_root_is_named_as_git_names_it() {
        let root = Path::new("/r");
        assert_eq!(
            relative(
                root,
                Path::new("/r/.agentic-task-manager/reviews/CR-1-abcd/review.md")
            )
            .as_deref(),
            Some(".agentic-task-manager/reviews/CR-1-abcd/review.md")
        );
        assert_eq!(relative(root, Path::new("/elsewhere/review.md")), None);
        assert_eq!(relative(root, root), None);
    }
}
