//! `atm review`: code reviews recorded in the project (docs/code-review-design.md §3.9, ATM-566), thin mappings to
//! atm-lib's `reviews` (which keeps the tasks in step). Comments are task text: printed for people through
//! `output::for_people` (I30), never run (I1).

use std::path::Path;

use atm_common::{Error, Result};
use atm_lib::Project;
use atm_lib::actor::agent_identity;
use atm_lib::reviews::comments_md::{self, CommentsFile};
use atm_lib::reviews::{
    self, Actor, CommentFilter, CommentId, FileStore, NewReview, NewRound, ReviewFilter,
    ReviewHeader, ReviewRef, ReviewStore, Reviews, Round, RoundKind, Side, Status, rounds,
    vcs as vcs_of,
};
use clap::{Args, Subcommand};
use serde_json::{Value, json};

use super::{Context, print_json};

/// What a round is made of: a commit range, the working tree against a commit, or files as they are.
#[derive(Debug, Args, Default)]
pub struct Change {
    /// A commit range, BASE..HEAD
    #[arg(long, value_name = "BASE..HEAD", conflicts_with_all = ["uncommitted", "files"])]
    range: Option<String>,
    /// Uncommitted changes (new files too) against --base (HEAD by default)
    #[arg(long, conflicts_with = "files")]
    uncommitted: bool,
    /// With --uncommitted: the commit they are against
    #[arg(long, value_name = "REV")]
    base: Option<String>,
    /// Files as they are, reviewed whole (repeatable)
    #[arg(long = "file", value_name = "PATH")]
    files: Vec<String>,
}

/// A comment's text: given, or from a file.
#[derive(Debug, Args, Default)]
pub struct Body {
    /// The text (Markdown)
    #[arg(long, conflicts_with = "body_file")]
    body: Option<String>,
    /// The text, from a file
    #[arg(long, value_name = "FILE")]
    body_file: Option<String>,
}

/// The review commands.
#[derive(Debug, Subcommand)]
pub enum ReviewCommand {
    /// Open a review of a change: a commit range, uncommitted changes, or files as they are.
    Create {
        /// One line saying what it is
        #[arg(long)]
        title: String,
        /// A task it reviews (repeatable; its key)
        #[arg(long = "task", value_name = "KEY")]
        tasks: Vec<String>,
        #[command(flatten)]
        change: Change,
        /// The summary, in Markdown, from a file
        #[arg(long, value_name = "FILE")]
        summary_file: Option<String>,
        /// Who decides it (repeatable); none: anyone but its author
        #[arg(long = "reviewer", value_name = "NAME")]
        reviewers: Vec<String>,
        /// Keep it a draft, not waiting on anyone yet
        #[arg(long)]
        draft: bool,
        /// Who opens it
        #[arg(long)]
        agent: Option<String>,
    },
    /// Open a live review: the working tree against --base, as it is being made.
    Live {
        /// The commit it is against (HEAD by default)
        #[arg(long, value_name = "REV")]
        base: Option<String>,
        /// A task it reviews (repeatable; its key)
        #[arg(long = "task", value_name = "KEY")]
        tasks: Vec<String>,
        /// One line saying what it is
        #[arg(long, default_value = "Live review")]
        title: String,
        /// Who decides it (repeatable)
        #[arg(long = "reviewer", value_name = "NAME")]
        reviewers: Vec<String>,
        /// Who opens it
        #[arg(long)]
        agent: Option<String>,
    },
    /// The author's next round, after addressing the comments (the same kind as the last one unless given).
    Round {
        /// The review (CR-7)
        review: String,
        #[command(flatten)]
        change: Change,
        /// A new summary (the review's Description), in Markdown, from a file: what the reviewers should check now
        #[arg(long, value_name = "FILE")]
        summary_file: Option<String>,
        /// Who adds it
        #[arg(long)]
        agent: Option<String>,
    },
    /// Name more tasks in a review, or fewer.
    Tasks {
        /// The review (CR-7)
        review: String,
        /// A task to add (repeatable)
        #[arg(long, value_name = "KEY")]
        add: Vec<String>,
        /// A task to take out (repeatable)
        #[arg(long, value_name = "KEY")]
        remove: Vec<String>,
        /// Who changes them
        #[arg(long)]
        agent: Option<String>,
    },
    /// List the reviews, of every working copy, newest first.
    List {
        /// Only these statuses (repeatable): open (any not decided yet: draft, waiting, live or changes_requested), or
        /// one of draft, waiting, live, changes_requested, approved, closed
        #[arg(long = "status", value_name = "STATUS")]
        statuses: Vec<String>,
        /// Only the reviews of this task
        #[arg(long, value_name = "KEY")]
        task: Option<String>,
        /// Only the ones you opened or decide
        #[arg(long)]
        mine: bool,
        /// Only the ones whose title, summary or comments hold every word
        #[arg(long, value_name = "TEXT")]
        search: Option<String>,
    },
    /// A review: its header, summary, rounds and comments; with --round, that round's diff (--against M: what
    /// changed since round M).
    Show {
        /// The review (CR-7)
        review: String,
        /// The round whose diff to print
        #[arg(long, value_name = "N")]
        round: Option<u32>,
        /// With --round: only what changed since this round
        #[arg(long, value_name = "M", requires = "round")]
        against: Option<u32>,
    },
    /// A review's comments, as comments.md has them.
    Comments {
        /// The review (CR-7)
        review: String,
        /// Only the chains still open
        #[arg(long)]
        open: bool,
    },
    /// Start a chain: on a file's lines, a whole file, or (without --file) the review as a whole.
    Comment {
        /// The review (CR-7)
        review: String,
        /// The file it is about
        #[arg(long, value_name = "PATH")]
        file: Option<String>,
        /// Its lines, A-B (or A)
        #[arg(long, value_name = "A-B", requires = "file")]
        lines: Option<String>,
        /// The text it is about
        #[arg(long)]
        quote: Option<String>,
        /// On the old side of the change
        #[arg(long)]
        old: bool,
        /// The round it is on (the latest by default)
        #[arg(long, value_name = "N")]
        round: Option<u32>,
        #[command(flatten)]
        body: Body,
        /// Who writes it
        #[arg(long)]
        agent: Option<String>,
    },
    /// Poke a review: a person's nudge to its agent, which hears it through its review watch and answers it.
    Poke {
        /// The review (CR-7)
        review: String,
        /// What to say with it (optional)
        #[arg(long)]
        body: Option<String>,
        /// Who pokes (a person)
        #[arg(long)]
        agent: Option<String>,
    },
    /// Your drafts on a review: comments and replies only you see, sent together with your next `atm review submit`
    /// (`--comment` alone sends them). Kept in your atm state folder, never in the repository; a person's, not an
    /// agent's.
    Draft {
        #[command(subcommand)]
        command: DraftCommand,
    },
    /// Reply to a comment.
    Reply {
        /// The review (CR-7)
        review: String,
        /// The comment answered (c9)
        comment: String,
        #[command(flatten)]
        body: Body,
        /// Who writes it
        #[arg(long)]
        agent: Option<String>,
    },
    /// Change one's own comment.
    Edit {
        /// The review (CR-7)
        review: String,
        /// The comment (c9)
        comment: String,
        #[command(flatten)]
        body: Body,
        /// Whose comment it is
        #[arg(long)]
        agent: Option<String>,
    },
    /// Resolve a top-level comment (a reviewer's, or its own author's), or reopen it.
    Resolve {
        /// The review (CR-7)
        review: String,
        /// The top-level comment (c9)
        comment: String,
        /// Open it again
        #[arg(long)]
        reopen: bool,
        /// Who resolves it
        #[arg(long)]
        agent: Option<String>,
    },
    /// A reviewer's verdict: approve (every top-level comment resolved), request changes, or a comment on the whole.
    Submit {
        /// The review (CR-7)
        review: String,
        /// Approve it
        #[arg(long, conflicts_with_all = ["request_changes", "comment"])]
        approve: bool,
        /// Send it back for fixes
        #[arg(long, conflicts_with = "comment")]
        request_changes: bool,
        /// Only comment on it as a whole
        #[arg(long)]
        comment: bool,
        #[command(flatten)]
        body: Body,
        /// Override who may decide it (your own review, say), with why: recorded in the review. A person's call: no
        /// skill pre-approves it, so an agent's harness asks the user every time
        #[arg(long, value_name = "REASON")]
        override_rules: Option<String>,
        /// Who submits
        #[arg(long)]
        agent: Option<String>,
    },
    /// Wait until a reviewer submits on a review (or decides it): what happened, and the comments added meanwhile.
    /// Exit code 3 when the time is up first.
    Wait {
        /// The review (CR-7)
        review: String,
        /// What to wait for: submitted (a decision, or a comment on the whole) or decision
        #[arg(long = "for", default_value = "submitted", value_parser = ["submitted", "decision"])]
        until: String,
        /// How long at most: 90s, 30m, 2h (seconds without a unit)
        #[arg(long, default_value = "2h")]
        timeout: String,
        /// Who waits (their own comments aren't news)
        #[arg(long)]
        agent: Option<String>,
    },
    /// The agent's one watch for its session: each person's comment about the agent's work, from any review, as it is
    /// added (a JSON object a line with --json), until the time is up. Names no review and no task.
    Watch {
        /// How long at most: 90s, 30m, 8h (seconds without a unit)
        #[arg(long, default_value = "8h")]
        timeout: String,
        /// Stop once something was told (exit code 3 if nothing was, at the time up)
        #[arg(long)]
        once: bool,
        /// Tell what came since this time (2026-10-24T18:00:00Z), whatever the last watch told
        #[arg(long)]
        since: Option<String>,
        /// Who watches
        #[arg(long)]
        agent: Option<String>,
    },
    /// What you haven't read about your work in the reviews (ATM-708): each person's comment and decision since you
    /// last read them, as a watch prints them (a JSON object a line with --json), oldest first; read from then on. A
    /// watch with --once counts as read; one that runs on doesn't (its lines may never be read).
    Inbox {
        /// Only look: leave them unread
        #[arg(long)]
        peek: bool,
        /// Run as Claude Code's hook (`atm agents hooks install`): its input on stdin, its answer on stdout
        #[arg(long, conflicts_with = "peek")]
        hook: bool,
        /// Whose inbox
        #[arg(long)]
        agent: Option<String>,
    },
    /// End a review without approval.
    Close {
        /// The review (CR-7)
        review: String,
        /// Why
        #[arg(long)]
        reason: String,
        /// Reject its tasks in review too
        #[arg(long)]
        reject_tasks: bool,
        /// Who closes it
        #[arg(long)]
        agent: Option<String>,
    },
}

fn opened(cwd: &Path, agent: Option<&str>) -> Result<Project> {
    Ok(Project::open(cwd)?.with_actor(Some(agent_identity(agent, cwd))))
}

fn actor(project: &Project) -> Actor {
    Actor(project.actor().unwrap_or("someone").to_string())
}

fn text_of(body: Body, cwd: &Path) -> Result<String> {
    let text = match (body.body, body.body_file) {
        (Some(text), _) => text,
        (None, Some(file)) => crate::option_file::read_option_file(cwd, "body-file", &file)?,
        (None, None) => {
            return Err(Error::Invalid(
                "give the text with --body or --body-file".into(),
            ));
        }
    };
    if text.trim().is_empty() {
        return Err(Error::Invalid("the text is empty".into()));
    }
    Ok(text)
}

/// A round from what the command names; with nothing named, the same kind as `previous` (a range needs naming), in
/// the same repository.
fn new_round(
    project: &Project,
    change: Change,
    cwd: &Path,
    previous: Option<&Round>,
    previous_files: &[String],
) -> Result<NewRound> {
    let vcs = vcs_of(project)?;
    let by = actor(project);
    if let Some(range) = change.range {
        let (base, head) = range
            .split_once("..")
            .ok_or_else(|| Error::Invalid(format!("--range takes BASE..HEAD, not {range}")))?;
        let here = reviews::repository_here(&vcs, cwd)?;
        return rounds::from_range(
            &here,
            base,
            if head.is_empty() { "HEAD" } else { head },
            &by,
        );
    }
    if !change.files.is_empty() {
        let mut paths = vcs.paths(&change.files, Some(cwd))?;
        // A later files round keeps the files the last one reviewed (ATM-622): naming only the file that changed made
        // a round of that file alone, whose whole change was that file and since which every other one looked deleted.
        // A file gone since isn't kept
        if previous.is_some_and(|last| last.kind == RoundKind::Files) {
            let root = project.root().map(Path::to_path_buf).unwrap_or_default();
            for kept in previous_files {
                if !paths.contains(kept) && root.join(kept).is_file() {
                    paths.push(kept.clone());
                }
            }
        }
        return rounds::from_files(&reviews::repository_of_files(&vcs, &paths)?, &paths, &by);
    }
    match (change.uncommitted, previous) {
        (true, _) => rounds::from_working(
            &reviews::repository_here(&vcs, cwd)?,
            change.base.as_deref().unwrap_or("HEAD"),
            &by,
        ),
        (false, Some(last)) => {
            let same = rounds::repository(&vcs, last)?;
            match &last.kind {
                RoundKind::Working { base } => {
                    rounds::from_working(&same, change.base.as_deref().unwrap_or(base), &by)
                }
                RoundKind::Files => rounds::from_files(&same, previous_files, &by),
                RoundKind::Range { .. } => Err(Error::Invalid(
                    "say what to review: --range BASE..HEAD, --uncommitted, or --file PATH".into(),
                )),
            }
        }
        _ => Err(Error::Invalid(
            "say what to review: --range BASE..HEAD, --uncommitted, or --file PATH".into(),
        )),
    }
}

fn reference(review: &str) -> ReviewRef {
    ReviewRef(review.to_string())
}

fn summary_line(header: &ReviewHeader) -> String {
    let tasks = if header.tasks.is_empty() {
        String::new()
    } else {
        format!(" · {}", header.tasks.join(", "))
    };
    format!(
        "{} · {} · {}{tasks}",
        header.reference,
        header.status.name().replace('_', " "),
        header.title
    )
}

/// A review's value as JSON (its types are serde's; the CLI names no serde itself).
macro_rules! as_json {
    ($value:expr) => {
        serde_json::to_value($value).unwrap_or(Value::Null)
    };
}

// The commands, a module per group (ATM-711), after the macro they use
mod decide;
mod listen;
mod open;
mod read;
mod talk;

#[cfg(test)]
use listen::duration_of;
#[cfg(test)]
use talk::parse_lines;

/// `atm review draft …` (ATM-612).
#[derive(Debug, Subcommand)]
pub enum DraftCommand {
    /// Draft a comment (on a file's lines, a whole file, or the review as a whole), or with --reply-to, a reply.
    Add {
        /// The review (CR-7)
        review: String,
        /// The comment it answers (c9): a reply
        #[arg(long, value_name = "ID", conflicts_with_all = ["file", "lines", "quote", "old", "round"])]
        reply_to: Option<String>,
        /// The file it is about
        #[arg(long, value_name = "PATH")]
        file: Option<String>,
        /// Its lines, A-B (or A)
        #[arg(long, value_name = "A-B", requires = "file")]
        lines: Option<String>,
        /// The text it is about
        #[arg(long)]
        quote: Option<String>,
        /// On the old side of the change
        #[arg(long)]
        old: bool,
        /// The round it is on (the latest by default)
        #[arg(long, value_name = "N")]
        round: Option<u32>,
        #[command(flatten)]
        body: Body,
        /// Who drafts (a person)
        #[arg(long)]
        agent: Option<String>,
    },
    /// Your drafts on a review, the oldest first.
    List {
        /// The review (CR-7)
        review: String,
        /// Whose (a person)
        #[arg(long)]
        agent: Option<String>,
    },
    /// Change a draft's text.
    Edit {
        /// The review (CR-7)
        review: String,
        /// The draft (d2)
        draft: String,
        #[command(flatten)]
        body: Body,
        /// Whose (a person)
        #[arg(long)]
        agent: Option<String>,
    },
    /// Throw a draft away.
    Remove {
        /// The review (CR-7)
        review: String,
        /// The draft (d2)
        draft: String,
        /// Whose (a person)
        #[arg(long)]
        agent: Option<String>,
    },
}

/// The statuses `--status <name>` stands for: `open`, every review not decided yet (ATM-613); any other, its own.
fn status_of(name: &str) -> Result<Vec<Status>> {
    Status::filter_named(name).ok_or_else(|| {
        Error::Invalid(format!(
            "no status {name}: open (any not decided yet), draft, waiting, live, changes_requested, approved or closed"
        ))
    })
}

/// Run a review command in `cwd`.
///
/// # Errors
/// What the review's rules or atm-lib refuse, and a project or review that can't be read.
pub fn run(command: ReviewCommand, ctx: Context, cwd: &Path) -> Result<u8> {
    match command {
        ReviewCommand::Wait { .. } | ReviewCommand::Watch { .. } | ReviewCommand::Inbox { .. } => {
            listen::run(command, ctx, cwd)
        }
        ReviewCommand::Create { .. }
        | ReviewCommand::Live { .. }
        | ReviewCommand::Round { .. }
        | ReviewCommand::Tasks { .. } => open::run(command, ctx, cwd),
        ReviewCommand::List { .. }
        | ReviewCommand::Show { .. }
        | ReviewCommand::Comments { .. } => read::run(command, ctx, cwd),
        ReviewCommand::Comment { .. }
        | ReviewCommand::Poke { .. }
        | ReviewCommand::Draft { .. }
        | ReviewCommand::Reply { .. }
        | ReviewCommand::Edit { .. }
        | ReviewCommand::Resolve { .. } => talk::run(command, ctx, cwd),
        ReviewCommand::Submit { .. } | ReviewCommand::Close { .. } => {
            decide::run(command, ctx, cwd)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_are_a_range_or_one_line_counted_from_one() {
        for (given, expected) in [("2", (2, 2)), ("3-7", (3, 7)), ("5-5", (5, 5))] {
            assert_eq!(parse_lines(given).unwrap(), expected, "{given}");
        }
        for bad in ["", "0", "0-3", "7-3", "a", "1-b", "-3", "3-", "1-2-3", " 2"] {
            let e = parse_lines(bad).unwrap_err();
            assert_eq!(e.code(), 2, "{bad}");
            assert_eq!(e.to_string(), format!("--lines takes A-B or A, not {bad}"));
        }
    }

    #[test]
    fn a_timeout_is_seconds_minutes_or_hours() {
        for (given, seconds) in [
            ("90s", 90.0),
            ("90", 90.0),
            ("30m", 1800.0),
            ("2h", 7200.0),
            ("1.5m", 90.0),
            ("0.5", 0.5),
        ] {
            assert_eq!(
                duration_of(given).unwrap(),
                std::time::Duration::from_secs_f64(seconds),
                "{given}"
            );
        }
        for bad in [
            "", "0", "0s", "-5m", "soon", "5d", "m", "NaN", "inf", "1e400", "1e20", "1e20h",
        ] {
            let e = duration_of(bad).unwrap_err();
            assert_eq!(e.code(), 2, "{bad}");
            assert!(e.to_string().starts_with("--timeout takes"), "{bad}: {e}");
        }
    }

    #[test]
    fn a_status_is_one_of_the_reviews() {
        for name in [
            "draft",
            "waiting",
            "live",
            "changes_requested",
            "approved",
            "closed",
        ] {
            let named: Vec<&str> = status_of(name).unwrap().iter().map(|s| s.name()).collect();
            assert_eq!(named, [name]);
        }
        // Open: every review not decided yet (ATM-613)
        assert_eq!(status_of("open").unwrap(), Status::OPEN);
        for bad in ["", "Open", "changes-requested", "merged"] {
            assert_eq!(status_of(bad).unwrap_err().code(), 2, "{bad}");
        }
    }

    #[test]
    fn a_text_is_given_or_read_and_never_empty() {
        let dir = std::env::temp_dir().join(format!("atm-cli-text-of-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("body.md"), "From the file.\n").unwrap();
        std::fs::write(dir.join("blank.md"), " \n\t\n").unwrap();
        let body = |text: Option<&str>, file: Option<&str>| Body {
            body: text.map(String::from),
            body_file: file.map(String::from),
        };
        assert_eq!(text_of(body(Some("Given"), None), &dir).unwrap(), "Given");
        assert_eq!(
            text_of(body(None, Some("body.md")), &dir).unwrap(),
            "From the file.\n"
        );
        // [body, expected code, the message's start]
        for (given, code, says) in [
            (
                body(None, None),
                2,
                "give the text with --body or --body-file",
            ),
            (body(Some(""), None), 2, "the text is empty"),
            (body(Some("  \n"), None), 2, "the text is empty"),
            (body(None, Some("blank.md")), 2, "the text is empty"),
            // A file that can't be used is a usage error, as every --*-file option says it (ATM-662)
            (
                body(None, Some("gone.md")),
                2,
                "--body-file gone.md: can't read it",
            ),
        ] {
            let e = text_of(given, &dir).unwrap_err();
            assert_eq!(e.code(), code, "{says}");
            assert!(e.to_string().starts_with(says), "{e}");
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
