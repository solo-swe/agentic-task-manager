//! Deciding a review: `submit`, `close`.

use super::{
    Context, Error, Path, Result, ReviewCommand, ReviewStore, Value, opened, print_json, reference,
    reviews, summary_line, text_of,
};

/// Run one of this group's review commands in `cwd` (`super::run` gives it only these).
///
/// # Errors
/// What the review's rules or atm-lib refuse, and a project or review that can't be read.
pub(super) fn run(command: ReviewCommand, ctx: Context, cwd: &Path) -> Result<u8> {
    match command {
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
        _ => unreachable!("review::run gives decide::run only its own commands"),
    }
    Ok(0)
}
