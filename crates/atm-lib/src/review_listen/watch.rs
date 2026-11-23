//! The agent's one watch (§3.9): each person's comment and decision about its work, as it comes.

use std::sync::mpsc::Sender;

use atm_db::subscription::{Keep, OnChange, Subscription, watch_tree_where};

use super::{
    Arc, Duration, FileStore, HashMap, HashSet, Instant, Lease, Mine, Path, PathBuf, Project,
    RECHECK, Receiver, Result, Review, ReviewFilter, ReviewHeader, ReviewStore, Status, Value,
    about_my_work, all_reviews, channel, cursor_file, decided, decision_about_my_work, heard,
    inbox_file, is_agent, json, keyed_now, lease_file, old_seen_key, parse_iso, reviews,
    save_cursor, seen_key, starting_point, with_parents, work_of,
};

/// What a look reads, each watched for changes (ATM-727: a watch looks only when told, never on a timer; a look reads
/// every review, the tasks and git's working copy):
/// - the reviews, every comment and decision;
/// - the task file: a task the agent takes or finishes changes which comments are its work (ATM-663, R5);
/// - the working copy, while a live review is open: a file the agent edits makes a comment on it the agent's. Without
///   one, nothing there matters to a watch, and building in it (`target/`) would wake it for nothing.
struct Sources {
    told: Receiver<()>,
    sent: Sender<()>,
    _reviews: Subscription,
    _tasks: Option<Subscription>,
    working_copy: Option<Subscription>,
}

impl Sources {
    fn notice(sent: &Sender<()>) -> OnChange {
        let sent = sent.clone();
        Arc::new(move || {
            let _ = sent.send(());
        })
    }

    fn of(store: &FileStore, root: Option<&Path>) -> Result<Self> {
        let (sent, told) = channel();
        let reviews = store.subscribe(Self::notice(&sent))?;
        // The task file alone: atm's lock files come and go beside it on every command
        let tasks = root
            .map(|r| r.join(atm_common::TASK_DIR))
            .filter(|dir| dir.is_dir())
            .map(|dir| {
                let keep: Keep = Arc::new(|paths: &[PathBuf]| {
                    paths
                        .iter()
                        .any(|p| p.file_name().is_some_and(|n| n == atm_common::TASKS_FILE))
                });
                watch_tree_where(&[dir], keep, Self::notice(&sent))
            })
            .transpose()?;
        Ok(Self {
            told,
            sent,
            _reviews: reviews,
            _tasks: tasks,
            working_copy: None,
        })
    }

    /// The working copy watched while a live review is open, not otherwise (a project outside git has none to watch).
    fn follow_working_copy(&mut self, project: &Project, store: &FileStore) {
        let live = ReviewFilter {
            statuses: vec![Status::Live],
            task: None,
        };
        let any_live = store.list_reviews(&live).is_ok_and(|l| !l.is_empty());
        if !any_live {
            self.working_copy = None;
        } else if self.working_copy.is_none() {
            self.working_copy =
                reviews::subscribe_working_copy(project, Self::notice(&self.sent)).ok();
        }
    }
}

/// The process that started this one, while it runs (ATM-727): once the shell that ran a watch is gone, the watch is
/// another process's (the system's), and nothing reads what it prints. 0 where it can't be known (Windows): it never
/// changes there.
fn parent() -> u32 {
    #[cfg(unix)]
    {
        std::os::unix::process::parent_id()
    }
    #[cfg(not(unix))]
    {
        0
    }
}

/// What a watch is asked for, beyond its time (ATM-624).
#[derive(Debug, Clone, Default)]
pub struct WatchOptions {
    /// Stop once something was told (what it caught up on counts).
    pub once: bool,
    /// Tell what came since this time (ISO 8601 UTC), whatever the last watch told.
    pub since: Option<String>,
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
    let file = cursor_file(root.as_deref(), me);
    let (mut seen, mut statuses) = starting_point(&store, &file, options.since.as_deref())?;
    let mut sources = Sources::of(&store, root.as_deref())?;
    // Known to be running while it does (ATM-642): renewed on every wake, removed when it ends
    let lease = Lease::take(lease_file(root.as_deref(), me));
    ready();
    let deadline = Instant::now() + timeout;
    let started_by = parent();
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
            // Woken by a change, or by its timer only to see whether its time is up or its shell gone
            let notified = sources.told.recv_timeout(left.min(RECHECK)).is_ok();
            // Its shell gone (stopped by its harness): no one reads it any more (ATM-727)
            if parent() != started_by {
                return Ok(count);
            }
            if !notified {
                continue;
            }
        }
        first = false;
        while sources.told.try_recv().is_ok() {}
        let news = look(
            project,
            root.as_deref(),
            me,
            &store,
            None,
            &mut seen,
            &mut statuses,
        )?;
        // A live review opened or ended meanwhile: the working copy watched as long as one is open
        sources.follow_working_copy(project, &store);
        let told_now = news.len();
        for heard in news {
            emit(heard);
        }
        count += told_now;
        save_cursor(&file, &mut seen, &mut statuses);
        if options.once && told_now > 0 {
            // A watch with --once ends to be read: the harness tells the agent when it does (Claude Code tells of a
            // background command only then), so what it told is read, and isn't unread in the inbox (ATM-708)
            save_cursor(&inbox_file(root.as_deref(), me), &mut seen, &mut statuses);
            return Ok(count);
        }
    }
}

/// One look at the reviews from where `seen` and `statuses` say the agent is: each person's comment about its work
/// not told yet, in the order they were written, then each decision on its work, after the words that came with it.
/// `seen` and `statuses` move past what it tells and what it passes over for good. With `changed_since` (ISO 8601),
/// only the reviews changed since then are read (a comment or a decision changes its review): the inbox's look on
/// every command, cheap while nothing changed.
pub(super) fn look(
    project: &Project,
    root: Option<&Path>,
    me: &str,
    store: &FileStore,
    changed_since: Option<&str>,
    seen: &mut HashSet<String>,
    statuses: &mut HashMap<String, Status>,
) -> Result<Vec<Value>> {
    let reviews: Vec<Review> = all_reviews(store)?
        .into_iter()
        .filter(|r| changed_since.is_none_or(|since| r.header.updated.as_str() >= since))
        .collect();
    if reviews.is_empty() {
        return Ok(Vec::new());
    }
    // The tasks as they are now: the agent may have taken another meanwhile
    let fresh = root.and_then(|r| Project::open(r).ok());
    let now = fresh.as_ref().unwrap_or(project);
    let work = work_of(now, me);
    // Told in the order they were written, oldest first, whichever review they are in
    let mut news: Vec<(String, u32, Value)> = Vec::new();
    for review in &reviews {
        let mut review = review.clone();
        keyed_now(now, store, &mut review.header);
        for chain in &review.chains {
            for (comment, parent) in with_parents(chain) {
                let key = seen_key(&review.header, comment);
                if seen.contains(&key) || seen.contains(&old_seen_key(&review.header, comment)) {
                    continue;
                }
                // Passed over for good: its own, an agent's, or in a review that has ended
                if comment.author.0 == me
                    || is_agent(Some(&comment.author.0))
                    || review.header.status.ended()
                {
                    seen.insert(key);
                    continue;
                }
                // Told once it is about the agent's work; until then judged again on each look (ATM-663, R5): the
                // agent may commit to its file, or start the task it is about, later
                if let Mine::About(task) = about_my_work(&work, me, &review, chain, comment) {
                    seen.insert(key);
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
    let mut told: Vec<Value> = news.into_iter().map(|(_, _, heard)| heard).collect();
    for review in &reviews {
        let mut header = review.header.clone();
        keyed_now(now, store, &mut header);
        let status = header.status;
        let id = header.id.0.clone();
        if statuses.get(&id) == Some(&status) {
            continue;
        }
        if !decided(status) {
            statuses.insert(id, status);
            continue;
        }
        // A decision is kept as seen once told; one not about the agent's work yet is judged again (R5)
        if let Mine::About(task) = decision_about_my_work(&work, me, &header) {
            statuses.insert(id, status);
            told.push(decision(&header, task.as_deref()));
        }
    }
    Ok(told)
}

/// A decision heard, as the watch prints it: `decision` the review's new status (`approved`, `changes_requested`,
/// `closed`); its words, if it had some, came as a comment just before.
pub(super) fn decision(review: &ReviewHeader, task: Option<&str>) -> Value {
    json!({
        "review": review.reference.0,
        "task": task,
        "decision": review.status.name(),
        "at": review.updated,
        "title": review.title,
    })
}
