//! `atm task`: a task's lifecycle. The Rust atm has `task list` and `task details` so far; the Python atm's
//! `cli/commands/task.py` has the rest.

use std::path::Path;

use atm_boot::open_project;
use atm_common::py::{py_str, py_str_opt, splitlines, strip, truthy};
use atm_common::{Error, Result, TaskId};
use atm_lib::query::{Filters, task_dependencies, task_views};
use atm_lib::tasks::{TaskView, show_tasks};
use clap::{Args, Subcommand};
use serde_json::{Value, json};

use super::{Context, print_json};

/// `atm task list`'s options.
#[derive(Debug, Args)]
#[allow(clippy::struct_excessive_bools)] // reason: each is one of the command's flags, as the Python atm has them
pub struct ListArgs {
    /// Only these statuses (repeatable)
    #[arg(long)]
    status: Vec<String>,
    /// Only this sprint's tasks (repeatable)
    #[arg(long)]
    sprint: Vec<String>,
    /// Only tasks with no sprint
    #[arg(long)]
    backlog: bool,
    /// Only tasks in this release
    #[arg(long)]
    release: Option<String>,
    /// Only tasks in this component
    #[arg(long)]
    component: Option<String>,
    /// Only this task type
    #[arg(long = "type")]
    type_: Option<String>,
    /// Only this priority
    #[arg(long)]
    priority: Option<String>,
    /// Only tasks with this label (repeatable)
    #[arg(long)]
    label: Vec<String>,
    /// Only tasks started by this agent
    #[arg(long)]
    agent: Option<String>,
    /// Only to-do tasks whose dependencies are done
    #[arg(long)]
    ready: bool,
    /// Only blocked tasks and to-do tasks still waiting
    #[arg(long)]
    blocked: bool,
    /// Only tasks in a PR that isn't recorded as merged
    #[arg(long)]
    in_pr: bool,
    /// Only tasks done together with the user
    #[arg(long)]
    with_user: bool,
    /// List the archive instead
    #[arg(long)]
    archived: bool,
    /// Each task in full as JSON
    #[arg(long)]
    derived: bool,
    /// With --derived: read a malformed tasks.json as far as it can be read
    #[arg(long)]
    tolerant: bool,
}

impl ListArgs {
    fn filters(self) -> Filters {
        Filters {
            sprint: self.sprint,
            backlog: self.backlog,
            release: self.release,
            component: self.component,
            status: self.status,
            type_: self.type_,
            priority: self.priority,
            label: self.label,
            agent: self.agent,
            ready: self.ready,
            blocked: self.blocked,
            in_pr: self.in_pr,
            with_user: self.with_user,
            archived: self.archived,
        }
    }
}

/// The task commands.
#[derive(Debug, Subcommand)]
pub enum TaskCommand {
    /// List tasks: one line per task: ID, status, priority, labels, name.
    List(ListArgs),
    /// Tasks in full (description, dependencies, files, notes).
    Details {
        /// Task IDs
        #[arg(required = true, value_name = "TASK_IDS")]
        task_ids: Vec<String>,
    },
    /// What a task waits for, and what waits for it.
    Deps {
        /// The task ID
        task_id: String,
    },
}

/// Run a task command in `cwd`.
///
/// # Errors
/// What atm-lib refuses (an unknown task), or a task file that can't be read.
pub fn run(command: TaskCommand, ctx: Context, cwd: &Path) -> Result<()> {
    match command {
        TaskCommand::List(args) => list(args, ctx, cwd),
        TaskCommand::Details { task_ids } => details(&task_ids, ctx, cwd),
        TaskCommand::Deps { task_id } => deps(&task_id, ctx, cwd),
    }
}

fn deps(task_id: &str, ctx: Context, cwd: &Path) -> Result<()> {
    let deps = task_dependencies(&open_project(cwd)?, task_id)?;
    if ctx.json {
        print_json(&deps);
        return Ok(());
    }
    let line = |key: &str| {
        let items: Vec<String> = deps[key]
            .as_array()
            .into_iter()
            .flatten()
            .map(|i| format!("{} ({})", py_str(&i["id"]), py_str(&i["status"])))
            .collect();
        if items.is_empty() {
            "nothing".to_string()
        } else {
            items.join(", ")
        }
    };
    println!("{task_id} waits for: {}", line("waits_for"));
    println!("{task_id} is waited on by: {}", line("waited_on_by"));
    Ok(())
}

fn list(args: ListArgs, ctx: Context, cwd: &Path) -> Result<()> {
    if args.derived && args.tolerant {
        // The tolerant read comes with `atm serve`, whose REST API reads that way (T-449)
        return Err(Error::Failed {
            message: "--tolerant isn't in the Rust atm yet (T-449)".into(),
            code: 2,
        });
    }
    let derived = args.derived;
    let filters = args.filters();
    let project = open_project(cwd)?;
    let views = task_views(&project, &filters)?;
    if derived {
        // JSON either way: it is for tools (the dashboard's tests take atm's own values from it)
        print_json(&Value::Array(views));
        return Ok(());
    }
    let rows: Vec<Value> = views.iter().map(view_row).collect();
    if ctx.json {
        print_json(&Value::Array(rows));
    } else if rows.is_empty() {
        println!("No tasks match.");
    } else {
        for line in list_lines(&rows) {
            println!("{line}");
        }
    }
    Ok(())
}

/// The fields `atm task list` shows of a task, from its view (`{task, derived}`), as `--json` prints them.
#[must_use]
pub fn view_row(view: &Value) -> Value {
    let task = &view["task"];
    let derived = &view["derived"];
    let mut row = json!({
        "id": task.get("id").cloned().unwrap_or(Value::Null),
        "status": task.get("status").cloned().unwrap_or(Value::Null),
        "kind": derived.get("kind").cloned().unwrap_or(Value::Null),
        "sprint": task.get("sprint").cloned().unwrap_or(Value::Null),
        "priority": derived.get("priority").cloned().unwrap_or(Value::Null),
        "labels": task.get("labels").cloned().unwrap_or_else(|| json!([])),
        "name": task.get("name").cloned().unwrap_or_else(|| json!("")),
    });
    if let Some(pr) = task.get("pr").filter(|p| p.is_object()) {
        row["pr"] = pr.clone();
        row["merged_at"] = task.get("merged_at").cloned().unwrap_or(Value::Null);
    }
    row
}

/// One line per task, as `atm task list` prints them: ID, status, priority, labels, name, the IDs and statuses
/// padded to the longest.
#[must_use]
pub fn list_lines(rows: &[Value]) -> Vec<String> {
    let width = |key: &str| {
        rows.iter()
            .map(|r| py_str(&r[key]).chars().count())
            .max()
            .unwrap_or(0)
    };
    let (id_width, status_width) = (width("id"), width("status"));
    rows.iter()
        .map(|t| {
            let labels = t
                .get("labels")
                .filter(|l| truthy(l))
                .map_or_else(String::new, |l| format!("[{}] ", join(l)));
            format!(
                "{:<id_width$}  {:<status_width$}  {}  {labels}{}",
                py_str(&t["id"]),
                py_str(&t["status"]),
                py_str(&t["priority"]),
                py_str(&t["name"])
            )
        })
        .collect()
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
    let mut lines = heading(view);
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
    lines.extend(who(view));
    if let Some(commit) = get("commit") {
        lines.push(format!("  commit: {}", py_str(commit)));
    }
    lines.extend(description_and_notes(task));
    lines.join("\n")
}

/// The task's first two lines: its ID and name, then its status, sprint, type, priority and labels.
fn heading(view: &TaskView) -> Vec<String> {
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
    vec![
        format!(
            "{}  {}",
            py_str_opt(task.get("id")),
            task.get("name").map_or_else(String::new, py_str)
        ),
        format!("  {}", header.join(" · ")),
    ]
}

/// When it was created, started, sent for review and completed, and by whom: one line, or none.
fn who(view: &TaskView) -> Option<String> {
    let task = &view.task;
    let get = |key: &str| task.get(key).filter(|v| truthy(v));
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
    (!who.is_empty()).then(|| format!("  {}", who.join(" · ")))
}

/// The description, indented, then the notes (or, in files from before notes, the last failure).
fn description_and_notes(task: &atm_common::Task) -> Vec<String> {
    let get = |key: &str| task.get(key).filter(|v| truthy(v));
    let mut lines = Vec::new();
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
    lines
}

#[cfg(test)]
mod tests;
