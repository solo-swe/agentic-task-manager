//! What a review waits on whom for (ATM-606): the list's "needs you", and every project's summary.

use super::{Actor, Project, Review, ReviewHeader, Status, Value, json, reviews};

/// Whether `header` waits on `me`: open or live, with `me` one of its reviewers (anyone but its author when it names
/// none); or sent back, with `me` its author.
#[must_use]
pub fn needs(header: &ReviewHeader, me: &Actor) -> bool {
    let authored = header.author.as_ref() == Some(me);
    match header.status {
        Status::Waiting | Status::Live => {
            if header.reviewers.is_empty() {
                !authored
            } else {
                header.reviewers.contains(me)
            }
        }
        Status::ChangesRequested => authored,
        Status::Draft | Status::Approved | Status::Closed => false,
    }
}

/// What a review waits on `me` for (ATM-606), if anything: `"review"`, its decision or round is theirs ([`needs`]);
/// `"answered"`, someone else wrote in it after `me` last did, in a review `me` takes part in (its author, a reviewer
/// it names, or one who wrote in it) that isn't decided yet. Nothing is stored of what `me` has seen: a GET writes
/// nothing (security I11), and an answer is cleared by answering, or by the review being decided.
#[must_use]
pub fn waiting_for(review: &Review, me: &Actor) -> Option<&'static str> {
    let header = &review.header;
    if needs(header, me) {
        return Some("review");
    }
    // Decided, or a draft its author is still putting together (§3.1: waiting on no one)
    if matches!(
        header.status,
        Status::Approved | Status::Closed | Status::Draft
    ) {
        return None;
    }
    // Which came last, by the comments' numbers (c1, c2, … in the order they were written; two in one second have the
    // same time)
    let number = |id: &str| id.trim_start_matches('c').parse::<u64>().unwrap_or(0);
    let (mut mine, mut others): (Option<u64>, Option<u64>) = (None, None);
    for comment in review.comments() {
        let latest = if &comment.author == me {
            &mut mine
        } else {
            &mut others
        };
        *latest = (*latest).max(Some(number(&comment.id.0)));
    }
    let takes_part =
        mine.is_some() || header.author.as_ref() == Some(me) || header.reviewers.contains(me);
    match (others, mine) {
        (Some(_), None) if takes_part => Some("answered"),
        (Some(theirs), Some(own)) if theirs > own => Some("answered"),
        _ => None,
    }
}

/// The reviews of the project's own store that wait on `me` (ATM-606), as [`waiting_for`] says: `(header, why)`.
/// Its working copies' reviews aren't read: this is for every project's summary, on each refresh. A store that can't
/// be read has none.
#[must_use]
pub fn waiting_reviews(project: &Project) -> Vec<(ReviewHeader, &'static str)> {
    let (Some(root), Ok(store)) = (project.root(), reviews::store(project)) else {
        return Vec::new();
    };
    // The user at the machine, whatever environment the summary is made in (an agent's shell started the server, say)
    let me = Actor(crate::actor::user_identity(root));
    let mut waiting: Vec<(ReviewHeader, &'static str)> = store
        .reviews()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|r| waiting_for(&r, &me).map(|why| (r.header, why)))
        .collect();
    // The longest waiting first
    waiting.sort_by(|a, b| a.0.updated.cmp(&b.0.updated));
    waiting
}

/// What changes when any of the project's reviews does ([`FileStore::stamp`]), for a summary's cache: `null` without
/// a review folder.
#[must_use]
pub fn reviews_stamp(project: &Project) -> Value {
    reviews::store(project).map_or(Value::Null, |s| {
        let (files, latest) = s.stamp();
        json!([files, latest.to_string()])
    })
}
