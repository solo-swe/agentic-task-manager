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
        if review.header.status != Status::Draft {
            return Err(not_now(&review, "be asked for again"));
        }
        if !is_author(&review.header, by) {
            return Err(refused(format!(
                "only {}'s author asks for its review",
                review.header.reference
            )));
        }
        self.store
            .set_status(&review.header.id, Status::Waiting, by)
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
                .set_status(&review.header.id, Status::Waiting, &by)?;
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
            // Sent back too (ATM-609): a reviewer may change their mind, or settle what they asked for themselves
            Verdict::Approve => {
                if !matches!(
                    status,
                    Status::Waiting | Status::Live | Status::ChangesRequested
                ) {
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
            // Sent back already, it isn't sent back again (CR-2): a reviewer's further words are a comment, which
            // the page calls *Publish comments*
            Verdict::RequestChanges => {
                if status != Status::Waiting {
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

    /// Change a review's summary (its Description), its author's: with a round, when the round changes what the
    /// reviewers should check (ATM-587's review: the steps named in it). The title stays.
    ///
    /// # Errors
    /// An ended review, someone who isn't its author, or the store's.
    pub fn edit_summary(&mut self, reference: &ReviewRef, summary: &str, by: &Actor) -> Result<()> {
        let review = self.not_ended(reference, "change its summary")?;
        if !is_author(&review.header, by) {
            return Err(refused(format!(
                "{}'s summary is its author's to change",
                review.header.reference
            )));
        }
        self.store
            .edit_summary(&review.header.id, &review.header.title, summary)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use atm_common::Error;

    use super::{Reviews, Verdict, decides, is_author};
    use crate::memory::MemoryStore;
    use crate::model::{
        Actor, Anchor, Chain, CommentId, Patch, ReviewHeader, ReviewRef, RoundKind, Status,
    };
    use crate::store::{NewReview, NewRound, ReviewFilter, ReviewStore};

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
            submodule: None,
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
        let header = reviews.create(new_review(Status::Waiting)).unwrap();
        assert_eq!(header.reference, cr(1));
        assert_eq!(header.number, 1);
        assert!(reviews.create(new_review(Status::Approved)).is_err());
        let mut untitled = new_review(Status::Waiting);
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
            .create(new_review(Status::Waiting))
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
        let mut own = new_review(Status::Waiting);
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
        let mut other = new_review(Status::Waiting);
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
        reviews.create(new_review(Status::Waiting)).unwrap();
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
        assert_eq!(header.status, Status::Waiting);
        assert!(decides(&header, &hakan()) && !decides(&header, &agent()));
    }

    #[test]
    fn sent_back_fixed_in_a_round_then_approved_once_every_comment_is_resolved() {
        let mut store = MemoryStore::new();
        let mut reviews = Reviews::new(&mut store);
        reviews.create(new_review(Status::Waiting)).unwrap();
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
        assert_eq!(status(reviews.store(), 1), Status::Waiting);

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
        reviews.create(new_review(Status::Waiting)).unwrap();
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
        reviews.create(new_review(Status::Waiting)).unwrap();
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
        assert_eq!(status(reviews.store(), 1), Status::Waiting);
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
        assert_eq!(status(reviews.store(), 1), Status::Waiting);
        assert!(reviews.checkpoint(&cr(1), checkpoint).is_err());
    }

    #[test]
    fn closed_by_its_author_or_a_reviewer_with_the_reason_kept() {
        let mut store = MemoryStore::new();
        let mut reviews = Reviews::new(&mut store);
        reviews.create(new_review(Status::Waiting)).unwrap();
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
        reviews.create(new_review(Status::Waiting)).unwrap();
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

    fn someone() -> Actor {
        Actor::from("someone else")
    }

    /// Store a review of `status`, whatever the rules say about starting there: a review that came to it.
    fn at(store: &mut MemoryStore, status: Status) -> ReviewRef {
        let header = store.create_review(new_review(Status::Waiting)).unwrap();
        store.set_status(&header.id, status, &hakan()).unwrap();
        header.reference
    }

    fn is_refused(result: &Result<impl std::fmt::Debug, Error>) -> bool {
        matches!(result, Err(Error::Refused(_)))
    }

    fn is_not_now(result: &Result<impl std::fmt::Debug, Error>) -> bool {
        matches!(result, Err(Error::Failed { .. }))
    }

    #[test]
    fn only_draft_open_and_live_are_statuses_a_review_starts_in() {
        for (status, starts) in [
            (Status::Draft, true),
            (Status::Waiting, true),
            (Status::Live, true),
            (Status::ChangesRequested, false),
            (Status::Approved, false),
            (Status::Closed, false),
        ] {
            let mut store = MemoryStore::new();
            let created = Reviews::new(&mut store).create(new_review(status));
            assert_eq!(created.is_ok(), starts, "{status:?}");
            if !starts {
                assert!(matches!(created, Err(Error::Invalid(_))), "{status:?}");
                assert_eq!(
                    store.list_reviews(&ReviewFilter::default()).unwrap(),
                    Vec::<ReviewHeader>::new()
                );
            }
        }
    }

    #[test]
    fn whose_change_it_is_and_who_decides_when_no_author_is_known() {
        let mut store = MemoryStore::new();
        let mut header = Reviews::new(&mut store)
            .create(new_review(Status::Waiting))
            .unwrap();
        assert!(is_author(&header, &agent()) && !is_author(&header, &hakan()));
        header.author = None;
        header.reviewers.clear();
        // Nobody is the author while it isn't known, so anyone decides
        assert!(!is_author(&header, &agent()));
        assert!(decides(&header, &agent()) && decides(&header, &hakan()));
    }

    #[test]
    fn a_review_that_isnt_there_is_not_found_by_every_rule() {
        let mut store = MemoryStore::new();
        let mut reviews = Reviews::new(&mut store);
        let missing = cr(9);
        let c1 = CommentId("c1".into());
        let results = [
            reviews.ask_for_review(&missing, &agent()),
            reviews.add_round(&missing, round(&agent())).map(|_| ()),
            reviews.checkpoint(&missing, round(&agent())).map(|_| ()),
            reviews
                .comment(&missing, on("a.rs", 1), "x", &hakan())
                .map(|_| ()),
            reviews.reply(&missing, &c1, "x", &hakan()).map(|_| ()),
            reviews.edit(&missing, &c1, "x", &hakan()),
            reviews.resolve(&missing, &c1, true, &hakan()),
            reviews.submit(&missing, Verdict::Approve, None, None, &hakan()),
            reviews.close(&missing, "x", &hakan()),
            reviews.set_tasks(&missing, Vec::new(), &hakan()),
        ];
        for (i, result) in results.into_iter().enumerate() {
            assert!(matches!(result, Err(Error::NotFound(_))), "rule {i}");
        }
    }

    #[test]
    fn an_ended_review_takes_nothing_more_and_says_where_it_stands() {
        for ended in [Status::Approved, Status::Closed] {
            let mut store = MemoryStore::new();
            let reference = at(&mut store, ended);
            let mut reviews = Reviews::new(&mut store);
            let c1 = CommentId("c1".into());
            let results = [
                reviews.add_round(&reference, round(&agent())).map(|_| ()),
                reviews
                    .comment(&reference, on("a.rs", 1), "x", &hakan())
                    .map(|_| ()),
                reviews.reply(&reference, &c1, "x", &hakan()).map(|_| ()),
                reviews.edit(&reference, &c1, "x", &hakan()),
                reviews.resolve(&reference, &c1, true, &hakan()),
                reviews.submit(&reference, Verdict::Comment, Some("x"), None, &hakan()),
                reviews.close(&reference, "x", &hakan()),
                reviews.set_tasks(&reference, Vec::new(), &hakan()),
            ];
            for (i, result) in results.into_iter().enumerate() {
                let message = result.unwrap_err().to_string();
                assert!(
                    message.starts_with(&format!("CR-1 is {}: it can't", ended.name())),
                    "{ended:?} rule {i}: {message}"
                );
            }
            let review = store.get_review(&reference).unwrap();
            assert_eq!((review.header.status, review.chains.len()), (ended, 0));
        }
    }

    #[test]
    fn a_status_is_named_in_words_when_a_rule_says_it_cant() {
        let mut store = MemoryStore::new();
        let reference = at(&mut store, Status::ChangesRequested);
        let message = Reviews::new(&mut store)
            .checkpoint(&reference, round(&hakan()))
            .unwrap_err()
            .to_string();
        assert!(
            message.starts_with("CR-1 is changes requested: it can't take a checkpoint"),
            "{message}"
        );
    }

    #[test]
    fn only_a_draft_is_asked_for_review() {
        for status in [Status::Waiting, Status::Live, Status::ChangesRequested] {
            let mut store = MemoryStore::new();
            let reference = at(&mut store, status);
            assert!(
                is_not_now(&Reviews::new(&mut store).ask_for_review(&reference, &agent())),
                "{status:?}"
            );
            assert_eq!(store.get_review(&reference).unwrap().header.status, status);
        }
        // Not its author's: refused, the draft stays one
        let mut store = MemoryStore::new();
        let reference = at(&mut store, Status::Draft);
        assert!(is_refused(
            &Reviews::new(&mut store).ask_for_review(&reference, &hakan())
        ));
        assert_eq!(
            store.get_review(&reference).unwrap().header.status,
            Status::Draft
        );
    }

    #[test]
    fn a_round_reopens_a_review_sent_back_or_live_and_leaves_a_draft_or_open_one_as_it_is() {
        for (before, after) in [
            (Status::Draft, Status::Draft),
            (Status::Waiting, Status::Waiting),
            (Status::Live, Status::Waiting),
            (Status::ChangesRequested, Status::Waiting),
        ] {
            let mut store = MemoryStore::new();
            let reference = at(&mut store, before);
            assert_eq!(
                Reviews::new(&mut store)
                    .add_round(&reference, round(&agent()))
                    .unwrap(),
                2,
                "{before:?}"
            );
            assert_eq!(
                store.get_review(&reference).unwrap().header.status,
                after,
                "{before:?}"
            );
        }
    }

    #[test]
    fn a_round_is_its_authors_and_someone_elses_is_refused_and_not_kept() {
        let mut store = MemoryStore::new();
        let mut reviews = Reviews::new(&mut store);
        reviews.create(new_review(Status::Waiting)).unwrap();
        for by in [hakan(), someone()] {
            assert!(is_refused(&reviews.add_round(&cr(1), round(&by))), "{by:?}");
        }
        assert_eq!(reviews.store().get_review(&cr(1)).unwrap().rounds.len(), 1);
    }

    #[test]
    fn a_review_without_an_author_yet_and_no_reviewers_takes_anyones_round_as_theirs() {
        let mut store = MemoryStore::new();
        let mut reviews = Reviews::new(&mut store);
        let mut live = new_review(Status::Live);
        live.author = None;
        live.reviewers.clear();
        reviews.create(live).unwrap();
        reviews.add_round(&cr(1), round(&someone())).unwrap();
        let header = reviews.store().get_review(&cr(1)).unwrap().header;
        assert_eq!(header.author, Some(someone()));
        // And from then on its rounds are theirs only
        assert!(is_refused(&reviews.add_round(&cr(1), round(&agent()))));
    }

    #[test]
    fn a_checkpoint_is_only_a_live_reviews_and_anyone_taking_part_makes_one() {
        for status in [Status::Draft, Status::Waiting, Status::ChangesRequested] {
            let mut store = MemoryStore::new();
            let reference = at(&mut store, status);
            assert!(
                is_not_now(&Reviews::new(&mut store).checkpoint(&reference, round(&hakan()))),
                "{status:?}"
            );
            assert_eq!(store.get_review(&reference).unwrap().rounds.len(), 1);
        }
        let mut store = MemoryStore::new();
        let mut reviews = Reviews::new(&mut store);
        reviews.create(new_review(Status::Live)).unwrap();
        // Not the author, not a reviewer, and the author is unchanged
        assert_eq!(reviews.checkpoint(&cr(1), round(&someone())).unwrap(), 2);
        let review = reviews.store().get_review(&cr(1)).unwrap();
        assert_eq!(review.header.author, Some(agent()));
        assert_eq!(review.rounds[1].by, someone());
    }

    #[test]
    fn anyone_comments_on_a_round_the_review_has_and_replies_with_some_text() {
        let mut store = MemoryStore::new();
        let mut reviews = Reviews::new(&mut store);
        reviews.create(new_review(Status::Draft)).unwrap();
        reviews.add_round(&cr(1), round(&agent())).unwrap();
        // On either round, by anyone, a draft too
        let first = reviews
            .comment(&cr(1), on("a.rs", 1), "On round 1.", &someone())
            .unwrap();
        reviews
            .comment(&cr(1), on("a.rs", 2), "On round 2.", &agent())
            .unwrap();
        for (body, round) in [("No round 0.", 0), ("No round 3.", 3)] {
            assert!(matches!(
                reviews.comment(&cr(1), on("a.rs", round), body, &hakan()),
                Err(Error::Invalid(_))
            ));
        }
        assert!(matches!(
            reviews.comment(&cr(1), on("a.rs", 1), "\n\t ", &hakan()),
            Err(Error::Invalid(_))
        ));
        // A reply needs text too, and a comment that is there
        assert!(matches!(
            reviews.reply(&cr(1), &first, "  ", &hakan()),
            Err(Error::Invalid(_))
        ));
        assert!(matches!(
            reviews.reply(&cr(1), &CommentId("c9".into()), "Hm.", &hakan()),
            Err(Error::NotFound(_))
        ));
        let reply = reviews.reply(&cr(1), &first, "Hm.", &someone()).unwrap();
        let review = reviews.store().get_review(&cr(1)).unwrap();
        assert_eq!(review.chains.len(), 2);
        assert_eq!(review.comment(&reply).unwrap().author, someone());
    }

    #[test]
    fn a_reply_is_edited_by_its_author_only_and_a_missing_comment_is_not_found() {
        let mut store = MemoryStore::new();
        let mut reviews = Reviews::new(&mut store);
        reviews.create(new_review(Status::Waiting)).unwrap();
        let question = reviews
            .comment(&cr(1), on("a.rs", 1), "Why?", &hakan())
            .unwrap();
        let answer = reviews
            .reply(&cr(1), &question, "Because.", &agent())
            .unwrap();
        // Not even a reviewer edits another's words
        assert!(is_refused(&reviews.edit(&cr(1), &answer, "No.", &hakan())));
        reviews
            .edit(&cr(1), &answer, "Because, edited.", &agent())
            .unwrap();
        assert_eq!(
            reviews
                .store()
                .get_review(&cr(1))
                .unwrap()
                .comment(&answer)
                .unwrap()
                .body,
            "Because, edited."
        );
        assert!(matches!(
            reviews.edit(&cr(1), &CommentId("c9".into()), "x", &agent()),
            Err(Error::NotFound(_))
        ));
    }

    #[test]
    fn a_comment_is_resolved_and_reopened_by_a_decider_or_its_author_only() {
        let mut store = MemoryStore::new();
        let mut reviews = Reviews::new(&mut store);
        reviews.create(new_review(Status::Waiting)).unwrap();
        let question = reviews
            .comment(&cr(1), on("a.rs", 1), "Why?", &hakan())
            .unwrap();
        assert!(is_refused(&reviews.resolve(
            &cr(1),
            &question,
            true,
            &someone()
        )));
        reviews.resolve(&cr(1), &question, true, &hakan()).unwrap();
        reviews.resolve(&cr(1), &question, false, &hakan()).unwrap();
        let resolved = |reviews: &Reviews<'_, MemoryStore>| {
            reviews
                .store()
                .get_review(&cr(1))
                .unwrap()
                .comment(&question)
                .unwrap()
                .resolved
        };
        assert_eq!(resolved(&reviews), Some(false));
        assert!(matches!(
            reviews.resolve(&cr(1), &CommentId("c9".into()), true, &hakan()),
            Err(Error::NotFound(_))
        ));
        // With no reviewers named, anyone but the review's author decides, so resolves another's comment
        let mut open = new_review(Status::Waiting);
        open.reviewers.clear();
        reviews.create(open).unwrap();
        let note = reviews
            .comment(&cr(2), on("a.rs", 1), "Note.", &hakan())
            .unwrap();
        assert!(is_refused(&reviews.resolve(&cr(2), &note, true, &agent())));
        reviews.resolve(&cr(2), &note, true, &someone()).unwrap();
    }

    #[test]
    fn a_comment_verdict_keeps_the_status_and_leaves_its_words_open() {
        for status in [
            Status::Draft,
            Status::Waiting,
            Status::Live,
            Status::ChangesRequested,
        ] {
            let mut store = MemoryStore::new();
            let reference = at(&mut store, status);
            Reviews::new(&mut store)
                .submit(
                    &reference,
                    Verdict::Comment,
                    Some("Looking."),
                    None,
                    &hakan(),
                )
                .unwrap();
            let review = store.get_review(&reference).unwrap();
            assert_eq!(review.header.status, status);
            let chain = &review.chains[0];
            assert_eq!(chain.anchor.path, None);
            assert_eq!(chain.comments[0].resolved, Some(false), "{status:?}");
        }
    }

    #[test]
    fn a_submits_words_are_about_the_last_round_and_blank_words_are_none() {
        let mut store = MemoryStore::new();
        let mut reviews = Reviews::new(&mut store);
        reviews.create(new_review(Status::Waiting)).unwrap();
        reviews.add_round(&cr(1), round(&agent())).unwrap();
        reviews
            .submit(&cr(1), Verdict::Comment, Some(" \n"), None, &hakan())
            .unwrap();
        reviews
            .submit(&cr(1), Verdict::Comment, None, None, &hakan())
            .unwrap();
        assert_eq!(
            reviews.store().get_review(&cr(1)).unwrap().chains,
            Vec::<Chain>::new()
        );
        reviews
            .submit(
                &cr(1),
                Verdict::RequestChanges,
                Some("Fix it."),
                None,
                &hakan(),
            )
            .unwrap();
        let review = reviews.store().get_review(&cr(1)).unwrap();
        assert_eq!(review.chains[0].anchor.round, 2);
        assert_eq!(review.chains[0].comments[0].resolved, Some(false));
    }

    #[test]
    fn who_isnt_a_decider_is_refused_every_verdict_without_an_override() {
        for verdict in [Verdict::Approve, Verdict::RequestChanges, Verdict::Comment] {
            let mut store = MemoryStore::new();
            let mut reviews = Reviews::new(&mut store);
            reviews.create(new_review(Status::Waiting)).unwrap();
            for by in [agent(), someone()] {
                assert!(
                    is_refused(&reviews.submit(&cr(1), verdict, Some("x"), None, &by)),
                    "{verdict:?} {by:?}"
                );
            }
            let review = reviews.store().get_review(&cr(1)).unwrap();
            assert_eq!(
                (review.header.status, review.chains.len()),
                (Status::Waiting, 0)
            );
        }
    }

    /// A comment and a reply in every status a review is still open in, a review sent back too (ATM-597): talking
    /// about the changes goes on until the next round. An ended one refuses them (`an_ended_review_takes_nothing_…`)
    #[test]
    fn a_review_not_ended_takes_comments_and_replies_whatever_its_status() {
        for status in [
            Status::Draft,
            Status::Waiting,
            Status::Live,
            Status::ChangesRequested,
        ] {
            let mut store = MemoryStore::new();
            let reference = at(&mut store, status);
            let mut reviews = Reviews::new(&mut store);
            let anchor = Anchor {
                round: 1,
                ..Anchor::default()
            };
            let id = reviews
                .comment(&reference, anchor, "Why this?", &hakan())
                .unwrap_or_else(|e| panic!("{status:?}: {e}"));
            reviews
                .reply(&reference, &id, "Because.", &agent())
                .unwrap_or_else(|e| panic!("{status:?}: {e}"));
            let review = store.get_review(&reference).unwrap();
            assert_eq!(review.header.status, status);
            assert_eq!(review.chains[0].comments[0].replies.len(), 1, "{status:?}");
        }
    }

    /// Each status and decision: allowed or refused, the status it leaves, and whether the reviewer's words are kept
    #[test]
    fn an_approval_is_for_an_open_live_or_sent_back_review_and_a_send_back_for_an_open_one() {
        for (status, verdict, after) in [
            (Status::Draft, Verdict::Approve, None),
            (Status::Waiting, Verdict::Approve, Some(Status::Approved)),
            (Status::Live, Verdict::Approve, Some(Status::Approved)),
            // Sent back, then approved after all (ATM-609)
            (
                Status::ChangesRequested,
                Verdict::Approve,
                Some(Status::Approved),
            ),
            (Status::Draft, Verdict::RequestChanges, None),
            (
                Status::Waiting,
                Verdict::RequestChanges,
                Some(Status::ChangesRequested),
            ),
            (Status::Live, Verdict::RequestChanges, None),
            // Sent back already: not again (CR-2); further words are a comment, below
            (Status::ChangesRequested, Verdict::RequestChanges, None),
            (Status::Draft, Verdict::Comment, Some(Status::Draft)),
            (Status::Waiting, Verdict::Comment, Some(Status::Waiting)),
            (Status::Live, Verdict::Comment, Some(Status::Live)),
            (
                Status::ChangesRequested,
                Verdict::Comment,
                Some(Status::ChangesRequested),
            ),
        ] {
            let mut store = MemoryStore::new();
            let reference = at(&mut store, status);
            let result =
                Reviews::new(&mut store).submit(&reference, verdict, Some("x"), None, &hakan());
            assert_eq!(result.is_ok(), after.is_some(), "{status:?} {verdict:?}");
            if let Some(after) = after {
                let review = store.get_review(&reference).unwrap();
                assert_eq!(review.header.status, after, "{status:?} {verdict:?}");
                assert_eq!(review.chains.len(), 1, "{status:?} {verdict:?}");
                assert_eq!(review.chains[0].comments[0].body, "x");
            } else {
                assert!(is_not_now(&result), "{status:?} {verdict:?}");
                // Refused before anything is written
                assert_eq!(
                    store.get_review(&reference).unwrap().chains,
                    Vec::<Chain>::new()
                );
            }
        }
    }

    #[test]
    fn an_override_by_someone_not_a_reviewer_is_recorded_as_such() {
        let mut store = MemoryStore::new();
        let mut reviews = Reviews::new(&mut store);
        reviews.create(new_review(Status::Waiting)).unwrap();
        reviews
            .submit(
                &cr(1),
                Verdict::RequestChanges,
                Some("Please fix."),
                Some("  Hakan is away.  "),
                &someone(),
            )
            .unwrap();
        let review = reviews.store().get_review(&cr(1)).unwrap();
        assert_eq!(review.header.status, Status::ChangesRequested);
        let record = &review.chains[0].comments[0];
        assert!(
            record
                .body
                .contains("someone else is not one of its reviewers")
                && record.body.ends_with("Why: Hakan is away."),
            "{}",
            record.body
        );
        assert_eq!(record.resolved, Some(true));
        // The request's own words follow, open
        assert_eq!(review.chains[1].comments[0].body, "Please fix.");
        assert_eq!(review.chains[1].comments[0].resolved, Some(false));
    }

    #[test]
    fn closing_without_a_reason_records_no_comment_and_a_draft_or_live_review_closes_too() {
        for status in [Status::Draft, Status::Live, Status::ChangesRequested] {
            let mut store = MemoryStore::new();
            let reference = at(&mut store, status);
            Reviews::new(&mut store)
                .close(&reference, "  ", &hakan())
                .unwrap();
            let review = store.get_review(&reference).unwrap();
            assert_eq!(
                (review.header.status, review.chains.len()),
                (Status::Closed, 0)
            );
        }
        // A reviewer closes it as well as its author, the reason resolved
        let mut store = MemoryStore::new();
        let reference = at(&mut store, Status::Waiting);
        Reviews::new(&mut store)
            .close(&reference, "Not needed.", &hakan())
            .unwrap();
        let review = store.get_review(&reference).unwrap();
        assert_eq!(review.chains[0].comments[0].resolved, Some(true));
        assert_eq!(review.chains[0].comments[0].author, hakan());
    }

    #[test]
    fn its_tasks_are_changed_by_its_author_too_even_to_none() {
        let mut store = MemoryStore::new();
        let mut reviews = Reviews::new(&mut store);
        reviews.create(new_review(Status::Draft)).unwrap();
        reviews.set_tasks(&cr(1), Vec::new(), &agent()).unwrap();
        assert_eq!(
            reviews.store().get_review(&cr(1)).unwrap().header.tasks,
            Vec::<String>::new()
        );
        assert!(is_refused(&reviews.set_tasks(
            &cr(1),
            vec!["ATM-1".into()],
            &someone()
        )));
    }

    /// ATM-587's review: the summary (its Description) is its author's to change, until it has ended; the title stays
    #[test]
    fn its_summary_is_changed_by_its_author_while_it_is_open() {
        let mut store = MemoryStore::new();
        let mut reviews = Reviews::new(&mut store);
        reviews.create(new_review(Status::Waiting)).unwrap();
        let title = reviews.store().get_review(&cr(1)).unwrap().header.title;
        reviews
            .edit_summary(&cr(1), "1. Do this.\n2. Then that.", &agent())
            .unwrap();
        let review = reviews.store().get_review(&cr(1)).unwrap();
        assert_eq!(review.summary, "1. Do this.\n2. Then that.");
        assert_eq!(review.header.title, title);
        // Not a reviewer's, nor anyone else's
        for who in [hakan(), someone()] {
            let refused = reviews.edit_summary(&cr(1), "mine", &who);
            assert!(is_refused(&refused), "{who:?}");
        }
        assert_eq!(
            reviews.store().get_review(&cr(1)).unwrap().summary,
            "1. Do this.\n2. Then that."
        );
        // Nor once it has ended
        reviews.close(&cr(1), "not needed", &agent()).unwrap();
        assert!(is_not_now(&reviews.edit_summary(&cr(1), "late", &agent())));
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
        reviews.create(new_review(Status::Waiting)).unwrap();
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
