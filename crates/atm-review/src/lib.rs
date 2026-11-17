//! Code reviews recorded in the project ([docs/code-review-design.md](../../docs/code-review-design.md)): a review (a
//! CR) is a change, the comments on it, and a decision.
//!
//! - [`model`]: the review, its statuses, rounds, chains of comments and anchors (§3.2 to §3.6);
//! - [`store`]: the [`ReviewStore`] trait, one method per thing a review does (§3.10), and what its methods take;
//! - [`rules`]: who may do what and when, above any store (§3.2's state diagram, §3.4's resolving);
//! - [`memory`]: a store in memory, for tests.
//!
//! A component of its own (§3.1): it knows a task only as a key (a string), and depends on neither atm-db nor atm-lib.

pub mod edits;
pub mod files;
pub mod folder;
pub mod memory;
pub mod model;
pub mod rounds;
pub mod rules;
pub mod store;

#[cfg(test)]
mod test_dir;

pub use model::{
    Actor, Anchor, Chain, ChainId, Comment, CommentId, FileChange, FileStatus, Patch, Review,
    ReviewHeader, ReviewId, ReviewRef, Round, RoundKind, Side, Status,
};
pub use rules::Reviews;
pub use store::{CommentFilter, NewReview, NewRound, ReviewFilter, ReviewStore};
