//! A review store in memory, for tests: everything a store keeps, nothing kept past it. Numbers go up from 1, and a
//! write tells its subscriptions (as atm-db's memory store does).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use atm_common::subscription::{OnChange, Subscription};
use atm_common::time::now_iso;
use atm_common::{Error, Result};

use crate::model::{
    Actor, Anchor, Chain, Comment, CommentId, Patch, Review, ReviewHeader, ReviewId, ReviewRef,
    Round, Status,
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

/// Tell `listeners` of a change.
pub(crate) fn tell(listeners: &Listeners) {
    let listeners = listeners.lock().map(|l| l.clone()).unwrap_or_default();
    for (_, on_change) in listeners {
        on_change();
    }
}

impl MemoryStore {
    /// An empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn by_id(&self, id: &ReviewId) -> Result<&Review> {
        self.reviews
            .iter()
            .find(|r| &r.header.id == id)
            .ok_or_else(|| Error::NotFound(format!("no review {}", id.0)))
    }

    /// Apply `edit` to a review, mark it updated and tell the subscriptions.
    fn edit<T>(&mut self, id: &ReviewId, edit: impl FnOnce(&mut Review) -> Result<T>) -> Result<T> {
        let review = self
            .reviews
            .iter_mut()
            .find(|r| &r.header.id == id)
            .ok_or_else(|| Error::NotFound(format!("no review {}", id.0)))?;
        let out = edit(review)?;
        review.header.updated = now_iso();
        tell(&self.listeners);
        Ok(out)
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
        tell(&self.listeners);
        Ok(header)
    }

    fn set_status(&mut self, id: &ReviewId, status: Status, _by: &Actor) -> Result<()> {
        self.edit(id, |review| {
            review.header.status = status;
            Ok(())
        })
    }

    fn edit_summary(&mut self, id: &ReviewId, title: &str, summary: &str) -> Result<()> {
        self.edit(id, |review| {
            title.clone_into(&mut review.header.title);
            summary.clone_into(&mut review.summary);
            Ok(())
        })
    }

    fn set_tasks(&mut self, id: &ReviewId, tasks: Vec<String>) -> Result<()> {
        self.edit(id, |review| {
            review.header.tasks = tasks;
            Ok(())
        })
    }

    fn set_author(&mut self, id: &ReviewId, author: &Actor) -> Result<()> {
        self.edit(id, |review| {
            review.header.author = Some(author.clone());
            Ok(())
        })
    }

    fn add_round(&mut self, id: &ReviewId, round: NewRound) -> Result<u32> {
        let number = self.edit(id, |review| Ok(review.push_round(&round, &now_iso())))?;
        self.patches.insert((id.clone(), number), round.patch);
        Ok(number)
    }

    fn round_diff(&self, id: &ReviewId, round: u32) -> Result<Patch> {
        self.patches
            .get(&(id.clone(), round))
            .cloned()
            .ok_or_else(|| Error::NotFound(format!("no round {round}")))
    }

    fn list_comments(&self, id: &ReviewId, filter: &CommentFilter) -> Result<Vec<Chain>> {
        Ok(self.by_id(id)?.chains_by(filter))
    }

    fn get_comment(&self, id: &ReviewId, comment: &CommentId) -> Result<Comment> {
        self.by_id(id)?.comment_owned(comment)
    }

    fn add_comment(
        &mut self,
        id: &ReviewId,
        anchor: Anchor,
        body: &str,
        by: &Actor,
    ) -> Result<CommentId> {
        self.edit(id, |review| {
            Ok(review.push_chain(anchor, body, by, &now_iso()))
        })
    }

    fn reply(
        &mut self,
        id: &ReviewId,
        to: &CommentId,
        body: &str,
        by: &Actor,
    ) -> Result<CommentId> {
        self.edit(id, |review| review.push_reply(to, body, by, &now_iso()))
    }

    fn edit_comment(&mut self, id: &ReviewId, comment: &CommentId, body: &str) -> Result<()> {
        self.edit(id, |review| review.set_body(comment, body))
    }

    fn resolve(&mut self, id: &ReviewId, top_level: &CommentId, resolved: bool) -> Result<()> {
        self.edit(id, |review| review.set_resolved(top_level, resolved))
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
