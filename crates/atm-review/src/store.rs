//! The review store (docs/code-review-design.md §3.10): what atm-lib reads and changes reviews through, whatever holds
//! them, with one method per thing a review does, so a store on another system can map each to its own calls. The
//! rules (who may, and when) are [`crate::rules`]', above every store; a store only keeps what it is given.

use atm_common::Result;
use atm_common::subscription::{OnChange, Subscription};

use crate::model::{
    Actor, Anchor, Chain, Comment, CommentId, FileChange, Patch, Review, ReviewHeader, ReviewId,
    ReviewRef, RoundKind, Status,
};

/// A new review (§3.2): its header's fields, and its first round.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewReview {
    /// One line.
    pub title: String,
    /// The author's summary, in Markdown.
    pub summary: String,
    /// The keys of the tasks it reviews.
    pub tasks: Vec<String>,
    /// Who asks for it.
    pub author: Actor,
    /// Who decides it; empty: anyone but the author.
    pub reviewers: Vec<Actor>,
    /// Draft, open or live.
    pub status: Status,
    /// Its first round.
    pub first: NewRound,
}

/// A new round: what it was made from, its files and its diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewRound {
    /// What it was made from.
    pub kind: RoundKind,
    /// Its files.
    pub files: Vec<FileChange>,
    /// Its diff.
    pub patch: Patch,
    /// Who makes it.
    pub by: Actor,
}

/// Which reviews to list; the default lists them all.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReviewFilter {
    /// Only these statuses (none: any).
    pub statuses: Vec<Status>,
    /// Only reviews of this task (its key).
    pub task: Option<String>,
}

/// Which chains to list; the default lists them all.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CommentFilter {
    /// Only chains with an open top-level comment.
    pub open_only: bool,
}

/// What holds a project's reviews: the folder under the task folder (ATM-563), or the memory, or (later) another
/// system. Every method keeps what it is given; [`crate::rules::Reviews`] decides whether it may be given.
pub trait ReviewStore {
    /// The reviews' headers, newest first.
    ///
    /// # Errors
    /// When the reviews can't be read.
    fn list_reviews(&self, filter: &ReviewFilter) -> Result<Vec<ReviewHeader>>;

    /// The review a reference names (`CR-7`, `CR-8.1`, or its ID).
    ///
    /// # Errors
    /// None by that reference (`NotFound`); a number two reviews have (`Invalid`, naming both, §3.10).
    fn get_review(&self, reference: &ReviewRef) -> Result<Review>;

    /// Keep a new review, with its first round; the store gives it its ID, number and reference.
    ///
    /// # Errors
    /// When it can't be written.
    fn create_review(&mut self, new: NewReview) -> Result<ReviewHeader>;

    /// Change its status.
    ///
    /// # Errors
    /// No such review, or it can't be written.
    fn set_status(&mut self, id: &ReviewId, status: Status, by: &Actor) -> Result<()>;

    /// Change its title and summary.
    ///
    /// # Errors
    /// No such review, or it can't be written.
    fn edit_summary(&mut self, id: &ReviewId, title: &str, summary: &str) -> Result<()>;

    /// Change the tasks it reviews.
    ///
    /// # Errors
    /// No such review, or it can't be written.
    fn set_tasks(&mut self, id: &ReviewId, tasks: Vec<String>) -> Result<()>;

    /// Add a round; its number.
    ///
    /// # Errors
    /// No such review, or it can't be written.
    fn add_round(&mut self, id: &ReviewId, round: NewRound) -> Result<u32>;

    /// A round's diff.
    ///
    /// # Errors
    /// No such review or round.
    fn round_diff(&self, id: &ReviewId, round: u32) -> Result<Patch>;

    /// Its chains.
    ///
    /// # Errors
    /// No such review.
    fn list_comments(&self, id: &ReviewId, filter: &CommentFilter) -> Result<Vec<Chain>>;

    /// One comment, a reply or a top-level one.
    ///
    /// # Errors
    /// No such review or comment.
    fn get_comment(&self, id: &ReviewId, comment: &CommentId) -> Result<Comment>;

    /// Start a chain with a top-level comment (open); the comment's ID.
    ///
    /// # Errors
    /// No such review, or it can't be written.
    fn add_comment(
        &mut self,
        id: &ReviewId,
        anchor: Anchor,
        body: &str,
        by: &Actor,
    ) -> Result<CommentId>;

    /// Reply to a comment; the reply's ID.
    ///
    /// # Errors
    /// No such review or comment, or it can't be written.
    fn reply(&mut self, id: &ReviewId, to: &CommentId, body: &str, by: &Actor)
    -> Result<CommentId>;

    /// Change a comment's text.
    ///
    /// # Errors
    /// No such review or comment, or it can't be written.
    fn edit_comment(&mut self, id: &ReviewId, comment: &CommentId, body: &str) -> Result<()>;

    /// Set a top-level comment's resolved box.
    ///
    /// # Errors
    /// No such review or top-level comment, or it can't be written.
    fn resolve(&mut self, id: &ReviewId, top_level: &CommentId, resolved: bool) -> Result<()>;

    /// Be told when any review changes, until the [`Subscription`] is dropped (push or poll: the store's business).
    ///
    /// # Errors
    /// When the store can't be watched.
    fn subscribe(&self, on_change: OnChange) -> Result<Subscription>;
}
