//! Reading the reviews: `list`, `show`, `comments`.

use clap::Subcommand;

use super::{
    CommentFilter, CommentsFile, Context, Error, FileStore, Path, Result, ReviewFilter, ReviewRef,
    ReviewStore, RoundKind, Value, comments_md, json, opened, print_json, reference, reviews,
    rounds, status_of, summary_line, vcs_of,
};

/// The commands that read the reviews (in `atm review`'s list, ATM-711).
#[derive(Debug, Subcommand)]
pub enum ReadCommand {
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
}

/// Run a command that reads the reviews, in `cwd`.
///
/// # Errors
/// What the review's rules or atm-lib refuse, and a project or review that can't be read.
#[allow(clippy::too_many_lines)] // reason: one arm per command
pub(super) fn run(command: ReadCommand, ctx: Context, cwd: &Path) -> Result<u8> {
    match command {
        ReadCommand::List {
            statuses,
            task,
            mine,
            search,
        } => {
            let project = opened(cwd, None)?;
            let me = project.review_actor();
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
        ReadCommand::Show {
            review,
            round,
            against,
        } => {
            let project = opened(cwd, None)?;
            let store = reviews::store(&project)?;
            let found = store.get_review(&reference(&review))?;
            if let Some(round) = round {
                // The same views as the page's (ATM-725): the whole change from the review's base, or since round M
                let since = against.map_or(rounds::Since::Base, rounds::Since::Round);
                let view = rounds::between(&vcs_of(&project)?, &store, &found, since, round)?;
                let patch = view.diff.patch;
                if ctx.json {
                    print_json(
                        &json!({ "review": found.header.reference, "round": round, "against": against, "diff": patch }),
                    );
                } else {
                    print!("{patch}");
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
        ReadCommand::Comments { review, open } => {
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
    }
    Ok(0)
}
