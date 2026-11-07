//! `atm task state <id> <state>`: a task to another state (the Python atm's `task_state` and `run_status`). The state
//! names the target; the command that gets there is the one for the kind the task is in (back to to do differs by it).

use std::path::Path;

use atm_boot::{agent_identity, open_project};
use atm_common::{Error, Result};
use atm_lib::changes::{StatusChange, change_task_status, todo_command};
use atm_lib::state::{Command, Options};
use clap::Args;
use serde_json::{Value, json};

use super::render_task;
use crate::commands::{Context, print_json};

/// `atm task state`'s arguments and options.
#[derive(Debug, Args)]
pub struct StateArgs {
    /// The task ID
    task_id: String,
    /// Next state: todo, `in_progress`, `in_review`, done, blocked, skipped
    next_state: String,
    /// A note
    #[arg(long)]
    note: Option<String>,
    /// A reason
    #[arg(long)]
    reason: Option<String>,
    /// The task's commit SHA
    #[arg(long)]
    commit: Option<String>,
    /// skipped: why it isn't needed (`wont_do`, obsolete or duplicate; default `wont_do`)
    #[arg(long)]
    resolution: Option<String>,
    /// The task it duplicates
    #[arg(long)]
    duplicate_of: Option<String>,
    /// blocked: who has to decide
    #[arg(long, value_parser = ["user", "agent"])]
    on: Option<String>,
    /// An open question (repeatable)
    #[arg(long)]
    question: Vec<String>,
    /// An answer (repeatable)
    #[arg(long)]
    answer: Vec<String>,
    /// Pick among same-kind statuses
    #[arg(long)]
    status: Option<String>,
    /// Who runs it
    #[arg(long)]
    agent: Option<String>,
}

/// The options each command takes, by the name `atm task state` gives them.
fn takes(command: Command) -> &'static [&'static str] {
    match command {
        Command::Start | Command::Drop => &[],
        Command::Review | Command::Approve | Command::Release => &["note"],
        Command::Done => &["note", "commit"],
        Command::Reject | Command::RequestChanges | Command::Reopen | Command::Fail => &["reason"],
        Command::Block => &["on", "question", "reason"],
        Command::Unblock => &["answer"],
        Command::Skip => &["reason", "resolution", "duplicate_of"],
    }
}

/// The options given, by name, in the order the Python atm names them in its messages.
fn given(args: &StateArgs) -> [(&'static str, bool); 8] {
    let text = |o: &Option<String>| o.as_deref().is_some_and(|v| !v.is_empty());
    [
        ("note", text(&args.note)),
        ("reason", text(&args.reason)),
        ("commit", text(&args.commit)),
        ("resolution", text(&args.resolution)),
        ("duplicate_of", text(&args.duplicate_of)),
        ("on", text(&args.on)),
        ("question", !args.question.is_empty()),
        ("answer", !args.answer.is_empty()),
    ]
}

/// The command's options, only those it takes (as Python passes them to atm-lib).
fn options(command: Command, args: &StateArgs) -> Options {
    let has = |name: &str| takes(command).contains(&name);
    let pick = |name: &str, value: &Option<String>| {
        if has(name) {
            value.clone().filter(|v| !v.is_empty())
        } else {
            None
        }
    };
    Options {
        status: args.status.clone(),
        note: pick("note", &args.note),
        reason: pick("reason", &args.reason),
        commit: pick("commit", &args.commit),
        waiting_on: pick("on", &args.on),
        questions: if has("question") {
            args.question.clone()
        } else {
            Vec::new()
        },
        answers: if has("answer") {
            args.answer.clone()
        } else {
            Vec::new()
        },
        resolution: pick("resolution", &args.resolution),
        duplicate_of: pick("duplicate_of", &args.duplicate_of),
    }
}

/// Run `atm task state` in `cwd`.
///
/// # Errors
/// An unknown state, options the command doesn't take or misses, and what atm-lib refuses.
pub fn run(args: &StateArgs, ctx: Context, cwd: &Path) -> Result<()> {
    let state = args.next_state.as_str();
    let mut command = match state {
        "in_progress" => Command::Start,
        "in_review" => Command::Review,
        "done" => Command::Done,
        "todo" => Command::Fail, // chosen below by the kind the task is in now
        "blocked" => Command::Block,
        "skipped" => Command::Skip,
        other => {
            return Err(Error::Invalid(format!(
                "unknown state '{other}'. Use: blocked, done, in_progress, in_review, skipped, todo"
            )));
        }
    };
    let actor = agent_identity(args.agent.as_deref(), cwd);
    let mut project = open_project(cwd)?.with_actor(Some(actor));
    let task_id = args.task_id.as_str();
    if state == "todo" {
        // Back to to do is a different command for each kind the task is in (a failure says why: --reason)
        command = todo_command(&project, task_id, args.reason.is_some())?;
    }
    let extra: Vec<String> = given(args)
        .into_iter()
        .filter(|(name, set)| *set && !takes(command).contains(name))
        .map(|(name, _)| format!("--{}", name.replace('_', "-")))
        .collect();
    if !extra.is_empty() {
        return Err(Error::Invalid(format!(
            "{}: not for {state} ({}) from where {task_id} is",
            extra.join(", "),
            command.name()
        )));
    }
    let required = match command {
        Command::Reopen | Command::Skip => Some("reason"),
        Command::Block => Some("on"),
        _ => None,
    };
    if let Some(required) = required
        && !given(args)
            .iter()
            .any(|(name, set)| *name == required && *set)
    {
        return Err(Error::Invalid(format!(
            "{state} ({}) needs --{required}",
            command.name()
        )));
    }
    if ctx.dry_run {
        println!("[dry-run] would {} {task_id}", command.name());
        return Ok(());
    }
    let result = change_task_status(&mut project, command, task_id, &options(command, args))?;
    report(command, &result, ctx);
    Ok(())
}

/// On stderr, the claims a command found expired (their tasks went back to to do).
fn report_expired(expired: &[Value]) {
    for claim in expired {
        eprintln!(
            "atm: lease of {} by {} expired; it is available again",
            atm_common::py::py_str_opt(claim.get("task")),
            atm_common::py::py_str_opt(claim.get("agent"))
        );
    }
}

/// Print what a status change did, as the Python atm's `run_status` prints it.
pub fn report(command: Command, result: &StatusChange, ctx: Context) {
    report_expired(&result.expired);
    if ctx.json {
        let mut out = result.task.clone();
        out.insert("claim".into(), result.claim.clone());
        if command == Command::Done {
            out.insert("unblocked".into(), json!(result.unblocked));
        }
        if let Some(hint) = &result.hint {
            out.insert("suggest".into(), json!({"archive": hint}));
        }
        if let Some(stranded) = &result.stranded {
            out.insert("hint".into(), json!(stranded));
        }
        print_json(&Value::Object(out));
        return;
    }
    let text = |v: Option<&Value>| atm_common::py::py_str_opt(v);
    println!(
        "{}: {} -> {}",
        text(result.task.get("id")),
        atm_common::py::py_str(&result.previous),
        text(result.task.get("status"))
    );
    if let Some(stranded) = &result.stranded {
        println!("  {stranded}");
    }
    if command == Command::Start
        && let Some(view) = &result.view
    {
        println!("{}", render_task(view));
    }
    if !result.unblocked.is_empty() {
        println!("Now ready: {}", result.unblocked.join(", "));
    }
    if let Some(hint) = &result.hint {
        println!("{}", text(hint.get("text")));
    }
}
