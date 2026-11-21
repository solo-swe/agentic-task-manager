//! Talking in a review: `comment`, `poke`, `reply`, `edit`, `resolve`, and the drafts.

use super::{
    CommentId, Context, DraftCommand, Error, Path, Result, ReviewCommand, ReviewStore, Reviews,
    Side, Value, actor, json, opened, print_json, reference, reviews, text_of,
};

/// Run one of this group's review commands in `cwd` (`super::run` gives it only these).
///
/// # Errors
/// What the review's rules or atm-lib refuse, and a project or review that can't be read.
#[allow(clippy::too_many_lines)] // reason: one arm per command
pub(super) fn run(command: ReviewCommand, ctx: Context, cwd: &Path) -> Result<u8> {
    match command {
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
        _ => unreachable!("review::run gives talk::run only its own commands"),
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
