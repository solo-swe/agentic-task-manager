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
                submodule: new.first.submodule,
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

    fn add_poke(
        &mut self,
        id: &ReviewId,
        anchor: Anchor,
        body: &str,
        by: &Actor,
    ) -> Result<CommentId> {
        self.edit(id, |review| {
            Ok(review.push_poke(anchor, body, by, &now_iso()))
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::MemoryStore;
    use crate::model::{Actor, Anchor, CommentId, Patch, ReviewId, RoundKind, Status};
    use crate::store::{NewReview, NewRound, ReviewStore};
    use crate::store_contract as contract;

    #[test]
    fn an_empty_store_lists_and_finds_nothing() {
        contract::an_empty_store_lists_and_finds_nothing(&mut MemoryStore::new());
    }

    #[test]
    fn a_new_review_is_kept_numbered_with_its_first_round() {
        contract::a_new_review_is_kept_numbered_with_its_first_round(&mut MemoryStore::new());
    }

    #[test]
    fn reviews_are_listed_newest_first_and_filtered() {
        contract::reviews_are_listed_newest_first_and_filtered(&mut MemoryStore::new());
    }

    #[test]
    fn a_reviews_header_and_rounds_are_changed_as_asked() {
        contract::a_reviews_header_and_rounds_are_changed_as_asked(&mut MemoryStore::new());
    }

    #[test]
    fn comments_are_kept_replied_to_edited_and_resolved() {
        contract::comments_are_kept_replied_to_edited_and_resolved(&mut MemoryStore::new());
    }

    #[test]
    fn every_method_on_a_review_it_hasnt_is_not_found() {
        contract::every_method_on_a_review_it_hasnt_is_not_found(&mut MemoryStore::new());
    }

    /// A write that fails tells no one; every subscription is told of one that succeeds, each until it is dropped.
    #[test]
    fn only_a_write_that_succeeds_tells_and_each_subscription_until_it_is_dropped() {
        let mut store = MemoryStore::new();
        let counter = || {
            let told = Arc::new(AtomicUsize::new(0));
            let count = told.clone();
            let on_change: atm_common::subscription::OnChange = Arc::new(move || {
                count.fetch_add(1, Ordering::Relaxed);
            });
            (told, on_change)
        };
        let (first, on_first) = counter();
        let (second, on_second) = counter();
        let keep = store.subscribe(on_first).unwrap();
        let dropped = store.subscribe(on_second).unwrap();
        let by = Actor::from("claude:A");
        let header = store
            .create_review(NewReview {
                title: "One".into(),
                summary: String::new(),
                tasks: Vec::new(),
                author: Some(by.clone()),
                reviewers: Vec::new(),
                status: Status::Waiting,
                first: NewRound {
                    kind: RoundKind::Files,
                    submodule: None,
                    files: Vec::new(),
                    patch: Patch::default(),
                    by: by.clone(),
                },
            })
            .unwrap();
        assert_eq!(
            (
                first.load(Ordering::Relaxed),
                second.load(Ordering::Relaxed)
            ),
            (1, 1)
        );
        drop(dropped);
        // Refused: no such review, no such comment
        let gone = ReviewId("gone".into());
        assert!(store.set_status(&gone, Status::Closed, &by).is_err());
        assert!(
            store
                .reply(&header.id, &CommentId("c9".into()), "x", &by)
                .is_err()
        );
        assert_eq!(first.load(Ordering::Relaxed), 1);
        store
            .add_comment(&header.id, Anchor::default(), "Fine.", &by)
            .unwrap();
        assert_eq!(
            (
                first.load(Ordering::Relaxed),
                second.load(Ordering::Relaxed)
            ),
            (2, 1)
        );
        drop(keep);
    }
}
