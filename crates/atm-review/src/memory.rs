//! A review store in memory, for tests: everything a store keeps, nothing kept past it. Numbers go up from 1, and a
//! write tells its subscriptions (as atm-db's memory store does).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use atm_common::subscription::{OnChange, Subscription};
use atm_common::time::now_iso;
use atm_common::{Error, Result};

use crate::model::{
    Actor, Anchor, Chain, ChainId, Comment, CommentId, Patch, Review, ReviewHeader, ReviewId,
    ReviewRef, Round, Status,
};
use crate::store::{CommentFilter, NewReview, NewRound, ReviewFilter, ReviewStore};

type Listeners = Arc<Mutex<Vec<(u64, OnChange)>>>;

/// Reviews in memory.
#[derive(Default)]
pub struct MemoryStore {
    reviews: Vec<Review>,
    patches: HashMap<(ReviewId, u32), Patch>,
    listeners: Listeners,
    next_listener: AtomicU64,
}

/// Removes its listener when the subscription is dropped.
struct Unlisten(Listeners, u64);

impl Drop for Unlisten {
    fn drop(&mut self) {
        if let Ok(mut listeners) = self.0.lock() {
            listeners.retain(|(id, _)| *id != self.1);
        }
    }
}

/// The number in an ID like `c9` or `t4`.
fn ordinal(id: &str) -> u32 {
    id[1..].parse().unwrap_or(0)
}

fn reply_into(comments: &mut [Comment], to: &CommentId, reply: &Comment) -> bool {
    for comment in comments {
        if &comment.id == to {
            comment.replies.push(reply.clone());
            return true;
        }
        if reply_into(&mut comment.replies, to, reply) {
            return true;
        }
    }
    false
}

fn find_mut<'a>(comments: &'a mut [Comment], id: &CommentId) -> Option<&'a mut Comment> {
    for comment in comments {
        if &comment.id == id {
            return Some(comment);
        }
        if let Some(found) = find_mut(&mut comment.replies, id) {
            return Some(found);
        }
    }
    None
}

impl MemoryStore {
    /// An empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn tell(&self) {
        let listeners = self.listeners.lock().map(|l| l.clone()).unwrap_or_default();
        for (_, on_change) in listeners {
            on_change();
        }
    }

    fn review_mut(&mut self, id: &ReviewId) -> Result<&mut Review> {
        self.reviews
            .iter_mut()
            .find(|r| &r.header.id == id)
            .ok_or_else(|| Error::NotFound(format!("no review {}", id.0)))
    }

    fn by_id(&self, id: &ReviewId) -> Result<&Review> {
        self.reviews
            .iter()
            .find(|r| &r.header.id == id)
            .ok_or_else(|| Error::NotFound(format!("no review {}", id.0)))
    }

    /// The next comment ID in a review.
    fn next_comment(review: &Review) -> CommentId {
        let last = review
            .comments()
            .map(|c| ordinal(&c.id.0))
            .max()
            .unwrap_or(0);
        CommentId(format!("c{}", last + 1))
    }

    fn changed(&mut self, id: &ReviewId) -> Result<()> {
        self.review_mut(id)?.header.updated = now_iso();
        self.tell();
        Ok(())
    }
}

impl ReviewStore for MemoryStore {
    fn list_reviews(&self, filter: &ReviewFilter) -> Result<Vec<ReviewHeader>> {
        let mut headers: Vec<ReviewHeader> = self
            .reviews
            .iter()
            .map(|r| r.header.clone())
            .filter(|h| filter.statuses.is_empty() || filter.statuses.contains(&h.status))
            .filter(|h| filter.task.as_ref().is_none_or(|t| h.tasks.contains(t)))
            .collect();
        headers.reverse();
        Ok(headers)
    }

    fn get_review(&self, reference: &ReviewRef) -> Result<Review> {
        self.reviews
            .iter()
            .find(|r| r.header.reference == *reference || r.header.id.0 == reference.0)
            .cloned()
            .ok_or_else(|| Error::NotFound(format!("no review {reference}")))
    }

    fn create_review(&mut self, new: NewReview) -> Result<ReviewHeader> {
        let number = self
            .reviews
            .iter()
            .map(|r| r.header.number)
            .max()
            .unwrap_or(0)
            + 1;
        let now = now_iso();
        let header = ReviewHeader {
            id: ReviewId::new(),
            reference: ReviewRef::numbered(number),
            number,
            title: new.title,
            status: new.status,
            tasks: new.tasks,
            author: new.author,
            reviewers: new.reviewers,
            created: now.clone(),
            updated: now.clone(),
        };
        self.patches.insert((header.id.clone(), 1), new.first.patch);
        self.reviews.push(Review {
            header: header.clone(),
            summary: new.summary,
            chains: Vec::new(),
            rounds: vec![Round {
                number: 1,
                kind: new.first.kind,
                files: new.first.files,
                by: new.first.by,
                at: now,
            }],
        });
        self.tell();
        Ok(header)
    }

    fn set_status(&mut self, id: &ReviewId, status: Status, _by: &Actor) -> Result<()> {
        self.review_mut(id)?.header.status = status;
        self.changed(id)
    }

    fn edit_summary(&mut self, id: &ReviewId, title: &str, summary: &str) -> Result<()> {
        let review = self.review_mut(id)?;
        title.clone_into(&mut review.header.title);
        summary.clone_into(&mut review.summary);
        self.changed(id)
    }

    fn set_tasks(&mut self, id: &ReviewId, tasks: Vec<String>) -> Result<()> {
        self.review_mut(id)?.header.tasks = tasks;
        self.changed(id)
    }

    fn add_round(&mut self, id: &ReviewId, round: NewRound) -> Result<u32> {
        let review = self.review_mut(id)?;
        let number = review.rounds.last().map_or(1, |r| r.number + 1);
        review.rounds.push(Round {
            number,
            kind: round.kind,
            files: round.files,
            by: round.by,
            at: now_iso(),
        });
        self.patches.insert((id.clone(), number), round.patch);
        self.changed(id)?;
        Ok(number)
    }

    fn round_diff(&self, id: &ReviewId, round: u32) -> Result<Patch> {
        self.patches
            .get(&(id.clone(), round))
            .cloned()
            .ok_or_else(|| Error::NotFound(format!("no round {round}")))
    }

    fn list_comments(&self, id: &ReviewId, filter: &CommentFilter) -> Result<Vec<Chain>> {
        Ok(self
            .by_id(id)?
            .chains
            .iter()
            .filter(|c| !filter.open_only || c.comments.iter().any(|t| t.resolved == Some(false)))
            .cloned()
            .collect())
    }

    fn get_comment(&self, id: &ReviewId, comment: &CommentId) -> Result<Comment> {
        self.by_id(id)?
            .comment(comment)
            .cloned()
            .ok_or_else(|| Error::NotFound(format!("no comment {comment}")))
    }

    fn add_comment(
        &mut self,
        id: &ReviewId,
        anchor: Anchor,
        body: &str,
        by: &Actor,
    ) -> Result<CommentId> {
        let review = self.review_mut(id)?;
        let comment_id = Self::next_comment(review);
        let chain = review
            .chains
            .iter()
            .map(|c| ordinal(&c.id.0))
            .max()
            .unwrap_or(0)
            + 1;
        review.chains.push(Chain {
            id: ChainId(format!("t{chain}")),
            anchor,
            comments: vec![Comment {
                id: comment_id.clone(),
                author: by.clone(),
                at: now_iso(),
                body: body.to_string(),
                resolved: Some(false),
                replies: Vec::new(),
            }],
        });
        self.changed(id)?;
        Ok(comment_id)
    }

    fn reply(
        &mut self,
        id: &ReviewId,
        to: &CommentId,
        body: &str,
        by: &Actor,
    ) -> Result<CommentId> {
        let review = self.review_mut(id)?;
        let reply = Comment {
            id: Self::next_comment(review),
            author: by.clone(),
            at: now_iso(),
            body: body.to_string(),
            resolved: None,
            replies: Vec::new(),
        };
        let placed = review
            .chains
            .iter_mut()
            .any(|chain| reply_into(&mut chain.comments, to, &reply));
        if !placed {
            return Err(Error::NotFound(format!("no comment {to}")));
        }
        self.changed(id)?;
        Ok(reply.id)
    }

    fn edit_comment(&mut self, id: &ReviewId, comment: &CommentId, body: &str) -> Result<()> {
        let review = self.review_mut(id)?;
        let found = review
            .chains
            .iter_mut()
            .find_map(|chain| find_mut(&mut chain.comments, comment))
            .ok_or_else(|| Error::NotFound(format!("no comment {comment}")))?;
        body.clone_into(&mut found.body);
        self.changed(id)
    }

    fn resolve(&mut self, id: &ReviewId, top_level: &CommentId, resolved: bool) -> Result<()> {
        let review = self.review_mut(id)?;
        let found = review
            .chains
            .iter_mut()
            .flat_map(|chain| chain.comments.iter_mut())
            .find(|c| &c.id == top_level)
            .ok_or_else(|| Error::NotFound(format!("no top-level comment {top_level}")))?;
        found.resolved = Some(resolved);
        self.changed(id)
    }

    fn subscribe(&self, on_change: OnChange) -> Result<Subscription> {
        let id = self.next_listener.fetch_add(1, Ordering::Relaxed);
        self.listeners
            .lock()
            .map_err(|_| Error::failed("the review store's listeners are poisoned"))?
            .push((id, on_change));
        Ok(Subscription::new(Unlisten(self.listeners.clone(), id)))
    }
}
