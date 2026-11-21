//! Rounds from git, compared, and anchors followed (docs/code-review-design.md §3.3, §3.6, ATM-564).
//!
//! A round is made from a commit range, the working tree against a commit (uncommitted changes, a live review's
//! checkpoints), or files as they are; its diff is kept (binary files only listed). atm's own task folder is never in
//! it: the task file a task's commit carries, and the reviews themselves, aren't the change under review. Two rounds compare by their
//! trees: a range's head, or the round's files rebuilt from its base and its diff (no snapshots). A chain's anchor
//! moves to a later round through the change between the two: the lines around it move it; inside a changed part its
//! quote finds it again; else it has **drifted**, shown at its last place, with the round and lines it was made on.

use atm_common::{Error, Result, TASK_DIR};
use atm_vcs::{ChangedFile, Diff, GitVcs, Rev};

use crate::model::{
    Actor, Anchor, FileChange, FileStatus, Patch, Review, ReviewId, Round, RoundKind,
};
use crate::store::{NewRound, ReviewStore};

/// What comparing rounds reads: a round's diff. A store's rounds, or those and one not stored: a live review's
/// working copy now, shown before it is a checkpoint (ATM-576).
pub trait RoundDiffs {
    /// Round `round`'s diff.
    ///
    /// # Errors
    /// No such round, or the store's.
    fn diff_of(&self, id: &ReviewId, round: u32) -> Result<Patch>;
}

impl<T: ReviewStore + ?Sized> RoundDiffs for T {
    fn diff_of(&self, id: &ReviewId, round: u32) -> Result<Patch> {
        self.round_diff(id, round)
    }
}

/// A review's working copy now as round `number`, after its stored rounds: what a live review shows until a comment
/// makes it a checkpoint (ATM-576).
#[must_use]
pub fn unstored(number: u32, new: NewRound, at: String) -> (Round, Patch) {
    (
        Round {
            number,
            kind: new.kind,
            submodule: new.submodule,
            files: new.files,
            by: new.by,
            at,
        },
        new.patch,
    )
}

fn file_changes(files: Vec<ChangedFile>) -> Vec<FileChange> {
    files
        .into_iter()
        .map(|f| FileChange {
            status: match (f.status, f.from) {
                ('A', _) => FileStatus::Added,
                ('D', _) => FileStatus::Deleted,
                ('R', Some(from)) => FileStatus::Renamed { from },
                _ => FileStatus::Modified,
            },
            path: f.path,
            binary: f.binary,
        })
        .collect()
}

/// A round made with `vcs`: the project's repository, or a submodule's ([`GitVcs::submodule_at`]), which it records.
fn round(vcs: &GitVcs, kind: RoundKind, diff: Diff, by: &Actor) -> NewRound {
    NewRound {
        kind,
        submodule: vcs.shown_folder().map(String::from),
        files: file_changes(diff.files),
        patch: Patch(diff.patch),
        by: by.clone(),
    }
}

/// The repository a round's commits are in (ATM-583): the project's (`vcs`), or the submodule's it records.
///
/// # Errors
/// `NotFound` when that submodule isn't checked out in this clone.
pub fn repository(vcs: &GitVcs, round: &Round) -> Result<GitVcs> {
    match &round.submodule {
        None => Ok(vcs.clone()),
        Some(folder) => vcs.submodule_at(folder).ok_or_else(|| {
            Error::NotFound(format!(
                "round {} is of the submodule {folder}, which isn't checked out here (git submodule update --init {folder})",
                round.number
            ))
        }),
    }
}

/// The one repository two rounds are compared in; `Invalid` when they are of different ones (the project's and a
/// submodule's): their trees are in different repositories.
fn shared_repository(vcs: &GitVcs, a: &Round, b: &Round) -> Result<GitVcs> {
    if a.submodule != b.submodule {
        let name = |r: &Round| {
            r.submodule.as_ref().map_or_else(
                || "the project".to_string(),
                |f| format!("the submodule {f}"),
            )
        };
        return Err(Error::Invalid(format!(
            "round {} is of {} and round {} of {}: rounds of different repositories don't compare",
            a.number,
            name(a),
            b.number,
            name(b)
        )));
    }
    repository(vcs, b)
}

/// A round of a commit range (both resolved to their commits first, so the round names them for good).
///
/// # Errors
/// Either isn't a commit, or git fails.
pub fn from_range(vcs: &GitVcs, base: &str, head: &str, by: &Actor) -> Result<NewRound> {
    let (base, head) = (commit(vcs, base)?, commit(vcs, head)?);
    let diff = vcs.diff_range(&base, &head, &[TASK_DIR])?;
    let (base, head) = (base.to_string(), head.to_string());
    Ok(round(vcs, RoundKind::Range { base, head }, diff, by))
}

/// A revision someone named, as its commit's full SHA: what a round records.
fn commit(vcs: &GitVcs, rev: &str) -> Result<Rev> {
    vcs.resolve(&Rev::new(rev)?)
        .ok_or_else(|| Error::Invalid(format!("{rev} isn't a commit here")))
        .and_then(|sha| Rev::object_id(&sha))
}

/// A round's commit, as `rounds/N.json` has it: a full object ID (ATM-659; a round's files are read with
/// [`RoundKind::check`] first, and atm writes only SHAs there).
fn recorded(id: &str) -> Result<Rev> {
    Rev::object_id(id)
}

/// A round of the working tree against `base` (`HEAD` by default): uncommitted changes, new files included, or a
/// live review's checkpoint.
///
/// # Errors
/// `base` isn't a commit, or git fails.
pub fn from_working(vcs: &GitVcs, base: &str, by: &Actor) -> Result<NewRound> {
    let base = commit(vcs, base)?;
    let diff = vcs.diff_working(&base, &[TASK_DIR])?;
    let base = base.to_string();
    Ok(round(vcs, RoundKind::Working { base }, diff, by))
}

/// A round of files as they are, reviewed whole (a plan, a design).
///
/// # Errors
/// A file git can't read, or git fails.
pub fn from_files(vcs: &GitVcs, paths: &[String], by: &Actor) -> Result<NewRound> {
    if paths.is_empty() {
        return Err(Error::Invalid("name at least one file to review".into()));
    }
    Ok(round(vcs, RoundKind::Files, vcs.diff_files(paths)?, by))
}

/// A round's tree: a range's head commit's, else its files rebuilt from its base (the empty tree for files reviewed
/// whole) and its diff, binary files left as they were.
///
/// # Errors
/// The base is gone from this clone, or the diff doesn't apply to it; the round's submodule isn't checked out here.
pub fn tree(vcs: &GitVcs, round: &Round, patch: &Patch) -> Result<Rev> {
    let vcs = &repository(vcs, round)?;
    let binary: Vec<String> = round
        .files
        .iter()
        .filter(|f| f.binary)
        .map(|f| f.path.clone())
        .collect();
    let tree = match &round.kind {
        RoundKind::Range { head, .. } => vcs.tree_of(&recorded(head)?),
        RoundKind::Working { base } => vcs.tree_after(Some(&recorded(base)?), &patch.0, &binary),
        RoundKind::Files => vcs.tree_after(None, &patch.0, &binary),
    }?;
    Rev::object_id(&tree)
}

fn round_of(review: &Review, number: u32) -> Result<&Round> {
    review
        .rounds
        .iter()
        .find(|r| r.number == number)
        .ok_or_else(|| {
            Error::NotFound(format!("{} has no round {number}", review.header.reference))
        })
}

/// What changed from round `from` to round `to` of a review: their trees' diff, limited to the files either round
/// has (§3.6's "between two rounds").
///
/// # Errors
/// No such round, or a round's tree can't be had here (its base is gone from this clone).
pub fn compare<S: RoundDiffs + ?Sized>(
    vcs: &GitVcs,
    store: &S,
    review: &Review,
    from: u32,
    to: u32,
) -> Result<Patch> {
    let (a, b) = (round_of(review, from)?, round_of(review, to)?);
    let repo = shared_repository(vcs, a, b)?;
    let tree_a = tree(vcs, a, &store.diff_of(&review.header.id, from)?)?;
    let tree_b = tree(vcs, b, &store.diff_of(&review.header.id, to)?)?;
    let vcs = &repo;
    let mut paths: Vec<String> = a
        .files
        .iter()
        .chain(&b.files)
        .map(|f| f.path.clone())
        .collect();
    // A files round speaks only for the files it names (ATM-622): its tree is rebuilt from them alone, so a file of the
    // earlier round it doesn't name isn't deleted, it just isn't in this round. One it alone names is new to the review
    if b.kind == RoundKind::Files {
        paths.retain(|p| b.files.iter().any(|f| &f.path == p));
    }
    paths.sort();
    paths.dedup();
    Ok(Patch(vcs.diff_trees(&tree_a, &tree_b, &paths)?))
}

/// A part of a file a diff changed: `old_len` lines from `old_start` became `new_len` from `new_start` (1-based).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    /// Where the old lines start.
    pub old_start: u32,
    /// How many old lines.
    pub old_len: u32,
    /// Where the new lines start.
    pub new_start: u32,
    /// How many new lines.
    pub new_len: u32,
    /// Its lines in order, each as git marks it: `' '` kept, `-` removed, `+` added.
    pub ops: String,
}

/// The hunks of a patch (`@@ -a,b +c,d @@`, a missing length being 1), with their lines' marks.
#[must_use]
pub fn hunks(patch: &str) -> Vec<Hunk> {
    let range = |text: &str| -> Option<(u32, u32)> {
        let (start, len) = text.split_once(',').unwrap_or((text, "1"));
        Some((start.parse().ok()?, len.parse().ok()?))
    };
    let mut out: Vec<Hunk> = Vec::new();
    for line in patch.lines() {
        if let Some(header) = line.strip_prefix("@@ -") {
            let mut parts = header.split(' ');
            let parsed = (|| {
                let (old_start, old_len) = range(parts.next()?)?;
                let (new_start, new_len) = range(parts.next()?.strip_prefix('+')?)?;
                Some(Hunk {
                    old_start,
                    old_len,
                    new_start,
                    new_len,
                    ops: String::new(),
                })
            })();
            out.extend(parsed);
        } else if line.starts_with("diff --git") {
            // The next file's: no more lines for the hunk before
            out.push(Hunk {
                old_start: 0,
                old_len: 0,
                new_start: 0,
                new_len: 0,
                ops: String::from("x"),
            });
        } else if let (Some(hunk), Some(op @ (' ' | '-' | '+'))) =
            (out.last_mut(), line.chars().next())
            && !hunk.ops.ends_with('x')
        {
            hunk.ops.push(op);
        }
    }
    out.retain(|h| h.ops != "x");
    out
}

/// Where an old line is in the new text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mapped {
    /// Kept, at this line.
    Kept(u32),
    /// Removed; the new text goes on at this line.
    Removed(u32),
}

impl Mapped {
    fn line(self) -> u32 {
        match self {
            Self::Kept(line) | Self::Removed(line) => line,
        }
    }
}

/// Where old line `line` went: through the hunk it is in, line by line, or moved by the hunks before it.
fn map_line(hunks: &[Hunk], line: u32) -> Mapped {
    let mut delta: i64 = 0;
    for hunk in hunks {
        let inside =
            hunk.old_len > 0 && hunk.old_start <= line && line < hunk.old_start + hunk.old_len;
        if inside {
            let (mut old, mut new) = (hunk.old_start, hunk.new_start);
            for op in hunk.ops.chars() {
                match op {
                    ' ' if old == line => return Mapped::Kept(new),
                    '-' if old == line => return Mapped::Removed(new),
                    ' ' => (old, new) = (old + 1, new + 1),
                    '-' => old += 1,
                    _ => new += 1,
                }
            }
            return Mapped::Removed(new);
        }
        // A hunk of n old lines covers old_start..old_start + n - 1; one of none inserts after old_start
        let before = if hunk.old_len == 0 {
            hunk.old_start < line
        } else {
            hunk.old_start + hunk.old_len <= line
        };
        if before {
            delta += i64::from(hunk.new_len) - i64::from(hunk.old_len);
        }
    }
    Mapped::Kept(u32::try_from(i64::from(line) + delta).unwrap_or(1).max(1))
}

/// Where a chain shows in a later round.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Placement {
    /// About the review, or a whole file: wherever it is.
    Whole,
    /// At these lines.
    At((u32, u32)),
    /// Its quote is gone: shown at its last place, linking to the round and lines it was made on.
    Drifted {
        /// Its last known place in the later round.
        last: (u32, u32),
        /// The round it was made on.
        made_on: u32,
        /// The lines it was made on.
        lines: (u32, u32),
    },
}

/// The lines `quote` is on in `text`, nearest to `near`: consecutive lines equal to its own (spaces at the ends
/// ignored), or a line holding it (a selection in the rendered view).
fn find_quote(quote: &str, text: &str, near: u32) -> Option<(u32, u32)> {
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let wanted: Vec<&str> = quote
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    if wanted.is_empty() {
        return None;
    }
    let count = u32::try_from(wanted.len()).ok()?;
    let mut found: Vec<u32> = lines
        .windows(wanted.len())
        .enumerate()
        .filter(|(_, window)| *window == wanted.as_slice())
        .filter_map(|(i, _)| u32::try_from(i + 1).ok())
        .collect();
    if found.is_empty() && wanted.len() == 1 {
        found = lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.contains(wanted[0]))
            .filter_map(|(i, _)| u32::try_from(i + 1).ok())
            .collect();
    }
    let start = found.into_iter().min_by_key(|start| start.abs_diff(near))?;
    Some((start, start + count - 1))
}

/// Where `lines` (with their `quote`) of a file go when `hunks` change it into `new_text`: carried over when every
/// line is kept and nothing is inserted between them; else found again by the quote; else drifted.
#[must_use]
pub fn follow(
    lines: (u32, u32),
    quote: Option<&str>,
    hunks: &[Hunk],
    new_text: Option<&str>,
    made_on: u32,
) -> Placement {
    // A reversed range is never written (ATM-670): read as its two ends in order, never an empty list to index
    let (first, last) = (lines.0.min(lines.1), lines.0.max(lines.1));
    let mapped: Vec<Mapped> = (first..=last).map(|line| map_line(hunks, line)).collect();
    let (Some(start), Some(end)) = (
        mapped.first().map(|m| m.line()),
        mapped.last().map(|m| m.line()),
    ) else {
        return Placement::At((first, last));
    };
    let moved = (start, end.max(start));
    let Some(new_text) = new_text else {
        return Placement::Drifted {
            last: moved,
            made_on,
            lines,
        };
    };
    let kept = mapped.iter().all(|m| matches!(m, Mapped::Kept(_)));
    if kept && end - start == last - first {
        return Placement::At(moved);
    }
    match quote.and_then(|q| find_quote(q, new_text, moved.0)) {
        Some(found) => Placement::At(found),
        None => Placement::Drifted {
            last: moved,
            made_on,
            lines,
        },
    }
}

/// Where a chain made on an earlier round shows in round `to`.
///
/// # Errors
/// No such round, or a round's tree can't be had here.
pub fn place<S: RoundDiffs + ?Sized>(
    vcs: &GitVcs,
    store: &S,
    review: &Review,
    anchor: &Anchor,
    to: u32,
) -> Result<Placement> {
    let (Some(path), Some(lines)) = (&anchor.path, anchor.lines) else {
        return Ok(Placement::Whole);
    };
    if anchor.round == to {
        return Ok(Placement::At(lines));
    }
    let (a, b) = (round_of(review, anchor.round)?, round_of(review, to)?);
    let repo = shared_repository(vcs, a, b)?;
    let tree_a = tree(vcs, a, &store.diff_of(&review.header.id, a.number)?)?;
    let tree_b = tree(vcs, b, &store.diff_of(&review.header.id, b.number)?)?;
    let vcs = &repo;
    let between = vcs.diff_trees(&tree_a, &tree_b, std::slice::from_ref(path))?;
    let new_text = vcs.file_in(&tree_b, path);
    Ok(follow(
        lines,
        anchor.quote.as_deref(),
        &hunks(&between),
        new_text.as_deref(),
        anchor.round,
    ))
}

#[cfg(test)]
mod tests;
