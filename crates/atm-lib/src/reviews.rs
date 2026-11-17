//! Code reviews and tasks (docs/code-review-design.md §3.1's link, §3.8, ATM-565): the one place the two meet. A
//! review names its tasks (by key), each task links its reviews (`code_reviews`, by UUID and reference), and a
//! reviewer's decision on a review is every named task's review decision, recorded as the reviewer (I23: the user's,
//! never an agent's). A live review's approval is a task's when the task's change hasn't moved since (§3.5).
//!
//! The front ends call these, never `atm-review`'s store directly, so the tasks always follow.

use atm_common::{Error, Kind, Result, TASK_DIR, Task, Vocabulary};
use serde_json::{Value, json};

pub use atm_review::folder::{FileStore, Found, list_everywhere};
pub use atm_review::rounds;
pub use atm_review::rules::Verdict;
pub use atm_review::{
    Actor, Anchor, Chain, Comment, CommentFilter, CommentId, NewReview, NewRound, Patch, Review,
    ReviewFilter, ReviewHeader, ReviewId, ReviewRef, ReviewStore, Reviews, RoundKind, Status,
};

use crate::changes::change_task_status;
use crate::project::Project;
use crate::state::{Command, Options};

/// The project's reviews, in its task folder.
///
/// # Errors
/// The project wasn't opened from a folder, or its reviews' folder is a symlink.
pub fn store(project: &Project) -> Result<FileStore> {
    let root = project
        .root()
        .ok_or_else(|| Error::failed("this project has no folder to keep reviews in"))?;
    FileStore::open(&root.join(TASK_DIR))
}

fn actor_of(project: &Project) -> Actor {
    Actor(project.actor().unwrap_or("someone").to_string())
}

fn id_of(task: &Value) -> Option<&str> {
    task.get("id").and_then(Value::as_str)
}

/// A task's kind and review state now, none when it is gone.
fn kind_of(project: &Project, id: &str) -> Result<Option<(Kind, Option<String>)>> {
    let vocab: &Vocabulary = project.store().vocabulary()?;
    Ok(project.store().get_task(id)?.map(|task: &Task| {
        (
            vocab.kind_of(task.get("status")).unwrap_or(Kind::Todo),
            task.get("review_state")
                .and_then(Value::as_str)
                .map(String::from),
        )
    }))
}

/// Add (or, with `add` false, remove) the link to `header`'s review on the tasks `ids`.
fn link(project: &mut Project, header: &ReviewHeader, ids: &[String], add: bool) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    let entry = json!({ "id": header.id.0, "ref": header.reference.0 });
    project.store_mut().change(&mut |work| {
        for task in work.tasks().iter_mut() {
            if !id_of(task).is_some_and(|id| ids.iter().any(|i| i == id)) {
                continue;
            }
            let Some(task) = task.as_object_mut() else {
                continue;
            };
            let mut links: Vec<Value> = task
                .get("code_reviews")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            links.retain(|l| l.get("id") != entry.get("id"));
            if add {
                links.push(entry.clone());
            }
            if links.is_empty() {
                task.shift_remove("code_reviews");
            } else {
                task.insert("code_reviews".into(), Value::Array(links));
            }
        }
        Ok(())
    })
}

/// The task IDs of `keys` (keys, old IDs or UUIDs).
fn resolve(project: &Project, keys: &[String]) -> Result<Vec<String>> {
    keys.iter().map(|k| project.resolve_task(k)).collect()
}

/// The task IDs a review names.
fn named(project: &Project, header: &ReviewHeader) -> Result<Vec<String>> {
    resolve(project, &header.tasks)
}

/// Put the in-progress tasks among `ids` in review, with the review's link in their note (§3.8).
fn ask_review(
    project: &mut Project,
    ids: &[String],
    header: &ReviewHeader,
    what: &str,
) -> Result<()> {
    for id in ids {
        if kind_of(project, id)?.is_some_and(|(kind, _)| kind == Kind::Active) {
            let options = Options {
                note: Some(format!("{}: {what}", header.reference)),
                ..Options::default()
            };
            change_task_status(project, Command::Review, id, &options)?;
        }
    }
    Ok(())
}

/// Open a review, link its tasks, and put those in progress in review (an open one; a draft or a live one leaves
/// them as they are).
///
/// # Errors
/// What the review's rules refuse (`atm_review::rules`), an unknown task, or a store error.
pub fn create(project: &mut Project, mut new: NewReview) -> Result<ReviewHeader> {
    let ids = resolve(project, &new.tasks)?;
    // The review names its tasks by key, as people read them
    new.tasks = ids
        .iter()
        .map(|id| {
            project
                .store()
                .get_task(id)
                .ok()
                .flatten()
                .map_or_else(|| id.clone(), |t| project.task_label(t))
        })
        .collect();
    let mut reviews = store(project)?;
    let header = Reviews::new(&mut reviews).create(new)?;
    link(project, &header, &ids, true)?;
    if header.status == Status::Open {
        ask_review(project, &ids, &header, &header.title)?;
    }
    Ok(header)
}

/// The author's next round: the review is open again, and its tasks in progress go back in review.
///
/// # Errors
/// What the review's rules refuse, or a store error.
pub fn add_round(project: &mut Project, reference: &ReviewRef, round: NewRound) -> Result<u32> {
    let mut reviews = store(project)?;
    let number = Reviews::new(&mut reviews).add_round(reference, round)?;
    let header = reviews.get_review(reference)?.header;
    let ids = named(project, &header)?;
    ask_review(project, &ids, &header, &format!("round {number}"))?;
    Ok(number)
}

/// A reviewer submits (§3.11's *Submit*); approving or sending back is every named task's decision, recorded as the
/// reviewer: approved, or changes requested with a note linking the review (`atm loop next` then hands the task back).
/// A task not in review (a live review's, still in progress) waits for [`apply_live_approval`].
///
/// # Errors
/// What the review's rules refuse, or a store error.
pub fn submit(
    project: &mut Project,
    reference: &ReviewRef,
    verdict: Verdict,
    body: Option<&str>,
) -> Result<()> {
    let by = actor_of(project);
    let mut reviews = store(project)?;
    Reviews::new(&mut reviews).submit(reference, verdict, body, &by)?;
    let review = reviews.get_review(reference)?;
    let command = match verdict {
        Verdict::Approve => Command::Approve,
        Verdict::RequestChanges => Command::RequestChanges,
        Verdict::Comment => return Ok(()),
    };
    let round = review.rounds.last().map_or(1, |r| r.number);
    let open = review.open_comments().count();
    let options = Options {
        note: Some(format!("in {} (round {round})", review.header.reference)),
        reason: Some(format!(
            "{} round {round}: {open} open comment{}",
            review.header.reference,
            if open == 1 { "" } else { "s" }
        )),
        ..Options::default()
    };
    for id in named(project, &review.header)? {
        if kind_of(project, &id)?.is_some_and(|(kind, _)| kind == Kind::Review) {
            change_task_status(project, command, &id, &options)?;
        }
    }
    Ok(())
}

/// End a review without approval; with `reject`, its tasks in review are rejected too (only when the user says so,
/// §3.8).
///
/// # Errors
/// What the review's rules refuse, or a store error.
pub fn close(
    project: &mut Project,
    reference: &ReviewRef,
    reason: &str,
    reject: bool,
) -> Result<()> {
    let by = actor_of(project);
    let mut reviews = store(project)?;
    Reviews::new(&mut reviews).close(reference, reason, &by)?;
    if reject {
        let header = reviews.get_review(reference)?.header;
        let options = Options {
            reason: Some(format!("{} closed: {reason}", header.reference)),
            ..Options::default()
        };
        for id in named(project, &header)? {
            if kind_of(project, &id)?.is_some_and(|(kind, _)| kind == Kind::Review) {
                change_task_status(project, Command::Reject, &id, &options)?;
            }
        }
    }
    Ok(())
}

/// Name more tasks, or fewer (§3.2 ③): added ones are linked (and, for an open review, those in progress go in
/// review); removed ones are unlinked, with a note saying so.
///
/// # Errors
/// What the review's rules refuse, an unknown task, or a store error.
pub fn set_tasks(
    project: &mut Project,
    reference: &ReviewRef,
    add: &[String],
    remove: &[String],
) -> Result<ReviewHeader> {
    let by = actor_of(project);
    let mut reviews = store(project)?;
    let header = reviews.get_review(reference)?.header;
    let (added, removed) = (resolve(project, add)?, resolve(project, remove)?);
    let label = |project: &Project, id: &str| {
        project
            .store()
            .get_task(id)
            .ok()
            .flatten()
            .map_or_else(|| id.to_string(), |t| project.task_label(t))
    };
    let removed_labels: Vec<String> = removed.iter().map(|id| label(project, id)).collect();
    let mut tasks: Vec<String> = header
        .tasks
        .iter()
        .filter(|t| !removed_labels.contains(t) && !remove.contains(t))
        .cloned()
        .collect();
    for id in &added {
        let key = label(project, id);
        if !tasks.contains(&key) {
            tasks.push(key);
        }
    }
    Reviews::new(&mut reviews).set_tasks(reference, tasks, &by)?;
    let header = reviews.get_review(reference)?.header;
    link(project, &header, &added, true)?;
    link(project, &header, &removed, false)?;
    for id in &removed {
        crate::changes::add_task_note(
            project,
            id,
            &format!("No longer reviewed in {}", header.reference),
        )?;
    }
    if header.status == Status::Open {
        ask_review(project, &added, &header, &header.title)?;
    }
    Ok(header)
}

/// Whether an approved live review covers a task's change now (§3.5): the review named the task, and the working
/// tree's diff against the review's base is the approved checkpoint's. The review, when one does.
///
/// # Errors
/// The reviews can't be read, or git fails.
pub fn live_approval_for(project: &Project, task: &str) -> Result<Option<ReviewHeader>> {
    let id = project.resolve_task(task)?;
    let key = project
        .store()
        .get_task(&id)?
        .map_or_else(|| id.clone(), |t| project.task_label(t));
    let Some(vcs) = project.vcs() else {
        return Ok(None);
    };
    let reviews = store(project)?;
    let filter = ReviewFilter {
        statuses: vec![Status::Approved],
        task: Some(key),
    };
    for header in reviews.list_reviews(&filter)? {
        let review = reviews.get_review(&header.reference)?;
        let Some(last) = review.rounds.last() else {
            continue;
        };
        let RoundKind::Working { base } = &last.kind else {
            continue;
        };
        let approved = reviews.round_diff(&header.id, last.number)?;
        let now = rounds::from_working(&vcs, base, &actor_of(project))?;
        if now.patch == approved {
            return Ok(Some(header));
        }
    }
    Ok(None)
}

/// A task in progress whose change an approved live review covers: put in review and approved, its note naming the
/// review, so it can be done with no other review (§3.5). None when no live review covers it as it is now.
///
/// # Errors
/// The reviews can't be read, git fails, or the task can't change status.
pub fn apply_live_approval(project: &mut Project, task: &str) -> Result<Option<ReviewHeader>> {
    let Some(header) = live_approval_for(project, task)? else {
        return Ok(None);
    };
    let id = project.resolve_task(task)?;
    if kind_of(project, &id)?.is_some_and(|(kind, _)| kind == Kind::Active) {
        let note = format!("approved in the live review {}", header.reference);
        let options = Options {
            note: Some(note),
            ..Options::default()
        };
        change_task_status(project, Command::Review, &id, &options)?;
        change_task_status(project, Command::Approve, &id, &options)?;
    }
    Ok(Some(header))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use serde_json::{Value, json};

    use super::{NewReview, NewRound, Patch, ReviewRef, ReviewStore, RoundKind, Status, Verdict};
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
        let path =
            std::env::temp_dir().join(format!("atm-lib-reviews-{}-{count}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Dir(std::fs::canonicalize(&path).unwrap())
    }
    fn git(args: &[&str], cwd: &Path) {
        let mut all = vec!["-c", "user.name=t", "-c", "user.email=t@example.com"];
        all.extend_from_slice(args);
        assert!(atm_db::git::git(&all, cwd).is_some(), "git {args:?}");
    }
    /// A project in git with tasks T-1 and T-2 (in progress, `blocker`), as `actor`.
    fn project(root: &Path, actor: &str) -> Project {
        let tasks = json!({
            "meta": { "project": "Reviews", "review": "blocker" },
            "tasks": [
                { "id": "T-1", "name": "One", "type": "feature", "status": "in_progress" },
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
    fn round() -> NewRound {
        NewRound {
            kind: RoundKind::Range {
                base: "a".into(),
                head: "b".into(),
            },
            files: Vec::new(),
            patch: Patch(String::new()),
            by: "claude:1".into(),
        }
    }
    fn task(project: &Project, id: &str) -> Value {
        Value::Object(project.store().get_task(id).unwrap().unwrap().clone())
    }

    #[test]
    fn a_review_of_two_tasks_puts_both_in_review_and_its_decisions_are_theirs() {
        let dir = dir();
        let mut project = project(&dir.0, "claude:1");
        let header = super::create(
            &mut project,
            NewReview {
                title: "Both".into(),
                summary: String::new(),
                tasks: vec!["T-1".into(), "T-2".into()],
                author: "claude:1".into(),
                reviewers: vec!["Hakan".into()],
                status: Status::Open,
                first: round(),
            },
        )
        .unwrap();
        let cr = header.reference.clone();
        for id in ["T-1", "T-2"] {
            let t = task(&project, id);
            assert_eq!(t["status"], "in_review", "{id}");
            assert_eq!(
                t["code_reviews"],
                json!([{ "id": header.id.0, "ref": "CR-1" }])
            );
            assert!(t["notes"].to_string().contains("CR-1: Both"));
        }

        // The reviewer comments and sends it back: both tasks have changes asked for, naming the review
        let mut project = reopen(&dir.0, "Hakan");
        let mut reviews = super::store(&project).unwrap();
        super::Reviews::new(&mut reviews)
            .comment(
                &cr,
                super::Anchor {
                    round: 1,
                    ..super::Anchor::default()
                },
                "Fix it.",
                &"Hakan".into(),
            )
            .unwrap();
        super::submit(&mut project, &cr, Verdict::RequestChanges, None).unwrap();
        let t1 = task(&project, "T-1");
        assert_eq!(t1["review_state"], "changes_requested");
        assert!(
            t1["notes"]
                .to_string()
                .contains("CR-1 round 1: 1 open comment"),
            "{}",
            t1["notes"]
        );

        // The agent takes it back, fixes it, adds a round: in review again
        let mut agent = reopen(&dir.0, "claude:1");
        crate::changes::change_task_status(
            &mut agent,
            crate::state::Command::Start,
            "T-1",
            &crate::state::Options::default(),
        )
        .unwrap();
        super::add_round(&mut agent, &cr, round()).unwrap();
        assert_eq!(task(&agent, "T-1")["review_state"], "waiting");

        // Approved once the comment is resolved: approved for the task in review
        let mut project = reopen(&dir.0, "Hakan");
        let mut reviews = super::store(&project).unwrap();
        let open = reviews
            .get_review(&cr)
            .unwrap()
            .open_comments()
            .next()
            .unwrap()
            .id
            .clone();
        super::Reviews::new(&mut reviews)
            .resolve(&cr, &open, true, &"Hakan".into())
            .unwrap();
        super::submit(&mut project, &cr, Verdict::Approve, None).unwrap();
        assert_eq!(task(&project, "T-1")["review_state"], "approved");
        // T-2 stayed in review with changes asked for: approved now too
        assert_eq!(task(&project, "T-2")["review_state"], "approved");
    }

    #[test]
    fn tasks_named_later_are_linked_and_removed_ones_unlinked_with_a_note() {
        let dir = dir();
        let mut project = project(&dir.0, "claude:1");
        let header = super::create(
            &mut project,
            NewReview {
                title: "Draft".into(),
                summary: String::new(),
                tasks: vec!["T-1".into()],
                author: "claude:1".into(),
                reviewers: Vec::new(),
                status: Status::Draft,
                first: round(),
            },
        )
        .unwrap();
        // A draft puts nothing in review
        assert_eq!(task(&project, "T-1")["status"], "in_progress");
        let header2 = super::set_tasks(
            &mut project,
            &header.reference,
            &["T-2".into()],
            &["T-1".into()],
        )
        .unwrap();
        assert_eq!(header2.tasks, ["T-2"]);
        assert!(task(&project, "T-1").get("code_reviews").is_none());
        assert!(
            task(&project, "T-1")["notes"]
                .to_string()
                .contains("No longer reviewed in CR-1")
        );
        assert_eq!(task(&project, "T-2")["code_reviews"][0]["ref"], "CR-1");
    }

    #[test]
    fn an_approved_live_review_covers_a_task_until_its_change_moves() {
        let dir = dir();
        git(&["init", "-q"], &dir.0);
        std::fs::write(dir.0.join("a.txt"), "one\n").unwrap();
        let mut project = project(&dir.0, "claude:1");
        git(&["add", "-A"], &dir.0);
        git(&["commit", "-q", "-m", "base"], &dir.0);
        let vcs = project.vcs().unwrap();
        std::fs::write(dir.0.join("a.txt"), "one\ntwo\n").unwrap();
        let checkpoint = super::rounds::from_working(&vcs, "HEAD", &"claude:1".into()).unwrap();
        let header = super::create(
            &mut project,
            NewReview {
                title: "Live".into(),
                summary: String::new(),
                tasks: vec!["T-1".into()],
                author: "claude:1".into(),
                reviewers: vec!["Hakan".into()],
                status: Status::Live,
                first: checkpoint,
            },
        )
        .unwrap();
        // Live: the task stays in progress
        assert_eq!(task(&project, "T-1")["status"], "in_progress");
        let mut hakan = reopen(&dir.0, "Hakan");
        super::submit(
            &mut hakan,
            &ReviewRef(header.reference.0.clone()),
            Verdict::Approve,
            None,
        )
        .unwrap();

        // Changed since: not covered
        std::fs::write(dir.0.join("a.txt"), "one\ntwo\nthree\n").unwrap();
        let mut agent = reopen(&dir.0, "claude:1");
        assert_eq!(super::apply_live_approval(&mut agent, "T-1").unwrap(), None);
        // As approved: covered, the task approved naming the review
        std::fs::write(dir.0.join("a.txt"), "one\ntwo\n").unwrap();
        assert_eq!(
            super::apply_live_approval(&mut agent, "T-1")
                .unwrap()
                .unwrap()
                .reference
                .0,
            "CR-1"
        );
        let t1 = task(&agent, "T-1");
        assert_eq!(t1["review_state"], "approved");
        assert!(
            t1["notes"]
                .to_string()
                .contains("approved in the live review CR-1")
        );
        crate::changes::change_task_status(
            &mut agent,
            crate::state::Command::Done,
            "T-1",
            &crate::state::Options::default(),
        )
        .unwrap();
    }
}
