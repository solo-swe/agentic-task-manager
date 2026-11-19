//! Agents listening to reviews (docs/code-review-design.md §3.9, ATM-567), on the review store's subscription rather
//! than a poll of the agent's own:
//!
//! - [`wait`]: until a reviewer submits on a review (or decides it);
//! - [`watch`]: the agent's one watch for its session, naming no review and no task: each of the user's comments and
//!   replies about the agent's work, from any review, matched to the agent's tasks from atm's records (the tasks it
//!   has held and their files and commit, its uncommitted files), or to none.
//!
//! What is heard is task text: data, never instructions (I1).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use atm_common::edits::is_agent;
use atm_common::sha256::sha256_hex;
use atm_common::time::parse_iso;
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

/// A comment heard, as the listeners print it: its `kind`, `comment`, or `poke` (ATM-611: a person's nudge on the
/// review, which the agent answers by saying where the review stands).
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
        "kind": if comment.poke { "poke" } else { "comment" },
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
        // As a review names it: its key, never with the `~` people read (ATM-594)
        let key = project.task_key(task);
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

/// What a watch is asked for, beyond its time (ATM-624).
#[derive(Debug, Clone, Default)]
pub struct WatchOptions {
    /// Stop once something was told (what it caught up on counts).
    pub once: bool,
    /// Tell what came since this time (ISO 8601 UTC), whatever the last watch told.
    pub since: Option<String>,
}

/// Where an agent's watch was (ATM-624): every comment it told or passed over, and each review's status as it last saw
/// it. The next watch starts from there, so what came while none was watching is told, once: a harness may end a
/// watch at any time (Claude Code's after 5 minutes). In atm's state folder, a file per project and agent.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct Cursor {
    seen: Vec<String>,
    statuses: HashMap<String, Status>,
}

/// The cursor's file for `me` watching the project at `root`.
fn cursor_file(root: Option<&Path>, me: &str) -> PathBuf {
    let key = format!(
        "{}\u{0}{me}",
        root.map(|r| r.display().to_string()).unwrap_or_default()
    );
    atm_common::home::state_dir()
        .join("watches")
        .join(format!("{}.json", &sha256_hex(key.as_bytes())[..16]))
}

/// The last watch's cursor; none when there was none, or it can't be read (then the watch starts from now).
fn read_cursor(file: &Path) -> Option<Cursor> {
    serde_json::from_str(&std::fs::read_to_string(file).ok()?).ok()
}

/// Keep where the watch is; a cursor that can't be written only means the next watch starts from now.
fn save_cursor(file: &Path, seen: &HashSet<String>, statuses: &HashMap<String, Status>) {
    let mut seen: Vec<String> = seen.iter().cloned().collect();
    seen.sort();
    let cursor = Cursor {
        seen,
        statuses: statuses.clone(),
    };
    if let (Some(dir), Ok(text)) = (file.parent(), serde_json::to_string(&cursor)) {
        let _ = std::fs::create_dir_all(dir);
        let _ = atm_common::home::write_text_atomic(file, &text);
    }
}

/// Every review the store holds, whole.
fn all_reviews(store: &FileStore) -> Result<Vec<Review>> {
    store
        .list_reviews(&ReviewFilter::default())?
        .into_iter()
        .map(|h| store.get_review(&ReviewRef(h.id.0)))
        .collect()
}

/// Where a watch starts: the comments to pass over, and each review's status as last seen (a decision is a change to
/// a decided one, ATM-600). From the last watch's cursor (ATM-624); from `since`, what came after it is left to tell;
/// else from now, the cursor written so the next watch starts here.
fn starting_point(
    store: &FileStore,
    file: &Path,
    since: Option<&str>,
) -> Result<(HashSet<String>, HashMap<String, Status>)> {
    if since.is_none()
        && let Some(cursor) = read_cursor(file)
    {
        return Ok((cursor.seen.into_iter().collect(), cursor.statuses));
    }
    let mut seen = HashSet::new();
    let mut statuses = HashMap::new();
    for review in all_reviews(store)? {
        seen.extend(
            review
                .comments()
                .filter(|c| since.is_none_or(|s| c.at.as_str() < s))
                .map(|c| format!("{}/{}", review.header.id.0, c.id.0)),
        );
        // Decided since then: left out, so it is told
        if since.is_none_or(|s| review.header.updated.as_str() < s) {
            statuses.insert(review.header.id.0.clone(), review.header.status);
        }
    }
    if since.is_none() {
        save_cursor(file, &seen, &statuses);
    }
    Ok((seen, statuses))
}

/// The agent's one watch (§3.9): calls `emit` with each comment by a person (not the agent itself, nor another agent)
/// about the agent's work, and each decision on it, until `timeout`; `ready` once it is listening. It starts where the
/// agent's last watch on this project stopped (ATM-624), so what came meanwhile is told first; the first watch ever,
/// or one whose cursor can't be read, from now; `since`, from that time. With `once`, it stops after the first look
/// that told something. What it told, counted.
///
/// # Errors
/// The reviews can't be read or watched; `since` isn't a time.
pub fn watch(
    project: &Project,
    me: &str,
    timeout: Duration,
    options: &WatchOptions,
    ready: &mut dyn FnMut(),
    emit: &mut dyn FnMut(Value),
) -> Result<usize> {
    if let Some(since) = &options.since
        && parse_iso(since).is_none()
    {
        return Err(atm_common::Error::Invalid(format!(
            "--since needs a time like 2026-10-24T18:00:00Z, not {since}"
        )));
    }
    let root = project.root().map(Path::to_path_buf);
    let store = reviews::store(project)?;
    let all = all_reviews;
    let file = cursor_file(root.as_deref(), me);
    let (mut seen, mut statuses) = starting_point(&store, &file, options.since.as_deref())?;
    let (_subscription, told) = listen(&store)?;
    // Known to be running while it does (ATM-642): renewed on every look, removed when it ends
    let lease = Lease::take(lease_file(root.as_deref(), me));
    ready();
    let deadline = Instant::now() + timeout;
    let mut count = 0;
    // The first look is at once: what came while no watch ran
    let mut first = true;
    loop {
        lease.renew();
        if !first {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Ok(count);
            }
            let _ = told.recv_timeout(left.min(RECHECK));
        }
        first = false;
        while told.try_recv().is_ok() {}
        let before = count;
        // The tasks as they are now: the agent may have taken another meanwhile
        let fresh = root.as_deref().and_then(|r| Project::open(r).ok());
        let now = fresh.as_ref().unwrap_or(project);
        let work = work_of(now, me);
        // Told in the order they were written, oldest first, whichever review they are in
        let mut news: Vec<(String, u32, Value)> = Vec::new();
        for mut review in all(&store)? {
            keyed_now(now, &store, &mut review.header);
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
            count += 1;
        }
        // Then the decisions on reviews of its work, after the words that came with them
        for mut review in all(&store)? {
            keyed_now(now, &store, &mut review.header);
            let status = review.header.status;
            let was = statuses.insert(review.header.id.0.clone(), status);
            if was == Some(status) || !decided(status) {
                continue;
            }
            if let Mine::About(task) = decision_about_my_work(&work, me, &review.header) {
                emit(decision(&review.header, task.as_deref()));
                count += 1;
            }
        }
        save_cursor(&file, &seen, &statuses);
        if options.once && count > before {
            return Ok(count);
        }
    }
}

/// A review's tasks as they are keyed now (ATM-630): a review written before a task's number moved at a merge names
/// the key it had (an alias now), an older one `~KEY` or an earlier prefix's; each resolved as every command resolves
/// one, so the agent's work is found whatever key its review was written with; an old key another task holds now as
/// the review in `store` meant it ([`reviews::key_now`]). One the project doesn't have stays.
fn keyed_now(project: &Project, store: &FileStore, header: &mut ReviewHeader) {
    header.tasks = header
        .tasks
        .iter()
        .map(|key| reviews::key_now(project, Some(store), header, key))
        .collect();
}

/// Whether a decided review is of the agent's work, and the task it decides: its own review, or one naming a task it
/// has held (ATM-600).
fn decision_about_my_work(work: &Work, me: &str, review: &ReviewHeader) -> Mine {
    let task = work
        .tasks
        .iter()
        .map(|(key, _)| key)
        .find(|key| review.tasks.contains(key))
        .cloned();
    let own = review.author.as_ref().is_some_and(|a| a.0 == me);
    if task.is_some() || own {
        Mine::About(task)
    } else {
        Mine::No
    }
}

/// A decision heard, as the watch prints it: `decision` the review's new status (`approved`, `changes_requested`,
/// `closed`); its words, if it had some, came as a comment just before.
fn decision(review: &ReviewHeader, task: Option<&str>) -> Value {
    json!({
        "review": review.reference.0,
        "task": task,
        "decision": review.status.name(),
        "at": review.updated,
        "title": review.title,
    })
}

/// How long a watch's lease lasts unrenewed (ATM-642). A watch renews it on every look, at least every [`RECHECK`], so
/// one that stopped, crashed or was killed is known to within this.
const LEASE_SECS: i64 = 60;

/// How long atm stays quiet after reminding an agent that it isn't watching, unless something new came meanwhile.
const REMIND_EVERY_SECS: i64 = 600;

/// The lease of `me`'s watch of the project at `root`: beside its cursor.
fn lease_file(root: Option<&Path>, me: &str) -> PathBuf {
    cursor_file(root, me).with_extension("live")
}

/// When atm last reminded `me` that it wasn't watching: beside its cursor.
fn reminded_file(root: Option<&Path>, me: &str) -> PathBuf {
    cursor_file(root, me).with_extension("reminded")
}

/// A running watch's lease (ATM-642): until when it is known to run, in seconds since 1970, written while it runs
/// and removed when it ends. Several watches of one agent share it; the others renew it within [`RECHECK`].
struct Lease(PathBuf);

impl Lease {
    fn take(file: PathBuf) -> Self {
        let lease = Self(file);
        lease.renew();
        lease
    }

    /// A lease that can't be written only means atm may remind the agent that it isn't watching.
    fn renew(&self) {
        if let Some(dir) = self.0.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let until = atm_common::time::real_now() + LEASE_SECS;
        let _ = atm_common::home::write_text_atomic(&self.0, &until.to_string());
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Whether a lease reading `text` still holds at `now`.
fn holds(text: Option<&str>, now: i64) -> bool {
    text.and_then(|t| t.trim().parse::<i64>().ok())
        .is_some_and(|until| until > now)
}

/// Whether an agent's reviews are watched (ATM-642): what `atm loop next --json` says as `watch`, and what atm reminds
/// an agent of when they aren't.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct WatchState {
    /// A watch of the agent's runs now.
    pub running: bool,
    /// Without one: the reviews of its work not decided yet (its own, or naming a task it worked), what a watch would
    /// tell it about, by number.
    pub open: Vec<String>,
    /// Of those, the ones changed since its last watch stopped: a comment or a decision it hasn't been told. None
    /// before its first watch (that one starts from then).
    pub since_last_watch: Vec<String>,
}

/// The open reviews of `me`'s work among `headers`, and those of them changed after `last_watch` (ISO 8601).
fn open_and_changed(
    headers: &[ReviewHeader],
    mine: &HashSet<String>,
    me: &str,
    last_watch: Option<&str>,
) -> (Vec<String>, Vec<String>) {
    let mut open: Vec<&ReviewHeader> = headers
        .iter()
        .filter(|h| {
            matches!(
                h.status,
                Status::Waiting | Status::Live | Status::ChangesRequested
            )
        })
        .filter(|h| {
            h.author.as_ref().is_some_and(|a| a.0 == me) || h.tasks.iter().any(|t| mine.contains(t))
        })
        .collect();
    open.sort_by_key(|h| h.number);
    let changed = open
        .iter()
        .filter(|h| last_watch.is_some_and(|at| h.updated.as_str() > at))
        .map(|h| h.reference.0.clone())
        .collect();
    (
        open.iter().map(|h| h.reference.0.clone()).collect(),
        changed,
    )
}

/// Whether `me`'s reviews are watched now, and if not, what is open and what changed since its last watch. Cheap
/// enough for every command: the reviews' headers and the task list, no diffs.
///
/// # Errors
/// The reviews or the tasks can't be read.
pub fn watch_state(project: &Project, me: &str) -> Result<WatchState> {
    let root = project.root().map(Path::to_path_buf);
    let lease = std::fs::read_to_string(lease_file(root.as_deref(), me)).ok();
    if holds(lease.as_deref(), atm_common::time::real_now()) {
        return Ok(WatchState {
            running: true,
            ..WatchState::default()
        });
    }
    let store = reviews::store(project)?;
    let mut headers = store.list_reviews(&ReviewFilter::default())?;
    for header in &mut headers {
        keyed_now(project, &store, header);
    }
    let mine: HashSet<String> = project
        .store()
        .list_tasks()?
        .iter()
        .filter(|t| {
            ["started_by", "completed_by"]
                .iter()
                .any(|f| t.get(*f).and_then(Value::as_str) == Some(me))
        })
        .map(|t| project.task_key(t))
        .collect();
    // The last watch stopped when it last wrote its cursor
    let last_watch = std::fs::metadata(cursor_file(root.as_deref(), me))
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| atm_common::time::format_iso(i64::try_from(d.as_secs()).unwrap_or(i64::MAX)));
    let (open, since_last_watch) = open_and_changed(&headers, &mine, me, last_watch.as_deref());
    Ok(WatchState {
        running: false,
        open,
        since_last_watch,
    })
}

/// When atm last reminded the agent, and what had changed then.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct Reminded {
    at: i64,
    changed: Vec<String>,
}

/// The reminder for `state` at `now`, after the one `last`: none while a watch runs or nothing of the agent's is
/// open; else once every [`REMIND_EVERY_SECS`], and at once when a review changed that the last one didn't name.
fn reminder_for(state: &WatchState, last: Option<&Reminded>, now: i64) -> Option<String> {
    if state.running || state.open.is_empty() {
        return None;
    }
    let new = state
        .since_last_watch
        .iter()
        .any(|r| last.is_none_or(|l| !l.changed.contains(r)));
    if !new && last.is_some_and(|l| now - l.at < REMIND_EVERY_SECS) {
        return None;
    }
    let n = state.open.len();
    let open = if n <= 3 {
        state.open.join(", ")
    } else {
        format!("{}, … {}", state.open[..2].join(", "), state.open[n - 1])
    };
    let changed = if state.since_last_watch.is_empty() {
        String::new()
    } else {
        format!(
            "; changed since your last watch: {}",
            state.since_last_watch.join(", ")
        )
    };
    Some(format!(
        "you aren't watching your reviews ({n} open: {open}{changed}). Start `atm review watch --json` as a \
         background command and read what it prints (atm docs reviews)"
    ))
}

/// What atm says on stderr when agent `me` runs a command with no watch of its reviews running and one of them open
/// (ATM-642), at most every ten minutes unless one changed; none otherwise. Never in the way of the command: a state
/// that can't be read says nothing.
#[must_use]
pub fn reminder(project: &Project, me: &str) -> Option<String> {
    if !is_agent(Some(me)) {
        return None;
    }
    let state = watch_state(project, me).ok()?;
    let file = reminded_file(project.root(), me);
    let last: Option<Reminded> = std::fs::read_to_string(&file)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok());
    let now = atm_common::time::real_now();
    let line = reminder_for(&state, last.as_ref(), now)?;
    let reminded = Reminded {
        at: now,
        changed: state.since_last_watch,
    };
    if let (Some(dir), Ok(text)) = (file.parent(), serde_json::to_string(&reminded)) {
        let _ = std::fs::create_dir_all(dir);
        let _ = atm_common::home::write_text_atomic(&file, &text);
    }
    Some(line)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reviews::{Actor, Anchor, CommentId, ReviewId};
    use atm_review::model::ChainId;

    fn comment(id: &str, author: &str, body: &str, replies: Vec<Comment>) -> Comment {
        Comment {
            id: CommentId(id.into()),
            author: Actor::from(author),
            at: "2026-11-17T10:00:00Z".into(),
            body: body.into(),
            resolved: None,
            poke: false,
            replies,
        }
    }

    fn chain(path: Option<&str>, comments: Vec<Comment>) -> Chain {
        Chain {
            id: ChainId("t1".into()),
            anchor: Anchor {
                round: 1,
                path: path.map(String::from),
                lines: path.map(|_| (3, 4)),
                ..Anchor::default()
            },
            comments,
        }
    }

    fn review(status: Status, tasks: &[&str]) -> Review {
        Review {
            header: ReviewHeader {
                id: ReviewId("r".into()),
                reference: ReviewRef("CR-1".into()),
                number: 1,
                title: "T".into(),
                status,
                tasks: tasks.iter().map(|t| (*t).to_string()).collect(),
                author: Some(Actor::from("claude:me")),
                reviewers: Vec::new(),
                created: String::new(),
                updated: String::new(),
            },
            summary: String::new(),
            chains: Vec::new(),
            rounds: Vec::new(),
        }
    }

    fn files(list: &[&str]) -> Vec<String> {
        list.iter().map(|f| (*f).to_string()).collect()
    }

    /// The agent held ATM-1 (src/a.rs, now) and finished ATM-2 (docs/); ATM-3, another's, names src/b.rs; src/c.rs
    /// and src/b.rs are changed in its working copy.
    fn work() -> Work {
        Work {
            held: Some("ATM-1".into()),
            tasks: vec![
                ("ATM-1".into(), files(&["src/a.rs"])),
                ("ATM-2".into(), files(&["docs/"])),
            ],
            others: vec![("ATM-3".into(), files(&["src/b.rs"]))],
            uncommitted: files(&["src/c.rs", "src/b.rs"]),
        }
    }

    /// Whether the chain's first comment is the agent's, in words: `not mine`, `no task` (the agent's, about none of
    /// its tasks), or the task's key.
    fn about(review: &Review, chain: &Chain) -> String {
        let c = &chain.comments[0];
        match about_my_work(&work(), "claude:me", review, chain, c) {
            Mine::No => NOT_MINE.into(),
            Mine::About(task) => task.unwrap_or_else(|| NO_TASK.into()),
        }
    }

    const NOT_MINE: &str = "not mine";
    const NO_TASK: &str = "no task";

    fn by_hakan(path: Option<&str>, body: &str) -> Chain {
        chain(path, vec![comment("c1", "Hakan", body, Vec::new())])
    }

    /// ATM-600: a decision is the agent's when the review is its own or names a task it has held: the task it decides
    /// (the first of its own the review names), or none for its own review of no task of its
    #[test]
    fn a_decision_is_about_the_agents_work_when_the_review_is_its_own_or_names_its_task() {
        let mut theirs = review(Status::Approved, &["ATM-3"]).header;
        theirs.author = Some(Actor::from("claude:other"));
        for (header, expected) in [
            (
                review(Status::Approved, &["ATM-2"]).header,
                Some(Some("ATM-2")),
            ),
            (
                review(Status::Approved, &["ATM-3", "ATM-1"]).header,
                Some(Some("ATM-1")),
            ),
            (review(Status::Closed, &[]).header, Some(None)),
            (theirs.clone(), None),
            (
                ReviewHeader {
                    tasks: vec!["ATM-1".into()],
                    ..theirs
                },
                Some(Some("ATM-1")),
            ),
        ] {
            let got = match decision_about_my_work(&work(), "claude:me", &header) {
                Mine::No => None,
                Mine::About(task) => Some(task),
            };
            assert_eq!(got, expected.map(|t| t.map(String::from)), "{header:?}");
        }
    }

    #[test]
    fn a_decision_is_told_by_the_review_its_task_and_the_status_it_has_now() {
        let mut header = review(Status::ChangesRequested, &["ATM-1"]).header;
        header.updated = "2026-11-18T10:00:00Z".into();
        assert_eq!(
            decision(&header, Some("ATM-1")),
            json!({
                "review": "CR-1",
                "task": "ATM-1",
                "decision": "changes_requested",
                "at": "2026-11-18T10:00:00Z",
                "title": "T",
            })
        );
    }

    #[test]
    fn a_file_is_touched_when_named_or_under_a_folder_named() {
        let list = files(&["src/a.rs", "docs/"]);
        for (path, expected) in [
            ("src/a.rs", true),
            ("docs/x.md", true),
            ("docs/deep/y.md", true),
            ("src/a.rs.bak", false),
            ("src/", false),
            ("docs", false),
            ("other.rs", false),
        ] {
            assert_eq!(touches(&list, path), expected, "{path}");
        }
        assert!(!touches(&[], "a"));
    }

    // ATM-624: where a watch was, kept for the next
    #[test]
    fn a_watch_cursor_is_read_back_as_written_and_a_missing_or_broken_one_is_none() {
        let dir = std::env::temp_dir().join(format!("atm-lib-watch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let file = dir.join("watches").join("x.json");
        assert!(read_cursor(&file).is_none());
        let seen: HashSet<String> = ["r1/c2".to_string(), "r1/c1".to_string()].into();
        let statuses: HashMap<String, Status> = [("r1".to_string(), Status::Approved)].into();
        save_cursor(&file, &seen, &statuses);
        let back = read_cursor(&file).unwrap();
        assert_eq!(back.seen, ["r1/c1", "r1/c2"]);
        assert_eq!(back.statuses, statuses);
        std::fs::write(&file, "{ not json").unwrap();
        assert!(read_cursor(&file).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_watch_cursor_is_one_file_per_project_and_agent() {
        let (a, b) = (Path::new("/p/one"), Path::new("/p/two"));
        assert_eq!(
            cursor_file(Some(a), "claude:A"),
            cursor_file(Some(a), "claude:A")
        );
        assert_ne!(
            cursor_file(Some(a), "claude:A"),
            cursor_file(Some(a), "claude:B")
        );
        assert_ne!(
            cursor_file(Some(a), "claude:A"),
            cursor_file(Some(b), "claude:A")
        );
    }

    #[test]
    fn a_decision_is_approved_changes_requested_or_closed() {
        for (status, expected) in [
            (Status::Draft, false),
            (Status::Waiting, false),
            (Status::Live, false),
            (Status::ChangesRequested, true),
            (Status::Approved, true),
            (Status::Closed, true),
        ] {
            assert_eq!(decided(status), expected, "{status:?}");
        }
    }

    #[test]
    fn every_comment_comes_with_the_one_it_answers_depth_first() {
        let deep = comment("c3", "a", "", Vec::new());
        let c = chain(
            None,
            vec![
                comment("c1", "a", "", vec![comment("c2", "b", "", vec![deep])]),
                comment("c4", "a", "", Vec::new()),
            ],
        );
        let pairs: Vec<(&str, Option<&str>)> = with_parents(&c)
            .into_iter()
            .map(|(c, p)| (c.id.0.as_str(), p.map(|p| p.id.0.as_str())))
            .collect();
        assert_eq!(
            pairs,
            [
                ("c1", None),
                ("c2", Some("c1")),
                ("c3", Some("c2")),
                ("c4", None)
            ]
        );
        assert_eq!(with_parents(&chain(None, Vec::new())).len(), 0);
    }

    fn whole_review() -> Chain {
        chain(None, Vec::new())
    }

    #[test]
    fn a_comment_heard_names_its_review_chain_place_and_text() {
        let r = review(Status::Waiting, &["ATM-1"]);
        let parent = comment("c1", "claude:me", "Done", Vec::new());
        let reply = comment("c2", "Hakan", "Why?", Vec::new());
        let c = chain(Some("src/a.rs"), Vec::new());
        assert_eq!(
            heard(&r.header, &c, &reply, Some(&parent), Some("ATM-1")),
            json!({
                "review": "CR-1", "task": "ATM-1", "chain": "t1", "comment": "c2", "kind": "comment",
                "reply_to": "c1", "author": "Hakan", "at": "2026-11-17T10:00:00Z", "path": "src/a.rs",
                "lines": [3, 4], "body": "Why?"
            })
        );
        // ATM-611: a poke is heard as one
        let mut poke = comment("c3", "Hakan", "Where are we?", Vec::new());
        poke.poke = true;
        assert_eq!(
            heard(&r.header, &whole_review(), &poke, None, Some("ATM-1"))["kind"],
            "poke"
        );
        let whole = chain(None, Vec::new());
        let v = heard(&r.header, &whole, &reply, None, None);
        assert_eq!(
            (
                v["path"].clone(),
                v["lines"].clone(),
                v["reply_to"].clone(),
                v["task"].clone()
            ),
            (Value::Null, Value::Null, Value::Null, Value::Null)
        );
    }

    #[test]
    fn a_live_review_is_the_agents_about_the_task_that_touched_the_file_else_the_one_held() {
        let live = review(Status::Live, &[]);
        for (path, task) in [
            (Some("src/a.rs"), "ATM-1"),
            (Some("docs/guide.md"), "ATM-2"),
            // a file changed in its copy: the task it holds now
            (Some("src/c.rs"), "ATM-1"),
            // the review as a whole: the task held
            (None, "ATM-1"),
            // a file neither touched nor changed: about no task, still the agent's
            (Some("README.md"), NO_TASK),
        ] {
            assert_eq!(about(&live, &by_hakan(path, "x")), task, "{path:?}");
        }
    }

    #[test]
    fn a_review_of_none_of_the_agents_tasks_isnt_its() {
        let theirs = review(Status::Waiting, &["ATM-3"]);
        assert_eq!(
            about(&theirs, &by_hakan(Some("src/a.rs"), "ATM-1")),
            NOT_MINE
        );
    }

    #[test]
    fn a_review_of_one_of_its_tasks_alone_is_all_about_that_task() {
        let one = review(Status::ChangesRequested, &["ATM-2"]);
        for path in [Some("anything.rs"), None] {
            assert_eq!(about(&one, &by_hakan(path, "x")), "ATM-2", "{path:?}");
        }
    }

    #[test]
    fn in_a_batch_review_a_comment_is_the_agents_by_file_by_task_named_or_by_its_own_chain() {
        let batch = review(Status::Waiting, &["ATM-1", "ATM-2", "ATM-3"]);
        for (path, body, expected) in [
            // a file its task touched
            (Some("docs/a.md"), "x", "ATM-2"),
            // a file changed in its copy, which no other named task claims: the task held
            (Some("src/c.rs"), "x", "ATM-1"),
            // a file another named task declares is that task's, even changed here
            (Some("src/b.rs"), "x", NOT_MINE),
            // its task named in the comment
            (None, "see ATM-2 again", "ATM-2"),
            (Some("other.rs"), "nothing of its", NOT_MINE),
        ] {
            assert_eq!(
                about(&batch, &by_hakan(path, body)),
                expected,
                "{path:?} {body}"
            );
        }
        // a chain it wrote in is its, about no task
        let own = chain(
            Some("other.rs"),
            vec![comment(
                "c1",
                "Hakan",
                "x",
                vec![comment("c2", "claude:me", "y", Vec::new())],
            )],
        );
        assert_eq!(about(&batch, &own), NO_TASK);
    }

    #[test]
    fn a_watch_lease_holds_until_its_time_and_a_missing_or_broken_one_never() {
        for (text, holds_now) in [
            (Some("1000"), true),
            (Some(" 1000\n"), true),
            (Some("999"), false),
            (Some("500"), false),
            (Some("soon"), false),
            (Some(""), false),
            (None, false),
        ] {
            assert_eq!(holds(text, 999), holds_now, "{text:?}");
        }
    }

    #[test]
    fn a_watch_writes_its_lease_while_it_runs_and_removes_it_when_it_ends() {
        let dir = std::env::temp_dir().join(format!("atm-lease-{}", std::process::id()));
        let file = dir.join("w.live");
        {
            let lease = Lease::take(file.clone());
            let now = atm_common::time::real_now();
            let text = std::fs::read_to_string(&file).unwrap();
            assert!(holds(Some(&text), now));
            assert!(
                !holds(Some(&text), now + LEASE_SECS + 1),
                "it runs out unrenewed"
            );
            lease.renew();
            assert!(file.exists());
        }
        assert!(!file.exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    fn header(n: u32, status: Status, author: &str, tasks: &[&str], updated: &str) -> ReviewHeader {
        let mut h = review(status, tasks).header;
        h.number = n;
        h.reference = ReviewRef(format!("CR-{n}"));
        h.author = Some(Actor::from(author));
        h.updated = updated.into();
        h
    }

    #[test]
    fn open_reviews_are_the_agents_own_or_naming_its_tasks_not_yet_decided_and_changed_ones_after_its_last_watch()
     {
        let at = "2026-11-19T05:00:00Z";
        let headers = [
            header(4, Status::Waiting, "claude:me", &[], "2026-11-19T05:30:00Z"),
            header(
                2,
                Status::ChangesRequested,
                "Hakan",
                &["ATM-1"],
                "2026-11-19T04:00:00Z",
            ),
            header(3, Status::Live, "claude:me", &[], at),
            header(
                5,
                Status::Waiting,
                "claude:other",
                &["ATM-9"],
                "2026-11-19T06:00:00Z",
            ),
            header(
                6,
                Status::Approved,
                "claude:me",
                &["ATM-1"],
                "2026-11-19T06:00:00Z",
            ),
            header(7, Status::Closed, "claude:me", &[], "2026-11-19T06:00:00Z"),
            header(8, Status::Draft, "claude:me", &[], "2026-11-19T06:00:00Z"),
        ];
        let mine: HashSet<String> = ["ATM-1".to_string()].into();
        let (open, changed) = open_and_changed(&headers, &mine, "claude:me", Some(at));
        assert_eq!(open, ["CR-2", "CR-3", "CR-4"]);
        // Changed after the last watch only: CR-3 at that very time was told
        assert_eq!(changed, ["CR-4"]);
        // Before any watch, nothing counts as changed since one
        let (_, none) = open_and_changed(&headers, &mine, "claude:me", None);
        assert_eq!(none, Vec::<String>::new());
        let (theirs, _) = open_and_changed(&headers, &HashSet::new(), "claude:other", Some(at));
        assert_eq!(theirs, ["CR-5"]);
    }

    #[test]
    fn the_reminder_is_said_only_without_a_watch_with_reviews_open_and_again_after_ten_minutes_or_a_change()
     {
        let state = |running: bool, open: &[&str], changed: &[&str]| WatchState {
            running,
            open: open.iter().map(|r| (*r).to_string()).collect(),
            since_last_watch: changed.iter().map(|r| (*r).to_string()).collect(),
        };
        let at = |at: i64, changed: &[&str]| Reminded {
            at,
            changed: changed.iter().map(|r| (*r).to_string()).collect(),
        };
        let now = 10_000;
        // A watch runs, or nothing of its is open: nothing said
        assert_eq!(reminder_for(&state(true, &["CR-1"], &[]), None, now), None);
        assert_eq!(reminder_for(&state(false, &[], &[]), None, now), None);
        // Open, never said: said, naming them
        let said = reminder_for(&state(false, &["CR-1", "CR-2"], &[]), None, now).unwrap();
        assert!(said.contains("(2 open: CR-1, CR-2)"), "{said}");
        assert!(said.contains("atm review watch --json"), "{said}");
        // Said a minute ago, nothing new: quiet; ten minutes on, said again
        let quiet = state(false, &["CR-1"], &["CR-1"]);
        assert_eq!(
            reminder_for(&quiet, Some(&at(now - 60, &["CR-1"])), now),
            None
        );
        assert!(reminder_for(&quiet, Some(&at(now - REMIND_EVERY_SECS, &["CR-1"])), now).is_some());
        // A review changed that the last one didn't name: said at once, naming it
        let new = reminder_for(
            &state(false, &["CR-1", "CR-2"], &["CR-2"]),
            Some(&at(now - 60, &[])),
            now,
        )
        .unwrap();
        assert!(new.contains("changed since your last watch: CR-2"), "{new}");
        // Many open: the first two and the last
        let many = reminder_for(
            &state(false, &["CR-1", "CR-2", "CR-3", "CR-9"], &[]),
            None,
            now,
        )
        .unwrap();
        assert!(many.contains("(4 open: CR-1, CR-2, … CR-9)"), "{many}");
    }
}
