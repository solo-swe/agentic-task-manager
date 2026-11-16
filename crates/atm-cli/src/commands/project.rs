//! `atm project`: the project (status here; config, workflow and logo in `project/settings.rs`), as the Python atm's
//! `cli/commands/project.py` has them. The lists of projects are `atm workspace`'s (ADR-003 §3).

use std::fmt::Write;
use std::path::Path;

use atm_common::Result;
use atm_common::py::{py_str, py_str_opt, truthy};
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
        /// Also the counts by sprint, type and label
        #[arg(long)]
        all: bool,
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
    /// The project's vocabularies: its statuses, types and priorities.
    #[command(subcommand)]
    Vocab(VocabCommand),
    /// The project's key, its tasks' prefix (ATM in ATM-527): print it, or set it.
    #[command(subcommand)]
    Key(KeyCommand),
}

/// The project key commands (ADR-007, T-540).
#[derive(Debug, Subcommand)]
pub enum KeyCommand {
    /// Print the project's key and the earlier ones that still find tasks.
    Show,
    /// Set the project's key (3 to 6 letters and digits): numbers stay, and the old key still finds each task.
    Set {
        /// The new key
        key: String,
    },
}

fn key(command: KeyCommand, ctx: Context, cwd: &Path) -> Result<()> {
    match command {
        KeyCommand::Show => {
            let project = Project::open(cwd)?;
            let document = project.store().document()?;
            let key = atm_common::keys::project_key(document).map(String::from);
            let history = document["meta"]
                .get("key_history")
                .cloned()
                .unwrap_or_else(|| serde_json::json!([]));
            if ctx.json {
                print_json(&serde_json::json!({
                    "key": key,
                    "history": history,
                    "format": atm_common::ids::file_format(document),
                }));
            } else {
                match key {
                    Some(k) => println!("key: {k}"),
                    None => println!("key: none yet (set one with atm project key set <KEY>)"),
                }
                let earlier: Vec<String> = history
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(py_str)
                    .collect();
                if !earlier.is_empty() {
                    println!("earlier, still finding tasks: {}", earlier.join(", "));
                }
            }
            Ok(())
        }
        KeyCommand::Set { key } => {
            if ctx.dry_run {
                println!("[dry-run] would set the project's key to {key}");
                return Ok(());
            }
            let mut project = Project::open(cwd)?;
            let root = project
                .store()
                .working_copy()
                .map_or_else(|| cwd.to_path_buf(), Path::to_path_buf);
            // A project of IDs (T-527) gets task keys now, with this key: the first migration (ADR-007 §6)
            let legacy = !atm_common::keys::keys_format(project.store().document()?);
            let current =
                atm_common::keys::project_key(project.store().document()?).map(String::from);
            let set = if legacy && current.as_deref() == Some(key.trim()) {
                atm_lib::settings::KeySet {
                    from: None,
                    to: key.trim().to_string(),
                    history: Vec::new(),
                }
            } else {
                atm_lib::settings::set_project_key(&mut project, &key)?
            };
            let migrated = if legacy {
                super::db::maintenance(cwd)?.migrate_to_keys()?
            } else {
                None
            };
            let others = atm_lib::workspaces::Workspaces::open().key_users(&set.to, &root);
            if ctx.json {
                print_json(&serde_json::json!({
                    "from": set.from,
                    "to": set.to,
                    "history": set.history,
                    "migrated": migrated.as_ref().map(|(json, _)| json.clone()),
                    "also_used_by": others
                        .iter()
                        .map(|(w, p)| serde_json::json!({"workspace": w, "project": p}))
                        .collect::<Vec<_>>(),
                }));
            } else {
                if let Some((_, lines)) = &migrated {
                    for line in lines {
                        println!("{line}");
                    }
                } else {
                    match &set.from {
                        Some(from) => println!(
                            "The project's key is {} now (was {from}; {from}-<number> still finds each task).",
                            set.to
                        ),
                        None => println!("The project's key is {} now.", set.to),
                    }
                }
                for (workspace, project) in &others {
                    eprintln!(
                        "warning: {project} (workspace {workspace}) uses the key {} too: its keys and this project's \
                         would read the same across the workspace; give one another key",
                        set.to
                    );
                }
            }
            Ok(())
        }
    }
}

/// The vocabulary commands (T-500).
#[derive(Debug, Subcommand)]
pub enum VocabCommand {
    /// Rename a status, a type or a priority: the tasks using it, archived ones too, follow.
    Rename {
        /// Which list: status, type or priority
        #[arg(value_parser = ["status", "type", "priority"])]
        list: String,
        /// The entry's ID
        id: String,
        /// Its new ID (leave out to change only its label)
        new: Option<String>,
        /// Its new label, as the dashboard shows it
        #[arg(long)]
        label: Option<String>,
    },
}

fn vocab(command: VocabCommand, ctx: Context, cwd: &Path) -> Result<()> {
    use atm_lib::settings::{VocabList, rename_term};
    let VocabCommand::Rename {
        list,
        id,
        new,
        label,
    } = command;
    let which = match list.as_str() {
        "status" => VocabList::Status,
        "type" => VocabList::Type,
        _ => VocabList::Priority,
    };
    if ctx.dry_run {
        println!(
            "[dry-run] would rename the {list} {id}{}{}",
            new.as_deref()
                .map(|n| format!(" to {n}"))
                .unwrap_or_default(),
            label
                .as_deref()
                .map(|l| format!(" (label {l})"))
                .unwrap_or_default()
        );
        return Ok(());
    }
    let renamed = rename_term(
        &mut Project::open(cwd)?,
        which,
        &id,
        new.as_deref(),
        label.as_deref(),
    )?;
    if ctx.json {
        print_json(&serde_json::json!({
            "list": list,
            "from": renamed.from,
            "to": renamed.to,
            "label": renamed.label,
            "tasks": renamed.tasks,
            "archived": renamed.archived,
        }));
        return Ok(());
    }
    let n = renamed.tasks + renamed.archived;
    let archived = if renamed.archived > 0 {
        format!(" ({} archived)", renamed.archived)
    } else {
        String::new()
    };
    let what = if renamed.from == renamed.to {
        format!(
            "Relabelled the {list} {} “{}”",
            renamed.from,
            py_str(&renamed.label)
        )
    } else {
        format!(
            "Renamed the {list} {} to {} (“{}”)",
            renamed.from,
            renamed.to,
            py_str(&renamed.label)
        )
    };
    println!(
        "{what}: {n} task{}{archived} use it.",
        if n == 1 { "" } else { "s" }
    );
    Ok(())
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
            all,
        } => {
            let filters = Filters {
                sprint,
                backlog,
                release,
                component,
                ..Filters::default()
            };
            let project = Project::open(cwd)?;
            let mut status = status_report(&project, &filters)?;
            // Another registered project with this project's key (ADR-007 §8)
            let shared = atm_lib::check::shared_key_warning(&project, project.store().document()?);
            if let (Some(warning), Some(report)) = (&shared, status.report.as_object_mut()) {
                report.insert("key_warning".into(), serde_json::json!(warning));
            }
            if ctx.json {
                print_json(&status.report);
            } else {
                let labels = super::Labeller::open(cwd);
                let sprints =
                    atm_common::vocabulary::Vocabulary::of_document(project.store().document()?)
                        .map(|v| {
                            v.sprints
                                .iter()
                                .map(|s| (py_str_opt(s.get("id")), py_str_opt(s.get("state"))))
                                .collect()
                        })
                        .unwrap_or_default();
                let with = StatusText {
                    labels: &labels,
                    sprints,
                    all,
                };
                for line in status_lines(&status, &with) {
                    crate::output::emit(&line);
                }
                if let Some(warning) = shared {
                    crate::output::emit(&format!(
                        "{} {}",
                        crate::output::warning_prefix(),
                        labels.text(&warning)
                    ));
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
        ProjectCommand::Vocab(command) => vocab(command, ctx, cwd),
        ProjectCommand::Key(command) => key(command, ctx, cwd),
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

/// What the report's text needs besides the report: the task labels, and the project's sprints in order with their
/// states (for `--all`'s breakdown by sprint).
pub struct StatusText<'a> {
    /// Task UUIDs as labels.
    pub labels: &'a super::Labeller,
    /// The project's sprints, in order: their names and states.
    pub sprints: Vec<(String, String)>,
    /// `--all`: the counts by sprint, type and label too.
    pub all: bool,
}

/// The report as text, line by line (docs/cli-output-design.md §4): the active sprint, what is worked on, what
/// waits, what's next, then the counts in short; the breakdowns with `--all`.
#[must_use]
pub fn status_lines(status: &StatusReport, with: &StatusText) -> Vec<String> {
    use crate::output::{DIM, HEADING, paint, warning_prefix};
    let r = &status.report;
    let label = |v: &Value| with.labels.text(&text(v));
    let mut lines = Vec::new();
    let sprint = &r["sprint"];
    if sprint.is_object() {
        let mut details = vec![text(&sprint["id"]), "active".into()];
        for key in ["release", "due"] {
            if let Some(v) = sprint.get(key).filter(|v| truthy(v)) {
                details.push(format!("{key} {}", text(v)));
            }
        }
        details.push(format!(
            "{} of {} done",
            text(&sprint["done"]),
            text(&sprint["tasks"])
        ));
        lines.push(paint(HEADING, &details.join(" · ")));
        if let Some(goal) = sprint.get("goal").filter(|g| truthy(g)) {
            lines.extend(crate::output::wrap(
                &format!("  {}", text(goal)),
                crate::output::width(),
            ));
        }
        let pending = list(&sprint["review_pending"]);
        if !pending.is_empty() {
            let ids: Vec<String> = pending.iter().map(label).collect();
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
            .map(|t| format!("{} ({})", label(&t["id"]), text(&t["sprint"])))
            .collect();
        lines.push(format!(
            "{} {n} open task{} in closed sprints, which the loop won't take: {} (atm sprint move <id> --to backlog or --to active)",
            warning_prefix(),
            if n == 1 { "" } else { "s" },
            each.join(", ")
        ));
    }
    lines.extend(task_sections(r, &label));
    let next = &r["next"];
    lines.extend(crate::output::section(
        "Next",
        vec![if next.is_object() {
            format!("  {}  {}", label(&next["id"]), text(&next["name"]))
                .trim_end()
                .to_string()
        } else {
            format!("  none: {}", text(&r["next_reason"]))
        }],
        None,
    ));
    lines.push(String::new());
    lines.extend(totals(r, &label));
    if with.all {
        lines.extend(breakdowns(r, status.has_sprints, &with.sprints));
    } else if r["by_type"].as_object().is_some_and(|m| !m.is_empty()) {
        lines.push(paint(
            DIM,
            "`atm project status --all` adds the counts by sprint, type and label",
        ));
    }
    // Without a sprint line the report starts with a section, and no blank line before it
    if lines.first().is_some_and(String::is_empty) {
        lines.remove(0);
    }
    lines
}

/// What is worked on and what waits: a section each, a line per task, the name cut on a terminal.
fn task_sections(r: &Value, label: &dyn Fn(&Value) -> String) -> Vec<String> {
    use crate::output::{Cell, DIM, Table};
    let sections: [Section; 4] = [
        ("In progress", "in_progress", |i| text(&i["by"])),
        ("In review", "in_review", |i| {
            let since = if truthy(&i["since"]) {
                format!("since {}", text(&i["since"]))
            } else {
                String::new()
            };
            (since + &elsewhere(i)).trim().to_string()
        }),
        ("Blocked", "blocked", |i| {
            let on = if truthy(&i["waiting_on"]) {
                format!("on {}", text(&i["waiting_on"]))
            } else {
                String::new()
            };
            (on + &elsewhere(i)).trim().to_string()
        }),
        ("Waiting on dependencies", "waiting_on_dependencies", |i| {
            format!(
                "on {}",
                list(&i["on"])
                    .iter()
                    .map(text)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }),
    ];
    let mut lines = Vec::new();
    for (title, key, extra) in sections {
        let items = list(&r[key]);
        let mut table = Table::new().indent(2).cut(1);
        for i in &items {
            table.row(vec![
                Cell::new(label(&i["id"])),
                Cell::new(text(&i["name"])),
                Cell::styled(label(&Value::String(extra(i))), DIM),
            ]);
        }
        lines.extend(crate::output::section(
            title,
            table.lines(),
            Some(items.len()),
        ));
    }
    lines
}

/// The project in short: its tasks by status, the file's size, the archive, and the archive's hint.
fn totals(r: &Value, label: &dyn Fn(&Value) -> String) -> Vec<String> {
    use crate::output::{DIM, paint};
    let n = r["tasks"].as_u64().unwrap_or(0);
    let mut lines = vec![format!(
        "{n} task{}: {}",
        if n == 1 { "" } else { "s" },
        pairs(&r["by_status"]).replace(", ", " · ")
    )];
    let largest: Vec<String> = list(&r["largest_tasks"])
        .iter()
        .map(|i| {
            let kb = i["bytes"].as_u64().unwrap_or(0) / 1024;
            format!("{} {} KB", label(&i["id"]), if kb == 0 { 1 } else { kb })
        })
        .collect();
    let file_kb = r["file_bytes"].as_u64().unwrap_or(0) / 1024;
    let mut file = format!("file: {file_kb} KB");
    if !largest.is_empty() {
        let _ = write!(file, " (largest {})", largest.join(", "));
    }
    if truthy(&r["archived"]) {
        let _ = write!(
            file,
            " · archive: {} tasks, {} KB",
            text(&r["archived"]),
            r["archive_bytes"].as_u64().unwrap_or(0) / 1024
        );
    }
    lines.push(paint(DIM, &file));
    if let Some(hint) = r
        .get("suggest")
        .and_then(|s| s.get("archive"))
        .filter(|h| truthy(h))
    {
        lines.push(text(&hint["text"]));
    }
    lines
}

/// `--all`: the counts by sprint (in the project's order, the open ones marked), by type and by label (largest
/// first).
fn breakdowns(r: &Value, has_sprints: bool, sprints: &[(String, String)]) -> Vec<String> {
    use crate::output::{Cell, DIM, Table};
    let counts = |key: &str| -> Vec<(String, u64)> {
        r[key]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(k, v)| (k.clone(), v.as_u64().unwrap_or(0)))
            .collect()
    };
    let mut lines = Vec::new();
    if has_sprints {
        let by_sprint = counts("by_sprint");
        let mut ordered: Vec<(String, u64, String)> = sprints
            .iter()
            .filter_map(|(name, state)| {
                by_sprint.iter().find(|(s, _)| s == name).map(|(_, n)| {
                    let open = if state == "closed" { "" } else { state };
                    (name.clone(), *n, open.to_string())
                })
            })
            .collect();
        // What isn't one of the sprints (the backlog) comes last
        ordered.extend(
            by_sprint
                .iter()
                .filter(|(s, _)| !sprints.iter().any(|(name, _)| name == s))
                .map(|(s, n)| (s.clone(), *n, String::new())),
        );
        let mut table = Table::new().indent(2).right(1);
        for (name, n, state) in ordered {
            table.row(vec![
                Cell::new(name),
                Cell::new(n.to_string()),
                Cell::styled(state, DIM),
            ]);
        }
        lines.extend(crate::output::section("By sprint", table.lines(), None));
    }
    for (title, key) in [("By type", "by_type"), ("By label", "by_label")] {
        let mut each = counts(key);
        each.sort_by_key(|a| std::cmp::Reverse(a.1));
        let mut table = Table::new().indent(2).right(1);
        for (name, n) in each {
            table.row(vec![Cell::new(name), Cell::new(n.to_string())]);
        }
        lines.extend(crate::output::section(title, table.lines(), None));
    }
    lines
}
