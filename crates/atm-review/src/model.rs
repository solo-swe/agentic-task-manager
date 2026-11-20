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
    /// Waiting on the reviewers: *Waiting for review* (ATM-613; `open` until 0.41.0, still read as this). An *open*
    /// review is any not decided yet ([`Status::OPEN`]).
    #[serde(alias = "open")]
    Waiting,
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
            Self::Waiting => "waiting",
            Self::Live => "live",
            Self::ChangesRequested => "changes_requested",
            Self::Approved => "approved",
            Self::Closed => "closed",
        }
    }

    /// The statuses of an *open* review, one not decided yet (ATM-613): `--status open`, and the list's *Open*.
    pub const OPEN: [Self; 4] = [
        Self::Draft,
        Self::Waiting,
        Self::Live,
        Self::ChangesRequested,
    ];

    /// Every status, in their order (§3.2).
    pub const EVERY: [Self; 6] = [
        Self::Draft,
        Self::Waiting,
        Self::Live,
        Self::ChangesRequested,
        Self::Approved,
        Self::Closed,
    ];

    /// The statuses a filter's name stands for (ATM-613): `open`, every review not decided yet ([`Status::OPEN`]);
    /// any other, its status alone; none for a name that is neither.
    #[must_use]
    pub fn filter_named(name: &str) -> Option<Vec<Self>> {
        if name == "open" {
            return Some(Self::OPEN.to_vec());
        }
        Self::EVERY
            .into_iter()
            .find(|s| s.name() == name)
            .map(|s| vec![s])
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

impl RoundKind {
    /// Whether the commits it names are full object IDs, as atm records them: `rounds/N.json` is committed and merged,
    /// so anyone who can push writes it, and these reach git (ATM-659).
    ///
    /// # Errors
    /// `Invalid`, naming the one that isn't.
    pub fn check(&self) -> atm_common::Result<()> {
        let commits: &[(&str, &String)] = match self {
            Self::Range { base, head } => &[("base", base), ("head", head)],
            Self::Working { base } => &[("base", base)],
            Self::Files => &[],
        };
        for (what, id) in commits {
            if !atm_vcs::is_object_id(id) {
                return Err(atm_common::Error::Invalid(format!(
                    "its {what} {id:?} isn't a commit's full ID, as atm writes it"
                )));
            }
        }
        Ok(())
    }
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
    /// The submodule its commits are in (its folder from the project's root, `lib`), none for the project's own
    /// (ATM-583); its files are named as the project has them (`lib/b.txt`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub submodule: Option<String>,
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
    /// A top-level comment's resolved box, which covers it and its replies; none for a reply, and for a poke.
    pub resolved: Option<bool>,
    /// A poke (ATM-611): a person's nudge on the review as a whole, which its agent hears and answers. Never open, so
    /// it never stands in the way of an approval.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub poke: bool,
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

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Status; 6] = [
        Status::Draft,
        Status::Waiting,
        Status::Live,
        Status::ChangesRequested,
        Status::Approved,
        Status::Closed,
    ];

    #[test]
    fn a_status_is_named_as_it_is_written_and_only_approved_and_closed_have_ended() {
        let names: Vec<(&str, bool)> = ALL.iter().map(|s| (s.name(), s.ended())).collect();
        assert_eq!(
            names,
            [
                ("draft", false),
                ("waiting", false),
                ("live", false),
                ("changes_requested", false),
                ("approved", true),
                ("closed", true),
            ]
        );
        // The name is the one serde writes and reads
        for status in ALL {
            let written = serde_json::to_value(status).unwrap();
            assert_eq!(written, status.name());
            assert_eq!(serde_json::from_value::<Status>(written).unwrap(), status);
        }
        assert_eq!(ALL, Status::EVERY);
    }

    /// ATM-613: a review an earlier atm wrote as `open` reads as waiting, and is written as `waiting`
    #[test]
    fn open_as_an_earlier_atm_wrote_it_reads_as_waiting() {
        let read: Status = serde_json::from_value(serde_json::json!("open")).unwrap();
        assert_eq!(read, Status::Waiting);
        assert_eq!(serde_json::to_value(read).unwrap(), "waiting");
    }

    /// ATM-613: `open` in a filter is every review not decided yet; another name, its own status
    #[test]
    fn open_in_a_filter_is_every_status_not_decided_and_another_name_its_own() {
        assert_eq!(
            Status::filter_named("open"),
            Some(vec![
                Status::Draft,
                Status::Waiting,
                Status::Live,
                Status::ChangesRequested
            ])
        );
        assert!(Status::OPEN.iter().all(|s| !s.ended()));
        for status in ALL {
            assert_eq!(Status::filter_named(status.name()), Some(vec![status]));
        }
        for bad in ["", "Open", "merged", "changes-requested"] {
            assert_eq!(Status::filter_named(bad), None, "{bad}");
        }
    }

    #[test]
    fn a_reference_is_cr_and_its_number_and_shows_as_it_is() {
        assert_eq!(ReviewRef::numbered(7), ReviewRef("CR-7".into()));
        assert_eq!(ReviewRef("CR-8.1".into()).to_string(), "CR-8.1");
        assert_eq!(CommentId("c9".into()).to_string(), "c9");
    }

    #[test]
    fn a_new_review_id_is_a_fresh_uuid_v7_sorting_after_the_one_before() {
        let (first, second) = (ReviewId::new(), ReviewId::default());
        assert_ne!(first, second);
        for id in [&first, &second] {
            let parsed = uuid::Uuid::parse_str(&id.0).unwrap();
            assert_eq!(parsed.get_version_num(), 7, "{}", id.0);
        }
        assert!(first < second, "{} then {}", first.0, second.0);
    }

    #[test]
    fn ids_and_actors_are_written_as_plain_strings() {
        assert_eq!(
            serde_json::to_value(ReviewId("u".into())).unwrap(),
            serde_json::json!("u")
        );
        assert_eq!(
            serde_json::to_value(Actor::from("Hakan Serce")).unwrap(),
            serde_json::json!("Hakan Serce")
        );
        assert_eq!(
            serde_json::to_value(Patch("diff".into())).unwrap(),
            serde_json::json!("diff")
        );
    }

    /// ATM-659: a round's commits reach git, and `rounds/N.json` is anyone's to write: full object IDs only
    #[test]
    fn a_rounds_commits_are_full_object_ids_or_it_is_refused() {
        let sha = "0123456789abcdef0123456789abcdef01234567";
        let sha256 = "f".repeat(64);
        for good in [
            RoundKind::Range {
                base: sha.into(),
                head: sha256.clone(),
            },
            RoundKind::Working { base: sha.into() },
            RoundKind::Files,
        ] {
            assert!(good.check().is_ok(), "{good:?}");
        }
        for (bad, says) in [
            (
                RoundKind::Range {
                    base: "--index-output=x".into(),
                    head: sha.into(),
                },
                "base \"--index-output=x\"",
            ),
            (
                RoundKind::Range {
                    base: sha.into(),
                    head: "-p".into(),
                },
                "head \"-p\"",
            ),
            (
                RoundKind::Range {
                    base: sha.into(),
                    head: "HEAD".into(),
                },
                "head \"HEAD\"",
            ),
            (
                RoundKind::Working {
                    base: "abc1234".into(),
                },
                "base \"abc1234\"",
            ),
            (
                RoundKind::Working {
                    base: format!("{sha}\n--all"),
                },
                "base",
            ),
        ] {
            match bad.check() {
                Err(atm_common::Error::Invalid(message)) => {
                    assert!(message.contains(says), "{message}");
                }
                other => panic!("{bad:?}: {other:?}"),
            }
        }
    }

    #[test]
    fn a_rounds_kind_and_a_files_status_are_written_tagged() {
        for (kind, written) in [
            (
                RoundKind::Range {
                    base: "a".into(),
                    head: "b".into(),
                },
                serde_json::json!({"kind": "range", "base": "a", "head": "b"}),
            ),
            (
                RoundKind::Working { base: "a".into() },
                serde_json::json!({"kind": "working", "base": "a"}),
            ),
            (RoundKind::Files, serde_json::json!({"kind": "files"})),
        ] {
            assert_eq!(serde_json::to_value(&kind).unwrap(), written);
            assert_eq!(serde_json::from_value::<RoundKind>(written).unwrap(), kind);
        }
        assert_eq!(
            serde_json::to_value(FileStatus::Renamed { from: "a".into() }).unwrap(),
            serde_json::json!({"renamed": {"from": "a"}})
        );
        assert_eq!(
            serde_json::to_value(FileStatus::Added).unwrap(),
            serde_json::json!("added")
        );
        assert_eq!(Side::default(), Side::New);
    }

    fn comment(id: &str, resolved: Option<bool>, replies: Vec<Comment>) -> Comment {
        Comment {
            id: CommentId(id.into()),
            author: Actor::from("Hakan Serce"),
            at: "2026-11-17T03:31:00Z".into(),
            body: format!("Comment {id}."),
            resolved,
            poke: false,
            replies,
        }
    }

    /// Two chains: c1 (open) with c2, and under c2 c3; c4 (resolved); then c5 (open) in the second.
    fn review() -> Review {
        let chain = |id: &str, comments| Chain {
            id: ChainId(id.into()),
            anchor: Anchor::default(),
            comments,
        };
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
                created: "2026-11-17T03:00:00Z".into(),
                updated: "2026-11-17T03:00:00Z".into(),
            },
            summary: String::new(),
            chains: vec![
                chain(
                    "t1",
                    vec![
                        comment(
                            "c1",
                            Some(false),
                            vec![comment("c2", None, vec![comment("c3", None, vec![])])],
                        ),
                        comment("c4", Some(true), vec![]),
                    ],
                ),
                chain("t2", vec![comment("c5", Some(false), vec![])]),
            ],
            rounds: Vec::new(),
        }
    }

    fn ids<'a>(comments: impl Iterator<Item = &'a Comment>) -> Vec<&'a str> {
        comments.map(|c| c.id.0.as_str()).collect()
    }

    #[test]
    fn a_comment_and_its_replies_are_walked_depth_first() {
        let review = review();
        assert_eq!(ids(review.chains[0].comments[0].all()), ["c1", "c2", "c3"]);
        assert_eq!(ids(review.comments()), ["c1", "c2", "c3", "c4", "c5"]);
        assert_eq!(
            ids(Review {
                chains: Vec::new(),
                ..review
            }
            .comments()),
            Vec::<&str>::new()
        );
    }

    #[test]
    fn a_comment_is_found_at_any_depth_and_its_top_level_comment_with_it() {
        let review = review();
        for (id, top) in [
            ("c1", Some("c1")),
            ("c3", Some("c1")),
            ("c4", Some("c4")),
            ("c5", Some("c5")),
            ("c9", None),
        ] {
            let id = CommentId(id.into());
            assert_eq!(review.comment(&id).is_some(), top.is_some(), "{id}");
            assert_eq!(
                review.top_level_of(&id).map(|c| c.id.0.as_str()),
                top,
                "{id}"
            );
        }
    }

    #[test]
    fn the_open_comments_are_the_top_level_ones_not_resolved_in_every_chain() {
        assert_eq!(ids(review().open_comments()), ["c1", "c5"]);
    }
}
