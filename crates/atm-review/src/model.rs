//! A review and what it is made of (docs/code-review-design.md §3.2 to §3.6). Plain data: what may change it, and how,
//! is [`crate::rules`]'.

use serde::{Deserialize, Serialize};

/// A review's identity: a UUID, which merges and references use underneath (§3.3). Stores give it; it never changes.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ReviewId(pub String);

impl ReviewId {
    /// A new identity, a UUID v7 (they sort by creation).
    #[must_use]
    pub fn new() -> Self {
        Self(uuid::Uuid::now_v7().to_string())
    }
}

impl Default for ReviewId {
    fn default() -> Self {
        Self::new()
    }
}

/// What people type and read for a review: `CR-7`, or `CR-8.1` when two clones took the same number (§3.10), or its
/// [`ReviewId`]. The store gives references and resolves them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ReviewRef(pub String);

impl ReviewRef {
    /// The reference `CR-<number>`.
    #[must_use]
    pub fn numbered(number: u32) -> Self {
        Self(format!("CR-{number}"))
    }
}

impl std::fmt::Display for ReviewRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Who did something: a person (`Hakan Serce`) or an agent (`claude:0f56a916`), as atm's actor names them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Actor(pub String);

impl From<&str> for Actor {
    fn from(name: &str) -> Self {
        Self(name.to_string())
    }
}

/// Where a review stands (§3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// The author is still putting it together; not waiting on anyone.
    Draft,
    /// Waiting on the reviewers.
    Open,
    /// The change is being made; reviewers comment as it grows (§3.5).
    Live,
    /// Sent back for fixes.
    ChangesRequested,
    /// Approved, every top-level comment resolved.
    Approved,
    /// Ended without approval (abandoned, replaced).
    Closed,
}

impl Status {
    /// Its name as written in `review.md` and `--json`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Open => "open",
            Self::Live => "live",
            Self::ChangesRequested => "changes_requested",
            Self::Approved => "approved",
            Self::Closed => "closed",
        }
    }

    /// A review that has ended: approved or closed.
    #[must_use]
    pub fn ended(self) -> bool {
        matches!(self, Self::Approved | Self::Closed)
    }
}

/// A review's header (`review.md`'s, §3.3): what lists and searches need.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewHeader {
    /// Its identity.
    pub id: ReviewId,
    /// What people call it: `CR-7`.
    pub reference: ReviewRef,
    /// Its number in the project.
    pub number: u32,
    /// One line.
    pub title: String,
    /// Where it stands.
    pub status: Status,
    /// The keys of the tasks it reviews (`ATM-482`), none for a review of no task (§3.2).
    pub tasks: Vec<String>,
    /// Whose change it is: who asked for it, or, for a live review a person opened on an agent's working copy, the
    /// agent at work there; none until it is known (a task linked, or the first round of the change, names it).
    pub author: Option<Actor>,
    /// Who decides it; empty: anyone but the author.
    pub reviewers: Vec<Actor>,
    /// When it was opened (ISO 8601).
    pub created: String,
    /// When it last changed.
    pub updated: String,
}

/// A whole review: its header, the author's summary, its chains of comments and its rounds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Review {
    /// The header.
    pub header: ReviewHeader,
    /// The author's summary, in Markdown.
    pub summary: String,
    /// Every chain of comments, in the order of the files they are about.
    pub chains: Vec<Chain>,
    /// Its rounds, the first first.
    pub rounds: Vec<Round>,
}

/// What a round's diff was made from (§3.2, §3.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RoundKind {
    /// A commit range.
    Range {
        /// The commit it starts from.
        base: String,
        /// The commit it ends at.
        head: String,
    },
    /// The working tree against a commit: uncommitted changes, or a live review's checkpoint (§3.5).
    Working {
        /// The commit it is against.
        base: String,
    },
    /// Files as they are, reviewed whole (a plan, a design), as if added.
    Files,
}

/// How a file changed in a round.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileStatus {
    /// New.
    Added,
    /// Changed.
    Modified,
    /// Gone.
    Deleted,
    /// Moved, from this path.
    Renamed {
        /// Where it was.
        from: String,
    },
}

/// A file a round changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileChange {
    /// Its path, from the project's root.
    pub path: String,
    /// How it changed.
    pub status: FileStatus,
    /// A binary file: listed, never in the diff (§3.3).
    pub binary: bool,
}

/// A round: what the author asked the reviewers to look at (§3.6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Round {
    /// 1 for the first.
    pub number: u32,
    /// What it was made from.
    pub kind: RoundKind,
    /// Its files.
    pub files: Vec<FileChange>,
    /// Who made it.
    pub by: Actor,
    /// When.
    pub at: String,
}

/// A round's diff, as git prints it (§3.3).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Patch(pub String);

/// Which side of a file's change a comment is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    /// The round's result (the usual).
    #[default]
    New,
    /// Before the round.
    Old,
}

/// What a chain is about (§3.6): a round, a file and its lines on one side, and the text quoted; or the review as a
/// whole (no file).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Anchor {
    /// The round it was made on.
    pub round: u32,
    /// The file, none for a chain about the whole review.
    pub path: Option<String>,
    /// The side.
    pub side: Side,
    /// The first and last line (1-based), none for a whole file.
    pub lines: Option<(u32, u32)>,
    /// The text it was made on: the lines, or the selection in the rendered view.
    pub quote: Option<String>,
}

/// A chain's ID in its review: `t4`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ChainId(pub String);

/// A comment's ID in its review: `c9`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CommentId(pub String);

impl std::fmt::Display for CommentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A comment, with the replies to it nested under it (§3.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Comment {
    /// Its ID.
    pub id: CommentId,
    /// Who wrote it.
    pub author: Actor,
    /// When (ISO 8601).
    pub at: String,
    /// Its text, in Markdown.
    pub body: String,
    /// A top-level comment's resolved box, which covers it and its replies; none for a reply.
    pub resolved: Option<bool>,
    /// The replies to it, oldest first.
    pub replies: Vec<Comment>,
}

impl Comment {
    /// This comment and every reply under it, depth first.
    pub fn all(&self) -> Box<dyn Iterator<Item = &Comment> + '_> {
        Box::new(std::iter::once(self).chain(self.replies.iter().flat_map(Comment::all)))
    }
}

/// A chain: what it is about, and its top-level comments, each with its replies (§3.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chain {
    /// Its ID.
    pub id: ChainId,
    /// What it is about.
    pub anchor: Anchor,
    /// Its top-level comments, oldest first.
    pub comments: Vec<Comment>,
}

impl Review {
    /// Every comment in the review, replies included.
    pub fn comments(&self) -> impl Iterator<Item = &Comment> {
        self.chains
            .iter()
            .flat_map(|chain| chain.comments.iter().flat_map(Comment::all))
    }

    /// The comment with this ID, if there is one.
    #[must_use]
    pub fn comment(&self, id: &CommentId) -> Option<&Comment> {
        self.comments().find(|c| &c.id == id)
    }

    /// The top-level comments that aren't resolved: what keeps the review from being approved.
    pub fn open_comments(&self) -> impl Iterator<Item = &Comment> {
        self.chains
            .iter()
            .flat_map(|chain| chain.comments.iter())
            .filter(|c| c.resolved == Some(false))
    }

    /// The comment `id` is in, as the top-level comment it belongs to (itself, for a top-level one).
    #[must_use]
    pub fn top_level_of(&self, id: &CommentId) -> Option<&Comment> {
        self.chains
            .iter()
            .flat_map(|chain| chain.comments.iter())
            .find(|top| top.all().any(|c| &c.id == id))
    }
}
