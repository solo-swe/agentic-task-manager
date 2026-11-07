//! `atm loop`: the work loop, the next task to work on (the Python atm's `cli/commands/loop.py`).

use std::path::Path;

use atm_boot::{agent_identity, open_project};
use atm_common::py::{py_str, py_str_opt, truthy};
use atm_common::{Error, Result};
use atm_lib::changes::next_task_for;
use atm_lib::loop_rules::Selection;
use clap::{Args, Subcommand};
use serde_json::json;

use super::{Context, print_json};

/// `atm loop next`'s options.
#[derive(Debug, Args)]
#[allow(clippy::struct_excessive_bools)] // reason: each is one of the command's flags, as the Python atm has them
pub struct NextArgs {
    /// This sprint's tasks (repeatable; default: the active one)
    #[arg(long)]
    sprint: Vec<String>,
    /// These tasks, in this order
    #[arg(long, value_name = "ID,ID")]
    tasks: Option<String>,
    /// Every task: the active sprint, the planned ones, the backlog
    #[arg(long)]
    all: bool,
    /// The tasks with no sprint
    #[arg(long)]
    backlog: bool,
    /// Only this task type
    #[arg(long = "type")]
    type_: Option<String>,
    /// Also tasks done together with the user (the user is watching)
    #[arg(long)]
    with_user: bool,
    /// Also start and claim it, atomically
    #[arg(long)]
    claim: bool,
    /// Pick among same-kind statuses
    #[arg(long)]
    status: Option<String>,
    /// Who you are
    #[arg(long)]
    agent: Option<String>,
}

/// The loop's commands.
#[derive(Debug, Subcommand)]
pub enum LoopCommand {
    /// The task /atm:loop-tasks works on next (exit code 3 if there is none).
    Next(NextArgs),
}

/// Run a loop command in `cwd`; its exit code (3 when no task is available).
///
/// # Errors
/// Two selections at once, and what atm-lib refuses.
pub fn run(command: LoopCommand, ctx: Context, cwd: &Path) -> Result<u8> {
    match command {
        LoopCommand::Next(args) => next(args, ctx, cwd),
    }
}

fn next(args: NextArgs, ctx: Context, cwd: &Path) -> Result<u8> {
    let given: Vec<&str> = [
        ("--sprint", !args.sprint.is_empty()),
        (
            "--tasks",
            args.tasks.as_deref().is_some_and(|t| !t.is_empty()),
        ),
        ("--all", args.all),
        ("--backlog", args.backlog),
    ]
    .into_iter()
    .filter(|(_, set)| *set)
    .map(|(flag, _)| flag)
    .collect();
    if given.len() > 1 {
        return Err(Error::Invalid(format!(
            "{} can't both be given: they choose the tasks in different ways",
            given.join(" and ")
        )));
    }
    if ctx.dry_run {
        println!(
            "[dry-run] would {} the next task",
            if args.claim { "claim" } else { "show" }
        );
        return Ok(0);
    }
    let sel = Selection {
        sprints: args.sprint,
        tasks: args
            .tasks
            .as_deref()
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|i| !i.is_empty())
            .map(String::from)
            .collect(),
        all: args.all,
        backlog: args.backlog,
        task_type: args.type_,
    };
    let actor = agent_identity(args.agent.as_deref(), cwd);
    let mut project = open_project(cwd)?.with_actor(Some(actor));
    let result = next_task_for(
        &mut project,
        &sel,
        args.with_user,
        args.claim,
        args.status.as_deref(),
    )?;
    for claim in &result.expired {
        eprintln!(
            "atm: lease of {} by {} expired; it is available again",
            py_str_opt(claim.get("task")),
            py_str_opt(claim.get("agent"))
        );
    }
    if ctx.json {
        print_json(&json!({"task": result.task, "claim": result.claim, "reason": result.reason}));
    } else if let Some(task) = &result.task {
        let verb = if result.claim.is_some() {
            "Claimed"
        } else {
            "Next"
        };
        let where_ = task
            .get("sprint")
            .filter(|s| truthy(s))
            .map_or_else(|| "backlog".to_string(), py_str);
        println!(
            "{verb}: {} {} ({where_}, {})",
            py_str_opt(task.get("id")),
            task.get("name").map_or_else(String::new, py_str),
            py_str_opt(task.get("type"))
        );
    } else {
        println!(
            "No task available: {}.",
            result.reason.as_deref().unwrap_or("None")
        );
    }
    Ok(if result.task.is_some() { 0 } else { 3 })
}
