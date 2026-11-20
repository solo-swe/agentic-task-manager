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

/// A draft, for people: its ID, where it goes, its text.
fn draft_line(draft: &atm_lib::review_drafts::Draft) -> String {
    use atm_lib::review_drafts::Place;
    let place = match &draft.place {
        Place::Reply { to } => format!("reply to {to}"),
        Place::Comment {
            path: Some(path),
            lines: Some((a, b)),
            ..
        } => format!("{path}:{a}-{b}"),
        Place::Comment {
            path: Some(path), ..
        } => path.clone(),
        Place::Comment { .. } => "the review".into(),
    };
    format!("{} ({place}): {}", draft.id, draft.body.replace('\n', " "))
}

/// Run a draft command.
fn run_draft(command: DraftCommand, ctx: Context, cwd: &Path) -> Result<u8> {
    use atm_lib::review_drafts::{self as drafts, DraftUpdate, NewDraft};
    match command {
        DraftCommand::Add {
            review,
            reply_to,
            file,
            lines,
            quote,
            old,
            round,
            body,
            agent,
        } => {
            let project = opened(cwd, agent.as_deref())?;
            let new = NewDraft {
                reply_to,
                path: file,
                lines: lines.as_deref().map(parse_lines).transpose()?,
                side: if old { Side::Old } else { Side::New },
                quote,
                round,
                body: text_of(body, cwd)?,
            };
            if ctx.dry_run {
                println!("[dry-run] would draft on {review}");
                return Ok(0);
            }
            let draft = drafts::add_draft(&project, &reference(&review), new)?;
            if ctx.json {
                print_json(&as_json!(&draft));
            } else {
                println!("{review}: drafted {}", draft_line(&draft));
            }
        }
        DraftCommand::List { review, agent } => {
            let project = opened(cwd, agent.as_deref())?;
            let listed = drafts::list_drafts(&project, &reference(&review))?;
            if ctx.json {
                print_json(&json!({ "review": review, "drafts": as_json!(&listed) }));
            } else if listed.is_empty() {
                println!("{review}: no drafts");
            } else {
                for draft in &listed {
                    println!("{}", draft_line(draft));
                }
            }
        }
        DraftCommand::Edit {
            review,
            draft,
            body,
            agent,
        } => {
            let project = opened(cwd, agent.as_deref())?;
            let update = DraftUpdate {
                body: text_of(body, cwd)?,
            };
            if ctx.dry_run {
                println!("[dry-run] would change {draft} on {review}");
                return Ok(0);
            }
            let changed = drafts::update_draft(&project, &reference(&review), &draft, &update)?;
            if ctx.json {
                print_json(&as_json!(&changed));
            } else {
                println!("{review}: changed {}", draft_line(&changed));
            }
        }
        DraftCommand::Remove {
            review,
            draft,
            agent,
        } => {
            let project = opened(cwd, agent.as_deref())?;
            if ctx.dry_run {
                println!("[dry-run] would remove {draft} from {review}");
                return Ok(0);
            }
            drafts::remove_draft(&project, &reference(&review), &draft)?;
            if ctx.json {
                print_json(&json!({ "review": review, "draft": draft, "removed": true }));
            } else {
                println!("{review}: removed {draft}");
            }
        }
    }
    Ok(0)
}

fn parse_lines(lines: &str) -> Result<(u32, u32)> {
    let bad = || Error::Invalid(format!("--lines takes A-B or A, not {lines}"));
    let (first, last) = lines.split_once('-').unwrap_or((lines, lines));
    let (first, last): (u32, u32) = (
        first.parse().map_err(|_| bad())?,
        last.parse().map_err(|_| bad())?,
    );
    if first == 0 || last < first {
        return Err(bad());
    }
    Ok((first, last))
}

/// `90s`, `30m`, `2h`, or seconds without a unit.
fn duration_of(text: &str) -> Result<std::time::Duration> {
    let bad = || {
        Error::Invalid(format!(
            "--timeout takes 90s, 30m, 2h or seconds, not {text}"
        ))
    };
    let (number, unit) = match text.char_indices().last() {
        Some((at, 's' | 'm' | 'h')) => (&text[..at], &text[at..]),
        _ => (text, "s"),
    };
    let n: f64 = number.parse().map_err(|_| bad())?;
    if !n.is_finite() || n <= 0.0 {
        return Err(bad());
    }
    let seconds = n * match unit {
        "m" => 60.0,
        "h" => 3600.0,
        _ => 1.0,
    };
    Ok(std::time::Duration::from_secs_f64(seconds))
}

/// A comment heard, for people: where, who, and its text.
fn heard_line(heard: &Value) -> String {
    let text = |key: &str| {
        heard
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let task = heard
        .get("task")
        .and_then(Value::as_str)
        .map_or_else(String::new, |t| format!(" ({t})"));
    let place = match (
        heard.get("path").and_then(Value::as_str),
        heard.get("lines").and_then(Value::as_array),
    ) {
        (Some(path), Some(lines)) => format!(" on {path}:{}-{}", lines[0], lines[1]),
        (Some(path), None) => format!(" on {path}"),
        _ => String::new(),
    };
    format!(
        "{} {}{task}{place}, {}: {}",
        text("review"),
        text("comment"),
        text("author"),
        text("body").replace('\n', " ")
    )
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
#[allow(clippy::too_many_lines)] // reason: one arm per command
pub fn run(command: ReviewCommand, ctx: Context, cwd: &Path) -> Result<u8> {
    match command {
        ReviewCommand::Wait {
            review,
            until,
            timeout,
            agent,
        } => {
            let project = opened(cwd, agent.as_deref())?;
            let me = actor(&project);
            let until = if until == "decision" {
                atm_lib::review_listen::WaitFor::Decision
            } else {
                atm_lib::review_listen::WaitFor::Submitted
            };
            let waited = atm_lib::review_listen::wait(
                &project,
                &reference(&review),
                until,
                duration_of(&timeout)?,
                &me.0,
            )?;
            let Some(waited) = waited else {
                if ctx.json {
                    print_json(&json!({ "review": review, "timed_out": true }));
                } else {
                    println!("{review}: nothing yet (the time is up)");
                }
                return Ok(3);
            };
            if ctx.json {
                print_json(&waited);
            } else {
                let header: atm_lib::reviews::ReviewHeader =
                    serde_json::from_value(waited["review"].clone())
                        .map_err(|e| Error::failed(e.to_string()))?;
                println!("{}", summary_line(&header));
                for heard in waited["comments"].as_array().into_iter().flatten() {
                    println!("  {}", heard_line(heard));
                }
            }
            return Ok(0);
        }
        ReviewCommand::Watch {
            timeout,
            once,
            since,
            agent,
        } => {
            let project = opened(cwd, agent.as_deref())?;
            let me = actor(&project);
            let mut ready = || eprintln!("Watching the reviews for {}'s work.", me.0);
            let mut emit = |heard: Value| {
                if ctx.json {
                    // One object a line, exactly, as print_json prints (I30): a reader takes each as it comes
                    ::std::println!("{heard}");
                } else {
                    println!("{}", heard_line(&heard));
                }
            };
            let told = atm_lib::review_listen::watch(
                &project,
                &me.0,
                duration_of(&timeout)?,
                &atm_lib::review_listen::WatchOptions { once, since },
                &mut ready,
                &mut emit,
            )?;
            // --once with nothing told by the time up: exit code 3, as `atm review wait` (ATM-624)
            return Ok(if once && told == 0 { 3 } else { 0 });
        }
        ReviewCommand::Create {
            title,
            tasks,
            change,
            summary_file,
            reviewers,
            draft,
            agent,
        } => {
            let mut project = opened(cwd, agent.as_deref())?;
            let summary = match summary_file {
                Some(file) => crate::option_file::read_option_file(cwd, "summary-file", &file)?,
                None => String::new(),
            };
            let first = new_round(&project, change, cwd, None, &[])?;
            if ctx.dry_run {
                println!(
                    "[dry-run] would open a review “{title}” of {} file(s)",
                    first.files.len()
                );
                return Ok(0);
            }
            let author = actor(&project);
            let header = reviews::create(
                &mut project,
                NewReview {
                    title,
                    summary,
                    tasks,
                    author: Some(author),
                    reviewers: reviewers.into_iter().map(Actor).collect(),
                    status: if draft {
                        Status::Draft
                    } else {
                        Status::Waiting
                    },
                    first,
                },
            )?;
            if ctx.json {
                print_json(&as_json!(&header));
            } else {
                println!("Opened {}", summary_line(&header));
            }
        }
        ReviewCommand::Live {
            base,
            tasks,
            title,
            reviewers,
            agent,
        } => {
            let mut project = opened(cwd, agent.as_deref())?;
            let change = Change {
                uncommitted: true,
                base,
                ..Change::default()
            };
            let first = new_round(&project, change, cwd, None, &[])?;
            if ctx.dry_run {
                println!("[dry-run] would open a live review “{title}”");
                return Ok(0);
            }
            let author = actor(&project);
            let header = reviews::create(
                &mut project,
                NewReview {
                    title,
                    summary: String::new(),
                    tasks,
                    // A person opening it reviews an agent's work: atm-lib names the author (§3.5)
                    author: Some(author),
                    reviewers: reviewers.into_iter().map(Actor).collect(),
                    status: Status::Live,
                    first,
                },
            )?;
            if ctx.json {
                print_json(&as_json!(&header));
            } else {
                println!("Opened {}", summary_line(&header));
            }
        }
        ReviewCommand::Round {
            review,
            change,
            summary_file,
            agent,
        } => {
            let mut project = opened(cwd, agent.as_deref())?;
            let store = reviews::store(&project)?;
            let found = store.get_review(&reference(&review))?;
            let last = found.rounds.last();
            let files: Vec<String> = last
                .map(|r| r.files.iter().map(|f| f.path.clone()).collect())
                .unwrap_or_default();
            // The new summary read before anything is written
            let summary = summary_file
                .map(|file| crate::option_file::read_option_file(cwd, "summary-file", &file))
                .transpose()?;
            let round = new_round(&project, change, cwd, last, &files)?;
            if ctx.dry_run {
                let with = if summary.is_some() {
                    ", with a new summary"
                } else {
                    ""
                };
                println!(
                    "[dry-run] would add a round to {}{with}",
                    found.header.reference
                );
                return Ok(0);
            }
            let number = reviews::add_round(&mut project, &found.header.reference, round)?;
            if let Some(summary) = summary {
                reviews::edit_summary(&mut project, &found.header.reference, summary.trim_end())?;
            }
            if ctx.json {
                print_json(&json!({ "review": found.header.reference, "round": number }));
            } else {
                println!("{}: round {number}", found.header.reference);
            }
        }
        ReviewCommand::Tasks {
            review,
            add,
            remove,
            agent,
        } => {
            let mut project = opened(cwd, agent.as_deref())?;
            if ctx.dry_run {
                println!("[dry-run] would change {review}'s tasks");
                return Ok(0);
            }
            let header = reviews::set_tasks(&mut project, &reference(&review), &add, &remove)?;
            if ctx.json {
                print_json(&as_json!(&header));
            } else {
                println!(
                    "{}: {}",
                    header.reference,
                    if header.tasks.is_empty() {
                        "no tasks".into()
                    } else {
                        header.tasks.join(", ")
                    }
                );
            }
        }
        ReviewCommand::List {
            statuses,
            task,
            mine,
            search,
        } => {
            let project = opened(cwd, None)?;
            let me = actor(&project);
            let filter = ReviewFilter {
                statuses: statuses
                    .iter()
                    .map(|s| status_of(s))
                    .collect::<Result<Vec<_>>>()?
                    .concat(),
                task: task.map(|t| reviews::task_reference(&project, &t)),
            };
            let root = project
                .root()
                .ok_or_else(|| Error::failed("no project here"))?;
            let store = reviews::store(&project)?;
            let words: Vec<String> = search
                .as_deref()
                .unwrap_or_default()
                .split_whitespace()
                .map(str::to_lowercase)
                .collect();
            let mut found = Vec::new();
            for f in reviews::list_naming(&project, root, &filter)? {
                if mine
                    && f.header.author.as_ref() != Some(&me)
                    && !f.header.reviewers.contains(&me)
                {
                    continue;
                }
                if !words.is_empty() {
                    let review = FileStore::open(&f.working_copy.join(atm_common::TASK_DIR))
                        .and_then(|s| s.get_review(&ReviewRef(f.header.id.0.clone())))
                        .or_else(|_| store.get_review(&f.header.reference))?;
                    let mut text = format!("{} {}", review.header.title, review.summary);
                    for comment in review.comments() {
                        text.push(' ');
                        text.push_str(&comment.body);
                    }
                    let text = text.to_lowercase();
                    if !words.iter().all(|w| text.contains(w)) {
                        continue;
                    }
                }
                found.push(f);
            }
            // Whatever the filter (ATM-617): a review atm can't read is listed as such, never left out without a word.
            // With --json, on stderr: the list stays the array older readers expect
            let unreadable: Vec<String> = store
                .unreadable()?
                .iter()
                .map(|u| {
                    format!(
                        "{} ({}): can't be read: {}. A newer atm may have written it.",
                        u.reference, u.folder, u.problem
                    )
                })
                .collect();
            if ctx.json {
                print_json(&Value::Array(
                    found
                        .iter()
                        .map(|f| json!({ "review": as_json!(&f.header), "branch": f.branch }))
                        .collect(),
                ));
                for line in &unreadable {
                    eprintln!("atm: {line}");
                }
            } else {
                if found.is_empty() {
                    println!("No reviews.");
                }
                for f in &found {
                    let branch = f
                        .branch
                        .as_deref()
                        .map_or_else(String::new, |b| format!(" · {b}"));
                    println!("{}{branch}", summary_line(&f.header));
                }
                for line in &unreadable {
                    println!("{line}");
                }
            }
        }
        ReviewCommand::Show {
            review,
            round,
            against,
        } => {
            let project = opened(cwd, None)?;
            let store = reviews::store(&project)?;
            let found = store.get_review(&reference(&review))?;
            if let Some(round) = round {
                let patch = match against {
                    Some(from) => rounds::compare(&vcs_of(&project)?, &store, &found, from, round)?,
                    None => store.round_diff(&found.header.id, round)?,
                };
                if ctx.json {
                    print_json(
                        &json!({ "review": found.header.reference, "round": round, "against": against, "diff": patch.0 }),
                    );
                } else {
                    print!("{}", patch.0);
                }
                return Ok(0);
            }
            if ctx.json {
                print_json(&as_json!(&found));
                return Ok(0);
            }
            println!("{}", summary_line(&found.header));
            let reviewers: Vec<&str> = found
                .header
                .reviewers
                .iter()
                .map(|r| r.0.as_str())
                .collect();
            println!(
                "  by {} · {} · opened {}",
                found
                    .header
                    .author
                    .as_ref()
                    .map_or("nobody yet (an agent's first round names them)", |a| a
                        .0
                        .as_str()),
                if reviewers.is_empty() {
                    "decided by anyone but its author".to_string()
                } else {
                    format!("decided by {}", reviewers.join(", "))
                },
                found.header.created
            );
            if !found.summary.is_empty() {
                println!();
                for line in found.summary.lines() {
                    println!("  {line}");
                }
            }
            println!();
            for r in &found.rounds {
                let what = match &r.kind {
                    RoundKind::Range { base, head } => format!(
                        "{}..{}",
                        &base[..base.len().min(7)],
                        &head[..head.len().min(7)]
                    ),
                    RoundKind::Working { base } => {
                        format!("uncommitted, on {}", &base[..base.len().min(7)])
                    }
                    RoundKind::Files => "files as they are".into(),
                };
                println!(
                    "  round {} · {what} · {} file(s) · {} · {}",
                    r.number,
                    r.files.len(),
                    r.by.0,
                    r.at
                );
            }
            let open = found.open_comments().count();
            println!("  {} chain(s), {open} open", found.chains.len());
        }
        ReviewCommand::Comments { review, open } => {
            let project = opened(cwd, None)?;
            let store = reviews::store(&project)?;
            let found = store.get_review(&reference(&review))?;
            let chains =
                store.list_comments(&found.header.id, &CommentFilter { open_only: open })?;
            if ctx.json {
                print_json(&as_json!(&chains));
            } else if chains.is_empty() {
                println!(
                    "{}: no comments{}",
                    found.header.reference,
                    if open { " open" } else { "" }
                );
            } else {
                print!(
                    "{}",
                    comments_md::write(&CommentsFile {
                        chains,
                        unread: Vec::new()
                    })
                );
            }
        }
        ReviewCommand::Comment {
            review,
            file,
            lines,
            quote,
            old,
            round,
            body,
            agent,
        } => {
            let project = opened(cwd, agent.as_deref())?;
            let text = text_of(body, cwd)?;
            let found = reviews::store(&project)?.get_review(&reference(&review))?;
            let comment = atm_lib::review_requests::NewComment {
                path: file,
                lines: lines.as_deref().map(parse_lines).transpose()?,
                side: if old { Side::Old } else { Side::New },
                quote,
                round,
                body: text,
            };
            if ctx.dry_run {
                println!("[dry-run] would comment on {}", found.header.reference);
                return Ok(0);
            }
            // On a live review, the working copy as it is now first becomes a checkpoint (ATM-576)
            let id =
                atm_lib::review_requests::add_comment(&project, &found.header.reference, comment)?;
            if ctx.json {
                print_json(&json!({ "review": found.header.reference, "comment": id }));
            } else {
                println!("{}: {id}", found.header.reference);
            }
        }
        ReviewCommand::Poke {
            review,
            body,
            agent,
        } => {
            let project = opened(cwd, agent.as_deref())?;
            if ctx.dry_run {
                println!("[dry-run] would poke {review}");
                return Ok(0);
            }
            let id = atm_lib::review_requests::poke_review(
                &project,
                &reference(&review),
                &atm_lib::review_requests::NewPoke { body },
            )?;
            if ctx.json {
                print_json(&json!({ "review": review, "poke": id }));
            } else {
                println!("{review}: poked ({id})");
            }
        }
        ReviewCommand::Draft { command } => return run_draft(command, ctx, cwd),
        ReviewCommand::Reply {
            review,
            comment,
            body,
            agent,
        } => {
            let project = opened(cwd, agent.as_deref())?;
            let text = text_of(body, cwd)?;
            let mut store = reviews::store(&project)?;
            if ctx.dry_run {
                println!("[dry-run] would reply to {comment} in {review}");
                return Ok(0);
            }
            let id = Reviews::new(&mut store).reply(
                &reference(&review),
                &CommentId(comment.clone()),
                &text,
                &actor(&project),
            )?;
            if ctx.json {
                print_json(&json!({ "review": review, "comment": id, "reply_to": comment }));
            } else {
                println!("{review}: {id}, in reply to {comment}");
            }
        }
        ReviewCommand::Edit {
            review,
            comment,
            body,
            agent,
        } => {
            let project = opened(cwd, agent.as_deref())?;
            let text = text_of(body, cwd)?;
            let mut store = reviews::store(&project)?;
            if ctx.dry_run {
                println!("[dry-run] would edit {comment} in {review}");
                return Ok(0);
            }
            Reviews::new(&mut store).edit(
                &reference(&review),
                &CommentId(comment.clone()),
                &text,
                &actor(&project),
            )?;
            if ctx.json {
                print_json(&json!({ "review": review, "comment": comment }));
            } else {
                println!("{review}: {comment} changed");
            }
        }
        ReviewCommand::Resolve {
            review,
            comment,
            reopen,
            agent,
        } => {
            let project = opened(cwd, agent.as_deref())?;
            let mut store = reviews::store(&project)?;
            if ctx.dry_run {
                println!(
                    "[dry-run] would {} {comment} in {review}",
                    if reopen { "reopen" } else { "resolve" }
                );
                return Ok(0);
            }
            Reviews::new(&mut store).resolve(
                &reference(&review),
                &CommentId(comment.clone()),
                !reopen,
                &actor(&project),
            )?;
            if ctx.json {
                print_json(&json!({ "review": review, "comment": comment, "resolved": !reopen }));
            } else {
                println!(
                    "{review}: {comment} {}",
                    if reopen { "open again" } else { "resolved" }
                );
            }
        }
        ReviewCommand::Submit {
            review,
            approve,
            request_changes,
            comment,
            body,
            override_rules,
            agent,
        } => {
            use atm_lib::review_requests::{Decision, DecisionKind, RulesOverride};
            let decision = match (approve, request_changes, comment) {
                (true, _, _) => DecisionKind::Approve,
                (_, true, _) => DecisionKind::RequestChanges,
                (_, _, true) => DecisionKind::Comment,
                _ => {
                    return Err(Error::Invalid(
                        "submit with --approve, --request-changes or --comment".into(),
                    ));
                }
            };
            let mut project = opened(cwd, agent.as_deref())?;
            let text = if body.body.is_some() || body.body_file.is_some() {
                Some(text_of(body, cwd)?)
            } else {
                None
            };
            if ctx.dry_run {
                println!("[dry-run] would submit on {review}");
                return Ok(0);
            }
            // With the person's drafts on it (ATM-612): `--comment` without a text only sends them
            let header = atm_lib::review_requests::submit_decision(
                &mut project,
                &reference(&review),
                &Decision {
                    decision,
                    body: text,
                    override_rules: override_rules.map(|reason| RulesOverride { reason }),
                },
            )?;
            if ctx.json {
                print_json(&as_json!(&header));
            } else {
                println!("{}", summary_line(&header));
            }
        }
        ReviewCommand::Close {
            review,
            reason,
            reject_tasks,
            agent,
        } => {
            let mut project = opened(cwd, agent.as_deref())?;
            if ctx.dry_run {
                println!("[dry-run] would close {review}");
                return Ok(0);
            }
            reviews::close(&mut project, &reference(&review), &reason, reject_tasks)?;
            let header = reviews::store(&project)?
                .get_review(&reference(&review))?
                .header;
            if ctx.json {
                print_json(&as_json!(&header));
            } else {
                println!("{}", summary_line(&header));
            }
        }
    }
    Ok(0)
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
            "", "0", "0s", "-5m", "soon", "5d", "m", "NaN", "inf", "1e400",
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
            (body(None, Some("gone.md")), 2, "--body-file gone.md: can't read it"),
        ] {
            let e = text_of(given, &dir).unwrap_err();
            assert_eq!(e.code(), code, "{says}");
            assert!(e.to_string().starts_with(says), "{e}");
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
