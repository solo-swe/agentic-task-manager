//! The agent's one watch (§3.9): each person's comment and decision about its work, as it comes.

use super::{
    Duration, FileStore, HashMap, HashSet, Instant, Lease, Mine, Path, Project, RECHECK, Result,
    Review, ReviewHeader, Status, Value, about_my_work, all_reviews, cursor_file, decided,
    decision_about_my_work, heard, inbox_file, is_agent, json, keyed_now, lease_file, listen,
    old_seen_key, parse_iso, reviews, save_cursor, seen_key, starting_point, with_parents, work_of,
};

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
        let news = look(
            project,
            root.as_deref(),
            me,
            &store,
            None,
            &mut seen,
            &mut statuses,
        )?;
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
