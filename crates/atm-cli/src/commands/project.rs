//! `atm project`: the project (status here; config, workflow and logo in `project/settings.rs`), as the Python atm's
//! `cli/commands/project.py` has them. The lists of projects are `atm workspace`'s (ADR-003 §3).

use std::path::Path;

use atm_common::Result;
use atm_common::py::{py_str, truthy};
use atm_lib::Project;
use atm_lib::query::Filters;
use atm_lib::status::{StatusReport, status_report};
use clap::Subcommand;
use serde_json::Value;

use super::{Context, print_json};

/// The project commands.
#[derive(Debug, Subcommand)]
pub enum ProjectCommand {
    /// The overview: counts, what is in progress, in review or blocked, the next task, the file's size.
    Status {
        /// Only this sprint's tasks (repeatable)
        #[arg(long)]
        sprint: Vec<String>,
        /// Only the tasks with no sprint
        #[arg(long)]
        backlog: bool,
        /// Only the tasks in this release
        #[arg(long)]
        release: Option<String>,
        /// Only the tasks in this component
        #[arg(long)]
        component: Option<String>,
    },
    /// Print where this project's task files are.
    Config,
    /// Print the project's workflow, or set it (atm project workflow set TYPE).
    Workflow(settings::WorkflowArgs),
    /// Print, set or unset the project's logo for the dashboard (meta.logo).
    Logo {
        /// set or unset it
        action: Option<String>,
        /// set: the image (SVG, PNG, WebP or ICO), relative to here
        path: Option<String>,
    },
    /// The project's summary, as the dashboard's overview gets it (JSON).
    #[command(hide = true)]
    Summary,
}

/// Run a project command in `cwd`.
///
/// # Errors
/// An unknown sprint or release in the scope; task data that can't be read.
pub fn run(command: ProjectCommand, ctx: Context, cwd: &Path) -> Result<()> {
    match command {
        ProjectCommand::Status {
            sprint,
            backlog,
            release,
            component,
        } => {
            let filters = Filters {
                sprint,
                backlog,
                release,
                component,
                ..Filters::default()
            };
            let status = status_report(&Project::open(cwd)?, &filters)?;
            if ctx.json {
                print_json(&status.report);
            } else {
                for line in status_lines(&status) {
                    println!("{line}");
                }
            }
            Ok(())
        }
        ProjectCommand::Config => settings::config(ctx, cwd),
        ProjectCommand::Workflow(args) => settings::workflow(args, ctx, cwd),
        ProjectCommand::Logo { action, path } => {
            settings::logo(action.as_deref(), path.as_deref(), ctx, cwd)
        }
        ProjectCommand::Summary => {
            print_json(&atm_lib::summary::project_summary(&Project::open(cwd)?)?);
            Ok(())
        }
    }
}

mod settings;

fn text(value: &Value) -> String {
    py_str(value)
}

/// `key value, key value` of an object's counts.
fn pairs(counts: &Value) -> String {
    counts
        .as_object()
        .map(|m| {
            m.iter()
                .map(|(k, v)| format!("{k} {}", text(v)))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default()
}

fn list(value: &Value) -> Vec<Value> {
    value.as_array().cloned().unwrap_or_default()
}

/// ` [in another worktree, <branch>]` for an item waiting in another worktree.
fn elsewhere(item: &Value) -> String {
    match item.get("branch") {
        None => String::new(),
        Some(branch) => format!(
            " [in another worktree, {}]",
            if truthy(branch) {
                text(branch)
            } else {
                "detached".into()
            }
        ),
    }
}

/// A section of the report as text: its title, its key in the report, and what follows each item's ID and name.
type Section = (&'static str, &'static str, fn(&Value) -> String);

/// The report as text, line by line, as the Python atm's `show_status` prints it: short and stable, for agents.
#[must_use]
#[allow(clippy::too_many_lines)] // reason: one block per line of the text, as the Python atm's show_status prints them
pub fn status_lines(status: &StatusReport) -> Vec<String> {
    let r = &status.report;
    let mut lines = Vec::new();
    let sprint = &r["sprint"];
    if sprint.is_object() {
        let mut details = vec![format!(
            "{} of {} done",
            text(&sprint["done"]),
            text(&sprint["tasks"])
        )];
        for key in ["due", "release"] {
            if let Some(v) = sprint.get(key).filter(|v| truthy(v)) {
                details.push(format!("{key} {}", text(v)));
            }
        }
        lines.push(format!(
            "sprint: {} (active): {}",
            text(&sprint["id"]),
            details.join(" · ")
        ));
        if let Some(goal) = sprint.get("goal").filter(|g| truthy(g)) {
            lines.push(format!("  goal: {}", text(goal)));
        }
        let pending = list(&sprint["review_pending"]);
        if !pending.is_empty() {
            let ids: Vec<String> = pending.iter().map(text).collect();
            lines.push(format!(
                "  waiting for the sprint review: {} (/atm-close-sprint)",
                ids.join(", ")
            ));
        }
    } else if status.has_sprints {
        lines.push("sprint: none active (plan one with /atm-plan-sprint)".into());
    }
    let stranded = r
        .get("open_in_closed_sprints")
        .map(list)
        .unwrap_or_default();
    if !stranded.is_empty() {
        let n = stranded.len();
        let each: Vec<String> = stranded
            .iter()
            .map(|t| format!("{} ({})", text(&t["id"]), text(&t["sprint"])))
            .collect();
        lines.push(format!(
            "warning: {n} open task{} in closed sprints, which the loop won't take: {} (atm sprint move <id> --to backlog or --to active)",
            if n == 1 { "" } else { "s" },
            each.join(", ")
        ));
    }
    lines.push(format!(
        "{} tasks: {}",
        text(&r["tasks"]),
        pairs(&r["by_status"])
    ));
    if status.has_sprints {
        lines.push(format!("by sprint: {}", pairs(&r["by_sprint"])));
    }
    lines.push(format!("by type: {}", pairs(&r["by_type"])));
    if r["by_label"].as_object().is_some_and(|m| !m.is_empty()) {
        lines.push(format!("by label: {}", pairs(&r["by_label"])));
    }
    let sections: [Section; 4] = [
        ("in progress", "in_progress", |i| {
            if truthy(&i["by"]) {
                format!(" ({})", text(&i["by"]))
            } else {
                String::new()
            }
        }),
        ("in review", "in_review", |i| {
            let since = if truthy(&i["since"]) {
                format!(" (since {})", text(&i["since"]))
            } else {
                String::new()
            };
            since + &elsewhere(i)
        }),
        ("blocked", "blocked", |i| {
            let on = if truthy(&i["waiting_on"]) {
                format!(" (on {})", text(&i["waiting_on"]))
            } else {
                String::new()
            };
            on + &elsewhere(i)
        }),
        ("waiting on dependencies", "waiting_on_dependencies", |i| {
            format!(
                " (on {})",
                list(&i["on"])
                    .iter()
                    .map(text)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }),
    ];
    for (title, key, extra) in sections {
        let items = list(&r[key]);
        if !items.is_empty() {
            let each: Vec<String> = items
                .iter()
                .map(|i| format!("{} {}{}", text(&i["id"]), text(&i["name"]), extra(i)))
                .collect();
            lines.push(format!("{title}: {}", each.join("; ")));
        }
    }
    let next = &r["next"];
    lines.push(if next.is_object() {
        format!("next: {} {}", text(&next["id"]), text(&next["name"]))
    } else {
        format!("next: none ({})", text(&r["next_reason"]))
    });
    let largest: Vec<String> = list(&r["largest_tasks"])
        .iter()
        .map(|i| {
            let kb = i["bytes"].as_u64().unwrap_or(0) / 1024;
            format!("{} {} KB", text(&i["id"]), if kb == 0 { 1 } else { kb })
        })
        .collect();
    let file_kb = r["file_bytes"].as_u64().unwrap_or(0) / 1024;
    let tail = if largest.is_empty() {
        String::new()
    } else {
        format!("; largest tasks: {}", largest.join(", "))
    };
    lines.push(format!("file: {file_kb} KB{tail}"));
    if truthy(&r["archived"]) {
        lines.push(format!(
            "archive: {} tasks, {} KB",
            text(&r["archived"]),
            r["archive_bytes"].as_u64().unwrap_or(0) / 1024
        ));
    }
    if let Some(hint) = r
        .get("suggest")
        .and_then(|s| s.get("archive"))
        .filter(|h| truthy(h))
    {
        lines.push(text(&hint["text"]));
    }
    lines
}
