//! `atm review`: code reviews recorded in the project (docs/code-review-design.md §3.9, ATM-566), thin mappings to
//! atm-lib's `reviews` (which keeps the tasks in step). Comments are task text: printed for people through
//! `output::for_people` (I30), never run (I1).

use std::path::Path;

use atm_common::{Error, Result};
use atm_lib::Project;
use atm_lib::actor::agent_identity;
use atm_lib::reviews::comments_md::{self, CommentsFile};
use atm_lib::reviews::{
    self, Actor, Anchor, CommentFilter, CommentId, FileStore, NewReview, NewRound, ReviewFilter,
    ReviewHeader, ReviewRef, ReviewStore, Reviews, RoundKind, Side, Status, Verdict, rounds,
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
        /// Only these statuses (repeatable): draft, open, live, changes_requested, approved, closed
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
        /// Who submits
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
        (None, Some(file)) => std::fs::read_to_string(cwd.join(&file))
            .map_err(|e| Error::failed(format!("can't read {file}: {e}")))?,
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

/// A round from what the command names; with nothing named, the same kind as `previous` (a range needs naming).
fn new_round(
    project: &Project,
    change: Change,
    cwd: &Path,
    previous: Option<&RoundKind>,
    previous_files: &[String],
) -> Result<NewRound> {
    let vcs = vcs_of(project)?;
    let by = actor(project);
    if let Some(range) = change.range {
        let (base, head) = range
            .split_once("..")
            .ok_or_else(|| Error::Invalid(format!("--range takes BASE..HEAD, not {range}")))?;
        return rounds::from_range(&vcs, base, if head.is_empty() { "HEAD" } else { head }, &by);
    }
    if !change.files.is_empty() {
        let paths = vcs.paths(&change.files, Some(cwd))?;
        return rounds::from_files(&vcs, &paths, &by);
    }
    match (change.uncommitted, previous) {
        (true, _) => rounds::from_working(&vcs, change.base.as_deref().unwrap_or("HEAD"), &by),
        (false, Some(RoundKind::Working { base })) => {
            rounds::from_working(&vcs, change.base.as_deref().unwrap_or(base), &by)
        }
        (false, Some(RoundKind::Files)) => rounds::from_files(&vcs, previous_files, &by),
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

fn status_of(name: &str) -> Result<Status> {
    serde_json::from_value(json!(name)).map_err(|_| {
        Error::Invalid(format!(
            "no status {name}: draft, open, live, changes_requested, approved or closed"
        ))
    })
}

/// Run a review command in `cwd`.
///
/// # Errors
/// What the review's rules or atm-lib refuse, and a project or review that can't be read.
#[allow(clippy::too_many_lines)] // reason: one arm per command
pub fn run(command: ReviewCommand, ctx: Context, cwd: &Path) -> Result<()> {
    match command {
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
                Some(file) => std::fs::read_to_string(cwd.join(&file))
                    .map_err(|e| Error::failed(format!("can't read {file}: {e}")))?,
                None => String::new(),
            };
            let first = new_round(&project, change, cwd, None, &[])?;
            if ctx.dry_run {
                println!(
                    "[dry-run] would open a review “{title}” of {} file(s)",
                    first.files.len()
                );
                return Ok(());
            }
            let author = actor(&project);
            let header = reviews::create(
                &mut project,
                NewReview {
                    title,
                    summary,
                    tasks,
                    author,
                    reviewers: reviewers.into_iter().map(Actor).collect(),
                    status: if draft { Status::Draft } else { Status::Open },
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
                return Ok(());
            }
            let author = actor(&project);
            let header = reviews::create(
                &mut project,
                NewReview {
                    title,
                    summary: String::new(),
                    tasks,
                    author,
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
            agent,
        } => {
            let mut project = opened(cwd, agent.as_deref())?;
            let store = reviews::store(&project)?;
            let found = store.get_review(&reference(&review))?;
            let last = found.rounds.last();
            let files: Vec<String> = last
                .map(|r| r.files.iter().map(|f| f.path.clone()).collect())
                .unwrap_or_default();
            let round = new_round(&project, change, cwd, last.map(|r| &r.kind), &files)?;
            if ctx.dry_run {
                println!("[dry-run] would add a round to {}", found.header.reference);
                return Ok(());
            }
            let number = reviews::add_round(&mut project, &found.header.reference, round)?;
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
                return Ok(());
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
                    .collect::<Result<_>>()?,
                task,
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
            for f in reviews::list_everywhere(root, &filter)? {
                if mine && f.header.author != me && !f.header.reviewers.contains(&me) {
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
            if ctx.json {
                print_json(&Value::Array(
                    found
                        .iter()
                        .map(|f| json!({ "review": as_json!(&f.header), "branch": f.branch }))
                        .collect(),
                ));
            } else if found.is_empty() {
                println!("No reviews.");
            } else {
                for f in &found {
                    let branch = f
                        .branch
                        .as_deref()
                        .map_or_else(String::new, |b| format!(" · {b}"));
                    println!("{}{branch}", summary_line(&f.header));
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
                return Ok(());
            }
            if ctx.json {
                print_json(&as_json!(&found));
                return Ok(());
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
                found.header.author.0,
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
            let mut store = reviews::store(&project)?;
            let found = store.get_review(&reference(&review))?;
            let anchor = Anchor {
                round: round.unwrap_or_else(|| found.rounds.last().map_or(1, |r| r.number)),
                path: file,
                side: if old { Side::Old } else { Side::New },
                lines: lines.as_deref().map(parse_lines).transpose()?,
                quote,
            };
            if ctx.dry_run {
                println!("[dry-run] would comment on {}", found.header.reference);
                return Ok(());
            }
            let id = Reviews::new(&mut store).comment(
                &found.header.reference,
                anchor,
                &text,
                &actor(&project),
            )?;
            if ctx.json {
                print_json(&json!({ "review": found.header.reference, "comment": id }));
            } else {
                println!("{}: {id}", found.header.reference);
            }
        }
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
                return Ok(());
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
                return Ok(());
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
                return Ok(());
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
            agent,
        } => {
            let verdict = match (approve, request_changes, comment) {
                (true, _, _) => Verdict::Approve,
                (_, true, _) => Verdict::RequestChanges,
                (_, _, true) => Verdict::Comment,
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
            if verdict == Verdict::Comment && text.is_none() {
                return Err(Error::Invalid(
                    "--comment needs its text: --body or --body-file".into(),
                ));
            }
            if ctx.dry_run {
                println!("[dry-run] would submit on {review}");
                return Ok(());
            }
            reviews::submit(&mut project, &reference(&review), verdict, text.as_deref())?;
            let header = reviews::store(&project)?
                .get_review(&reference(&review))?
                .header;
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
                return Ok(());
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
    Ok(())
}
