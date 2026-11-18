//! The changes a store makes to a review it holds, the same for every store: a store loads the review, applies one of
//! these, and keeps the result its own way. Who may make them is [`crate::rules`]'.

use atm_common::{Error, Result};

use crate::model::{Actor, Anchor, Chain, ChainId, Comment, CommentId, Review, Round};
use crate::store::{CommentFilter, NewRound};

/// The number in an ID like `c9` or `t4`.
fn ordinal(id: &str) -> u32 {
    id.get(1..).and_then(|n| n.parse().ok()).unwrap_or(0)
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

fn no_comment(id: &CommentId) -> Error {
    Error::NotFound(format!("no comment {id}"))
}

impl Review {
    /// The ID the next comment gets: one past the highest (`c10` after `c9`).
    #[must_use]
    pub fn next_comment_id(&self) -> CommentId {
        let last = self.comments().map(|c| ordinal(&c.id.0)).max().unwrap_or(0);
        CommentId(format!("c{}", last + 1))
    }

    /// The ID the next chain gets (`t5` after `t4`).
    #[must_use]
    pub fn next_chain_id(&self) -> ChainId {
        let last = self
            .chains
            .iter()
            .map(|c| ordinal(&c.id.0))
            .max()
            .unwrap_or(0);
        ChainId(format!("t{}", last + 1))
    }

    /// Add a round, numbered after the last; its number.
    pub fn push_round(&mut self, round: &NewRound, at: &str) -> u32 {
        let number = self.rounds.last().map_or(1, |r| r.number + 1);
        self.rounds.push(Round {
            number,
            kind: round.kind.clone(),
            submodule: round.submodule.clone(),
            files: round.files.clone(),
            by: round.by.clone(),
            at: at.to_string(),
        });
        number
    }

    /// Start a chain with a top-level comment, open; the comment's ID.
    pub fn push_chain(&mut self, anchor: Anchor, body: &str, by: &Actor, at: &str) -> CommentId {
        let id = self.next_comment_id();
        let chain = self.next_chain_id();
        self.chains.push(Chain {
            id: chain,
            anchor,
            comments: vec![Comment {
                id: id.clone(),
                author: by.clone(),
                at: at.to_string(),
                body: body.trim_end().to_string(),
                resolved: Some(false),
                replies: Vec::new(),
            }],
        });
        id
    }

    /// Reply to a comment; the reply's ID.
    ///
    /// # Errors
    /// No such comment.
    pub fn push_reply(
        &mut self,
        to: &CommentId,
        body: &str,
        by: &Actor,
        at: &str,
    ) -> Result<CommentId> {
        let id = self.next_comment_id();
        let parent = self
            .chains
            .iter_mut()
            .find_map(|chain| find_mut(&mut chain.comments, to))
            .ok_or_else(|| no_comment(to))?;
        parent.replies.push(Comment {
            id: id.clone(),
            author: by.clone(),
            at: at.to_string(),
            body: body.trim_end().to_string(),
            resolved: None,
            replies: Vec::new(),
        });
        Ok(id)
    }

    /// Change a comment's text.
    ///
    /// # Errors
    /// No such comment.
    pub fn set_body(&mut self, id: &CommentId, body: &str) -> Result<()> {
        let comment = self
            .chains
            .iter_mut()
            .find_map(|chain| find_mut(&mut chain.comments, id))
            .ok_or_else(|| no_comment(id))?;
        body.trim_end().clone_into(&mut comment.body);
        Ok(())
    }

    /// Set a top-level comment's resolved box.
    ///
    /// # Errors
    /// No such top-level comment.
    pub fn set_resolved(&mut self, top_level: &CommentId, resolved: bool) -> Result<()> {
        let comment = self
            .chains
            .iter_mut()
            .flat_map(|chain| chain.comments.iter_mut())
            .find(|c| &c.id == top_level)
            .ok_or_else(|| Error::NotFound(format!("no top-level comment {top_level}")))?;
        comment.resolved = Some(resolved);
        Ok(())
    }

    /// Its chains, as a filter keeps them.
    #[must_use]
    pub fn chains_by(&self, filter: &CommentFilter) -> Vec<Chain> {
        self.chains
            .iter()
            .filter(|c| !filter.open_only || c.comments.iter().any(|t| t.resolved == Some(false)))
            .cloned()
            .collect()
    }

    /// One comment, cloned.
    ///
    /// # Errors
    /// No such comment.
    pub fn comment_owned(&self, id: &CommentId) -> Result<Comment> {
        self.comment(id).cloned().ok_or_else(|| no_comment(id))
    }
}

#[cfg(test)]
mod tests {
    use super::ordinal;
    use crate::model::{
        Actor, Anchor, ChainId, Comment, CommentId, Patch, Review, ReviewHeader, ReviewId,
        ReviewRef, RoundKind, Status,
    };
    use crate::store::{CommentFilter, NewRound};
    use atm_common::Error;

    const AT: &str = "2026-11-17T03:31:00Z";

    fn hakan() -> Actor {
        Actor::from("Hakan Serce")
    }

    fn empty() -> Review {
        Review {
            header: ReviewHeader {
                id: ReviewId("u".into()),
                reference: ReviewRef::numbered(1),
                number: 1,
                title: "T".into(),
                status: Status::Waiting,
                tasks: Vec::new(),
                author: None,
                reviewers: Vec::new(),
                created: AT.into(),
                updated: AT.into(),
            },
            summary: String::new(),
            chains: Vec::new(),
            rounds: Vec::new(),
        }
    }

    fn id(text: &str) -> CommentId {
        CommentId(text.into())
    }

    #[test]
    fn an_ids_number_is_after_its_letter_and_none_reads_as_0() {
        for (text, number) in [
            ("c9", 9),
            ("t12", 12),
            ("c", 0),
            ("", 0),
            ("cx", 0),
            ("c-1", 0),
        ] {
            assert_eq!(ordinal(text), number, "{text:?}");
        }
    }

    #[test]
    fn the_first_comment_and_chain_are_c1_and_t1_then_one_past_the_highest() {
        let mut review = empty();
        assert_eq!(review.next_comment_id(), id("c1"));
        assert_eq!(review.next_chain_id(), ChainId("t1".into()));
        let first = review.push_chain(Anchor::default(), "One.", &hakan(), AT);
        assert_eq!(first, id("c1"));
        assert_eq!(review.chains[0].id, ChainId("t1".into()));
        // A reply's ID counts too, at any depth
        let reply = review.push_reply(&first, "Two.", &hakan(), AT).unwrap();
        let deeper = review.push_reply(&reply, "Three.", &hakan(), AT).unwrap();
        assert_eq!((reply, deeper), (id("c2"), id("c3")));
        // IDs edited by hand: the next is past the highest, not past the count
        review.chains[0].id = ChainId("t9".into());
        review.chains[0].comments[0].replies[0].replies[0].id = id("c41");
        assert_eq!(review.next_comment_id(), id("c42"));
        assert_eq!(review.next_chain_id(), ChainId("t10".into()));
    }

    #[test]
    fn a_new_chain_is_one_open_top_level_comment_its_text_trimmed_at_the_end() {
        let mut review = empty();
        let anchor = Anchor {
            round: 2,
            path: Some("a.rs".into()),
            lines: Some((3, 4)),
            ..Anchor::default()
        };
        review.push_chain(anchor.clone(), "  Why?\n\n", &hakan(), AT);
        let chain = &review.chains[0];
        assert_eq!(chain.anchor, anchor);
        let comment = &chain.comments[0];
        assert_eq!(
            (comment.body.as_str(), comment.resolved, comment.at.as_str()),
            ("  Why?", Some(false), AT)
        );
        assert_eq!(comment.author, hakan());
        assert_eq!(comment.replies, Vec::<Comment>::new());
    }

    #[test]
    fn a_reply_goes_under_the_comment_it_answers_without_a_box() {
        let mut review = empty();
        review.push_chain(Anchor::default(), "One.", &hakan(), AT);
        let second = review.push_chain(Anchor::default(), "Two.", &hakan(), AT);
        let reply = review
            .push_reply(&second, "Answer.\n", &Actor::from("claude:A"), AT)
            .unwrap();
        assert_eq!(review.chains[0].comments[0].replies, Vec::<Comment>::new());
        let under = &review.chains[1].comments[0].replies[0];
        assert_eq!(
            (&under.id, under.body.as_str(), under.resolved),
            (&reply, "Answer.", None)
        );
        assert!(matches!(
            review.push_reply(&id("c9"), "Lost.", &hakan(), AT),
            Err(Error::NotFound(_))
        ));
    }

    #[test]
    fn a_comments_text_is_changed_at_any_depth_and_a_missing_one_is_not_found() {
        let mut review = empty();
        let first = review.push_chain(Anchor::default(), "One.", &hakan(), AT);
        let reply = review.push_reply(&first, "Two.", &hakan(), AT).unwrap();
        review.set_body(&reply, "Two, edited.\n").unwrap();
        review.set_body(&first, "One, edited.").unwrap();
        assert_eq!(review.comment(&reply).unwrap().body, "Two, edited.");
        assert_eq!(review.comment(&first).unwrap().body, "One, edited.");
        assert!(matches!(
            review.set_body(&id("c9"), "x"),
            Err(Error::NotFound(_))
        ));
    }

    #[test]
    fn only_a_top_level_comment_is_resolved_or_reopened() {
        let mut review = empty();
        let first = review.push_chain(Anchor::default(), "One.", &hakan(), AT);
        let reply = review.push_reply(&first, "Two.", &hakan(), AT).unwrap();
        review.set_resolved(&first, true).unwrap();
        assert_eq!(review.comment(&first).unwrap().resolved, Some(true));
        review.set_resolved(&first, false).unwrap();
        assert_eq!(review.comment(&first).unwrap().resolved, Some(false));
        for not_top in [reply, id("c9")] {
            assert!(
                matches!(review.set_resolved(&not_top, true), Err(Error::NotFound(_))),
                "{not_top}"
            );
        }
        assert_eq!(review.comment(&id("c2")).unwrap().resolved, None);
    }

    #[test]
    fn chains_are_listed_all_or_only_those_with_an_open_top_level_comment() {
        let mut review = empty();
        review.push_chain(Anchor::default(), "Open.", &hakan(), AT);
        let done = review.push_chain(Anchor::default(), "Done.", &hakan(), AT);
        review.set_resolved(&done, true).unwrap();
        // A chain with a resolved and an open top-level comment is open
        let still_open = Comment {
            id: id("c3"),
            ..review.chains[0].comments[0].clone()
        };
        review.chains[1].comments.push(still_open);
        let third = review.push_chain(Anchor::default(), "Done too.", &hakan(), AT);
        review.set_resolved(&third, true).unwrap();
        let chains = |open_only| -> Vec<String> {
            review
                .chains_by(&CommentFilter { open_only })
                .iter()
                .map(|c| c.id.0.clone())
                .collect()
        };
        assert_eq!(chains(false), ["t1", "t2", "t3"]);
        assert_eq!(chains(true), ["t1", "t2"]);
        assert_eq!(
            empty().chains_by(&CommentFilter { open_only: true }).len(),
            0
        );
    }

    #[test]
    fn a_comment_is_handed_out_as_a_copy_or_not_found() {
        let mut review = empty();
        let first = review.push_chain(Anchor::default(), "One.", &hakan(), AT);
        assert_eq!(review.comment_owned(&first).unwrap().body, "One.");
        assert!(matches!(
            review.comment_owned(&id("c2")),
            Err(Error::NotFound(_))
        ));
    }

    #[test]
    fn rounds_are_numbered_after_the_last_from_1() {
        let mut review = empty();
        let round = NewRound {
            kind: RoundKind::Files,
            submodule: None,
            files: Vec::new(),
            patch: Patch::default(),
            by: hakan(),
        };
        assert_eq!(review.push_round(&round, AT), 1);
        assert_eq!(review.push_round(&round, AT), 2);
        // After a round numbered by hand, the next is one past it
        review.rounds[1].number = 5;
        assert_eq!(review.push_round(&round, AT), 6);
        let last = review.rounds.last().unwrap();
        assert_eq!(
            (&last.kind, &last.by, last.at.as_str()),
            (&RoundKind::Files, &hakan(), AT)
        );
    }
}
