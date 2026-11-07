//! `atm task`: a task's lifecycle. This slice has `task details`; the Python atm's `cli/commands/task.py` has the rest.

use std::path::Path;

use atm_boot::open_project;
use atm_common::py::{py_str, py_str_opt, splitlines, strip, truthy};
use atm_common::{Result, TaskId};
use atm_lib::tasks::{TaskView, show_tasks};
use clap::Subcommand;
use serde_json::Value;

use super::{Context, print_json};

/// The task commands.
#[derive(Debug, Subcommand)]
pub enum TaskCommand {
    /// Tasks in full (description, dependencies, files, notes).
    Details {
        /// Task IDs
        #[arg(required = true, value_name = "TASK_IDS")]
        task_ids: Vec<String>,
    },
}

/// Run a task command in `cwd`.
///
/// # Errors
/// What atm-lib refuses (an unknown task), or a task file that can't be read.
pub fn run(command: TaskCommand, ctx: Context, cwd: &Path) -> Result<()> {
    match command {
        TaskCommand::Details { task_ids } => details(&task_ids, ctx, cwd),
    }
}

fn details(task_ids: &[String], ctx: Context, cwd: &Path) -> Result<()> {
    let ids: Vec<TaskId> = task_ids.iter().map(TaskId::new).collect();
    let views = show_tasks(&open_project(cwd)?, &ids)?;
    if ctx.json {
        // An archived task says so (T-353)
        let tasks = views
            .into_iter()
            .map(|v| {
                let mut task = v.task;
                if v.archived {
                    task.insert("archived".into(), Value::Bool(true));
                }
                Value::Object(task)
            })
            .collect();
        print_json(&Value::Array(tasks));
    } else {
        println!(
            "{}",
            views
                .iter()
                .map(render_task)
                .collect::<Vec<_>>()
                .join("\n\n")
        );
    }
    Ok(())
}

/// `", ".join(value)`, as Python joins a list (or the characters of a text).
fn join(value: &Value) -> String {
    match value {
        Value::Array(items) => items.iter().map(py_str).collect::<Vec<_>>().join(", "),
        Value::String(text) => text
            .chars()
            .map(String::from)
            .collect::<Vec<_>>()
            .join(", "),
        other => py_str(other),
    }
}

/// A task in full, as text: what an agent needs to work on it (`atm task state … in_progress` prints the same).
#[must_use]
pub fn render_task(view: &TaskView) -> String {
    let task = &view.task;
    let get = |key: &str| task.get(key).filter(|v| truthy(v));
    let mut header = vec![
        py_str_opt(task.get("status")),
        get("sprint").map_or_else(|| "backlog".to_string(), py_str),
    ];
    // A task from the archive says so (found in T-277, T-353)
    if view.archived {
        header.push("archived".into());
    }
    header.push(py_str_opt(task.get("type")));
    header.push(view.priority.clone());
    if let Some(labels) = get("labels") {
        header.push(format!("labels: {}", join(labels)));
    }
    let mut lines = vec![
        format!(
            "{}  {}",
            py_str_opt(task.get("id")),
            task.get("name").map_or_else(String::new, py_str)
        ),
        format!("  {}", header.join(" · ")),
    ];
    if !view.dependencies.is_empty() {
        let deps: Vec<String> = view
            .dependencies
            .iter()
            .map(|d| match d.status.as_ref().filter(|s| !s.is_null()) {
                None => format!("{} (missing)", d.id),
                Some(status) => format!(
                    "{} ({}{})",
                    d.id,
                    py_str(status),
                    if d.archived { ", archived" } else { "" }
                ),
            })
            .collect();
        lines.push(format!("  depends on: {}", deps.join(", ")));
    }
    // How it ended, when it isn't the default, and its relations (SR-2.8)
    if let Some(resolution) = get("resolution") {
        let of = get("duplicate_of").map_or_else(String::new, |d| format!(" (of {})", py_str(d)));
        lines.push(format!("  resolution: {}{of}", py_str(resolution)));
    }
    if !view.related.is_empty() {
        lines.push(format!("  related: {}", view.related.join(", ")));
    }
    if !view.duplicated_by.is_empty() {
        lines.push(format!(
            "  duplicated by: {}",
            view.duplicated_by.join(", ")
        ));
    }
    for key in ["requirements", "use_cases", "files"] {
        if let Some(items) = get(key) {
            lines.push(format!("  {}: {}", key.replace('_', " "), join(items)));
        }
    }
    let started_by = view.started_by.as_ref().filter(|v| truthy(v));
    let who: Vec<String> = [
        ("created", "created_at", get("created_by")),
        ("started", "started_at", started_by),
        ("review requested", "review_requested_at", None),
        ("completed", "completed_at", get("completed_by")),
    ]
    .into_iter()
    .filter_map(|(what, at, by)| {
        get(at).map(|at| {
            format!("{what} {}", py_str(at))
                + &by.map_or_else(String::new, |b| format!(" by {}", py_str(b)))
        })
    })
    .collect();
    if !who.is_empty() {
        lines.push(format!("  {}", who.join(" · ")));
    }
    if let Some(commit) = get("commit") {
        lines.push(format!("  commit: {}", py_str(commit)));
    }
    let description = get("description").map_or_else(String::new, py_str);
    let description = strip(&description);
    if !description.is_empty() {
        lines.push("  description:".into());
        lines.extend(
            splitlines(description)
                .into_iter()
                .map(|line| format!("    {line}")),
        );
    }
    if let Some(notes) = get("notes") {
        lines.push("  notes:".into());
        for note in notes.as_array().into_iter().flatten() {
            let field = |key: &str| py_str_opt(note.get(key));
            let by = note
                .get("by")
                .filter(|b| truthy(b))
                .map_or_else(String::new, |b| format!(" ({})", py_str(b)));
            lines.push(format!(
                "    {} {}{by}: {}",
                field("at"),
                field("kind"),
                field("text")
            ));
        }
    } else if let Some(failure) = get("last_failure") {
        lines.push(format!(
            "  last failure: {}: {}",
            py_str_opt(failure.get("at")),
            py_str_opt(failure.get("reason"))
        ));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests;
