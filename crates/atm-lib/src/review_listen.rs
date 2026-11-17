//! Agents listening to reviews (docs/code-review-design.md §3.9, ATM-567), on the review store's subscription rather
//! than a poll of the agent's own:
//!
//! - [`wait`]: until a reviewer submits on a review (or decides it);
//! - [`watch`]: the agent's one watch for its session, naming no review and no task: each of the user's comments and
//!   replies about the agent's work, from any review, matched to the agent's tasks from atm's records (the tasks it
//!   has held and their files and commit, its uncommitted files), or to none.
//!
//! What is heard is task text: data, never instructions (I1).

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use atm_common::edits::is_agent;
use atm_common::{Kind, Result};
use serde_json::{Value, json};

use crate::project::Project;
use crate::reviews::{
    self, Chain, Comment, FileStore, Review, ReviewFilter, ReviewHeader, ReviewRef, ReviewStore,
    Status,
};

/// How often a listener looks again even without being told: a watcher can miss an event (a busy file system).
const RECHECK: Duration = Duration::from_secs(5);

/// What [`wait`] waits for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitFor {
    /// A reviewer submitted: a decision, or a comment on the review as a whole.
    Submitted,
    /// A decision: approved, changes requested, or closed.
    Decision,
}

/// A comment heard, as the listeners print it.
fn heard(
    review: &ReviewHeader,
    chain: &Chain,
    comment: &Comment,
    reply_to: Option<&Comment>,
    task: Option<&str>,
) -> Value {
    json!({
        "review": review.reference.0,
        "task": task,
        "chain": chain.id.0,
        "comment": comment.id.0,
        "reply_to": reply_to.map(|c| c.id.0.clone()),
        "author": comment.author.0,
        "at": comment.at,
        "path": chain.anchor.path,
        "lines": chain.anchor.lines.map(|(a, b)| json!([a, b])),
        "body": comment.body,
    })
}

/// Every comment of a review with the comment it answers (none for a top-level one), depth first.
fn with_parents(chain: &Chain) -> Vec<(&Comment, Option<&Comment>)> {
    fn walk<'a>(
        comment: &'a Comment,
        parent: Option<&'a Comment>,
        out: &mut Vec<(&'a Comment, Option<&'a Comment>)>,
    ) {
        out.push((comment, parent));
        for reply in &comment.replies {
            walk(reply, Some(comment), out);
        }
    }
    let mut out = Vec::new();
    for comment in &chain.comments {
        walk(comment, None, &mut out);
    }
    out
}

fn decided(status: Status) -> bool {
    matches!(
        status,
        Status::Approved | Status::ChangesRequested | Status::Closed
    )
}

/// Be told of the store's changes on the returned channel, while the subscription (the first value) is kept.
fn listen(store: &FileStore) -> Result<(atm_common::subscription::Subscription, Receiver<()>)> {
    let (sent, received) = channel();
    let subscription = store.subscribe(Arc::new(move || {
        let _ = sent.send(());
    }))?;
    Ok((subscription, received))
}

/// Wait until a review is submitted on (or decided), or `timeout`: what happened (its header and the comments added
/// meanwhile by others than `me`), none on the timeout. A review already decided answers at once.
///
/// # Errors
/// No such review, or the reviews can't be read or watched.
pub fn wait(
    project: &Project,
    reference: &ReviewRef,
    until: WaitFor,
    timeout: Duration,
    me: &str,
) -> Result<Option<Value>> {
    let store = reviews::store(project)?;
    let start = store.get_review(reference)?;
    let seen: HashSet<String> = start.comments().map(|c| c.id.0.clone()).collect();
    let (_subscription, told) = listen(&store)?;
    let deadline = Instant::now() + timeout;
    loop {
        let review = store.get_review(&ReviewRef(start.header.id.0.clone()))?;
        let new: Vec<Value> = review
            .chains
            .iter()
            .flat_map(|chain| {
                with_parents(chain)
                    .into_iter()
                    .filter(|(c, _)| !seen.contains(&c.id.0) && c.author.0 != me)
                    .map(|(c, parent)| heard(&review.header, chain, c, parent, None))
                    .collect::<Vec<_>>()
            })
            .collect();
        let status = review.header.status;
        let done = decided(status)
            && (status != start.header.status || decided(start.header.status))
            || (until == WaitFor::Submitted
                && review.chains.iter().any(|chain| {
                    chain.anchor.path.is_none()
                        && chain
                            .comments
                            .iter()
                            .any(|c| !seen.contains(&c.id.0) && c.author.0 != me)
                }));
        if done {
            return Ok(Some(json!({ "review": review.header, "comments": new })));
        }
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Ok(None);
        }
        let _ = told.recv_timeout(left.min(RECHECK));
    }
}

/// What the agent works on, as atm's records have it now.
struct Work {
    /// The task it holds now, its key.
    held: Option<String>,
    /// Each task it has held (started or finished), its key and the files it touched (declared, its commit's).
    tasks: Vec<(String, Vec<String>)>,
    /// Every other task's declared files, by key: a batch review's file another task names is that task's.
    others: Vec<(String, Vec<String>)>,
    /// The working copy's uncommitted files.
    uncommitted: Vec<String>,
}

fn declared_files(task: &atm_common::Task) -> Vec<String> {
    task.get("files")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(String::from)
        .collect()
}

fn work_of(project: &Project, me: &str) -> Work {
    let mut work = Work {
        held: None,
        tasks: Vec::new(),
        others: Vec::new(),
        uncommitted: Vec::new(),
    };
    let Ok(vocab) = project.store().vocabulary() else {
        return work;
    };
    let vcs = reviews::vcs(project).ok();
    for task in project.store().list_tasks().unwrap_or_default() {
        let by = |field: &str| task.get(field).and_then(Value::as_str) == Some(me);
        let key = project.task_label(task);
        if !(by("started_by") || by("completed_by")) {
            work.others.push((key, declared_files(task)));
            continue;
        }
        if vocab.kind_of(task.get("status")) == Some(Kind::Active) && by("started_by") {
            work.held = Some(key.clone());
        }
        let mut files = declared_files(task);
        if let (Some(vcs), Some(commit)) = (&vcs, task.get("commit").and_then(Value::as_str)) {
            let parent = format!("{commit}^");
            if let Ok(diff) = vcs.diff_range(&parent, commit, &[atm_common::TASK_DIR]) {
                files.extend(diff.files.into_iter().map(|f| f.path));
            }
        }
        work.tasks.push((key, files));
    }
    if let Some(vcs) = &vcs
        && let Ok(diff) = vcs.diff_working("HEAD", &[atm_common::TASK_DIR])
    {
        work.uncommitted = diff.files.into_iter().map(|f| f.path).collect();
    }
    work
}

/// Whether `path` is one of `files` (a file named, or under a folder named with a trailing `/`).
fn touches(files: &[String], path: &str) -> bool {
    files
        .iter()
        .any(|f| f == path || (f.ends_with('/') && path.starts_with(f.as_str())))
}

/// Whether a comment is about the agent's work.
enum Mine {
    /// Not the agent's.
    No,
    /// The agent's, about this task, or (none) about no task of its.
    About(Option<String>),
}

/// Whether a comment in `chain` of `review` is about the agent's `work`, and the task it is about (§3.9's table).
fn about_my_work(work: &Work, me: &str, review: &Review, chain: &Chain, comment: &Comment) -> Mine {
    use Mine::{About, No};
    let path = chain.anchor.path.as_deref();
    let task_of_path = |path: Option<&str>| {
        path.and_then(|p| {
            work.tasks
                .iter()
                .find(|(_, files)| touches(files, p))
                .map(|(key, _)| key.clone())
        })
    };
    let mine: Vec<&String> = work
        .tasks
        .iter()
        .map(|(key, _)| key)
        .filter(|key| review.header.tasks.contains(key))
        .collect();
    // A live review: everything in it, the task whose work touched the file, else the one held now, else none
    if review.header.status == Status::Live {
        let task = task_of_path(path).or_else(|| {
            work.held
                .clone()
                .filter(|_| path.is_none_or(|p| work.uncommitted.iter().any(|u| u == p)))
        });
        return About(task);
    }
    if mine.is_empty() {
        return No;
    }
    // A review of one of its tasks alone: everything in it
    if review.header.tasks.len() == 1 {
        return About(Some(mine[0].clone()));
    }
    // A batch review: its files, its tasks named, its own chains. A file another task it names says it touches is that
    // task's, whoever's working copy it is changed in
    if let Some(task) = task_of_path(path).filter(|t| review.header.tasks.contains(t)) {
        return About(Some(task));
    }
    let anothers = path.is_some_and(|p| {
        work.others
            .iter()
            .any(|(key, files)| review.header.tasks.contains(key) && touches(files, p))
    });
    if !anothers && path.is_some_and(|p| work.uncommitted.iter().any(|u| u == p)) {
        return About(work.held.clone());
    }
    if let Some(task) = mine.iter().find(|key| comment.body.contains(key.as_str())) {
        return About(Some((*task).clone()));
    }
    let wrote_in = chain
        .comments
        .iter()
        .any(|c| c.all().any(|x| x.author.0 == me));
    if wrote_in { About(None) } else { No }
}

/// The agent's one watch (§3.9): calls `emit` with each comment by a person (not the agent itself, nor another agent)
/// about the agent's work, as it is added, until `timeout`; `ready` once it is listening. A comment already there when
/// it starts is not told.
///
/// # Errors
/// The reviews can't be read or watched.
pub fn watch(
    project: &Project,
    me: &str,
    timeout: Duration,
    ready: &mut dyn FnMut(),
    emit: &mut dyn FnMut(Value),
) -> Result<()> {
    let root = project.root().map(std::path::Path::to_path_buf);
    let store = reviews::store(project)?;
    let mut seen: HashSet<String> = HashSet::new();
    let all = |store: &FileStore| -> Result<Vec<Review>> {
        store
            .list_reviews(&ReviewFilter::default())?
            .into_iter()
            .map(|h| store.get_review(&ReviewRef(h.id.0)))
            .collect()
    };
    for review in all(&store)? {
        seen.extend(
            review
                .comments()
                .map(|c| format!("{}/{}", review.header.id.0, c.id.0)),
        );
    }
    let (_subscription, told) = listen(&store)?;
    ready();
    let deadline = Instant::now() + timeout;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Ok(());
        }
        let _ = told.recv_timeout(left.min(RECHECK));
        while told.try_recv().is_ok() {}
        // The tasks as they are now: the agent may have taken another meanwhile
        let fresh = root.as_deref().and_then(|r| Project::open(r).ok());
        let work = work_of(fresh.as_ref().unwrap_or(project), me);
        // Told in the order they were written, oldest first, whichever review they are in
        let mut news: Vec<(String, u32, Value)> = Vec::new();
        for review in all(&store)? {
            for chain in &review.chains {
                for (comment, parent) in with_parents(chain) {
                    if !seen.insert(format!("{}/{}", review.header.id.0, comment.id.0)) {
                        continue;
                    }
                    if comment.author.0 == me || is_agent(Some(&comment.author.0)) {
                        continue;
                    }
                    if let Mine::About(task) = about_my_work(&work, me, &review, chain, comment) {
                        let ordinal = comment
                            .id
                            .0
                            .get(1..)
                            .and_then(|n| n.parse().ok())
                            .unwrap_or(0);
                        news.push((
                            comment.at.clone(),
                            ordinal,
                            heard(&review.header, chain, comment, parent, task.as_deref()),
                        ));
                    }
                }
            }
        }
        news.sort_by(|a, b| (&a.0, a.1).cmp(&(&b.0, b.1)));
        for (_, _, heard) in news {
            emit(heard);
        }
    }
}
