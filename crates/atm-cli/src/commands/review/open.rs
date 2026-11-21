//! Opening a review and its rounds: `create`, `live`, `round`, `tasks`.

use clap::Subcommand;

use super::{
    Actor, Change, Context, NewReview, Path, Result, ReviewStore, Status, Value, actor, json,
    new_round, opened, print_json, reference, reviews, summary_line,
};

/// The commands that open a review and add to it (in `atm review`'s list, ATM-711).
#[derive(Debug, Subcommand)]
pub enum Command {
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
}

/// Run a command that opens a review or adds to it, in `cwd`.
///
/// # Errors
/// What the review's rules or atm-lib refuse, and a project or review that can't be read.
#[allow(clippy::too_many_lines)] // reason: one arm per command
pub(super) fn run(command: Command, ctx: Context, cwd: &Path) -> Result<u8> {
    match command {
        Command::Create {
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
        Command::Live {
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
        Command::Round {
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
        Command::Tasks {
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
    }
    Ok(0)
}
