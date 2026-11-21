//! Talking in a review: `comment`, `poke`, `reply`, `edit`, `resolve`, and the drafts.

use clap::Subcommand;

use super::{
    Body, CommentId, Context, Error, Path, Result, ReviewStore, Reviews, Side, Value, actor, json,
    opened, print_json, reference, reviews, text_of,
};

/// The commands that talk in a review: comments, replies, pokes and drafts (in `atm review`'s list, ATM-711).
#[derive(Debug, Subcommand)]
pub enum Command {
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

/// Run a command that talks in a review, in `cwd`.
///
/// # Errors
/// What the review's rules or atm-lib refuse, and a project or review that can't be read.
#[allow(clippy::too_many_lines)] // reason: one arm per command
pub(super) fn run(command: Command, ctx: Context, cwd: &Path) -> Result<u8> {
    match command {
        Command::Comment {
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
        Command::Poke {
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
        Command::Draft { command } => return run_draft(command, ctx, cwd),
        Command::Reply {
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
        Command::Edit {
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
        Command::Resolve {
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
    }
    Ok(0)
}

/// A draft, for people: its ID, where it goes, its text.
pub(super) fn draft_line(draft: &atm_lib::review_drafts::Draft) -> String {
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
pub(super) fn run_draft(command: DraftCommand, ctx: Context, cwd: &Path) -> Result<u8> {
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

pub(super) fn parse_lines(lines: &str) -> Result<(u32, u32)> {
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
