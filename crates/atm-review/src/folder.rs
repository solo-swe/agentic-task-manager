//! The file store (docs/code-review-design.md §3.3, §3.10): a project's reviews in `.agentic-task-manager/reviews/`,
//! one folder per review, committed with the code.
//!
//! ```text
//! reviews/
//!   CR-7-3fa1/         the number, and the end of its UUID (random: two clones' CR-8s are two folders)
//!     review.md        the header and the summary (files::review_md)
//!     comments.md      every chain (files::comments_md)
//!     rounds/1.json    what round 1 is
//!     rounds/1.diff    its diff
//! ```
//!
//! A new review's number comes from a counter the clone's worktrees share (`<git common dir>/atm/reviews/`, the task
//! folder's `reviews/` outside git), so two worktrees never take the same one. Two reviews with one number (made in two
//! clones, then merged) are `CR-8.1` and `CR-8.2`, in the order they were created (then by UUID, so every clone agrees);
//! `CR-8` then names both. Each review has its own lock (`atm-fs`'s), and every file is written whole, through a
//! temporary file renamed into place. Nothing is read or written through a symlink (I6).

use std::path::{Path, PathBuf};

use atm_common::home::write_text_atomic;
use atm_common::subscription::{OnChange, Subscription};
use atm_common::time::now_iso;
use atm_common::{Error, Result, Shown};
use serde_json::json;

use crate::files::comments_md::{self, CommentsFile};
use crate::files::review_md;
use crate::model::{
    Actor, Anchor, Chain, Comment, CommentId, Patch, Review, ReviewHeader, ReviewId, ReviewRef,
    Round, Status,
};
use crate::store::{CommentFilter, NewReview, NewRound, ReviewFilter, ReviewStore};

/// The folder of reviews in the task folder.
pub const REVIEWS_DIR: &str = "reviews";
const REVIEW_FILE: &str = "review.md";
const COMMENTS_FILE: &str = "comments.md";
const ROUNDS_DIR: &str = "rounds";
const COUNTER_FILE: &str = "counter.json";

/// A review as the folder holds it.
#[derive(Debug, Clone)]
struct Held {
    folder: PathBuf,
    review: Review,
    comments: CommentsFile,
}

/// A review's folder atm can't read (ATM-617): listed as such, never left out without a word.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Unreadable {
    /// The folder's name (`CR-13-3b8e`).
    pub folder: String,
    /// The review it is, by its folder's number (`CR-13`).
    pub reference: String,
    /// Why it can't be read.
    pub problem: String,
}

/// A project's reviews in its task folder.
#[derive(Debug, Clone)]
pub struct FileStore {
    reviews_dir: PathBuf,
    counter_dir: PathBuf,
}

fn io(path: &Path) -> impl Fn(std::io::Error) -> Error + '_ {
    move |e| Error::failed(format!("{}: {e}", path.shown()))
}

fn is_symlink(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink())
}

/// A file of a review's folder, read only if it is a regular file (never through a symlink, I6); none when missing.
fn read_file(path: &Path) -> Result<Option<String>> {
    if is_symlink(path) {
        return Err(Error::Refused(format!(
            "{} is a symlink; atm reads only regular files there",
            path.shown()
        )));
    }
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(io(path)(e)),
    }
}

/// `CR-<n>-<4 hex>`: the number, if a folder name is a review's.
fn folder_number(name: &str) -> Option<u32> {
    let (number, tag) = name.strip_prefix("CR-")?.split_once('-')?;
    (tag.len() == 4
        && tag
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()))
    .then(|| number.parse().ok())
    .flatten()
}

/// The folder a new review gets: its number and the last four hex digits of its UUID (v7's random end).
fn folder_name(number: u32, id: &ReviewId) -> String {
    let hex: String = id.0.chars().filter(char::is_ascii_hexdigit).collect();
    format!("CR-{number}-{}", &hex[hex.len().saturating_sub(4)..])
}

/// Give each review its reference: `CR-n`, or `CR-n.1`, `CR-n.2`, … when several share n (by creation, then UUID).
fn number_references(held: &mut [Held]) {
    held.sort_by(|a, b| {
        let (a, b) = (&a.review.header, &b.review.header);
        (a.number, &a.created, &a.id).cmp(&(b.number, &b.created, &b.id))
    });
    let mut at = 0;
    while at < held.len() {
        let number = held[at].review.header.number;
        let same = held[at..]
            .iter()
            .take_while(|h| h.review.header.number == number)
            .count();
        for (i, h) in held[at..at + same].iter_mut().enumerate() {
            h.review.header.reference = if same == 1 {
                ReviewRef::numbered(number)
            } else {
                ReviewRef(format!("CR-{number}.{}", i + 1))
            };
        }
        at += same;
    }
}

impl FileStore {
    /// The reviews of the project whose task folder is `task_dir` (`.agentic-task-manager`).
    ///
    /// # Errors
    /// The task folder, or its `reviews/`, is a symlink (I6).
    pub fn open(task_dir: &Path) -> Result<Self> {
        let reviews_dir = task_dir.join(REVIEWS_DIR);
        for path in [task_dir, reviews_dir.as_path()] {
            if is_symlink(path) {
                return Err(Error::Refused(format!(
                    "{} is a symlink; atm keeps reviews only in a real folder",
                    path.shown()
                )));
            }
        }
        let root = task_dir.parent().unwrap_or(task_dir);
        let counter_dir = match atm_fs::git::where_in_git("--git-common-dir", root) {
            Some(common) => root.join(common).join("atm").join(REVIEWS_DIR),
            None => reviews_dir.clone(),
        };
        Ok(Self {
            reviews_dir,
            counter_dir,
        })
    }

    /// The folder the reviews are in.
    #[must_use]
    pub fn folder(&self) -> &Path {
        &self.reviews_dir
    }

    /// The folder of the review `header` is of, in [`FileStore::folder`] (whether or not it exists).
    #[must_use]
    pub fn folder_of(&self, header: &ReviewHeader) -> PathBuf {
        self.reviews_dir
            .join(folder_name(header.number, &header.id))
    }

    /// The first round of the review `header` is of, as its folder has it; none when it has no readable one (ATM-630:
    /// the commit it was made at says which task its keys meant then).
    #[must_use]
    pub fn first_round(&self, header: &ReviewHeader) -> Option<Round> {
        let rounds = self.folder_of(header).join(ROUNDS_DIR);
        if is_symlink(&rounds) {
            return None;
        }
        let path = rounds.join("1.json");
        let text = read_file(&path).ok()??;
        serde_json::from_str::<Round>(&text)
            .ok()
            .filter(|r| r.number == 1 && r.kind.check().is_ok())
    }

    /// One review's folder, read: its header, summary, comments and rounds.
    fn read_review(folder: &Path) -> Result<Held> {
        if is_symlink(folder) {
            return Err(Error::Refused(format!(
                "{} is a symlink; atm reads reviews only from real folders",
                folder.shown()
            )));
        }
        let text = read_file(&folder.join(REVIEW_FILE))?
            .ok_or_else(|| Error::failed(format!("{} has no {REVIEW_FILE}", folder.shown())))?;
        let (header, summary) = review_md::read(&text)
            .map_err(|e| Error::failed(format!("{}: {e}", folder.join(REVIEW_FILE).shown())))?;
        let comments =
            comments_md::read(&read_file(&folder.join(COMMENTS_FILE))?.unwrap_or_default());
        let rounds_dir = folder.join(ROUNDS_DIR);
        if is_symlink(&rounds_dir) {
            return Err(Error::Refused(format!(
                "{} is a symlink",
                rounds_dir.shown()
            )));
        }
        let mut rounds: Vec<Round> = Vec::new();
        for entry in std::fs::read_dir(&rounds_dir)
            .into_iter()
            .flatten()
            .flatten()
        {
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(number) = name
                .strip_suffix(".json")
                .and_then(|n| n.parse::<u32>().ok())
            {
                let text = read_file(&entry.path())?.unwrap_or_default();
                let round: Round = serde_json::from_str(&text)
                    .map_err(|e| Error::failed(format!("{}: {e}", entry.path().shown())))?;
                if round.number != number {
                    return Err(Error::failed(format!(
                        "{} says it is round {}",
                        entry.path().shown(),
                        round.number
                    )));
                }
                // Its commits reach git: full IDs only, never a name git could read as an option (ATM-659)
                round
                    .kind
                    .check()
                    .map_err(|e| Error::failed(format!("{}: {e}", entry.path().shown())))?;
                rounds.push(round);
            }
        }
        rounds.sort_by_key(|r| r.number);
        Ok(Held {
            folder: folder.to_path_buf(),
            review: Review {
                header,
                summary,
                chains: comments.chains.clone(),
                rounds,
            },
            comments,
        })
    }

    /// Every review, with its reference given, what couldn't be read (a line each), and the reviews that couldn't.
    fn read_all(&self) -> Result<(Vec<Held>, Vec<String>, Vec<Unreadable>)> {
        let mut held = Vec::new();
        let mut problems = Vec::new();
        let mut unreadable = Vec::new();
        let entries = match std::fs::read_dir(&self.reviews_dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok((held, problems, unreadable));
            }
            Err(e) => return Err(io(&self.reviews_dir)(e)),
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(number) = folder_number(&name) else {
                continue;
            };
            match Self::read_review(&entry.path()) {
                Ok(review) => {
                    problems.extend(
                        review
                            .comments
                            .problems()
                            .into_iter()
                            .map(|p| format!("{name}: {p}")),
                    );
                    held.push(review);
                }
                Err(e) => {
                    problems.push(format!("{name}: {e}"));
                    unreadable.push(Unreadable {
                        folder: name,
                        reference: format!("CR-{number}"),
                        problem: e.to_string(),
                    });
                }
            }
        }
        number_references(&mut held);
        unreadable.sort_by(|a, b| a.folder.cmp(&b.folder));
        Ok((held, problems, unreadable))
    }

    /// The reviews' folders atm can't read, by folder (ATM-617: `atm review list` and the Reviews tab list them).
    ///
    /// # Errors
    /// The reviews' folder can't be read.
    pub fn unreadable(&self) -> Result<Vec<Unreadable>> {
        Ok(self.read_all()?.2)
    }

    /// What the reviews' files hold that atm can't read: for `atm doctor`.
    ///
    /// # Errors
    /// The reviews' folder can't be read.
    pub fn problems(&self) -> Result<Vec<String>> {
        Ok(self.read_all()?.1)
    }

    /// Every review the folder can read, whole, with its comments (ATM-606: what waits on the user, in one read
    /// rather than a read per review).
    ///
    /// # Errors
    /// The reviews' folder can't be read.
    pub fn reviews(&self) -> Result<Vec<Review>> {
        Ok(self.read_all()?.0.into_iter().map(|h| h.review).collect())
    }

    /// What changes when any review does, without reading one (ATM-606, for a summary's cache and its `ETag`): how
    /// many files the reviews' folders hold, and the latest modification among them in nanoseconds. Every change to a
    /// review writes one of its files (`review.md`, `comments.md`, a round's), so a change moves one or the other.
    #[must_use]
    pub fn stamp(&self) -> (u64, u128) {
        let (mut files, mut latest) = (0u64, 0u128);
        let mut look = |dir: &Path| {
            for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
                let Ok(meta) = entry.metadata() else { continue };
                files += 1;
                let at = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_nanos());
                latest = latest.max(at);
            }
        };
        for entry in std::fs::read_dir(&self.reviews_dir)
            .into_iter()
            .flatten()
            .flatten()
        {
            let path = entry.path();
            if path.is_dir() {
                look(&path);
                look(&path.join("rounds"));
            }
        }
        (files, latest)
    }

    /// The review a reference names, as the folder holds it.
    fn find(&self, reference: &ReviewRef) -> Result<Held> {
        let (held, _, unreadable) = self.read_all()?;
        if let Some(found) = held.iter().find(|h| {
            h.review.header.reference == *reference || h.review.header.id.0 == reference.0
        }) {
            return Ok(found.clone());
        }
        // `CR-8` when CR-8.1 and CR-8.2 share the number
        let prefix = format!("{}.", reference.0);
        let clashing: Vec<&Held> = held
            .iter()
            .filter(|h| h.review.header.reference.0.starts_with(&prefix))
            .collect();
        if clashing.is_empty() {
            // A review whose folder atm can't read says why, rather than that there is none (ATM-659: a round refused)
            if let Some(bad) = unreadable.iter().find(|u| u.reference == reference.0) {
                return Err(Error::failed(format!(
                    "{reference} can't be read: {}",
                    bad.problem
                )));
            }
            return Err(Error::NotFound(format!("no review {reference}")));
        }
        let names: Vec<String> = clashing
            .iter()
            .map(|h| format!("{} ({})", h.review.header.reference, h.review.header.title))
            .collect();
        Err(Error::Invalid(format!(
            "{reference} is {} reviews: {} (two clones took the number; name one)",
            clashing.len(),
            names.join(", ")
        )))
    }

    fn by_id(&self, id: &ReviewId) -> Result<Held> {
        self.find(&ReviewRef(id.0.clone()))
    }

    /// Write a review's header, summary and comments, each whole.
    fn write_review(held: &Held) -> Result<()> {
        let review_path = held.folder.join(REVIEW_FILE);
        let text = review_md::write(&held.review.header, &held.review.summary)?;
        write_text_atomic(&review_path, &text).map_err(io(&review_path))?;
        let comments_path = held.folder.join(COMMENTS_FILE);
        let comments = CommentsFile {
            chains: held.review.chains.clone(),
            unread: held.comments.unread.clone(),
        };
        write_text_atomic(&comments_path, &comments_md::write(&comments))
            .map_err(io(&comments_path))
    }

    fn write_round(folder: &Path, round: &Round, patch: &Patch) -> Result<()> {
        let rounds_dir = folder.join(ROUNDS_DIR);
        std::fs::create_dir_all(&rounds_dir).map_err(io(&rounds_dir))?;
        let json_path = rounds_dir.join(format!("{}.json", round.number));
        let json =
            serde_json::to_string_pretty(round).map_err(|e| Error::failed(e.to_string()))? + "\n";
        write_text_atomic(&json_path, &json).map_err(io(&json_path))?;
        let diff_path = rounds_dir.join(format!("{}.diff", round.number));
        write_text_atomic(&diff_path, &patch.0).map_err(io(&diff_path))
    }

    /// Change one review under its lock: read again there, apply `edit`, mark it updated, write it.
    fn change<T>(&self, id: &ReviewId, edit: impl FnOnce(&mut Review) -> Result<T>) -> Result<T> {
        let folder = self.by_id(id)?.folder;
        let _lock = atm_fs::lock::lock(&folder, None)?;
        let mut held = Self::read_review(&folder)?;
        let out = edit(&mut held.review)?;
        held.review.header.updated = now_iso();
        Self::write_review(&held)?;
        Ok(out)
    }

    /// The next number, from the counter the clone's worktrees share, never below what this folder already has.
    fn take_number(&self) -> Result<u32> {
        std::fs::create_dir_all(&self.counter_dir).map_err(io(&self.counter_dir))?;
        let _lock = atm_fs::lock::lock(&self.counter_dir, None)?;
        let path = self.counter_dir.join(COUNTER_FILE);
        let counted = atm_fs::read_json(&path)
            .and_then(|v| v.get("next_number").and_then(serde_json::Value::as_u64))
            .and_then(|n| u32::try_from(n).ok())
            .unwrap_or(1);
        let here = std::fs::read_dir(&self.reviews_dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| folder_number(&e.file_name().to_string_lossy()))
            .max()
            .map_or(1, |n| n + 1);
        let number = counted.max(here);
        let text = json!({ "next_number": number + 1 }).to_string() + "\n";
        write_text_atomic(&path, &text).map_err(io(&path))?;
        Ok(number)
    }
}

impl ReviewStore for FileStore {
    fn list_reviews(&self, filter: &ReviewFilter) -> Result<Vec<ReviewHeader>> {
        let (held, ..) = self.read_all()?;
        let mut headers: Vec<ReviewHeader> = held
            .into_iter()
            .map(|h| h.review.header)
            .filter(|h| filter.statuses.is_empty() || filter.statuses.contains(&h.status))
            .filter(|h| filter.task.as_ref().is_none_or(|t| h.tasks.contains(t)))
            .collect();
        headers.sort_by(|a, b| (&b.created, &b.id).cmp(&(&a.created, &a.id)));
        Ok(headers)
    }

    fn get_review(&self, reference: &ReviewRef) -> Result<Review> {
        Ok(self.find(reference)?.review)
    }

    fn create_review(&mut self, new: NewReview) -> Result<ReviewHeader> {
        let number = self.take_number()?;
        let id = ReviewId::new();
        let now = now_iso();
        let folder = self.reviews_dir.join(folder_name(number, &id));
        std::fs::create_dir_all(&folder).map_err(io(&folder))?;
        let _lock = atm_fs::lock::lock(&folder, None)?;
        let header = ReviewHeader {
            id,
            reference: ReviewRef::numbered(number),
            number,
            title: new.title,
            status: new.status,
            tasks: new.tasks,
            author: new.author,
            reviewers: new.reviewers,
            created: now.clone(),
            updated: now.clone(),
        };
        let mut review = Review {
            header: header.clone(),
            summary: new.summary,
            chains: Vec::new(),
            rounds: Vec::new(),
        };
        review.push_round(&new.first, &now);
        Self::write_round(&folder, &review.rounds[0], &new.first.patch)?;
        Self::write_review(&Held {
            folder,
            review,
            comments: CommentsFile::default(),
        })?;
        Ok(header)
    }

    fn set_status(&mut self, id: &ReviewId, status: Status, _by: &Actor) -> Result<()> {
        self.change(id, |review| {
            review.header.status = status;
            Ok(())
        })
    }

    fn edit_summary(&mut self, id: &ReviewId, title: &str, summary: &str) -> Result<()> {
        self.change(id, |review| {
            title.clone_into(&mut review.header.title);
            summary.clone_into(&mut review.summary);
            Ok(())
        })
    }

    fn set_tasks(&mut self, id: &ReviewId, tasks: Vec<String>) -> Result<()> {
        self.change(id, |review| {
            review.header.tasks = tasks;
            Ok(())
        })
    }

    fn set_author(&mut self, id: &ReviewId, author: &Actor) -> Result<()> {
        self.change(id, |review| {
            review.header.author = Some(author.clone());
            Ok(())
        })
    }

    fn add_round(&mut self, id: &ReviewId, round: NewRound) -> Result<u32> {
        let folder = self.by_id(id)?.folder;
        self.change(id, |review| {
            let number = review.push_round(&round, &now_iso());
            let added = review.rounds.last().expect("just added");
            Self::write_round(&folder, added, &round.patch)?;
            Ok(number)
        })
    }

    fn round_diff(&self, id: &ReviewId, round: u32) -> Result<Patch> {
        let folder = self.by_id(id)?.folder;
        let path = folder.join(ROUNDS_DIR).join(format!("{round}.diff"));
        read_file(&path)?
            .map(Patch)
            .ok_or_else(|| Error::NotFound(format!("no round {round}")))
    }

    fn list_comments(&self, id: &ReviewId, filter: &CommentFilter) -> Result<Vec<Chain>> {
        Ok(self.by_id(id)?.review.chains_by(filter))
    }

    fn get_comment(&self, id: &ReviewId, comment: &CommentId) -> Result<Comment> {
        self.by_id(id)?.review.comment_owned(comment)
    }

    fn add_comment(
        &mut self,
        id: &ReviewId,
        anchor: Anchor,
        body: &str,
        by: &Actor,
    ) -> Result<CommentId> {
        self.change(id, |review| {
            Ok(review.push_chain(anchor, body, by, &now_iso()))
        })
    }

    fn add_poke(
        &mut self,
        id: &ReviewId,
        anchor: Anchor,
        body: &str,
        by: &Actor,
    ) -> Result<CommentId> {
        self.change(id, |review| {
            Ok(review.push_poke(anchor, body, by, &now_iso()))
        })
    }

    fn reply(
        &mut self,
        id: &ReviewId,
        to: &CommentId,
        body: &str,
        by: &Actor,
    ) -> Result<CommentId> {
        self.change(id, |review| review.push_reply(to, body, by, &now_iso()))
    }

    fn edit_comment(&mut self, id: &ReviewId, comment: &CommentId, body: &str) -> Result<()> {
        self.change(id, |review| review.set_body(comment, body))
    }

    fn resolve(&mut self, id: &ReviewId, top_level: &CommentId, resolved: bool) -> Result<()> {
        self.change(id, |review| review.set_resolved(top_level, resolved))
    }

    fn subscribe(&self, on_change: OnChange) -> Result<Subscription> {
        std::fs::create_dir_all(&self.reviews_dir).map_err(io(&self.reviews_dir))?;
        atm_fs::watch::watch_tree(std::slice::from_ref(&self.reviews_dir), on_change)
    }
}

/// A review found in one of the project's working copies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// Its header.
    pub header: ReviewHeader,
    /// The working copy's branch, none outside git or on a detached HEAD's copy that has none.
    pub branch: Option<String>,
    /// The working copy's folder.
    pub working_copy: PathBuf,
}

/// The reviews of every working copy of the project at `root` (its main checkout and its git worktrees, §3.10), each
/// with its branch; a review in several (a branch merged into another) is listed once, from the first copy (the main).
///
/// # Errors
/// The main copy's reviews can't be read.
pub fn list_everywhere(root: &Path, filter: &ReviewFilter) -> Result<Vec<Found>> {
    let mut copies = vec![root.to_path_buf()];
    if let Some(list) = atm_fs::git::git(&["worktree", "list", "--porcelain"], root) {
        for line in list.lines() {
            if let Some(path) = line.strip_prefix("worktree ") {
                let path = atm_common::pathtext::real(Path::new(path));
                if !copies.contains(&path) {
                    copies.push(path);
                }
            }
        }
    }
    let mut found: Vec<Found> = Vec::new();
    for (i, copy) in copies.iter().enumerate() {
        let task_dir = copy.join(atm_common::TASK_DIR);
        let headers = match FileStore::open(&task_dir).and_then(|s| s.list_reviews(filter)) {
            Ok(headers) => headers,
            Err(e) if i == 0 => return Err(e),
            Err(_) => continue,
        };
        let branch = atm_fs::git::checked_out_branch(copy);
        for header in headers {
            if !found.iter().any(|f| f.header.id == header.id) {
                found.push(Found {
                    header,
                    branch: branch.clone(),
                    working_copy: copy.clone(),
                });
            }
        }
    }
    Ok(found)
}

#[cfg(test)]
mod tests;
