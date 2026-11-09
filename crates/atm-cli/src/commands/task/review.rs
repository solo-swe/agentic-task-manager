//! `atm task review approve|request-changes|reject` (the Python atm's `review_app`, T-438): a review's decisions. The
//! task stays in review, and each opens one way out of it, which `task state` takes: approved, `done` (or
//! `in_progress` for what is left); changes requested, `in_progress`; rejected, `todo` or `skipped`. The decision comes
//! first on the line, so a permission rule can tell them apart; no skill pre-approves them (I23).

use std::path::Path;

use atm_common::{Error, Result};
use atm_lib::Project;
use atm_lib::actor::agent_identity;
use atm_lib::changes::change_task_status;
use atm_lib::state::{Command, Options};
use clap::{Args, Subcommand};

use super::state::report;
use crate::commands::Context;

/// The options every decision takes.
#[derive(Debug, Args)]
pub struct Decider {
    /// Pick among same-kind statuses
    #[arg(long)]
    status: Option<String>,
    /// Who decides
    #[arg(long)]
    agent: Option<String>,
}

/// A review's decisions.
#[derive(Debug, Subcommand)]
pub enum ReviewCommand {
    /// Approve a task in review: then it can be done, or taken back for what is left.
    Approve {
        /// The task ID
        task_id: String,
        /// What is left to do, or a word for the agent
        #[arg(long)]
        note: Option<String>,
        #[command(flatten)]
        decider: Decider,
    },
    /// Ask for changes to a task in review: its agent takes it back first.
    RequestChanges {
        /// The task ID
        task_id: String,
        /// What needs to change (a rejection note) [required]
        #[arg(long)]
        reason: Option<String>,
        #[command(flatten)]
        decider: Decider,
    },
    /// Reject a task in review: then it goes back to to do (its work dropped) or is skipped.
    Reject {
        /// The task ID
        task_id: String,
        /// Why (a rejection note) [required]
        #[arg(long)]
        reason: Option<String>,
        #[command(flatten)]
        decider: Decider,
    },
}

/// Run a review's decision in `cwd`.
///
/// # Errors
/// A task that isn't in review (or has no such decision to take), and what atm-lib refuses.
pub fn run(command: ReviewCommand, ctx: Context, cwd: &Path) -> Result<()> {
    let filled = |v: Option<String>| v.filter(|t| !t.is_empty());
    // A usage error, as Click words it (the Python atm's message; the skills and tests read it)
    let required =
        |v: Option<String>| v.ok_or_else(|| Error::Invalid("Missing option '--reason'.".into()));
    let (command, task_id, decider, options) = match command {
        ReviewCommand::Approve {
            task_id,
            note,
            decider,
        } => {
            let options = Options {
                note: filled(note),
                ..Options::default()
            };
            (Command::Approve, task_id, decider, options)
        }
        ReviewCommand::RequestChanges {
            task_id,
            reason,
            decider,
        } => {
            let options = Options {
                reason: filled(Some(required(reason)?)),
                ..Options::default()
            };
            (Command::RequestChanges, task_id, decider, options)
        }
        ReviewCommand::Reject {
            task_id,
            reason,
            decider,
        } => {
            let options = Options {
                reason: filled(Some(required(reason)?)),
                ..Options::default()
            };
            (Command::Reject, task_id, decider, options)
        }
    };
    let actor = agent_identity(decider.agent.as_deref(), cwd);
    let mut project = Project::open(cwd)?.with_actor(Some(actor));
    if ctx.dry_run {
        println!("[dry-run] would {} {task_id}", command.name());
        return Ok(());
    }
    let options = Options {
        status: decider.status,
        ..options
    };
    let result = change_task_status(&mut project, command, &task_id, &options)?;
    report(command, &result, ctx);
    Ok(())
}
