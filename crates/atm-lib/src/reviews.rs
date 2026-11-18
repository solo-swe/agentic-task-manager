//! Code reviews and tasks (docs/code-review-design.md §3.1's link, §3.8, ATM-565): the one place the two meet. A
//! review names its tasks (by key), each task links its reviews (`code_reviews`, by UUID and reference), and a
//! reviewer's decision on a review is every named task's review decision, recorded as the reviewer (I23: the user's,
//! never an agent's). A live review's approval is a task's when the task's change hasn't moved since (§3.5).
//!
//! The front ends call these, never `atm-review`'s store directly, so the tasks always follow.

use atm_common::edits::is_agent;
use atm_common::{Error, Kind, Result, TASK_DIR, Task, Vocabulary};
use serde_json::{Value, json};

pub use atm_review::files::comments_md;
pub use atm_review::folder::{FileStore, Found, list_everywhere};
pub use atm_review::rounds;
pub use atm_review::{
    Side,
    model::{FileStatus, Round},
};

/// The project's git, for a review's rounds.
///
/// # Errors
/// The project isn't in a git repository: reviews are of changes in git.
pub fn vcs(project: &Project) -> Result<atm_vcs::GitVcs> {
    project
        .vcs()
        .filter(atm_vcs::GitVcs::in_repository)
        .ok_or_else(|| {
            Error::failed("reviews are of changes in git: this project isn't in a git repository")
        })
}
/// The repository a round made in `cwd` is of (ATM-583): in a submodule, the submodule's (its revisions and
/// uncommitted changes are its own), its files named as the project has them; else the project's (`vcs`).
///
/// # Errors
/// `cwd` is in a submodule nested in another one: rounds are of the project's repository or a submodule of it.
pub fn repository_here(vcs: &atm_vcs::GitVcs, cwd: &std::path::Path) -> Result<atm_vcs::GitVcs> {
    match vcs.submodule_containing(cwd) {
        None => Ok(vcs.clone()),
        Some(folder) => vcs.submodule_at(&folder).ok_or_else(|| {
            Error::Invalid(format!(
                "{folder} is in a submodule inside another one: review from the outer submodule's folder, or the project's"
            ))
        }),
    }
}

/// The repository files reviewed whole are read from (ATM-583): a submodule's when every file is in it, else the
/// project's (`vcs`).
///
/// # Errors
/// `Invalid` when the files are in different repositories: each repository's are a review of their own.
pub fn repository_of_files(vcs: &atm_vcs::GitVcs, paths: &[String]) -> Result<atm_vcs::GitVcs> {
    let mut found: Option<Option<String>> = None;
    for path in paths {
        // The outermost folder on the way that is a submodule, if any
        let parts: Vec<&str> = path.split('/').collect();
        let folder = (1..parts.len())
            .map(|n| parts[..n].join("/"))
            .find(|f| vcs.submodule_at(f).is_some());
        match &found {
            None => found = Some(folder),
            Some(first) if *first == folder => {}
            Some(_) => {
                return Err(Error::Invalid(
                    "these files are in different repositories (the project's and a submodule's): review each repository's files in a review of their own".into(),
                ));
            }
        }
    }
    match found.flatten() {
        None => Ok(vcs.clone()),
        Some(folder) => vcs
            .submodule_at(&folder)
            .ok_or_else(|| Error::failed(format!("{folder}: not a submodule here"))),
    }
}

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

/// `atm doctor`'s checks of the reviews (docs/code-review-design.md §4): errors, a review atm can't read; warnings,
/// lines of `comments.md` the comment format doesn't expect, numbers two reviews share (CR-8.1 and CR-8.2), and tasks
/// linked to reviews the project doesn't have. A project without reviews has nothing to say.
#[must_use]
pub fn check(project: &Project, document: &Value) -> (Vec<String>, Vec<String>) {
    let (mut errors, mut warnings) = (Vec::new(), Vec::new());
    let Ok(store) = store(project) else {
        return (errors, warnings);
    };
    match store.problems() {
        Ok(problems) => {
            for problem in problems {
                if problem.contains("comments.md line ") {
                    warnings.push(format!("review {problem}"));
                } else {
                    errors.push(format!("review {problem}"));
                }
            }
        }
        Err(e) => errors.push(format!("reviews: {e}")),
    }
    let headers = store
        .list_reviews(&ReviewFilter::default())
        .unwrap_or_default();
    let mut shared: Vec<u32> = headers
        .iter()
        .filter(|h| h.reference.0.contains('.'))
        .map(|h| h.number)
        .collect();
    shared.dedup();
    for number in shared {
        let refs: Vec<&str> = headers
            .iter()
            .filter(|h| h.number == number)
            .map(|h| h.reference.0.as_str())
            .collect();
        warnings.push(format!(
            "reviews {} share the number {number} (made in two clones; both are kept, by those names)",
            refs.join(" and ")
        ));
    }
    // A decision that didn't reach a task still waiting on it (an atm before ATM-594 named a provisional key with its
    // `~`): the review's decision, the user's, is recorded on the task with the command named (I23: never by atm). A
    // task follows its latest review (§3.8): the reviews are listed newest first, and only a task's first counts
    let mut followed = std::collections::HashSet::new();
    for header in &headers {
        let latest: Vec<&String> = header
            .tasks
            .iter()
            .filter(|k| followed.insert((*k).clone()))
            .collect();
        let (state, command) = match header.status {
            Status::Approved => ("approved", "approve"),
            Status::ChangesRequested => ("changes_requested", "request-changes"),
            _ => continue,
        };
        for key in latest {
            let Ok(id) = project.resolve_task(key) else {
                warnings.push(format!(
                    "review {}: it names {key}, which isn't one of the project's tasks",
                    header.reference
                ));
                continue;
            };
            if let Ok(Some((Kind::Review, now))) = kind_of(project, &id)
                && now.as_deref() != Some(state)
                && now.as_deref() != Some("approved")
            {
                let reason = if command == "approve" {
                    String::new()
                } else {
                    " --reason '…'".to_string()
                };
                warnings.push(format!(
                    "{key}: {} is {} but the task still waits for its review: `atm task review {command} {key}{reason}` records the decision",
                    header.reference,
                    header.status.name().replace('_', " ")
                ));
            }
        }
    }
    let known: std::collections::HashSet<&str> = headers.iter().map(|h| h.id.0.as_str()).collect();
    for task in document["tasks"].as_array().into_iter().flatten() {
        for link in task
            .get("code_reviews")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let id = link.get("id").and_then(Value::as_str).unwrap_or_default();
            if !known.contains(id) {
                let name = link.get("ref").and_then(Value::as_str).unwrap_or(id);
                warnings.push(format!(
                    "{}: its code review {name} isn't in the project (not merged here yet, or deleted)",
                    id_of(task).unwrap_or("?")
                ));
            }
        }
    }
    (errors, warnings)
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
/// The agent holding a claim in this working copy (a live review's author, §3.5), if one does.
fn agent_at_work(project: &Project) -> Option<Actor> {
    let root = project.root()?;
    let here = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    crate::query::list_claims(project)
        .into_iter()
        .find_map(|claim| {
            let worktree = claim.get("worktree").and_then(Value::as_str)?;
            let there = std::fs::canonicalize(worktree).unwrap_or_else(|_| worktree.into());
            let agent = claim.get("agent").and_then(Value::as_str)?;
            (there == here && is_agent(Some(agent))).then(|| Actor(agent.to_string()))
        })
}

/// The agent that worked the first of these tasks (who started it), if an agent did.
fn agent_of_tasks(project: &Project, ids: &[String]) -> Option<Actor> {
    ids.iter().find_map(|id| {
        let task = project.store().get_task(id).ok().flatten()?;
        let agent = task.get("started_by").and_then(Value::as_str)?;
        is_agent(Some(agent)).then(|| Actor(agent.to_string()))
    })
}

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

/// The task a filter names (`--task`, `?task=`: a key with or without its `~`, an older ID, the UUID), as reviews name
/// it: its key (ATM-594). One the project doesn't have is kept as given, without a leading `~` (it fits no review).
#[must_use]
pub fn task_reference(project: &Project, reference: &str) -> String {
    project.resolve_task(reference).map_or_else(
        |_| reference.strip_prefix('~').unwrap_or(reference).to_string(),
        |id| key_of_id(project, &id),
    )
}

/// How a review names the task `id`: its key, never with the `~` people read (ATM-594), else its ID.
fn key_of_id(project: &Project, id: &str) -> String {
    project
        .store()
        .get_task(id)
        .ok()
        .flatten()
        .map_or_else(|| id.to_string(), |t| project.task_key(t))
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
    // A live review a person opens is of an agent's work (§3.5): the person reviews it; the change is the agent's at
    // work in this copy, else its tasks', else nobody's yet (an agent's first round, or a task linked later, names it)
    if new.status == Status::Live
        && let Some(opener) = new.author.clone()
        && !is_agent(Some(&opener.0))
    {
        if !new.reviewers.contains(&opener) {
            new.reviewers.push(opener);
        }
        new.author = agent_at_work(project).or_else(|| agent_of_tasks(project, &ids));
    }
    // The review names its tasks by key, without the `~` of a provisional one (ATM-594): a reference atm resolves
    new.tasks = ids.iter().map(|id| key_of_id(project, id)).collect();
    let mut reviews = store(project)?;
    let header = Reviews::new(&mut reviews).create(new)?;
    // The first review: its files merge by atm on branches too (staged with the review's folder, by whoever commits)
    if let Some(root) = project.root()
        && let Ok(paths) = atm_db::jsongit::paths::find_paths(root)
    {
        let _ = atm_db::jsongit::setup::ensure_review_merge(&paths);
    }
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
    // Its tasks found first (ATM-594): a round is never kept without its tasks going back in review
    let ids = named(project, &reviews.get_review(reference)?.header).map_err(|e| {
        Error::failed(format!(
            "{}'s round can't reach its tasks, so it isn't added: {e}",
            reference.0
        ))
    })?;
    let number = Reviews::new(&mut reviews).add_round(reference, round)?;
    let header = reviews.get_review(reference)?.header;
    ask_review(project, &ids, &header, &format!("round {number}"))?;
    Ok(number)
}

/// A reviewer submits (§3.11's *Submit*); approving or sending back is every named task's decision, recorded as the
/// reviewer: approved, or changes requested with a note linking the review (`atm loop next` then hands the task back).
/// A task not in review (a live review's, still in progress) waits for [`apply_live_approval`]. `overriding`: a
/// person's explicit override of who may decide, with why, recorded in the review (the rules' `submit`).
///
/// # Errors
/// What the review's rules refuse, or a store error.
pub fn submit(
    project: &mut Project,
    reference: &ReviewRef,
    verdict: Verdict,
    body: Option<&str>,
    overriding: Option<&str>,
) -> Result<()> {
    let by = actor_of(project);
    let mut reviews = store(project)?;
    // Every task it names found first (ATM-594): a decision is never kept without reaching its tasks
    let tasks = named(project, &reviews.get_review(reference)?.header).map_err(|e| {
        Error::failed(format!(
            "{reference}'s decision can't reach its tasks, so it isn't recorded: {e}",
            reference = reference.0
        ))
    })?;
    Reviews::new(&mut reviews).submit(reference, verdict, body, overriding, &by)?;
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
    for id in tasks {
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
    let removed_keys: Vec<String> = removed.iter().map(|id| key_of_id(project, id)).collect();
    let mut tasks: Vec<String> = header
        .tasks
        .iter()
        .filter(|t| !removed_keys.contains(t) && !remove.contains(t))
        .cloned()
        .collect();
    for id in &added {
        let key = key_of_id(project, id);
        if !tasks.contains(&key) {
            tasks.push(key);
        }
    }
    Reviews::new(&mut reviews).set_tasks(reference, tasks, &by)?;
    // A review nobody's change yet becomes the agent's whose task it now names
    if header.author.is_none()
        && let Some(agent) = agent_of_tasks(project, &added)
        && !header.reviewers.contains(&agent)
    {
        reviews.set_author(&header.id, &agent)?;
    }
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
    let key = key_of_id(project, &id);
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
    fn round() -> NewRound {
        NewRound {
            kind: RoundKind::Range {
                base: "a".into(),
                head: "b".into(),
            },
            submodule: None,
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
                author: Some("claude:1".into()),
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
        super::submit(&mut project, &cr, Verdict::RequestChanges, None, None).unwrap();
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
        super::submit(&mut project, &cr, Verdict::Approve, None, None).unwrap();
        assert_eq!(task(&project, "T-1")["review_state"], "approved");
        // T-2 stayed in review with changes asked for: approved now too
        assert_eq!(task(&project, "T-2")["review_state"], "approved");
    }

    fn open_review(project: &mut Project, tasks: &[&str]) -> super::ReviewHeader {
        super::create(
            project,
            NewReview {
                title: "Both".into(),
                summary: String::new(),
                tasks: tasks.iter().map(|t| (*t).to_string()).collect(),
                author: Some("claude:1".into()),
                reviewers: vec!["Hakan".into()],
                status: Status::Open,
                first: round(),
            },
        )
        .unwrap()
    }

    #[test]
    fn a_project_without_a_folder_or_git_has_no_reviews() {
        let memory = Project::new(Box::new(
            atm_db::MemoryStore::new(Vec::new(), Vec::new(), &serde_json::Map::new()).unwrap(),
        ));
        assert_eq!(
            super::store(&memory).unwrap_err().to_string(),
            "this project has no folder to keep reviews in"
        );
        assert_eq!(
            super::vcs(&memory).unwrap_err().to_string(),
            "reviews are of changes in git: this project isn't in a git repository"
        );
        // nothing to check either
        assert_eq!(
            super::check(&memory, &json!({"tasks": []})),
            (Vec::new(), Vec::new())
        );
    }

    #[test]
    fn a_comment_only_submission_leaves_the_tasks_waiting() {
        let dir = dir();
        let mut project = project(&dir.0, "claude:1");
        let header = open_review(&mut project, &["T-1"]);
        let mut hakan = reopen(&dir.0, "Hakan");
        super::submit(
            &mut hakan,
            &header.reference,
            Verdict::Comment,
            Some("hm"),
            None,
        )
        .unwrap();
        assert_eq!(task(&hakan, "T-1")["review_state"], "waiting");
    }

    #[test]
    fn closing_a_review_rejects_its_tasks_only_when_asked() {
        for (reject, state) in [(false, "waiting"), (true, "rejected")] {
            let dir = dir();
            let mut project = project(&dir.0, "claude:1");
            let header = open_review(&mut project, &["T-1"]);
            let mut hakan = reopen(&dir.0, "Hakan");
            super::close(&mut hakan, &header.reference, "not needed", reject).unwrap();
            let t1 = task(&hakan, "T-1");
            assert_eq!(t1["review_state"], state, "reject {reject}");
            if reject {
                assert!(t1["notes"].to_string().contains("CR-1 closed: not needed"));
            }
        }
    }

    #[test]
    fn doctor_warns_of_a_task_linked_to_a_review_the_project_doesnt_have() {
        let dir = dir();
        let mut project = project(&dir.0, "claude:1");
        let header = open_review(&mut project, &["T-1"]);
        let document = json!({"tasks": [
            {"id": "T-1", "code_reviews": [{"id": header.id.0, "ref": "CR-1"}]},
            {"id": "T-2", "code_reviews": [{"id": "gone", "ref": "CR-7"}]},
            {"id": "T-3", "code_reviews": [{"id": "gone-too"}]}
        ]});
        let (errors, warnings) = super::check(&project, &document);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(
            warnings,
            [
                "T-2: its code review CR-7 isn't in the project (not merged here yet, or deleted)",
                "T-3: its code review gone-too isn't in the project (not merged here yet, or deleted)"
            ]
        );
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
                author: Some("claude:1".into()),
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
        let project = project(&dir.0, "claude:1");
        git(&["add", "-A"], &dir.0);
        git(&["commit", "-q", "-m", "base"], &dir.0);
        let vcs = project.vcs().unwrap();
        std::fs::write(dir.0.join("a.txt"), "one\ntwo\n").unwrap();
        let checkpoint = super::rounds::from_working(&vcs, "HEAD", &"Hakan".into()).unwrap();
        // Hakan opens a live review on the agent's work (§3.5): he reviews it, the change is the agent's who started
        // T-1, so he decides it
        let mut hakan = reopen(&dir.0, "Hakan");
        let header = super::create(
            &mut hakan,
            NewReview {
                title: "Live".into(),
                summary: String::new(),
                tasks: vec!["T-1".into()],
                author: Some("Hakan".into()),
                reviewers: Vec::new(),
                status: Status::Live,
                first: checkpoint,
            },
        )
        .unwrap();
        assert_eq!(header.author, Some("claude:1".into()));
        assert_eq!(header.reviewers, vec![super::Actor::from("Hakan")]);
        // Live: the task stays in progress
        assert_eq!(task(&project, "T-1")["status"], "in_progress");
        super::submit(
            &mut hakan,
            &ReviewRef(header.reference.0.clone()),
            Verdict::Approve,
            None,
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

    // ATM-594: a provisional key (`~ATM-12` as people read it) in reviews

    const PROVISIONAL: &str = "0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8012";

    /// A project in task keys' format: ATM-11 final, ATM-12 provisional, both in progress (`blocker`), as `actor`.
    fn keyed(root: &Path, actor: &str) -> Project {
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

    #[test]
    fn a_review_names_a_provisional_task_by_its_key_and_its_decision_reaches_it() {
        let dir = dir();
        let mut project = keyed(&dir.0, "claude:1");
        // Named as atm prints it, with the mark: the key itself
        let header = open_review(&mut project, &["~ATM-12", "ATM-11"]);
        assert_eq!(header.tasks, ["ATM-12", "ATM-11"]);
        let text = std::fs::read_to_string(
            std::fs::read_dir(dir.0.join(".agentic-task-manager/reviews"))
                .unwrap()
                .filter_map(Result::ok)
                .find(|e| e.path().is_dir())
                .unwrap()
                .path()
                .join("review.md"),
        )
        .unwrap();
        assert!(text.contains("tasks = [\"ATM-12\", \"ATM-11\"]"), "{text}");
        assert_eq!(task(&project, PROVISIONAL)["status"], "in_review");
        // The approval reaches both
        let mut hakan = reopen(&dir.0, "Hakan");
        super::submit(&mut hakan, &header.reference, Verdict::Approve, None, None).unwrap();
        assert_eq!(task(&hakan, PROVISIONAL)["review_state"], "approved");
        // And every command takes the key as people read it
        assert_eq!(hakan.resolve_task("~ATM-12").unwrap(), PROVISIONAL);
        assert_eq!(hakan.resolve_task("ATM-12").unwrap(), PROVISIONAL);
        assert!(hakan.resolve_task("~").is_err(), "a mark alone is no key");
    }

    #[test]
    fn a_decision_that_cant_reach_a_named_task_isnt_recorded_and_says_why() {
        let dir = dir();
        let mut project = keyed(&dir.0, "claude:1");
        let header = open_review(&mut project, &["ATM-12"]);
        // The task is gone from the project (a hand edit, a branch that dropped it)
        let mut store = super::store(&project).unwrap();
        store.set_tasks(&header.id, vec!["ATM-99".into()]).unwrap();
        let mut hakan = reopen(&dir.0, "Hakan");
        let error =
            super::submit(&mut hakan, &header.reference, Verdict::Approve, None, None).unwrap_err();
        assert!(
            error
                .to_string()
                .starts_with("CR-1's decision can't reach its tasks, so it isn't recorded: "),
            "{error}"
        );
        let reviews = super::store(&hakan).unwrap();
        assert_eq!(
            reviews.get_review(&header.reference).unwrap().header.status,
            Status::Open
        );
        // A round neither: refused before anything is written
        let mut agent = reopen(&dir.0, "claude:1");
        let error = super::add_round(&mut agent, &header.reference, round()).unwrap_err();
        assert!(
            error
                .to_string()
                .starts_with("CR-1's round can't reach its tasks, so it isn't added: "),
            "{error}"
        );
        let reviews = super::store(&agent).unwrap();
        assert_eq!(
            reviews.get_review(&header.reference).unwrap().rounds.len(),
            1
        );
    }

    #[test]
    fn doctor_names_a_decided_review_whose_task_still_waits_and_the_command_that_records_it() {
        let dir = dir();
        let mut project = keyed(&dir.0, "claude:1");
        let header = open_review(&mut project, &["ATM-12", "ATM-11"]);
        // As an atm before ATM-594 left it: the review approved, its decision not on the tasks
        let mut store = super::store(&project).unwrap();
        store
            .set_status(&header.id, Status::Approved, &"Hakan".into())
            .unwrap();
        let document = project.store().document().unwrap().clone();
        let (errors, warnings) = super::check(&project, &document);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(
            warnings,
            [
                "ATM-12: CR-1 is approved but the task still waits for its review: `atm task review approve ATM-12` records the decision",
                "ATM-11: CR-1 is approved but the task still waits for its review: `atm task review approve ATM-11` records the decision",
            ]
        );
        // Recorded: nothing more to say
        let mut hakan = reopen(&dir.0, "Hakan");
        for key in ["ATM-12", "ATM-11"] {
            let id = hakan.resolve_task(key).unwrap();
            crate::changes::change_task_status(
                &mut hakan,
                crate::state::Command::Approve,
                &id,
                &crate::state::Options::default(),
            )
            .unwrap();
        }
        let document = hakan.store().document().unwrap().clone();
        assert_eq!(super::check(&hakan, &document), (Vec::new(), Vec::new()));
    }
}
