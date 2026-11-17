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
