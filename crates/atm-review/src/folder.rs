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

    /// Every review, with its reference given, and what couldn't be read (a line each).
    fn read_all(&self) -> Result<(Vec<Held>, Vec<String>)> {
        let mut held = Vec::new();
        let mut problems = Vec::new();
        let entries = match std::fs::read_dir(&self.reviews_dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((held, problems)),
            Err(e) => return Err(io(&self.reviews_dir)(e)),
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if folder_number(&name).is_none() {
                continue;
            }
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
                Err(e) => problems.push(format!("{name}: {e}")),
            }
        }
        number_references(&mut held);
        Ok((held, problems))
    }

    /// What the reviews' files hold that atm can't read: for `atm doctor`.
    ///
    /// # Errors
    /// The reviews' folder can't be read.
    pub fn problems(&self) -> Result<Vec<String>> {
        Ok(self.read_all()?.1)
    }

    /// The review a reference names, as the folder holds it.
    fn find(&self, reference: &ReviewRef) -> Result<Held> {
        let (held, _) = self.read_all()?;
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
        let (held, _) = self.read_all()?;
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
mod tests {
    use std::path::Path;
    use std::sync::Arc;
    use std::time::Duration;

    use super::{FileStore, folder_name, list_everywhere};
    use crate::files::review_md;
    use crate::model::{
        Actor, Anchor, Patch, ReviewHeader, ReviewId, ReviewRef, RoundKind, Status,
    };
    use crate::rules::{Reviews, Verdict};
    use crate::store::{NewReview, NewRound, ReviewFilter, ReviewStore};
    use crate::test_dir::TestDir;

    fn agent() -> Actor {
        Actor::from("claude:0f56a916")
    }
    fn hakan() -> Actor {
        Actor::from("Hakan Serce")
    }
    fn new_review(title: &str) -> NewReview {
        NewReview {
            title: title.into(),
            summary: "Why and what.".into(),
            tasks: vec!["ATM-482".into()],
            author: agent(),
            reviewers: vec![hakan()],
            status: Status::Open,
            first: NewRound {
                kind: RoundKind::Range {
                    base: "a1".into(),
                    head: "b2".into(),
                },
                files: Vec::new(),
                patch: Patch("diff --git a/x b/x\n+one\n".into()),
                by: agent(),
            },
        }
    }
    fn git(args: &[&str], cwd: &Path) {
        let mut all = vec!["-c", "user.name=t", "-c", "user.email=t@example.com"];
        all.extend_from_slice(args);
        assert!(atm_fs::git::git(&all, cwd).is_some(), "git {args:?}");
    }

    #[test]
    fn a_review_is_kept_in_its_folder_and_read_back_by_another_store() {
        let dir = TestDir::new();
        let task_dir = dir.path().join(".agentic-task-manager");
        let mut store = FileStore::open(&task_dir).unwrap();
        let mut reviews = Reviews::new(&mut store);
        let header = reviews.create(new_review("Claim hold on Windows")).unwrap();
        assert_eq!(header.reference, ReviewRef::numbered(1));
        let cr = header.reference.clone();
        let anchor = Anchor {
            round: 1,
            path: Some("src/a.rs".into()),
            lines: Some((3, 4)),
            quote: Some("let shell = 0;".into()),
            ..Anchor::default()
        };
        let question = reviews.comment(&cr, anchor, "Why?", &hakan()).unwrap();
        reviews.reply(&cr, &question, "Because.", &agent()).unwrap();
        reviews
            .submit(&cr, Verdict::RequestChanges, None, &hakan())
            .unwrap();
        let mut second = new_review("").first;
        second.patch = Patch("diff --git a/x b/x\n+two\n".into());
        assert_eq!(reviews.add_round(&cr, second).unwrap(), 2);

        let folder = task_dir.join("reviews").join(folder_name(1, &header.id));
        for file in [
            "review.md",
            "comments.md",
            "rounds/1.json",
            "rounds/1.diff",
            "rounds/2.diff",
        ] {
            assert!(folder.join(file).is_file(), "{file}");
        }
        let comments = std::fs::read_to_string(folder.join("comments.md")).unwrap();
        assert!(
            comments.starts_with("## src/a.rs:3-4 · round 1 · t1\n> let shell = 0;\n"),
            "{comments}"
        );

        // Another store (another process) reads the same
        let again = FileStore::open(&task_dir).unwrap();
        let review = again.get_review(&cr).unwrap();
        assert_eq!(review.header.status, Status::Open);
        assert_eq!(review.rounds.len(), 2);
        assert_eq!(review.chains[0].comments[0].replies[0].body, "Because.");
        assert_eq!(
            again.round_diff(&header.id, 2).unwrap().0,
            "diff --git a/x b/x\n+two\n"
        );
        assert_eq!(
            again
                .get_review(&ReviewRef(header.id.0.clone()))
                .unwrap()
                .header
                .id,
            header.id
        );
        assert_eq!(
            again.list_reviews(&ReviewFilter::default()).unwrap().len(),
            1
        );
        assert_eq!(again.problems().unwrap(), Vec::<String>::new());
    }

    #[test]
    fn worktrees_of_one_clone_never_take_the_same_number_and_are_listed_together() {
        let dir = TestDir::new();
        let main = dir.path().join("main");
        std::fs::create_dir(&main).unwrap();
        git(&["init", "-q", "-b", "main"], &main);
        git(&["commit", "-q", "--allow-empty", "-m", "start"], &main);
        let wt = dir.path().join("wt");
        git(
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "agent-2",
                wt.to_str().unwrap(),
            ],
            &main,
        );

        let mut here = FileStore::open(&main.join(".agentic-task-manager")).unwrap();
        let mut there = FileStore::open(&wt.join(".agentic-task-manager")).unwrap();
        assert_eq!(here.create_review(new_review("one")).unwrap().number, 1);
        assert_eq!(there.create_review(new_review("two")).unwrap().number, 2);
        assert_eq!(here.create_review(new_review("three")).unwrap().number, 3);

        let found = list_everywhere(&main, &ReviewFilter::default()).unwrap();
        let listed: Vec<(u32, Option<&str>)> = found
            .iter()
            .map(|f| (f.header.number, f.branch.as_deref()))
            .collect();
        assert!(
            listed.contains(&(3, Some("main"))) && listed.contains(&(1, Some("main"))),
            "{listed:?}"
        );
        assert!(listed.contains(&(2, Some("agent-2"))), "{listed:?}");
    }

    #[test]
    fn outside_git_the_counter_is_in_the_reviews_folder() {
        let dir = TestDir::new();
        let task_dir = dir.path().join(".agentic-task-manager");
        let mut store = FileStore::open(&task_dir).unwrap();
        store.create_review(new_review("one")).unwrap();
        assert!(task_dir.join("reviews/counter.json").is_file());
        assert_eq!(store.create_review(new_review("two")).unwrap().number, 2);
    }

    /// A review written as another clone would have, with its own number.
    fn plant(task_dir: &Path, number: u32, title: &str, created: &str) -> ReviewHeader {
        let id = ReviewId::new();
        let header = ReviewHeader {
            id: id.clone(),
            reference: ReviewRef::numbered(number),
            number,
            title: title.into(),
            status: Status::Open,
            tasks: Vec::new(),
            author: agent(),
            reviewers: Vec::new(),
            created: created.into(),
            updated: created.into(),
        };
        let folder = task_dir.join("reviews").join(folder_name(number, &id));
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(
            folder.join("review.md"),
            review_md::write(&header, "").unwrap(),
        )
        .unwrap();
        header
    }

    #[test]
    fn two_clones_cr_8_are_cr_8_1_and_cr_8_2_and_cr_8_names_both() {
        let dir = TestDir::new();
        let task_dir = dir.path().join(".agentic-task-manager");
        let later = plant(&task_dir, 8, "Mine", "2026-11-17T05:00:00Z");
        let earlier = plant(&task_dir, 8, "Theirs", "2026-11-17T04:00:00Z");
        plant(&task_dir, 7, "Alone", "2026-11-17T03:00:00Z");
        let store = FileStore::open(&task_dir).unwrap();
        assert_eq!(
            store
                .get_review(&ReviewRef("CR-8.1".into()))
                .unwrap()
                .header
                .id,
            earlier.id
        );
        assert_eq!(
            store
                .get_review(&ReviewRef("CR-8.2".into()))
                .unwrap()
                .header
                .id,
            later.id
        );
        assert_eq!(
            store
                .get_review(&ReviewRef::numbered(7))
                .unwrap()
                .header
                .title,
            "Alone"
        );
        let both = store
            .get_review(&ReviewRef::numbered(8))
            .unwrap_err()
            .to_string();
        assert!(
            both.contains("CR-8.1 (Theirs)") && both.contains("CR-8.2 (Mine)"),
            "{both}"
        );
        // The next number is past them all
        let mut store = store;
        assert_eq!(store.create_review(new_review("next")).unwrap().number, 9);
    }

    #[test]
    fn a_review_folder_that_cant_be_read_is_reported_and_the_others_are_listed() {
        let dir = TestDir::new();
        let task_dir = dir.path().join(".agentic-task-manager");
        plant(&task_dir, 1, "Fine", "2026-11-17T03:00:00Z");
        std::fs::create_dir_all(task_dir.join("reviews/CR-2-abcd")).unwrap();
        std::fs::write(task_dir.join("reviews/CR-2-abcd/review.md"), "not a header").unwrap();
        let store = FileStore::open(&task_dir).unwrap();
        assert_eq!(
            store.list_reviews(&ReviewFilter::default()).unwrap().len(),
            1
        );
        let problems = store.problems().unwrap();
        assert!(problems[0].starts_with("CR-2-abcd: "), "{problems:?}");
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_reviews_folder_or_review_is_refused() {
        let dir = TestDir::new();
        let task_dir = dir.path().join(".agentic-task-manager");
        let elsewhere = dir.path().join("elsewhere");
        let planted = plant(&elsewhere, 3, "Planted", "2026-11-17T03:00:00Z");
        std::fs::create_dir_all(task_dir.join("reviews")).unwrap();
        let target = elsewhere.join("reviews").join(folder_name(3, &planted.id));
        std::os::unix::fs::symlink(&target, task_dir.join("reviews/CR-3-ffff")).unwrap();
        let store = FileStore::open(&task_dir).unwrap();
        assert_eq!(
            store.list_reviews(&ReviewFilter::default()).unwrap().len(),
            0
        );
        assert!(store.problems().unwrap()[0].contains("symlink"));

        let linked = dir.path().join("linked");
        std::fs::create_dir_all(&linked).unwrap();
        std::os::unix::fs::symlink(elsewhere.join("reviews"), linked.join("reviews")).unwrap();
        assert!(
            FileStore::open(&linked)
                .unwrap_err()
                .to_string()
                .contains("symlink")
        );
    }

    #[test]
    fn a_subscription_is_told_when_a_review_changes() {
        let dir = TestDir::new();
        let task_dir = dir.path().join(".agentic-task-manager");
        let mut store = FileStore::open(&task_dir).unwrap();
        let (sent, received) = std::sync::mpsc::channel();
        let subscription = store
            .subscribe(Arc::new(move || {
                let _ = sent.send(());
            }))
            .unwrap();
        store.create_review(new_review("watched")).unwrap();
        received
            .recv_timeout(Duration::from_secs(10))
            .expect("told of the new review");
        drop(subscription);
    }
}
