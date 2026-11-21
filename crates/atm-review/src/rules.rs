//! Who may do what to a review, and when (docs/code-review-design.md §3.2's state diagram, §3.4's resolving), above any
//! [`ReviewStore`], so every store keeps the same rules. A review's decision (approve, request changes) is a
//! reviewer's: for an agent's review, the user's (I23); agents comment and reply.

use atm_common::{Error, Result};

use crate::model::{Actor, Anchor, CommentId, FileStatus, Review, ReviewHeader, ReviewRef, Status};
use crate::store::{NewReview, NewRound, ReviewStore};

/// What a reviewer submits (§3.11's *Submit*).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Approved: every top-level comment must be resolved.
    Approve,
    /// Sent back for fixes.
    RequestChanges,
    /// Only a comment on the review as a whole; the status stays.
    Comment,
}

/// A project's reviews, through a store, with the rules kept.
pub struct Reviews<'a, S: ReviewStore + ?Sized> {
    store: &'a mut S,
}

fn refused(message: impl Into<String>) -> Error {
    Error::Refused(message.into())
}

fn not_now(review: &Review, what: &str) -> Error {
    Error::failed(format!(
        "{} is {}: it can't {what}",
        review.header.reference,
        review.header.status.name().replace('_', " ")
    ))
}

/// Whether `who` is whose change `header`'s review is (none is, while that isn't known).
#[must_use]
pub fn is_author(header: &ReviewHeader, who: &Actor) -> bool {
    header.author.as_ref() == Some(who)
}

/// Whether `who` decides `header`'s review: one of its reviewers; with none named, anyone but its author.
#[must_use]
pub fn decides(header: &ReviewHeader, who: &Actor) -> bool {
    if header.reviewers.is_empty() {
        !is_author(header, who)
    } else {
        header.reviewers.contains(who)
    }
}

impl<'a, S: ReviewStore + ?Sized> Reviews<'a, S> {
    /// The reviews in `store`.
    pub fn new(store: &'a mut S) -> Self {
        Self { store }
    }

    /// The store, for reads.
    #[must_use]
    pub fn store(&self) -> &S {
        self.store
    }

    fn open(&self, reference: &ReviewRef) -> Result<Review> {
        self.store.get_review(reference)
    }

    fn not_ended(&self, reference: &ReviewRef, what: &str) -> Result<Review> {
        let review = self.open(reference)?;
        if review.header.status.ended() {
            return Err(not_now(&review, what));
        }
        Ok(review)
    }

    /// Open a review: as a draft, open (waiting on its reviewers) or live (§3.5).
    ///
    /// # Errors
    /// Another status, an empty title, or the store's.
    pub fn create(&mut self, new: NewReview) -> Result<ReviewHeader> {
        if !matches!(new.status, Status::Draft | Status::Waiting | Status::Live) {
            return Err(Error::Invalid(format!(
                "a review starts as a draft, open or live, not {}",
                new.status.name()
            )));
        }
        if new.title.trim().is_empty() {
            return Err(Error::Invalid("a review needs a title".into()));
        }
        self.store.create_review(new)
    }

    /// A draft goes to its reviewers: open. The author's.
    ///
    /// # Errors
    /// Not a draft, not its author, or the store's.
    pub fn ask_for_review(&mut self, reference: &ReviewRef, by: &Actor) -> Result<()> {
        let review = self.open(reference)?;
        let next = review_next(&review, &ReviewEvent::AskForReview, by)?;
        self.apply(&review, &next, by)
    }

    /// What `next` says of the review's status, written.
    fn apply(&mut self, review: &Review, next: &Next, by: &Actor) -> Result<()> {
        match next.status {
            Some(status) => self.store.set_status(&review.header.id, status, by),
            None => Ok(()),
        }
    }

    /// The author's next round, after addressing the comments: a review sent back is open again, and a live one ends
    /// as an ordinary open review (§3.5). Its number. A review whose author isn't known yet (a live review a person
    /// opened) takes the round's maker as its author, unless they are one of its reviewers.
    ///
    /// # Errors
    /// An ended review, not its author (or a reviewer, while there is none), or the store's.
    pub fn add_round(&mut self, reference: &ReviewRef, round: NewRound) -> Result<u32> {
        let review = self.open(reference)?;
        let by = round.by.clone();
        let next = review_next(&review, &ReviewEvent::Round, &by)?;
        if let Some(author) = &next.author {
            self.store.set_author(&review.header.id, author)?;
        }
        let number = self.store.add_round(&review.header.id, round)?;
        self.apply(&review, &next, &by)?;
        Ok(number)
    }

    /// A live review's checkpoint (§3.5): a round of the change as it is, so a comment is anchored to what the
    /// reviewer saw; the review stays live. Anyone taking part makes one, with their comment.
    ///
    /// # Errors
    /// Not live, or the store's.
    pub fn checkpoint(&mut self, reference: &ReviewRef, round: NewRound) -> Result<u32> {
        let review = self.open(reference)?;
        review_next(&review, &ReviewEvent::Checkpoint, &round.by)?;
        self.store.add_round(&review.header.id, round)
    }

    /// Start a chain with a comment, open. Anyone.
    ///
    /// # Errors
    /// An ended review, an anchor on a round it hasn't, an empty comment, or the store's.
    pub fn comment(
        &mut self,
        reference: &ReviewRef,
        anchor: Anchor,
        body: &str,
        by: &Actor,
    ) -> Result<CommentId> {
        let review = self.not_ended(reference, "be commented on")?;
        let Some(round) = review.rounds.iter().find(|r| r.number == anchor.round) else {
            return Err(Error::Invalid(format!(
                "{} has no round {}",
                review.header.reference, anchor.round
            )));
        };
        // Lines run forwards from line 1 (ATM-670, audit R4: (5, 3) panicked the page that placed the comment)
        if let Some((first, last)) = anchor.lines
            && (first == 0 || last < first)
        {
            return Err(Error::Invalid(format!(
                "a comment's lines run from the first to the last, from line 1: not {first}-{last}"
            )));
        }
        // A path is one of the round's files, and has no control character: comments.md's heading carries it, so a
        // newline in it could add a heading or a comment of its own (ATM-658, I31)
        if let Some(path) = &anchor.path {
            if path.chars().any(char::is_control) {
                return Err(Error::Invalid(
                    "a comment's file can't have a control character (a newline, a tab, …) in its path".into(),
                ));
            }
            // A renamed file's old path is the round's too (its old side)
            let in_round = round.files.iter().any(|f| {
                &f.path == path || matches!(&f.status, FileStatus::Renamed { from } if from == path)
            });
            if !in_round {
                return Err(Error::Invalid(format!(
                    "{} isn't one of round {}'s files in {}",
                    path, anchor.round, review.header.reference
                )));
            }
        }
        if body.trim().is_empty() {
            return Err(Error::Invalid("a comment needs some text".into()));
        }
        self.store.add_comment(&review.header.id, anchor, body, by)
    }

    /// Poke the review (ATM-611): a nudge to its agent, on the review as a whole (its latest round, no file), with an
    /// optional message; never open, so it never stands in the way of an approval. Who may poke is atm-lib's (a person).
    ///
    /// # Errors
    /// An ended review (approved or closed: there is nothing to nudge), or the store's.
    pub fn poke(&mut self, reference: &ReviewRef, body: &str, by: &Actor) -> Result<CommentId> {
        let review = self.not_ended(reference, "be poked")?;
        let round = review.rounds.last().map_or(1, |r| r.number);
        let anchor = Anchor {
            round,
            ..Anchor::default()
        };
        self.store.add_poke(&review.header.id, anchor, body, by)
    }

    /// Reply to a comment. Anyone.
    ///
    /// # Errors
    /// An ended review, an empty reply, or the store's.
    pub fn reply(
        &mut self,
        reference: &ReviewRef,
        to: &CommentId,
        body: &str,
        by: &Actor,
    ) -> Result<CommentId> {
        let review = self.not_ended(reference, "be replied to")?;
        if body.trim().is_empty() {
            return Err(Error::Invalid("a reply needs some text".into()));
        }
        self.store.reply(&review.header.id, to, body, by)
    }

    /// Change one's own comment.
    ///
    /// # Errors
    /// An ended review, someone else's comment, or the store's.
    pub fn edit(
        &mut self,
        reference: &ReviewRef,
        comment: &CommentId,
        body: &str,
        by: &Actor,
    ) -> Result<()> {
        let review = self.not_ended(reference, "be edited")?;
        let found = self.store.get_comment(&review.header.id, comment)?;
        if &found.author != by {
            return Err(refused(format!(
                "{comment} is {}'s to edit",
                found.author.0
            )));
        }
        self.store.edit_comment(&review.header.id, comment, body)
    }

    /// Resolve (or reopen) a top-level comment, which covers its replies (§3.4): a reviewer's, or its own author's.
    ///
    /// # Errors
    /// An ended review, a reply (resolve its top-level comment), someone who may not, or the store's.
    pub fn resolve(
        &mut self,
        reference: &ReviewRef,
        comment: &CommentId,
        resolved: bool,
        by: &Actor,
    ) -> Result<()> {
        let review = self.not_ended(reference, "have its comments resolved")?;
        let found = self.store.get_comment(&review.header.id, comment)?;
        if found.resolved.is_none() {
            let top = review
                .top_level_of(comment)
                .map_or_else(String::new, |t| format!(" ({})", t.id));
            return Err(Error::Invalid(format!(
                "{comment} is a reply: resolve the comment it belongs to{top}"
            )));
        }
        if &found.author != by && !decides(&review.header, by) {
            return Err(refused(format!(
                "{comment} is resolved by a reviewer of {} or by the comment's author",
                review.header.reference
            )));
        }
        self.store.resolve(&review.header.id, comment, resolved)
    }

    /// A reviewer submits: approve (only when every top-level comment is resolved), request changes, or a comment on
    /// the review as a whole. A body becomes a chain about the whole review (§3.4's `(the review)`).
    ///
    /// `overriding`, a person's explicit override of who may decide, with why (feedback received elsewhere and entered
    /// by them, say): it lets someone decide who otherwise may not, the review's author among them, and is recorded
    /// in the review, a resolved chain about the whole review naming who and why. It lifts that rule only: an
    /// approval still needs every top-level comment resolved. Who may use it is the front end's to keep (the CLI:
    /// never pre-approved for an agent, so its harness asks the user each time).
    ///
    /// # Errors
    /// Not a reviewer (without an override), an override without a reason, a review not waiting on its reviewers, an
    /// open comment on approving, or the store's.
    pub fn submit(
        &mut self,
        reference: &ReviewRef,
        verdict: Verdict,
        body: Option<&str>,
        overriding: Option<&str>,
        by: &Actor,
    ) -> Result<()> {
        let review = self.open(reference)?;
        let event = ReviewEvent::Submit {
            verdict,
            overriding,
            also_open: &[],
        };
        let next = review_next(&review, &event, by)?;
        self.record_decision(&review, verdict, body, next.overridden, next.status, by)
    }

    /// Whether `by`'s decision would be taken, with `also_open` more top-level comments open by then (a reviewer's
    /// drafts, sent with it: ATM-612), without making it: what [`submit`](Self::submit) refuses, it refuses.
    ///
    /// # Errors
    /// As [`submit`](Self::submit)'s checks, `also_open` named among the open comments on approving.
    pub fn check_submit(
        &self,
        reference: &ReviewRef,
        verdict: Verdict,
        overriding: Option<&str>,
        by: &Actor,
        also_open: &[String],
    ) -> Result<()> {
        let review = self.open(reference)?;
        let event = ReviewEvent::Submit {
            verdict,
            overriding,
            also_open,
        };
        review_next(&review, &event, by).map(|_| ())
    }

    /// A decision taken: the override on the record, its words, the status.
    fn record_decision(
        &mut self,
        review: &Review,
        verdict: Verdict,
        body: Option<&str>,
        overridden: Option<&str>,
        next: Option<Status>,
        by: &Actor,
    ) -> Result<()> {
        // The override, on the record: who, why they may not decide otherwise, and their reason
        if let Some(reason) = overridden {
            let anchor = Anchor {
                round: review.rounds.last().map_or(1, |r| r.number),
                ..Anchor::default()
            };
            let record = format!(
                "**The review's rules were overridden** to decide it: {} is {}. Why: {}",
                by.0,
                not_a_decider(&review.header, by),
                reason.trim()
            );
            let id = self
                .store
                .add_comment(&review.header.id, anchor, &record, by)?;
            self.store.resolve(&review.header.id, &id, true)?;
        }
        if let Some(body) = body.filter(|b| !b.trim().is_empty()) {
            let anchor = Anchor {
                round: review.rounds.last().map_or(1, |r| r.number),
                ..Anchor::default()
            };
            let id = self
                .store
                .add_comment(&review.header.id, anchor, body, by)?;
            // An approval's words ask for nothing more; a request for changes stays open until the reviewer is
            // satisfied, like any other comment
            if verdict == Verdict::Approve {
                self.store.resolve(&review.header.id, &id, true)?;
            }
        }
        match next {
            Some(status) => self.store.set_status(&review.header.id, status, by),
            None => Ok(()),
        }
    }

    /// End a review without approval: its author's or a reviewer's.
    ///
    /// # Errors
    /// An ended review, someone who may not, or the store's.
    pub fn close(&mut self, reference: &ReviewRef, reason: &str, by: &Actor) -> Result<()> {
        let review = self.open(reference)?;
        let next = review_next(&review, &ReviewEvent::Close, by)?;
        if !reason.trim().is_empty() {
            let anchor = Anchor {
                round: review.rounds.last().map_or(1, |r| r.number),
                ..Anchor::default()
            };
            let id = self
                .store
                .add_comment(&review.header.id, anchor, reason, by)?;
            self.store.resolve(&review.header.id, &id, true)?;
        }
        self.apply(&review, &next, by)
    }

    /// Change the tasks a review names (§3.2 ③): its author's or a reviewer's.
    ///
    /// # Errors
    /// An ended review, someone who may not, or the store's.
    pub fn set_tasks(
        &mut self,
        reference: &ReviewRef,
        tasks: Vec<String>,
        by: &Actor,
    ) -> Result<()> {
        let review = self.open(reference)?;
        review_next(&review, &ReviewEvent::SetTasks, by)?;
        self.store.set_tasks(&review.header.id, tasks)
    }

    /// Change a review's summary (its Description), its author's: with a round, when the round changes what the
    /// reviewers should check (ATM-587's review: the steps named in it). The title stays.
    ///
    /// # Errors
    /// An ended review, someone who isn't its author, or the store's.
    pub fn edit_summary(&mut self, reference: &ReviewRef, summary: &str, by: &Actor) -> Result<()> {
        let review = self.open(reference)?;
        review_next(&review, &ReviewEvent::EditSummary, by)?;
        self.store
            .edit_summary(&review.header.id, &review.header.title, summary)
    }
}

mod table;

pub use table::*;

#[cfg(test)]
mod tests;
