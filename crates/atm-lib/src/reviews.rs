//! Code reviews and tasks (docs/code-review-design.md §3.1's link, §3.8, ATM-565): the one place the two meet. A
//! review names its tasks (by key), each task links its reviews (`code_reviews`, by UUID and reference), and a
//! reviewer's decision on a review is every named task's review decision, recorded as the reviewer (I23: the user's,
//! never an agent's). A live review's approval is a task's when the task's change hasn't moved since (§3.5).
//!
//! The front ends call these, never `atm-review`'s store directly, so the tasks always follow.

use atm_common::edits::is_agent;
use atm_common::{Error, Kind, Result, TASK_DIR, Task, Vocabulary};
use serde_json::{Value, json};

pub use atm_review::files::comments_md;
pub use atm_review::folder::{FileStore, Found, list_everywhere};
pub use atm_review::rounds;
pub use atm_review::{
    Side,
    model::{FileStatus, Round},
};

pub use atm_review::rules::Verdict;
pub use atm_review::{
    Actor, Anchor, Chain, Comment, CommentFilter, CommentId, NewReview, NewRound, Patch, Review,
    ReviewFilter, ReviewHeader, ReviewId, ReviewRef, ReviewStore, Reviews, RoundKind, Status,
};

use crate::changes::change_task_status;
use crate::project::Project;
use crate::review_keys;
use crate::state::{Command, Options};

/// The project's reviews, in its task folder.
///
/// # Errors
/// The project wasn't opened from a folder, or its reviews' folder is a symlink.
pub fn store(project: &Project) -> Result<FileStore> {
    let root = project
        .root()
        .ok_or_else(|| Error::failed("this project has no folder to keep reviews in"))?;
    FileStore::open(&root.join(TASK_DIR))
}

mod commands;
mod doctor;
mod linking;
mod live;
mod naming;
mod repo;

pub use commands::*;
pub use doctor::*;
use linking::{
    Opening, actor_of, agent_at_work, agent_of_tasks, ask_review, id_of, link, openings, resolve,
};
pub(crate) use linking::{kind_of, named};
pub use live::*;
pub use naming::*;
pub use repo::*;

#[cfg(test)]
mod tests;
