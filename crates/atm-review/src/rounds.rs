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

/// The files a diff changes, as a round lists them.
#[must_use]
pub fn file_changes(files: Vec<ChangedFile>) -> Vec<FileChange> {
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

/// Where a view of a review starts (ATM-725). A review is a chain of increments, one a round: round 1's from the
/// review's base to its result, round N's from round N−1's result to its own. A view adds up those between its start
/// and its round: the whole change of round N from the base, or what changed since round M.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Since {
    /// The review's base: round 1's (its commit; nothing, for files reviewed whole). The whole change.
    Base,
    /// Round M's result: what rounds M+1 … N changed.
    Round(u32),
}

/// A view of a review: from `before` (a commit or a tree) to `after` (a tree), its diff and the files it changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Between {
    /// Where it starts: the review's base commit, an earlier round's tree, or the empty tree.
    pub before: Rev,
    /// Where it ends: the round's tree.
    pub after: Rev,
    /// The diff and its files.
    pub diff: Diff,
}

/// The view of round `to` since `since` (ATM-725), the one way a view is made: round `to`'s tree against the
/// review's base or round M's tree, so it is the increments between them added up, however each round was opened (a
/// range from the review's base, from the round before, or from anywhere). Everything the two differ in, atm's task
/// folder left out; where a files round is in it, only the files those rounds name (ATM-622: a files round speaks
/// for the files it names, not for the others, which its tree doesn't have).
///
/// # Errors
/// No such round, a start not before `to`, rounds of different repositories, or a round's tree can't be had here (its
/// base is gone from this clone).
pub fn between<S: RoundDiffs + ?Sized>(
    vcs: &GitVcs,
    store: &S,
    review: &Review,
    since: Since,
    to: u32,
) -> Result<Between> {
    let b = round_of(review, to)?;
    let first = review
        .rounds
        .iter()
        .min_by_key(|r| r.number)
        .ok_or_else(|| Error::NotFound(format!("{} has no rounds", review.header.reference)))?;
    let from = match since {
        Since::Base => first,
        Since::Round(m) if m >= to => {
            return Err(Error::Invalid(format!(
                "round {to} since round {m}: name an earlier round"
            )));
        }
        Since::Round(m) => round_of(review, m)?,
    };
    let repo = shared_repository(vcs, from, b)?;
    let after = tree(vcs, b, &store.diff_of(&review.header.id, to)?)?;
    let before = match (since, &first.kind) {
        (Since::Base, RoundKind::Range { base, .. } | RoundKind::Working { base }) => {
            recorded(base)?
        }
        (Since::Base, RoundKind::Files) => Rev::object_id(&repo.empty_tree()?)?,
        (Since::Round(m), _) => tree(vcs, from, &store.diff_of(&review.header.id, m)?)?,
    };
    // The rounds the view spans: from the first (the base) or from round M, to round N
    let start = match since {
        Since::Base => first.number,
        Since::Round(m) => m,
    };
    let spanned: Vec<&Round> = review
        .rounds
        .iter()
        .filter(|r| (start..=to).contains(&r.number))
        .collect();
    let only: Vec<String> = if spanned.iter().any(|r| r.kind == RoundKind::Files) {
        let mut paths: Vec<String> = spanned
            .iter()
            .flat_map(|r| r.files.iter().map(|f| f.path.clone()))
            .collect();
        // Round N a files round: only its files (one of an earlier round it doesn't name isn't deleted)
        if b.kind == RoundKind::Files {
            paths.retain(|p| b.files.iter().any(|f| &f.path == p));
        }
        paths.sort();
        paths.dedup();
        paths
    } else {
        Vec::new()
    };
    let diff = repo.diff_between(&before, &after, &only, &[TASK_DIR])?;
    Ok(Between {
        before,
        after,
        diff,
    })
}

/// Whether a comment made on round `made_on` is in the view of round `to` since `since` (ATM-725): the view holds
/// that round's increment. A comment on a round after it, or (since round M) on round M or before, is on another round.
#[must_use]
pub fn in_view(since: Since, to: u32, made_on: u32) -> bool {
    made_on <= to
        && match since {
            Since::Base => true,
            Since::Round(m) => made_on > m,
        }
}

/// A new round as its increment (ATM-725): a commit range that doesn't start where the round before ended (a range
/// from the review's base, or from anywhere else) is recorded from that round's head, so each round's own diff is
/// what it changed; with the words saying so. Anything else as it is: a working or files round is rebuilt from its
/// own base, and a range after one of those has no commit to start from; so is a range whose previous head isn't in
/// this clone.
#[must_use]
pub fn as_increment(vcs: &GitVcs, previous: &Round, new: NewRound) -> (NewRound, Option<String>) {
    let (RoundKind::Range { head: last, .. }, RoundKind::Range { base, head }) =
        (&previous.kind, &new.kind)
    else {
        return (new, None);
    };
    if base == last || previous.submodule != new.submodule {
        return (new, None);
    }
    let short = |sha: &str| sha.chars().take(12).collect::<String>();
    let said = format!(
        "its range started at {}, not where round {} ended ({}): recorded from there, so the round is what it changed",
        short(base),
        previous.number,
        short(last)
    );
    // The round before's head not in this clone (made in another, or gone): kept as given, the views still add up
    // from the rounds' trees wherever they can be had
    let head = head.clone();
    match repository(vcs, previous).and_then(|repo| from_range(&repo, last, &head, &new.by)) {
        Ok(increment) => (increment, Some(said)),
        Err(_) => (new, None),
    }
}

/// What changed from round `from` to round `to` of a review (§3.6's "between two rounds"): [`between`] since round
/// `from`.
///
/// # Errors
/// As [`between`]'s.
pub fn compare<S: RoundDiffs + ?Sized>(
    vcs: &GitVcs,
    store: &S,
    review: &Review,
    from: u32,
    to: u32,
) -> Result<Patch> {
    Ok(Patch(
        between(vcs, store, review, Since::Round(from), to)?
            .diff
            .patch,
    ))
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
