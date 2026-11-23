//! What the reviews' pages read (docs/code-review-design.md §3.11, ATM-574): the list (search and filters, the latest
//! activity first, which need the user), and a review's page for one view of it: the whole change of a round, or a
//! round against an earlier one (§3.6), each file with its patch and the two sides to read it from (a commit, or a
//! tree a round is rebuilt to from its diff), and every chain placed in the round shown, or drifted. Both front ends
//! answer these (the REST API's read routes, the app's commands), with the project opened as the user.

use atm_common::{Error, Result, TASK_DIR};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::project::Project;
use crate::review_drafts::{Draft, Place};
use crate::reviews::{
    self, Actor, Anchor, Chain, FileStore, Found, Patch, Review, ReviewFilter, ReviewHeader,
    ReviewId, ReviewRef, ReviewStore, Round, RoundKind, Status, list_everywhere, rounds,
};
use atm_review::rounds::RoundDiffs;

fn me(project: &Project) -> Actor {
    Actor(project.actor().unwrap_or_default().to_string())
}

mod list;
mod page;
mod patches;
mod view;
mod waiting;

pub use list::*;
pub use page::*;
pub use patches::*;
#[cfg(test)]
use view::round_of;
use view::{drafts_of, shown_with};
pub use waiting::*;

#[cfg(test)]
mod tests;
