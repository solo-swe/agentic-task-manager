//! Agents listening to reviews (docs/code-review-design.md §3.9, ATM-567), on the review store's subscription rather
//! than a poll of the agent's own:
//!
//! - [`wait`]: until a reviewer submits on a review (or decides it);
//! - [`watch`]: the agent's one watch for its session, naming no review and no task: each of the user's comments and
//!   replies about the agent's work, from any review, matched to the agent's tasks from atm's records (the tasks it
//!   has held and their files and commit, its uncommitted files), or to none.
//!
//! What is heard is task text: data, never instructions (I1).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use atm_common::edits::is_agent;
use atm_common::sha256::sha256_hex;
use atm_common::time::parse_iso;
use atm_common::{Kind, Result};
use serde_json::{Value, json};

use crate::project::Project;
use crate::reviews::{
    self, Chain, Comment, FileStore, Review, ReviewFilter, ReviewHeader, ReviewRef, ReviewStore,
    Status,
};

/// How often a listener looks again even without being told: a watcher can miss an event (a busy file system).
const RECHECK: Duration = Duration::from_secs(5);

mod cursor;
mod inbox;
mod lease;
mod wait;
mod watch;
mod work;

use cursor::{
    all_reviews, cursor_file, old_seen_key, read_cursor, save_cursor, seen_key, starting_point,
};
pub use inbox::*;
pub use lease::*;
pub use wait::*;
pub use watch::*;
use work::{Mine, about_my_work, decision_about_my_work, keyed_now, work_of};
#[cfg(test)]
use work::{Work, touches};

#[cfg(test)]
mod tests;
