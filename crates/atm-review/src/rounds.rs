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
    fn a_files_status_is_added_deleted_renamed_from_or_else_modified() {
        use crate::model::FileStatus;
        let changed = |status: char, from: Option<&str>| atm_vcs::ChangedFile {
            path: format!("{status}.rs"),
            status,
            from: from.map(String::from),
            binary: status == 'A',
        };
        let read = super::file_changes(vec![
            changed('A', None),
            changed('D', None),
            changed('R', Some("old.rs")),
            changed('M', None),
            // A rename without its old path, a type change, a copy: modified
            changed('R', None),
            changed('T', None),
            changed('C', Some("src.rs")),
        ]);
        let statuses: Vec<FileStatus> = read.iter().map(|f| f.status.clone()).collect();
        assert_eq!(
            statuses,
            [
                FileStatus::Added,
                FileStatus::Deleted,
                FileStatus::Renamed {
                    from: "old.rs".into()
                },
                FileStatus::Modified,
                FileStatus::Modified,
                FileStatus::Modified,
                FileStatus::Modified,
            ]
        );
        assert_eq!((read[0].path.as_str(), read[0].binary), ("A.rs", true));
        assert!(!read[1].binary);
    }

    #[test]
    fn a_hunk_header_without_lengths_is_one_line_and_what_isnt_a_hunks_line_is_left_out() {
        assert_eq!(hunks(""), Vec::<Hunk>::new());
        // A new file without a last line end, then a file deleted: the file headers and git's note aren't lines
        let patch = "diff --git a/a b/a\nnew file mode 100644\n--- /dev/null\n+++ b/a\n@@ -0,0 +1,2 @@\n+x\n+y\n\
                     \\ No newline at end of file\ndiff --git a/b b/b\ndeleted file mode 100644\n--- a/b\n+++ /dev/null\n\
                     @@ -1 +0,0 @@\n-z\n";
        let read: Vec<(u32, u32, u32, u32, String)> = hunks(patch)
            .into_iter()
            .map(|h| (h.old_start, h.old_len, h.new_start, h.new_len, h.ops))
            .collect();
        assert_eq!(read, [(0, 0, 1, 2, "++".into()), (1, 1, 0, 0, "-".into())]);
        // A header that can't be read is no hunk
        for patch in [
            "@@ -x +1 @@\n+a\n",
            "@@ -1 1 @@\n+a\n",
            // A binary file's diff has no hunks
            "diff --git a/i.png b/i.png\nBinary files a/i.png and b/i.png differ\n",
        ] {
            assert_eq!(hunks(patch), Vec::<Hunk>::new(), "{patch}");
        }
    }

    #[test]
    fn lines_removed_above_move_the_anchor_up_and_a_change_below_leaves_it() {
        assert_eq!(
            follow((10, 12), None, &[hunk(1, 3, 1, 0)], Some(""), 1),
            Placement::At((7, 9))
        );
        assert_eq!(
            follow((10, 12), None, &[hunk(20, 2, 20, 5)], Some(""), 1),
            Placement::At((10, 12))
        );
        // Lines inserted after line 10 move line 11, not line 10
        assert_eq!(
            follow((10, 10), None, &[hunk(10, 0, 11, 2)], Some(""), 1),
            Placement::At((10, 10))
        );
        assert_eq!(
            follow((11, 11), None, &[hunk(10, 0, 11, 2)], Some(""), 1),
            Placement::At((13, 13))
        );
    }

    #[test]
    fn an_anchor_whose_lines_were_removed_drifts_unless_its_quote_is_found_again() {
        // Lines 3 and 4 removed
        let removed = [hunk(3, 2, 2, 0)];
        assert_eq!(
            follow((3, 4), None, &removed, Some("a\nb\n"), 1),
            Placement::Drifted {
                last: (2, 2),
                made_on: 1,
                lines: (3, 4)
            }
        );
        // Moved further down the file: found where it went
        let moved = "a\nb\nc\nd\ne\nf\nfn moved() {}\n";
        assert_eq!(
            follow(
                (3, 3),
                Some("fn moved() {}"),
                &[hunk(3, 1, 2, 0)],
                Some(moved),
                1
            ),
            Placement::At((7, 7))
        );
    }

    #[test]
    fn a_quote_is_found_nearest_its_last_place_with_its_indentation_ignored() {
        // On lines 2 and 9: the one nearer line 8
        let twice = "x\ntarget\nx\nx\nx\nx\nx\nx\ntarget\n";
        assert_eq!(
            follow((8, 8), Some("target"), &[hunk(8, 1, 8, 1)], Some(twice), 1),
            Placement::At((9, 9))
        );
        // Lines indented again: the same lines
        let reindented = "fn a() {\n        let x = 1;\n        let y = 2;\n}\n";
        assert_eq!(
            follow(
                (2, 3),
                Some("    let x = 1;\n    let y = 2;"),
                &[hunk(2, 2, 2, 2)],
                Some(reindented),
                1
            ),
            Placement::At((2, 3))
        );
        // Several lines are found only together, never by a part of one; a blank quote finds nothing
        let drifted = Placement::Drifted {
            last: (2, 2),
            made_on: 1,
            lines: (2, 3),
        };
        for quote in ["let x = 1;\nlet z = 3;", "x = 1;\ny = 2;", " \n\n"] {
            assert_eq!(
                follow(
                    (2, 3),
                    Some(quote),
                    &[hunk(2, 2, 2, 2)],
                    Some(reindented),
                    1
                ),
                drifted,
                "{quote:?}"
            );
        }
    }

    #[test]
    fn a_chain_about_the_review_or_a_whole_file_or_on_the_round_shown_isnt_moved() {
        use super::{compare, place};
        use crate::memory::MemoryStore;
        use crate::model::{Actor, Anchor, Patch, ReviewRef, RoundKind, Status};
        use crate::store::{NewReview, NewRound, ReviewStore};

        let mut store = MemoryStore::new();
        let by = Actor::from("claude:A");
        store
            .create_review(NewReview {
                title: "T".into(),
                summary: String::new(),
                tasks: Vec::new(),
                author: Some(by.clone()),
                reviewers: Vec::new(),
                status: Status::Open,
                first: NewRound {
                    kind: RoundKind::Files,
                    files: Vec::new(),
                    patch: Patch::default(),
                    by,
                },
            })
            .unwrap();
        let review = store.get_review(&ReviewRef::numbered(1)).unwrap();
        // No repository: none of these need one
        let vcs = atm_vcs::GitVcs::new(std::path::Path::new("/nonexistent/atm-review"));
        let on = |path: Option<&str>, lines| Anchor {
            round: 1,
            path: path.map(String::from),
            lines,
            ..Anchor::default()
        };
        for (anchor, to, placed) in [
            (on(None, None), 3, Placement::Whole),
            (on(Some("a.rs"), None), 3, Placement::Whole),
            (on(Some("a.rs"), Some((4, 6))), 1, Placement::At((4, 6))),
        ] {
            assert_eq!(place(&vcs, &store, &review, &anchor, to).unwrap(), placed);
        }
        // A round the review hasn't
        assert!(matches!(
            place(&vcs, &store, &review, &on(Some("a.rs"), Some((1, 1))), 2),
            Err(atm_common::Error::NotFound(_))
        ));
        assert!(matches!(
            compare(&vcs, &store, &review, 1, 2),
            Err(atm_common::Error::NotFound(_))
        ));
    }

    /// A throwaway repository, its files committed as `base`.
    fn repo(files: &[(&str, &[u8])]) -> (crate::test_dir::TestDir, atm_vcs::GitVcs) {
        let dir = crate::test_dir::TestDir::new();
        git(dir.path(), &["init", "-q"]);
        write(dir.path(), files);
        git(dir.path(), &["add", "-A"]);
        git(dir.path(), &["commit", "-q", "-m", "base"]);
        let vcs = atm_vcs::GitVcs::new(dir.path());
        (dir, vcs)
    }

    fn git(root: &std::path::Path, args: &[&str]) {
        let mut all = vec!["-c", "user.name=t", "-c", "user.email=t@example.com"];
        all.extend_from_slice(args);
        assert!(atm_fs::git::git(&all, root).is_some(), "git {args:?}");
    }

    fn write(root: &std::path::Path, files: &[(&str, &[u8])]) {
        for (path, data) in files {
            let path = root.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, data).unwrap();
        }
    }

    const TASKS: &str = ".agentic-task-manager/tasks.json";
    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\0";

    fn changes(round: &crate::store::NewRound) -> Vec<(String, crate::model::FileStatus, bool)> {
        let mut files: Vec<_> = round
            .files
            .iter()
            .map(|f| (f.path.clone(), f.status.clone(), f.binary))
            .collect();
        files.sort_by(|a, b| a.0.cmp(&b.0));
        files
    }

    #[test]
    fn a_range_round_names_its_commits_and_every_change_but_the_task_folders() {
        use super::{from_range, tree};
        use crate::model::{Actor, FileStatus, Round, RoundKind};

        let old = b"one\ntwo\nthree\nfour\nfive\n";
        let (dir, vcs) = repo(&[
            ("a.rs", b"fn a() {}\n"),
            ("b.txt", b"b\n"),
            ("old.md", old),
            (TASKS, b"{}\n"),
        ]);
        let root = dir.path();
        write(
            root,
            &[
                ("a.rs", b"fn a() { 1 }\n"),
                ("c.rs", b"fn c() {}\n"),
                ("img.png", PNG),
                (TASKS, b"{\"x\": 1}\n"),
            ],
        );
        git(root, &["rm", "-q", "b.txt"]);
        git(root, &["mv", "old.md", "new.md"]);
        git(root, &["add", "-A"]);
        git(root, &["commit", "-q", "-m", "head"]);
        let by = Actor::from("claude:A");
        let round = from_range(&vcs, "HEAD~1", "HEAD", &by).unwrap();
        assert_eq!(
            round.kind,
            RoundKind::Range {
                base: vcs.resolve("HEAD~1").unwrap(),
                head: vcs.resolve("HEAD").unwrap()
            }
        );
        assert_eq!(round.by, by);
        assert_eq!(
            changes(&round),
            [
                ("a.rs".into(), FileStatus::Modified, false),
                ("b.txt".into(), FileStatus::Deleted, false),
                ("c.rs".into(), FileStatus::Added, false),
                ("img.png".into(), FileStatus::Added, true),
                (
                    "new.md".into(),
                    FileStatus::Renamed {
                        from: "old.md".into()
                    },
                    false
                ),
            ]
        );
        assert!(round.patch.0.contains("+fn a() { 1 }"), "{}", round.patch.0);
        assert!(!round.patch.0.contains("tasks.json"), "{}", round.patch.0);
        // Its tree is the head commit's
        let kept = Round {
            number: 1,
            kind: round.kind.clone(),
            files: round.files.clone(),
            by: by.clone(),
            at: String::new(),
        };
        let head_tree = tree(&vcs, &kept, &round.patch).unwrap();
        assert_eq!(head_tree, vcs.tree_of("HEAD").unwrap());
        assert_eq!(
            vcs.file_in(&head_tree, "c.rs").as_deref(),
            Some("fn c() {}\n")
        );
        // An end that isn't a commit
        for (base, head) in [("nope", "HEAD"), ("HEAD", "nope")] {
            assert!(matches!(
                from_range(&vcs, base, head, &by),
                Err(atm_common::Error::Invalid(_))
            ));
        }
    }

    #[test]
    fn a_working_round_has_new_files_too_its_binary_ones_listed_and_its_tree_rebuilt() {
        use super::{from_working, tree};
        use crate::model::{Actor, FileStatus, Round, RoundKind};

        let (dir, vcs) = repo(&[("a.rs", b"fn a() {}\n"), (TASKS, b"{}\n")]);
        let root = dir.path();
        write(
            root,
            &[
                ("a.rs", b"fn a() { 1 }\n"),
                ("src/new.rs", b"fn new() {}\n"),
                ("img.png", PNG),
                (TASKS, b"{\"x\": 1}\n"),
            ],
        );
        let by = Actor::from("claude:A");
        let round = from_working(&vcs, "HEAD", &by).unwrap();
        assert_eq!(
            round.kind,
            RoundKind::Working {
                base: vcs.resolve("HEAD").unwrap()
            }
        );
        assert_eq!(
            changes(&round),
            [
                ("a.rs".into(), FileStatus::Modified, false),
                ("img.png".into(), FileStatus::Added, true),
                ("src/new.rs".into(), FileStatus::Added, false),
            ]
        );
        // The files stay as the user left them: new ones untracked
        let status = atm_fs::git::git(&["status", "--porcelain"], root).unwrap();
        assert!(status.contains("?? src/"), "{status}");
        // Rebuilt from its base and its diff, the binary file left as the base had it (not at all)
        let kept = Round {
            number: 1,
            kind: round.kind.clone(),
            files: round.files.clone(),
            by,
            at: String::new(),
        };
        let rebuilt = tree(&vcs, &kept, &round.patch).unwrap();
        assert_eq!(
            vcs.file_in(&rebuilt, "src/new.rs").as_deref(),
            Some("fn new() {}\n")
        );
        assert_eq!(
            vcs.file_in(&rebuilt, "a.rs").as_deref(),
            Some("fn a() { 1 }\n")
        );
        assert_eq!(vcs.file_in(&rebuilt, "img.png"), None);
        assert_eq!(vcs.file_in(&rebuilt, TASKS).as_deref(), Some("{}\n"));
        assert!(matches!(
            from_working(&vcs, "nope", &Actor::from("claude:A")),
            Err(atm_common::Error::Invalid(_))
        ));
    }

    #[test]
    fn a_files_round_is_its_files_whole_as_if_added_and_compares_with_the_next() {
        use super::{compare, from_files, place, tree};
        use crate::memory::MemoryStore;
        use crate::model::{Actor, Anchor, FileStatus, ReviewRef, RoundKind, Status};
        use crate::rules::Reviews;
        use crate::store::{NewReview, ReviewStore};

        let (dir, vcs) = repo(&[
            ("docs/plan.md", b"# Plan\n\nOld step.\nKept step.\n"),
            ("a.rs", b"fn a() {}\n"),
        ]);
        let by = Actor::from("claude:A");
        assert!(matches!(
            from_files(&vcs, &[], &by),
            Err(atm_common::Error::Invalid(_))
        ));
        assert!(from_files(&vcs, &["none.md".into()], &by).is_err());
        let first = from_files(&vcs, &["docs/plan.md".into()], &by).unwrap();
        assert_eq!(first.kind, RoundKind::Files);
        assert_eq!(
            changes(&first),
            [("docs/plan.md".into(), FileStatus::Added, false)]
        );
        assert!(first.patch.0.contains("+Old step."), "{}", first.patch.0);

        let mut store = MemoryStore::new();
        let mut reviews = Reviews::new(&mut store);
        reviews
            .create(NewReview {
                title: "Plan".into(),
                summary: String::new(),
                tasks: Vec::new(),
                author: Some(by.clone()),
                reviewers: Vec::new(),
                status: Status::Open,
                first,
            })
            .unwrap();
        write(
            dir.path(),
            &[("docs/plan.md", b"# Plan\n\nNew step.\nKept step.\n")],
        );
        reviews
            .add_round(
                &ReviewRef::numbered(1),
                from_files(&vcs, &["docs/plan.md".into()], &by).unwrap(),
            )
            .unwrap();
        let review = store.get_review(&ReviewRef::numbered(1)).unwrap();
        // Rebuilt from nothing: the named files only
        let rebuilt = tree(
            &vcs,
            &review.rounds[0],
            &store.round_diff(&review.header.id, 1).unwrap(),
        )
        .unwrap();
        assert!(vcs.file_in(&rebuilt, "docs/plan.md").is_some());
        assert_eq!(vcs.file_in(&rebuilt, "a.rs"), None);
        let between = compare(&vcs, &store, &review, 1, 2).unwrap();
        assert!(
            between.0.contains("-Old step.") && between.0.contains("+New step."),
            "{}",
            between.0
        );
        assert!(!between.0.contains("+# Plan"), "{}", between.0);
        // A comment on the kept line stays on it
        let kept = Anchor {
            round: 1,
            path: Some("docs/plan.md".into()),
            lines: Some((4, 4)),
            quote: Some("Kept step.".into()),
            ..Anchor::default()
        };
        assert_eq!(
            place(&vcs, &store, &review, &kept, 2).unwrap(),
            Placement::At((4, 4))
        );
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
