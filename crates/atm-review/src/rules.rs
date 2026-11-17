//! Who may do what to a review, and when (docs/code-review-design.md §3.2's state diagram, §3.4's resolving), above any
//! [`ReviewStore`], so every store keeps the same rules. A review's decision (approve, request changes) is a
//! reviewer's: for an agent's review, the user's (I23); agents comment and reply.

use atm_common::{Error, Result};

use crate::model::{Actor, Anchor, CommentId, Review, ReviewHeader, ReviewRef, Status};
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

/// Why `who` may not decide `header`'s review, said in the record of an override.
fn not_a_decider(header: &ReviewHeader, who: &Actor) -> &'static str {
    if is_author(header, who) {
        "its author"
    } else {
        "not one of its reviewers"
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
        if !matches!(new.status, Status::Draft | Status::Open | Status::Live) {
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
        if review.header.status != Status::Draft {
            return Err(not_now(&review, "be asked for again"));
        }
        if !is_author(&review.header, by) {
            return Err(refused(format!(
                "only {}'s author asks for its review",
                review.header.reference
            )));
        }
        self.store.set_status(&review.header.id, Status::Open, by)
    }

    /// The author's next round, after addressing the comments: a review sent back is open again, and a live one ends
    /// as an ordinary open review (§3.5). Its number. A review whose author isn't known yet (a live review a person
    /// opened) takes the round's maker as its author, unless they are one of its reviewers.
    ///
    /// # Errors
    /// An ended review, not its author (or a reviewer, while there is none), or the store's.
    pub fn add_round(&mut self, reference: &ReviewRef, round: NewRound) -> Result<u32> {
        let review = self.not_ended(reference, "get another round")?;
        match &review.header.author {
            Some(author) if *author != round.by => {
                return Err(refused(format!(
                    "only {}'s author adds its rounds",
                    review.header.reference
                )));
            }
            None if review.header.reviewers.contains(&round.by) => {
                return Err(refused(format!(
                    "{}'s rounds are its author's, not a reviewer's",
                    review.header.reference
                )));
            }
            None => self.store.set_author(&review.header.id, &round.by)?,
            Some(_) => {}
        }
        let by = round.by.clone();
        let number = self.store.add_round(&review.header.id, round)?;
        if matches!(
            review.header.status,
            Status::ChangesRequested | Status::Live
        ) {
            self.store
                .set_status(&review.header.id, Status::Open, &by)?;
        }
        Ok(number)
    }

    /// A live review's checkpoint (§3.5): a round of the change as it is, so a comment is anchored to what the
    /// reviewer saw; the review stays live. Anyone taking part makes one, with their comment.
    ///
    /// # Errors
    /// Not live, or the store's.
    pub fn checkpoint(&mut self, reference: &ReviewRef, round: NewRound) -> Result<u32> {
        let review = self.open(reference)?;
        if review.header.status != Status::Live {
            return Err(not_now(
                &review,
                "take a checkpoint (only a live review does)",
            ));
        }
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
        if !review.rounds.iter().any(|r| r.number == anchor.round) {
            return Err(Error::Invalid(format!(
                "{} has no round {}",
                review.header.reference, anchor.round
            )));
        }
        if body.trim().is_empty() {
            return Err(Error::Invalid("a comment needs some text".into()));
        }
        self.store.add_comment(&review.header.id, anchor, body, by)
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
        let review = self.not_ended(reference, "be submitted on")?;
        let overridden = match overriding {
            Some(reason) if reason.trim().is_empty() => {
                return Err(Error::Invalid(
                    "overriding the review's rules needs the reason why".into(),
                ));
            }
            // One who may decide anyway: nothing to override
            _ if decides(&review.header, by) => None,
            Some(reason) => Some(reason),
            None => {
                return Err(refused(format!(
                    "{} is decided by its reviewers{}; a person may override that, saying why",
                    review.header.reference,
                    if review.header.reviewers.is_empty() {
                        ", not its author"
                    } else {
                        ""
                    }
                )));
            }
        };
        let status = review.header.status;
        let next = match verdict {
            Verdict::Approve => {
                if !matches!(status, Status::Open | Status::Live) {
                    return Err(not_now(&review, "be approved"));
                }
                let open: Vec<String> = review.open_comments().map(|c| c.id.0.clone()).collect();
                if !open.is_empty() {
                    return Err(Error::failed(format!(
                        "{} can't be approved while comments are open: {}",
                        review.header.reference,
                        open.join(", ")
                    )));
                }
                Some(Status::Approved)
            }
            Verdict::RequestChanges => {
                if status != Status::Open {
                    return Err(not_now(&review, "be sent back"));
                }
                Some(Status::ChangesRequested)
            }
            Verdict::Comment => None,
        };
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
        let review = self.not_ended(reference, "be closed")?;
        if !is_author(&review.header, by) && !decides(&review.header, by) {
            return Err(refused(format!(
                "{} is closed by its author or a reviewer",
                review.header.reference
            )));
        }
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
        self.store.set_status(&review.header.id, Status::Closed, by)
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
        let review = self.not_ended(reference, "change its tasks")?;
        if !is_author(&review.header, by) && !decides(&review.header, by) {
            return Err(refused(format!(
                "{}'s tasks are changed by its author or a reviewer",
                review.header.reference
            )));
        }
        self.store.set_tasks(&review.header.id, tasks)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::{Reviews, Verdict, decides};
    use crate::memory::MemoryStore;
    use crate::model::{Actor, Anchor, Patch, ReviewRef, RoundKind, Status};
    use crate::store::{NewReview, NewRound, ReviewStore};

    fn agent() -> Actor {
        Actor::from("claude:0f56a916")
    }
    fn hakan() -> Actor {
        Actor::from("Hakan Serce")
    }
    fn round(by: &Actor) -> NewRound {
        NewRound {
            kind: RoundKind::Range {
                base: "a1".into(),
                head: "b2".into(),
            },
            files: Vec::new(),
            patch: Patch("diff --git a/x b/x\n".into()),
            by: by.clone(),
        }
    }
    fn new_review(status: Status) -> NewReview {
        NewReview {
            title: "Claim hold on Windows".into(),
            summary: "The watcher found its shell as pid 0.".into(),
            tasks: vec!["ATM-482".into()],
            author: Some(agent()),
            reviewers: vec![hakan()],
            status,
            first: round(&agent()),
        }
    }
    fn on(path: &str, round: u32) -> Anchor {
        Anchor {
            round,
            path: Some(path.into()),
            lines: Some((40, 52)),
            quote: Some("let shell = 0;".into()),
            ..Anchor::default()
        }
    }
    fn cr(n: u32) -> ReviewRef {
        ReviewRef::numbered(n)
    }
    fn status(store: &MemoryStore, n: u32) -> Status {
        store.get_review(&cr(n)).unwrap().header.status
    }

    #[test]
    fn a_review_starts_as_a_draft_open_or_live_with_a_title_and_its_first_round() {
        let mut store = MemoryStore::new();
        let mut reviews = Reviews::new(&mut store);
        let header = reviews.create(new_review(Status::Open)).unwrap();
        assert_eq!(header.reference, cr(1));
        assert_eq!(header.number, 1);
        assert!(reviews.create(new_review(Status::Approved)).is_err());
        let mut untitled = new_review(Status::Open);
        untitled.title = "  ".into();
        assert!(reviews.create(untitled).is_err());
        assert_eq!(
            reviews.create(new_review(Status::Draft)).unwrap().reference,
            cr(2)
        );
        let review = store.get_review(&cr(1)).unwrap();
        assert_eq!(review.rounds.len(), 1);
        assert_eq!(
            store.round_diff(&review.header.id, 1).unwrap().0,
            "diff --git a/x b/x\n"
        );
    }

    #[test]
    fn a_review_is_decided_by_its_reviewers_or_with_none_by_anyone_but_its_author() {
        let mut store = MemoryStore::new();
        let header = Reviews::new(&mut store)
            .create(new_review(Status::Open))
            .unwrap();
        assert!(decides(&header, &hakan()));
        assert!(!decides(&header, &agent()));
        assert!(!decides(&header, &Actor::from("someone else")));
        let mut unnamed = header;
        unnamed.reviewers.clear();
        assert!(decides(&unnamed, &Actor::from("someone else")));
        assert!(!decides(&unnamed, &agent()));
    }

    #[test]
    fn a_person_may_override_who_decides_saying_why_and_it_is_recorded() {
        let mut store = MemoryStore::new();
        let mut reviews = Reviews::new(&mut store);
        // Hakan's own review, no reviewers named: by the rules, not his to decide
        let mut own = new_review(Status::Open);
        own.author = Some(hakan());
        own.reviewers.clear();
        reviews.create(own).unwrap();
        let refused = reviews
            .submit(&cr(1), Verdict::Approve, None, None, &hakan())
            .unwrap_err()
            .to_string();
        assert!(refused.contains("may override"), "{refused}");
        // An override needs its reason
        assert!(
            reviews
                .submit(&cr(1), Verdict::Approve, None, Some("  "), &hakan())
                .is_err()
        );
        reviews
            .submit(
                &cr(1),
                Verdict::Approve,
                None,
                Some("Ana reviewed it on a call; her notes are in the chains."),
                &hakan(),
            )
            .unwrap();
        let review = reviews.store().get_review(&cr(1)).unwrap();
        assert_eq!(review.header.status, Status::Approved);
        let record = &review.chains.last().unwrap().comments[0];
        assert_eq!(record.author, hakan());
        assert_eq!(record.resolved, Some(true));
        assert!(
            record.body.contains("overridden")
                && record.body.contains("its author")
                && record.body.contains("Ana reviewed it"),
            "{}",
            record.body
        );

        // It lifts who may decide, nothing else: an approval still needs every comment resolved
        let mut other = new_review(Status::Open);
        other.author = Some(hakan());
        other.reviewers.clear();
        reviews.create(other).unwrap();
        reviews
            .comment(&cr(2), on("a.rs", 1), "Still open.", &agent())
            .unwrap();
        assert!(
            reviews
                .submit(
                    &cr(2),
                    Verdict::Approve,
                    None,
                    Some("Decided elsewhere."),
                    &hakan()
                )
                .unwrap_err()
                .to_string()
                .contains("comments are open")
        );
        // A reviewer who may decide records no override
        reviews.create(new_review(Status::Open)).unwrap();
        reviews
            .submit(
                &cr(3),
                Verdict::Approve,
                None,
                Some("Not needed."),
                &hakan(),
            )
            .unwrap();
        assert_eq!(
            reviews.store().get_review(&cr(3)).unwrap().chains.len(),
            0,
            "no override recorded"
        );
    }

    #[test]
    fn a_review_without_an_author_yet_takes_the_first_rounds_maker_never_a_reviewer() {
        let mut store = MemoryStore::new();
        let mut reviews = Reviews::new(&mut store);
        // A live review Hakan opened on an agent's working copy, nobody's change yet
        let mut live = new_review(Status::Live);
        live.author = None;
        reviews.create(live).unwrap();
        // Hakan reviews it: his round would make him the change's author, so it is refused
        assert!(reviews.add_round(&cr(1), round(&hakan())).is_err());
        // The agent ends it with a round: it becomes the author, and Hakan, a reviewer, decides
        reviews.add_round(&cr(1), round(&agent())).unwrap();
        let header = reviews.store().get_review(&cr(1)).unwrap().header;
        assert_eq!(header.author, Some(agent()));
        assert_eq!(header.status, Status::Open);
        assert!(decides(&header, &hakan()) && !decides(&header, &agent()));
    }

    #[test]
    fn sent_back_fixed_in_a_round_then_approved_once_every_comment_is_resolved() {
        let mut store = MemoryStore::new();
        let mut reviews = Reviews::new(&mut store);
        reviews.create(new_review(Status::Open)).unwrap();
        let question = reviews
            .comment(
                &cr(1),
                on("crates/atm-cli/src/process.rs", 1),
                "Find the parent instead?",
                &hakan(),
            )
            .unwrap();
        let answer = reviews
            .reply(&cr(1), &question, "Done in round 2.", &agent())
            .unwrap();

        // The agent can't resolve the reviewer's question, nor decide its own review
        assert!(reviews.resolve(&cr(1), &question, true, &agent()).is_err());
        assert!(
            reviews
                .submit(&cr(1), Verdict::Approve, None, None, &agent())
                .is_err()
        );

        reviews
            .submit(
                &cr(1),
                Verdict::RequestChanges,
                Some("One more thing."),
                None,
                &hakan(),
            )
            .unwrap();
        assert_eq!(status(reviews.store(), 1), Status::ChangesRequested);
        assert_eq!(reviews.add_round(&cr(1), round(&agent())).unwrap(), 2);
        assert_eq!(status(reviews.store(), 1), Status::Open);

        // Approving names what is still open: the question and the request's own words
        let refused = reviews
            .submit(&cr(1), Verdict::Approve, None, None, &hakan())
            .unwrap_err()
            .to_string();
        assert!(
            refused.contains("c1") && refused.contains("c3"),
            "{refused}"
        );
        // A reply has no box of its own: its top-level comment is resolved
        let reply = reviews
            .resolve(&cr(1), &answer, true, &hakan())
            .unwrap_err()
            .to_string();
        assert!(
            reply.contains("is a reply") && reply.contains("c1"),
            "{reply}"
        );
        reviews.resolve(&cr(1), &question, true, &hakan()).unwrap();
        let request = reviews
            .store()
            .get_review(&cr(1))
            .unwrap()
            .open_comments()
            .next()
            .unwrap()
            .id
            .clone();
        reviews.resolve(&cr(1), &request, true, &hakan()).unwrap();
        reviews
            .submit(&cr(1), Verdict::Approve, Some("Good."), None, &hakan())
            .unwrap();
        assert_eq!(status(reviews.store(), 1), Status::Approved);

        // An approved review takes no more comments or rounds
        assert!(
            reviews
                .comment(&cr(1), on("x", 1), "late", &hakan())
                .is_err()
        );
        assert!(reviews.add_round(&cr(1), round(&agent())).is_err());
    }

    #[test]
    fn a_comments_author_resolves_their_own_and_edits_only_their_own() {
        let mut store = MemoryStore::new();
        let mut reviews = Reviews::new(&mut store);
        reviews.create(new_review(Status::Open)).unwrap();
        let note = reviews
            .comment(&cr(1), on("a.rs", 1), "A note for the reviewer.", &agent())
            .unwrap();
        reviews.resolve(&cr(1), &note, true, &agent()).unwrap();
        assert!(reviews.edit(&cr(1), &note, "changed", &hakan()).is_err());
        reviews.edit(&cr(1), &note, "Changed.", &agent()).unwrap();
        assert_eq!(
            reviews
                .store()
                .get_review(&cr(1))
                .unwrap()
                .comment(&note)
                .unwrap()
                .body,
            "Changed."
        );
    }

    #[test]
    fn a_comment_needs_text_and_a_round_the_review_has() {
        let mut store = MemoryStore::new();
        let mut reviews = Reviews::new(&mut store);
        reviews.create(new_review(Status::Open)).unwrap();
        assert!(
            reviews
                .comment(&cr(1), on("a.rs", 2), "no round 2", &hakan())
                .is_err()
        );
        assert!(
            reviews
                .comment(&cr(1), on("a.rs", 1), " ", &hakan())
                .is_err()
        );
    }

    #[test]
    fn a_draft_goes_to_its_reviewers_by_its_author() {
        let mut store = MemoryStore::new();
        let mut reviews = Reviews::new(&mut store);
        reviews.create(new_review(Status::Draft)).unwrap();
        assert!(
            reviews
                .submit(&cr(1), Verdict::Approve, None, None, &hakan())
                .is_err()
        );
        assert!(reviews.ask_for_review(&cr(1), &hakan()).is_err());
        reviews.ask_for_review(&cr(1), &agent()).unwrap();
        assert_eq!(status(reviews.store(), 1), Status::Open);
        assert!(reviews.ask_for_review(&cr(1), &agent()).is_err());
    }

    #[test]
    fn a_live_review_takes_checkpoints_and_is_approved_or_ended_with_a_round() {
        let mut store = MemoryStore::new();
        let mut reviews = Reviews::new(&mut store);
        reviews.create(new_review(Status::Live)).unwrap();
        let checkpoint = NewRound {
            kind: RoundKind::Working { base: "a1".into() },
            ..round(&hakan())
        };
        assert_eq!(reviews.checkpoint(&cr(1), checkpoint.clone()).unwrap(), 2);
        assert_eq!(status(reviews.store(), 1), Status::Live);
        // Sending back is for an open review; a live one is commented on as it grows
        assert!(
            reviews
                .submit(&cr(1), Verdict::RequestChanges, None, None, &hakan())
                .is_err()
        );

        reviews.create(new_review(Status::Live)).unwrap();
        reviews
            .submit(&cr(2), Verdict::Approve, None, None, &hakan())
            .unwrap();
        assert_eq!(status(reviews.store(), 2), Status::Approved);

        reviews.add_round(&cr(1), round(&agent())).unwrap();
        assert_eq!(status(reviews.store(), 1), Status::Open);
        assert!(reviews.checkpoint(&cr(1), checkpoint).is_err());
    }

    #[test]
    fn closed_by_its_author_or_a_reviewer_with_the_reason_kept() {
        let mut store = MemoryStore::new();
        let mut reviews = Reviews::new(&mut store);
        reviews.create(new_review(Status::Open)).unwrap();
        assert!(
            reviews
                .close(&cr(1), "no", &Actor::from("someone else"))
                .is_err()
        );
        reviews
            .close(&cr(1), "Replaced by CR-2.", &agent())
            .unwrap();
        let review = reviews.store().get_review(&cr(1)).unwrap();
        assert_eq!(review.header.status, Status::Closed);
        assert_eq!(review.chains[0].comments[0].body, "Replaced by CR-2.");
        assert!(review.chains[0].anchor.path.is_none());
        assert!(reviews.close(&cr(1), "again", &agent()).is_err());
    }

    #[test]
    fn its_tasks_are_changed_by_its_author_or_a_reviewer() {
        let mut store = MemoryStore::new();
        let mut reviews = Reviews::new(&mut store);
        reviews.create(new_review(Status::Open)).unwrap();
        assert!(
            reviews
                .set_tasks(&cr(1), vec![], &Actor::from("someone else"))
                .is_err()
        );
        reviews
            .set_tasks(&cr(1), vec!["ATM-482".into(), "ATM-483".into()], &hakan())
            .unwrap();
        assert_eq!(
            reviews.store().get_review(&cr(1)).unwrap().header.tasks,
            ["ATM-482", "ATM-483"]
        );
    }

    #[test]
    fn a_subscription_is_told_of_every_write_until_it_is_dropped() {
        let mut store = MemoryStore::new();
        let told = Arc::new(AtomicUsize::new(0));
        let counter = told.clone();
        let subscription = store
            .subscribe(Arc::new(move || {
                counter.fetch_add(1, Ordering::Relaxed);
            }))
            .unwrap();
        let mut reviews = Reviews::new(&mut store);
        reviews.create(new_review(Status::Open)).unwrap();
        reviews
            .comment(&cr(1), on("a.rs", 1), "One.", &hakan())
            .unwrap();
        assert_eq!(told.load(Ordering::Relaxed), 2);
        drop(subscription);
        reviews
            .comment(&cr(1), on("a.rs", 1), "Two.", &hakan())
            .unwrap();
        assert_eq!(told.load(Ordering::Relaxed), 2);
    }
}
