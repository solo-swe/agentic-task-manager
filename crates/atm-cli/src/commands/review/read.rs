//! Reading the reviews: `list`, `show`, `comments`.

use super::{
    CommentFilter, CommentsFile, Context, Error, FileStore, Path, Result, ReviewCommand,
    ReviewFilter, ReviewRef, ReviewStore, RoundKind, Value, actor, comments_md, json, opened,
    print_json, reference, reviews, rounds, status_of, summary_line, vcs_of,
};

/// Run one of this group's review commands in `cwd` (`super::run` gives it only these).
///
/// # Errors
/// What the review's rules or atm-lib refuse, and a project or review that can't be read.
#[allow(clippy::too_many_lines)] // reason: one arm per command
pub(super) fn run(command: ReviewCommand, ctx: Context, cwd: &Path) -> Result<u8> {
    match command {
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
        _ => unreachable!("review::run gives read::run only its own commands"),
    }
    Ok(0)
}
