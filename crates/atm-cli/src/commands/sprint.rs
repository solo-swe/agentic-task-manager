//! `atm sprint`: sprints (the Python atm's `cli/commands/sprint.py`): list, add, edit, start, close, delete, move,
//! backlog, and how they print.

use std::fmt::Write;
use std::path::Path;

use atm_boot::{agent_identity, open_project};
use atm_common::Result;
use atm_common::py::{py_str, py_str_opt, truthy};
use atm_lib::sprints::{
    Closed, Deleted, add_sprint, backlog_tasks, close_sprint, delete_sprint, edit_sprint,
    list_sprints, move_tasks, start_sprint,
};
use clap::Subcommand;
use serde_json::{Value, json};

use super::task::{list_lines, view_row};
use super::{Context, print_json};

const NONE_ACTIVE: &str = "No sprint is active now: start the next one (atm sprint start <name>), or plan one with /atm-plan-sprint.";

/// The sprint commands.
#[derive(Debug, Subcommand)]
pub enum SprintCommand {
    /// List the sprints with their task counts, and the backlog's size.
    List {
        /// List closed sprints too
        #[arg(long)]
        all: bool,
    },
    /// Add a sprint (planned).
    Add {
        /// The sprint's name (generated when left out)
        name: Option<String>,
        /// What the sprint is for
        #[arg(long)]
        goal: Option<String>,
        /// The release it delivers to
        #[arg(long)]
        release: Option<String>,
        /// The planned end, YYYY-MM-DD
        #[arg(long, value_name = "DATE")]
        due: Option<String>,
        /// Put it after this sprint (default: last)
        #[arg(long, value_name = "NAME")]
        after: Option<String>,
        /// Who you are (default: ATM_AGENT, else this session)
        #[arg(long)]
        agent: Option<String>,
    },
    /// Change a sprint's goal, release or due date.
    Edit {
        /// The sprint's name
        name: String,
        /// What the sprint is for (empty: remove it)
        #[arg(long)]
        goal: Option<String>,
        /// The release it delivers to (empty: remove it)
        #[arg(long)]
        release: Option<String>,
        /// The planned end, YYYY-MM-DD (empty: remove it)
        #[arg(long, value_name = "DATE")]
        due: Option<String>,
        /// Who you are (default: ATM_AGENT, else this session)
        #[arg(long)]
        agent: Option<String>,
    },
    /// Make a planned sprint the active one.
    Start {
        /// The sprint's name
        name: String,
        /// Who you are (default: ATM_AGENT, else this session)
        #[arg(long)]
        agent: Option<String>,
    },
    /// Close a sprint: record its report and move its unfinished tasks on.
    Close {
        /// The sprint's name
        name: String,
        /// Where unfinished tasks go: a sprint's name, new, or backlog
        #[arg(long, value_name = "WHERE")]
        carry_to: Option<String>,
        /// With --carry-to new: the new sprint's name (default: generated)
        #[arg(long = "name", value_name = "NAME")]
        new_name: Option<String>,
        /// Its batch tasks were reviewed with the user (the sprint review)
        #[arg(long)]
        reviewed: bool,
        /// Who you are (default: ATM_AGENT, else this session)
        #[arg(long)]
        agent: Option<String>,
    },
    /// Delete a sprint created by mistake.
    Delete {
        /// The sprint's name
        name: String,
        /// Where its tasks go: a sprint's name or backlog (default)
        #[arg(long, value_name = "WHERE")]
        move_to: Option<String>,
        /// Also a sprint tasks finished in or moved on from
        #[arg(long)]
        force: bool,
        /// Who you are (default: ATM_AGENT, else this session)
        #[arg(long)]
        agent: Option<String>,
    },
    /// Move tasks into a sprint, the next sprint, the active one, or the backlog.
    Move {
        /// Task IDs
        #[arg(required = true, value_name = "IDS")]
        ids: Vec<String>,
        /// A sprint's name, next, active, or backlog
        #[arg(long, value_name = "WHERE")]
        to: Option<String>,
        /// Who you are (default: ATM_AGENT, else this session)
        #[arg(long)]
        agent: Option<String>,
    },
    /// The tasks with no sprint (not finished ones, unless --all).
    Backlog {
        /// Also the finished tasks with no sprint
        #[arg(long)]
        all: bool,
    },
}

/// With `--dry-run`, say what would be done; whether it was a dry run.
fn dry(ctx: Context, what: &str) -> bool {
    if ctx.dry_run {
        println!("[dry-run] would {what}");
    }
    ctx.dry_run
}

fn plural(n: u64) -> &'static str {
    if n == 1 { "" } else { "s" }
}

fn describe_sprint(s: &Value) -> String {
    let mut details = vec![py_str_opt(s.get("state"))];
    for key in ["release", "due"] {
        if let Some(value) = s.get(key).filter(|v| truthy(v)) {
            details.push(format!("{key} {}", py_str(value)));
        }
    }
    let counts: Vec<String> = s["by_kind"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(kind, n)| format!("{kind} {}", py_str(n)))
        .collect();
    let tasks = s["tasks"].as_u64().unwrap_or(0);
    let mut line = format!(
        "{} · {}: {tasks} task{}",
        py_str_opt(s.get("id")),
        details.join(" · "),
        plural(tasks)
    );
    if !counts.is_empty() {
        let _ = write!(line, " ({})", counts.join(", "));
    }
    if let Some(n) = s.get("review_pending").filter(|v| truthy(v)) {
        let _ = write!(line, "; {} waiting for the sprint review", py_str(n));
    }
    if let Some(goal) = s.get("goal").filter(|v| truthy(v)) {
        let _ = write!(line, "\n  goal: {}", py_str(goal));
    }
    line
}

fn show_close(result: &Closed, ctx: Context) {
    if ctx.json {
        print_json(&json!({"sprint": result.sprint, "created": result.created}));
        return;
    }
    let report = &result.sprint["report"];
    let ids = |key: &str| -> Vec<String> {
        report[key]
            .as_array()
            .into_iter()
            .flatten()
            .map(py_str)
            .collect()
    };
    let (done, carried, added) = (ids("done"), ids("carried"), ids("added"));
    let mut line = format!(
        "Closed the sprint “{}”: {} done",
        py_str(&result.sprint["id"]),
        done.len()
    );
    if !carried.is_empty() {
        let new = if result.created.is_some() {
            " (new, planned)"
        } else {
            ""
        };
        let place = result
            .target
            .as_ref()
            .map_or_else(|| "the backlog".to_string(), |t| format!("“{t}”{new}"));
        let _ = write!(
            line,
            ", {} carried to {place} ({})",
            carried.len(),
            carried.join(", ")
        );
    }
    if !added.is_empty() {
        let _ = write!(line, ", {} added mid-sprint", added.len());
    }
    println!("{line}.");
    if result.was_active {
        println!("{NONE_ACTIVE}");
    }
}

fn show_delete(result: &Deleted, ctx: Context) {
    if ctx.json {
        print_json(&json!({
            "deleted": result.sprint["id"],
            "state": result.state,
            "moved": result.moved,
            "to": result.target.clone().unwrap_or_else(|| "backlog".into()),
        }));
        return;
    }
    let mut line = format!(
        "Deleted the sprint “{}” ({})",
        py_str(&result.sprint["id"]),
        result.state
    );
    if !result.moved.is_empty() {
        let place = result
            .target
            .as_ref()
            .map_or_else(|| "the backlog".to_string(), |t| format!("“{t}”"));
        let n = result.moved.len() as u64;
        let _ = write!(
            line,
            ": {n} task{} moved to {place} ({})",
            plural(n),
            result.moved.join(", ")
        );
    }
    println!("{line}.");
    if result.state == "active" {
        println!("{NONE_ACTIVE}");
    }
}

fn actor_project(agent: Option<&str>, cwd: &Path) -> Result<atm_lib::Project> {
    Ok(open_project(cwd)?.with_actor(Some(agent_identity(agent, cwd))))
}

/// Run a sprint command in `cwd`.
///
/// # Errors
/// What atm-lib refuses, and a task file that can't be read.
#[allow(clippy::too_many_lines)] // reason: one arm per command, as the Python atm's sprint_app has them
pub fn run(command: SprintCommand, ctx: Context, cwd: &Path) -> Result<()> {
    match command {
        SprintCommand::List { all } => {
            let listed = list_sprints(&open_project(cwd)?, all)?;
            if ctx.json {
                print_json(&json!({"sprints": listed.sprints, "backlog": listed.backlog}));
                return Ok(());
            }
            if listed.sprints.is_empty() {
                println!(
                    "{}",
                    if listed.has_sprints {
                        "No open sprints (--all lists the closed ones)."
                    } else {
                        "No sprints yet: atm sprint add creates one."
                    }
                );
            }
            for s in &listed.sprints {
                println!("{}", describe_sprint(s));
            }
            let n = listed.backlog as u64;
            println!("backlog: {n} task{} (atm sprint backlog)", plural(n));
        }
        SprintCommand::Add {
            name,
            goal,
            release,
            due,
            after,
            agent,
        } => {
            let shown = name
                .clone()
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| "(a generated name)".into());
            if dry(ctx, &format!("add the sprint {shown}")) {
                return Ok(());
            }
            let mut project = actor_project(agent.as_deref(), cwd)?;
            let sprint = add_sprint(
                &mut project,
                name.as_deref(),
                goal.as_deref(),
                release.as_deref(),
                due.as_deref(),
                after.as_deref(),
            )?;
            if ctx.json {
                print_json(&sprint);
            } else {
                println!("Added the sprint “{}” (planned).", py_str(&sprint["id"]));
            }
        }
        SprintCommand::Edit {
            name,
            goal,
            release,
            due,
            agent,
        } => {
            if dry(ctx, &format!("edit the sprint {name}")) {
                return Ok(());
            }
            let mut project = actor_project(agent.as_deref(), cwd)?;
            let (sprint, changed) = edit_sprint(
                &mut project,
                &name,
                goal.as_deref(),
                release.as_deref(),
                due.as_deref(),
            )?;
            if ctx.json {
                print_json(&sprint);
            } else if changed.is_empty() {
                println!("{}: nothing changed", py_str(&sprint["id"]));
            } else {
                println!("{}: changed {}", py_str(&sprint["id"]), changed.join(", "));
            }
        }
        SprintCommand::Start { name, agent } => {
            if dry(ctx, &format!("start the sprint {name}")) {
                return Ok(());
            }
            let sprint = start_sprint(&mut actor_project(agent.as_deref(), cwd)?, &name)?;
            if ctx.json {
                print_json(&sprint);
            } else {
                println!("The sprint “{}” is now active.", py_str(&sprint["id"]));
            }
        }
        SprintCommand::Close {
            name,
            carry_to,
            new_name,
            reviewed,
            agent,
        } => {
            if dry(ctx, &format!("close the sprint {name}")) {
                return Ok(());
            }
            let mut project = actor_project(agent.as_deref(), cwd)?;
            let closed = close_sprint(
                &mut project,
                &name,
                carry_to.as_deref(),
                new_name.as_deref(),
                reviewed,
            )?;
            show_close(&closed, ctx);
        }
        SprintCommand::Delete {
            name,
            move_to,
            force,
            agent,
        } => {
            if dry(ctx, &format!("delete the sprint {name}")) {
                return Ok(());
            }
            let mut project = actor_project(agent.as_deref(), cwd)?;
            show_delete(
                &delete_sprint(&mut project, &name, move_to.as_deref(), force)?,
                ctx,
            );
        }
        SprintCommand::Move { ids, to, agent } => {
            // A usage error, as Click words it (the Python atm's message)
            let Some(to) = to else {
                return Err(atm_common::Error::Invalid("Missing option '--to'.".into()));
            };
            if dry(ctx, &format!("move {} to {to}", ids.join(", "))) {
                return Ok(());
            }
            let moved = move_tasks(&mut actor_project(agent.as_deref(), cwd)?, &ids, &to)?;
            if ctx.json {
                print_json(&Value::Array(moved));
                return Ok(());
            }
            for m in &moved {
                let place = |v: &Value| {
                    if truthy(v) {
                        py_str(v)
                    } else {
                        "backlog".to_string()
                    }
                };
                let (before, after) = (place(&m["from"]), place(&m["to"]));
                if before == after {
                    println!("{}: already in {after}", py_str(&m["id"]));
                } else {
                    println!("{}: {before} -> {after}", py_str(&m["id"]));
                }
            }
        }
        SprintCommand::Backlog { all } => {
            let (tasks, vocab) = backlog_tasks(&open_project(cwd)?, all)?;
            let rows: Vec<Value> = tasks
                .iter()
                .map(|t| {
                    let kind = vocab.kind_of(t.get("status")).map(atm_common::Kind::as_str);
                    view_row(&json!({"task": t, "derived": {"kind": kind, "priority": vocab.priority_of(t)}}))
                })
                .collect();
            if ctx.json {
                print_json(&Value::Array(rows));
            } else if rows.is_empty() {
                println!("The backlog is empty.");
            } else {
                for line in list_lines(&rows) {
                    println!("{line}");
                }
            }
        }
    }
    Ok(())
}
