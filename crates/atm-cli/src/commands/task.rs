//! `atm task`: a task's lifecycle. The Rust atm has `task list`, `details`, `deps`, `add`, `note` and `state` so far;
//! the Python atm's `cli/commands/task.py` has the rest.

use std::path::Path;

use atm_common::py::{py_str, py_str_opt, splitlines, strip, truthy};
use atm_common::{Result, TaskId};
use atm_lib::Project;
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
    /// Add a task with the next ID, or many from a JSON file.
    Add(add::AddArgs),
    /// Add a note to a task (at most 500 characters).
    Note(add::NoteArgs),
    /// Change a task's definition (not its status: use `atm task state`).
    Edit(edit::EditArgs),
    /// Move a task to another state: in_progress, in_review, done, todo, blocked or skipped. A review's decision is
    /// `atm task review`.
    State(state::StateArgs),
    /// Decide a task's review: approve it, ask for changes, or reject it; it stays in review
    #[command(subcommand)]
    Review(review::ReviewCommand),
}

/// Run a task command in `cwd`.
///
/// # Errors
/// What atm-lib refuses (an unknown task), or a task file that can't be read.
pub fn run(command: TaskCommand, ctx: Context, cwd: &Path) -> Result<()> {
    match command {
        TaskCommand::List(args) => list(args, ctx, cwd),
        TaskCommand::Details { task_ids } => details(&super::resolved(cwd, &task_ids)?, ctx, cwd),
        TaskCommand::Deps { task_id } => deps(&super::resolved_one(cwd, &task_id)?, ctx, cwd),
        TaskCommand::Add(args) => add::add(args, ctx, cwd),
        TaskCommand::Note(args) => add::note(&args, ctx, cwd),
        TaskCommand::Edit(args) => edit::edit(args, ctx, cwd),
        TaskCommand::State(args) => state::run(&args, ctx, cwd),
        TaskCommand::Review(command) => review::run(command, ctx, cwd),
    }
}

fn deps(task_id: &str, ctx: Context, cwd: &Path) -> Result<()> {
    let deps = task_dependencies(&Project::open(cwd)?, task_id)?;
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
    super::say(cwd, &format!("{task_id} waits for: {}", line("waits_for")));
    super::say(
        cwd,
        &format!("{task_id} is waited on by: {}", line("waited_on_by")),
    );
    Ok(())
}

fn list(args: ListArgs, ctx: Context, cwd: &Path) -> Result<()> {
    let (derived, tolerant) = (args.derived, args.tolerant);
    let filters = args.filters();
    if derived && tolerant {
        // The REST API's snapshot, as the dashboard reads a malformed task file (T-377), without its size
        let mut snapshot =
            atm_lib::archive::task_snapshot(&Project::open_tolerant(cwd)?, &filters)?;
        if let Some(snapshot) = snapshot.as_object_mut() {
            snapshot.shift_remove("bytes");
        }
        print_json(&snapshot);
        return Ok(());
    }
    let project = Project::open(cwd)?;
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
        let palette = crate::output::Palette::of_document(project.store().document()?);
        for line in list_lines(&rows, &palette) {
            crate::output::emit(&line);
        }
        crate::output::emit(&crate::output::paint(
            crate::output::DIM,
            &count_line(&rows),
        ));
    }
    Ok(())
}

/// `4 tasks: 1 in progress, 2 blocked, 1 done`: the list's size by status, in the order they first come.
#[must_use]
pub fn count_line(rows: &[Value]) -> String {
    let mut counts: Vec<(String, usize)> = Vec::new();
    for status in rows.iter().map(|r| py_str(&r["status"])) {
        match counts.iter_mut().find(|(s, _)| *s == status) {
            Some((_, n)) => *n += 1,
            None => counts.push((status, 1)),
        }
    }
    let each: Vec<String> = counts
        .iter()
        .map(|(s, n)| format!("{n} {}", s.replace('_', " ")))
        .collect();
    let n = rows.len();
    format!(
        "{n} task{}: {}",
        if n == 1 { "" } else { "s" },
        each.join(", ")
    )
}

/// The fields `atm task list` shows of a task, from its view (`{task, derived}`), as `--json` prints them.
#[must_use]
pub fn view_row(view: &Value) -> Value {
    let task = &view["task"];
    let derived = &view["derived"];
    let mut row = json!({
        "id": task.get("id").cloned().unwrap_or(Value::Null),
        "label": atm_common::keys::label_of(task),
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
    // With task keys (ADR-007 §1): the key, plain (ADR-009); before them, the ID is all there is
    if let (Some(key), Some(map)) = (task.get("key"), row.as_object_mut()) {
        map.insert("key".into(), key.clone());
    }
    if let Some(map) = row.as_object_mut()
        && !map.contains_key("key")
    {
        map.shift_remove("label");
    }
    row
}

/// One line per task, as `atm task list` prints them (docs/cli-output-design.md §4): ID, status, priority, name,
/// labels, each column padded to its widest; on a terminal the name is cut so the labels fit.
#[must_use]
pub fn list_lines(rows: &[Value], palette: &crate::output::Palette) -> Vec<String> {
    use crate::output::{Cell, DIM, Table};
    let mut table = Table::new().cut(3);
    for t in rows {
        let status = py_str(&t["status"]);
        let priority = py_str(&t["priority"]);
        table.row(vec![
            task_label_cell(t),
            Cell::styled(&status, palette.status(&status)),
            Cell::styled(&priority, palette.priority(&priority)),
            Cell::new(py_str(&t["name"])),
            Cell::styled(
                t.get("labels")
                    .filter(|l| truthy(l))
                    .map_or_else(String::new, join),
                DIM,
            ),
        ]);
    }
    table.lines()
}

/// A task's label (its key, `~` while provisional) where it has one, else its ID.
fn task_label_cell(row: &Value) -> crate::output::Cell {
    crate::output::Cell::new(row.get("label").map_or_else(|| py_str(&row["id"]), py_str))
}

fn details(task_ids: &[String], ctx: Context, cwd: &Path) -> Result<()> {
    let ids: Vec<TaskId> = task_ids.iter().map(TaskId::new).collect();
    let project = Project::open(cwd)?;
    let views = show_tasks(&project, &ids)?;
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
        let palette = crate::output::Palette::of_document(project.store().document()?);
        crate::output::emit(
            &views
                .iter()
                .map(|v| render_task(v, &palette))
                .collect::<Vec<_>>()
                .join("\n\n"),
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

/// A task in full, as text (docs/cli-output-design.md §4): what an agent needs to work on it (`atm task state …
/// in_progress` prints the same). Its heading, its facts in a labelled column (lists one item per line), then its
/// description and notes as sections.
#[must_use]
pub fn render_task(view: &TaskView, palette: &crate::output::Palette) -> String {
    use crate::output::{Cell, DIM, Table};
    let task = &view.task;
    let get = |key: &str| task.get(key).filter(|v| truthy(v));
    let mut lines = heading(view, palette);
    let mut fields: Vec<(&str, Vec<String>)> = Vec::new();
    if let Some(labels) = get("labels") {
        fields.push(("labels", vec![join(labels)]));
    }
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
        fields.push(("depends on", deps));
    }
    // How it ended, when it isn't the default, and its relations (SR-2.8)
    if let Some(resolution) = get("resolution") {
        let of = get("duplicate_of").map_or_else(String::new, |d| format!(" (of {})", py_str(d)));
        fields.push(("resolution", vec![format!("{}{of}", py_str(resolution))]));
    }
    if !view.related.is_empty() {
        fields.push(("related", view.related.clone()));
    }
    if !view.duplicated_by.is_empty() {
        fields.push(("duplicated by", view.duplicated_by.clone()));
    }
    for (key, title) in [
        ("requirements", "requirements"),
        ("use_cases", "use cases"),
        ("files", "files"),
    ] {
        if let Some(items) = get(key) {
            fields.push((title, entries(items)));
        }
    }
    fields.extend(who(view));
    if let Some(commit) = get("commit") {
        fields.push(("commit", vec![py_str(commit)]));
    }
    if !fields.is_empty() {
        let mut table = Table::new().indent(2);
        for (title, values) in fields {
            for (n, value) in values.into_iter().enumerate() {
                let title = if n == 0 { title } else { "" };
                table.row(vec![Cell::styled(title, DIM), Cell::new(value)]);
            }
        }
        lines.push(String::new());
        lines.extend(table.lines());
    }
    lines.extend(description_and_notes(task));
    lines.join("\n")
}

/// A list's entries as text, one each (a text: itself).
fn entries(value: &Value) -> Vec<String> {
    match value {
        Value::Array(items) => items.iter().map(py_str).collect(),
        other => vec![py_str(other)],
    }
}

/// The task's first two lines: its ID and name, bold, then its status, sprint, type and priority.
fn heading(view: &TaskView, palette: &crate::output::Palette) -> Vec<String> {
    use crate::output::{HEADING, paint};
    let task = &view.task;
    let get = |key: &str| task.get(key).filter(|v| truthy(v));
    let status = py_str_opt(task.get("status"));
    let mut header = vec![
        paint(palette.status(&status), &status),
        get("sprint").map_or_else(|| "backlog".to_string(), py_str),
    ];
    // A task from the archive says so (found in T-277, T-353)
    if view.archived {
        header.push("archived".into());
    }
    header.push(py_str_opt(task.get("type")));
    header.push(paint(palette.priority(&view.priority), &view.priority));
    vec![
        paint(
            HEADING,
            &format!(
                "{}  {}",
                // Its key (`~` while provisional) with task keys, else its ID (ADR-007 §1)
                atm_common::keys::label_of(&Value::Object(task.clone())),
                task.get("name").map_or_else(String::new, py_str)
            ),
        ),
        format!("  {}", header.join(" · ")),
    ]
}

/// When it was created, started, sent for review and completed, and by whom: a field each.
fn who(view: &TaskView) -> Vec<(&'static str, Vec<String>)> {
    let task = &view.task;
    let get = |key: &str| task.get(key).filter(|v| truthy(v));
    let started_by = view.started_by.as_ref().filter(|v| truthy(v));
    [
        ("created", "created_at", get("created_by")),
        ("started", "started_at", started_by),
        ("review requested", "review_requested_at", None),
        ("completed", "completed_at", get("completed_by")),
    ]
    .into_iter()
    .filter_map(|(what, at, by)| {
        get(at).map(|at| {
            (
                what,
                vec![py_str(at) + &by.map_or_else(String::new, |b| format!(" by {}", py_str(b)))],
            )
        })
    })
    .collect()
}

/// The description, indented (wrapped on a terminal), then the notes (or, in files from before notes, the last
/// failure).
fn description_and_notes(task: &atm_common::Task) -> Vec<String> {
    use crate::output::{DIM, HEADING, paint, width, wrap};
    let get = |key: &str| task.get(key).filter(|v| truthy(v));
    let mut lines = Vec::new();
    let description = get("description").map_or_else(String::new, py_str);
    let description = strip(&description);
    if !description.is_empty() {
        lines.extend([String::new(), paint(HEADING, "  description")]);
        for line in splitlines(description) {
            lines.extend(wrap(&format!("    {line}"), width()));
        }
    }
    if let Some(notes) = get("notes") {
        lines.extend([String::new(), paint(HEADING, "  notes")]);
        for note in notes.as_array().into_iter().flatten() {
            let field = |key: &str| py_str_opt(note.get(key));
            let by = note
                .get("by")
                .filter(|b| truthy(b))
                .map_or_else(String::new, |b| format!("  {}", py_str(b)));
            lines.push(format!(
                "    {}  {}{}",
                paint(DIM, &field("at")),
                field("kind"),
                paint(DIM, &by)
            ));
            for line in splitlines(&field("text")) {
                lines.extend(wrap(&format!("      {line}"), width()));
            }
        }
    } else if let Some(failure) = get("last_failure") {
        lines.push(String::new());
        lines.push(format!(
            "  last failure: {}: {}",
            py_str_opt(failure.get("at")),
            py_str_opt(failure.get("reason"))
        ));
    }
    lines
}

mod add;
mod edit;
mod review;
mod state;

#[cfg(test)]
mod tests;
