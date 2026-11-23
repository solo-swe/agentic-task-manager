//! `atm review wait`: until a reviewer submits on a review, or decides it.

use super::{
    Arc, Chain, Comment, Duration, FileStore, HashSet, Instant, Project, RECHECK, Receiver, Result,
    ReviewHeader, ReviewRef, ReviewStore, Status, Value, channel, json, reviews,
};

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
pub(super) fn heard(
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
pub(super) fn with_parents(chain: &Chain) -> Vec<(&Comment, Option<&Comment>)> {
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

pub(super) fn decided(status: Status) -> bool {
    status.is_decision()
}

/// Be told of the store's changes on the returned channel, while the subscription (the first value) is kept.
pub(super) fn listen(
    store: &FileStore,
) -> Result<(atm_common::subscription::Subscription, Receiver<()>)> {
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
