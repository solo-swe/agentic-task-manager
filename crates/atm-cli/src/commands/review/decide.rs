//! Deciding a review: `submit`, `close`.

use clap::Subcommand;

use super::{
    Body, Context, Error, Path, Result, ReviewStore, Value, opened, print_json, reference, reviews,
    summary_line, text_of,
};

/// The commands that decide a review (in `atm review`'s list, ATM-711).
#[derive(Debug, Subcommand)]
pub enum DecideCommand {
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

/// Run a command that decides a review, in `cwd`.
///
/// # Errors
/// What the review's rules or atm-lib refuse, and a project or review that can't be read.
pub(super) fn run(command: DecideCommand, ctx: Context, cwd: &Path) -> Result<u8> {
    match command {
        DecideCommand::Submit {
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
            // The review is found first, so a dry run refuses an unknown one as the real run does (audit C17)
            reviews::store(&project)?.get_review(&reference(&review))?;
            if ctx.dry(&format!("submit on {review}")) {
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
        DecideCommand::Close {
            review,
            reason,
            reject_tasks,
            agent,
        } => {
            let mut project = opened(cwd, agent.as_deref())?;
            reviews::store(&project)?.get_review(&reference(&review))?;
            if ctx.dry(&format!("close {review}")) {
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
