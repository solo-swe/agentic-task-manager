//! Rounds from git, compared, and anchors followed (docs/code-review-design.md §3.3, §3.6, ATM-564).
//!
//! A round is made from a commit range, the working tree against a commit (uncommitted changes, a live review's
//! checkpoints), or files as they are; its diff is kept (binary files only listed). atm's own task folder is never in
//! it: the task file a task's commit carries, and the reviews themselves, aren't the change under review. Two rounds compare by their
//! trees: a range's head, or the round's files rebuilt from its base and its diff (no snapshots). A chain's anchor
//! moves to a later round through the change between the two: the lines around it move it; inside a changed part its
//! quote finds it again; else it has **drifted**, shown at its last place, with the round and lines it was made on.

use atm_common::{Error, Result, TASK_DIR};
use atm_vcs::{ChangedFile, Diff, GitVcs};

use crate::model::{Actor, Anchor, FileChange, FileStatus, Patch, Review, Round, RoundKind};
use crate::store::{NewRound, ReviewStore};

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

fn round(kind: RoundKind, diff: Diff, by: &Actor) -> NewRound {
    NewRound {
        kind,
        files: file_changes(diff.files),
        patch: Patch(diff.patch),
        by: by.clone(),
    }
}

/// A round of a commit range (both resolved to their commits first, so the round names them for good).
///
/// # Errors
/// Either isn't a commit, or git fails.
pub fn from_range(vcs: &GitVcs, base: &str, head: &str, by: &Actor) -> Result<NewRound> {
    let resolve = |rev: &str| {
        vcs.resolve(rev)
            .ok_or_else(|| Error::Invalid(format!("{rev} isn't a commit here")))
    };
    let (base, head) = (resolve(base)?, resolve(head)?);
    let diff = vcs.diff_range(&base, &head, &[TASK_DIR])?;
    Ok(round(RoundKind::Range { base, head }, diff, by))
}

/// A round of the working tree against `base` (`HEAD` by default): uncommitted changes, new files included, or a
/// live review's checkpoint.
///
/// # Errors
/// `base` isn't a commit, or git fails.
pub fn from_working(vcs: &GitVcs, base: &str, by: &Actor) -> Result<NewRound> {
    let base = vcs
        .resolve(base)
        .ok_or_else(|| Error::Invalid(format!("{base} isn't a commit here")))?;
    let diff = vcs.diff_working(&base, &[TASK_DIR])?;
    Ok(round(RoundKind::Working { base }, diff, by))
}

/// A round of files as they are, reviewed whole (a plan, a design).
///
/// # Errors
/// A file git can't read, or git fails.
pub fn from_files(vcs: &GitVcs, paths: &[String], by: &Actor) -> Result<NewRound> {
    if paths.is_empty() {
        return Err(Error::Invalid("name at least one file to review".into()));
    }
    Ok(round(RoundKind::Files, vcs.diff_files(paths)?, by))
}

/// A round's tree: a range's head commit's, else its files rebuilt from its base (the empty tree for files reviewed
/// whole) and its diff, binary files left as they were.
///
/// # Errors
/// The base is gone from this clone, or the diff doesn't apply to it.
pub fn tree(vcs: &GitVcs, round: &Round, patch: &Patch) -> Result<String> {
    let binary: Vec<String> = round
        .files
        .iter()
        .filter(|f| f.binary)
        .map(|f| f.path.clone())
        .collect();
    match &round.kind {
        RoundKind::Range { head, .. } => vcs.tree_of(head),
        RoundKind::Working { base } => vcs.tree_after(Some(base), &patch.0, &binary),
        RoundKind::Files => vcs.tree_after(None, &patch.0, &binary),
    }
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
pub fn compare(
    vcs: &GitVcs,
    store: &dyn ReviewStore,
    review: &Review,
    from: u32,
    to: u32,
) -> Result<Patch> {
    let (a, b) = (round_of(review, from)?, round_of(review, to)?);
    let tree_a = tree(vcs, a, &store.round_diff(&review.header.id, from)?)?;
    let tree_b = tree(vcs, b, &store.round_diff(&review.header.id, to)?)?;
    let mut paths: Vec<String> = a
        .files
        .iter()
        .chain(&b.files)
        .map(|f| f.path.clone())
        .collect();
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
    let (first, last) = lines;
    let mapped: Vec<Mapped> = (first..=last).map(|line| map_line(hunks, line)).collect();
    let (start, end) = (mapped[0].line(), mapped[mapped.len() - 1].line());
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
pub fn place(
    vcs: &GitVcs,
    store: &dyn ReviewStore,
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
    let tree_a = tree(vcs, a, &store.round_diff(&review.header.id, a.number)?)?;
    let tree_b = tree(vcs, b, &store.round_diff(&review.header.id, b.number)?)?;
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
mod tests {
    use super::{Hunk, Placement, follow, hunks};

    /// A hunk that replaces `old_len` lines with `new_len` (or inserts, with no old lines).
    fn hunk(old_start: u32, old_len: u32, new_start: u32, new_len: u32) -> Hunk {
        let count = |n: u32| usize::try_from(n).unwrap();
        Hunk {
            old_start,
            old_len,
            new_start,
            new_len,
            ops: "-".repeat(count(old_len)) + &"+".repeat(count(new_len)),
        }
    }

    #[test]
    fn hunks_are_read_from_a_patch_with_their_lines_marks() {
        let patch = "diff --git a/a b/a\n--- a/a\n+++ b/a\n@@ -3,2 +3,3 @@ fn x\n a\n+b\n c\n@@ -10 +11 @@\n-c\n+d\n\
                     diff --git a/b b/b\n--- a/b\n+++ b/b\n@@ -1 +1 @@\n-x\n+y\n";
        let read = hunks(patch);
        let summary: Vec<(u32, u32, u32, u32, &str)> = read
            .iter()
            .map(|h| {
                (
                    h.old_start,
                    h.old_len,
                    h.new_start,
                    h.new_len,
                    h.ops.as_str(),
                )
            })
            .collect();
        assert_eq!(
            summary,
            [
                (3, 2, 3, 3, " + "),
                (10, 1, 11, 1, "-+"),
                (1, 1, 1, 1, "-+")
            ]
        );
    }

    #[test]
    fn lines_no_change_touches_move_with_the_lines_added_above_them() {
        // Two lines added at the top, one taken out after the anchor
        let changed = [hunk(1, 0, 1, 2), hunk(30, 1, 32, 0)];
        assert_eq!(
            follow((10, 12), Some("x"), &changed, Some(""), 1),
            Placement::At((12, 14))
        );
        assert_eq!(
            follow((10, 12), None, &[], Some(""), 1),
            Placement::At((10, 12))
        );
        // Lines inserted right after the anchor's last line leave it where it is
        assert_eq!(
            follow((10, 12), None, &[hunk(12, 0, 13, 3)], Some(""), 1),
            Placement::At((10, 12))
        );
    }

    /// A repository with `a.rs` committed, and two rounds of uncommitted changes to it in a review.
    #[test]
    fn two_uncommitted_rounds_compare_and_a_comment_moves_or_drifts_across_them() {
        use super::{compare, from_working, place};
        use crate::memory::MemoryStore;
        use crate::model::{Actor, Anchor, ReviewRef, Status};
        use crate::rules::Reviews;
        use crate::store::{NewReview, ReviewStore};

        let dir = crate::test_dir::TestDir::new();
        let root = dir.path();
        let git = |args: &[&str]| {
            let mut all = vec!["-c", "user.name=t", "-c", "user.email=t@example.com"];
            all.extend_from_slice(args);
            assert!(atm_fs::git::git(&all, root).is_some(), "git {args:?}");
        };
        git(&["init", "-q"]);
        std::fs::write(
            root.join("a.rs"),
            "fn a() {}\nfn b() {}\nlet shell = 0;\nfn c() {}\n",
        )
        .unwrap();
        git(&["add", "-A"]);
        git(&["commit", "-q", "-m", "base"]);
        let vcs = atm_vcs::GitVcs::new(root);
        let agent = Actor::from("claude:0f56a916");

        // Round 1: line 3 changed
        std::fs::write(
            root.join("a.rs"),
            "fn a() {}\nfn b() {}\nlet shell = 1;\nfn c() {}\n",
        )
        .unwrap();
        let mut store = MemoryStore::new();
        let mut reviews = Reviews::new(&mut store);
        reviews
            .create(NewReview {
                title: "Rounds".into(),
                summary: String::new(),
                tasks: Vec::new(),
                author: Some(agent.clone()),
                reviewers: Vec::new(),
                status: Status::Open,
                first: from_working(&vcs, "HEAD", &agent).unwrap(),
            })
            .unwrap();
        let on = |lines: (u32, u32), quote: &str| Anchor {
            round: 1,
            path: Some("a.rs".into()),
            lines: Some(lines),
            quote: Some(quote.into()),
            ..Anchor::default()
        };

        // Round 2: two lines added at the top, line 3 rewritten again
        std::fs::write(
            root.join("a.rs"),
            "// one\n// two\nfn a() {}\nfn b() {}\nlet shell = shell_pid();\nfn c() {}\n",
        )
        .unwrap();
        reviews
            .add_round(
                &ReviewRef::numbered(1),
                from_working(&vcs, "HEAD", &agent).unwrap(),
            )
            .unwrap();
        let review = store.get_review(&ReviewRef::numbered(1)).unwrap();
        let between = compare(&vcs, &store, &review, 1, 2).unwrap();
        assert!(
            between.0.contains("+// one") && between.0.contains("+let shell = shell_pid();"),
            "{}",
            between.0
        );
        assert!(
            !between.0.contains("+let shell = 1;"),
            "only what changed since round 1"
        );

        // `fn b` untouched: moved down two lines; `let shell = 1;` rewritten: drifted
        assert_eq!(
            place(&vcs, &store, &review, &on((2, 2), "fn b() {}"), 2).unwrap(),
            Placement::At((4, 4))
        );
        assert_eq!(
            place(&vcs, &store, &review, &on((3, 3), "let shell = 1;"), 2).unwrap(),
            Placement::Drifted {
                last: (5, 5),
                made_on: 1,
                lines: (3, 3)
            }
        );
        // The user's index was never touched: the file is still only modified in the working tree
        let status = atm_fs::git::git(&["status", "--porcelain"], root).unwrap();
        assert_eq!(status, "M a.rs");
    }

    #[test]
    fn lines_inserted_inside_the_anchor_change_it() {
        let new_text = "a\nb\nnew\nc\n";
        assert_eq!(
            follow(
                (1, 3),
                Some("a\nb\nc"),
                &[hunk(2, 0, 3, 1)],
                Some(new_text),
                1
            ),
            Placement::Drifted {
                last: (1, 4),
                made_on: 1,
                lines: (1, 3)
            }
        );
    }

    #[test]
    fn a_changed_part_finds_its_quote_again_or_drifts() {
        let new_text = "fn a() {}\n\nfn shell() {\n    let shell = shell_pid();\n}\n";
        // The line was rewritten, the function's first line kept: found by the quote
        let rewritten = [hunk(3, 2, 3, 3)];
        assert_eq!(
            follow((3, 3), Some("fn shell() {"), &rewritten, Some(new_text), 1),
            Placement::At((3, 3))
        );
        // A selection from the rendered view: a line holding it
        assert_eq!(
            follow((4, 4), Some("shell_pid"), &rewritten, Some(new_text), 1),
            Placement::At((4, 4))
        );
        // Its quote is gone: drifted, at its last place, with where it was made
        // (shown where the replacement starts)
        assert_eq!(
            follow(
                (4, 4),
                Some("let shell = 0;"),
                &rewritten,
                Some(new_text),
                1
            ),
            Placement::Drifted {
                last: (3, 3),
                made_on: 1,
                lines: (4, 4)
            }
        );
        // The file is gone
        assert_eq!(
            follow((4, 4), Some("x"), &[], None, 2),
            Placement::Drifted {
                last: (4, 4),
                made_on: 2,
                lines: (4, 4)
            }
        );
    }
}
