//! `atm component`: the project's components (the Python atm's `cli/commands/component.py`): list, add, edit, rename.

use std::path::Path;

use atm_common::Result;
use atm_common::py::{py_str, truthy};
use atm_lib::Project;
use atm_lib::components::{
    Moved, add_component, edit_component, list_components, rename_component,
};
use clap::Subcommand;
use serde_json::{Value, json};

use super::{Context, print_json};

/// The component commands.
#[derive(Debug, Subcommand)]
pub enum ComponentCommand {
    /// List components with their task counts.
    List,
    /// Add a component.
    Add {
        /// The component's ID (letters, digits, '.', '-', '_')
        component_id: String,
        /// Add, edit: the name shown
        #[arg(long)]
        label: Option<String>,
        /// Add: a glob over its files, e.g. 'dashboard/**' (repeatable)
        #[arg(long, value_name = "GLOB")]
        path: Vec<String>,
        /// Add, edit: how its tasks are reviewed
        #[arg(long)]
        review: Option<String>,
        /// Add, edit: every task with this label gets the component instead
        #[arg(long, value_name = "LABEL")]
        from_label: Option<String>,
        /// Add, edit: also give it to every task whose files its globs match
        #[arg(long)]
        assign: bool,
    },
    /// Edit a component.
    Edit {
        /// The component's ID (letters, digits, '.', '-', '_')
        component_id: String,
        /// The name shown (empty removes it)
        #[arg(long)]
        label: Option<String>,
        /// Add a glob over its files (repeatable)
        #[arg(long, value_name = "GLOB")]
        add_path: Vec<String>,
        /// How its tasks are reviewed (blocker, async, batch, none; empty removes it)
        #[arg(long)]
        review: Option<String>,
        /// Every task with this label gets the component instead
        #[arg(long, value_name = "LABEL")]
        from_label: Option<String>,
        /// Also give it to every task whose files its globs match
        #[arg(long)]
        assign: bool,
    },
    /// Rename a component's ID: the tasks listing it, archived ones too, follow.
    Rename {
        /// The component's ID
        component_id: String,
        /// Its new ID (letters, digits, '.', '-', '_')
        new: String,
    },
}

/// What `--from-label` and `--assign` did, e.g. "6 tasks from the label atm-cli, 2 more by their files".
fn moved_text(label: Option<&str>, moved: &Moved, assign: bool) -> String {
    let label = label.filter(|l| !l.is_empty());
    let mut parts = Vec::new();
    if let Some(label) = label {
        let n = moved.from_label.len();
        parts.push(format!(
            "{n} task{} from the label {label}",
            if n == 1 { "" } else { "s" }
        ));
    }
    if assign {
        let more = if label.is_some() { "more " } else { "" };
        parts.push(format!("{} {more}by their files", moved.by_files.len()));
    }
    parts.join(", ")
}

/// `--flag value` for each option given (`None` left out), in order, then `--assign`.
fn dry_line(what: &str, id: &str, options: &[(&str, Vec<&str>)], assign: bool) -> String {
    let mut parts = vec![id.to_string()];
    for (flag, values) in options {
        parts.extend(values.iter().map(|v| format!("--{flag} {v}")));
    }
    if assign {
        parts.push("--assign".into());
    }
    format!("[dry-run] would {what} component {}", parts.join(" "))
}

/// Run a component command in `cwd`.
///
/// # Errors
/// What atm-lib refuses, and a task file that can't be read.
#[allow(clippy::too_many_lines)] // reason: one arm per command, as the Python atm's component_app has them
pub fn run(command: ComponentCommand, ctx: Context, cwd: &Path) -> Result<()> {
    /// An option given with a value (an empty one counts as not given, as in Python).
    fn some(v: Option<&String>) -> Option<&str> {
        v.map(String::as_str).filter(|s| !s.is_empty())
    }
    match command {
        ComponentCommand::List => {
            let listed = list_components(&Project::open(cwd)?)?;
            if ctx.json {
                print_json(&Value::Array(listed));
            } else if listed.is_empty() {
                println!("No components yet: atm component add <id> --path GLOB.");
            } else {
                for c in &listed {
                    let mut details = Vec::new();
                    if let Some(label) = c.get("label").filter(|l| truthy(l)) {
                        details.push(format!("“{}”", py_str(label)));
                    }
                    let paths: Vec<String> = c["paths"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(py_str)
                        .collect();
                    details.push(if paths.is_empty() {
                        "no paths".into()
                    } else {
                        paths.join(", ")
                    });
                    if let Some(review) = c.get("review").filter(|r| truthy(r)) {
                        details.push(format!("review {}", py_str(review)));
                    }
                    let n = c["tasks"].as_u64().unwrap_or(0);
                    println!(
                        "{} · {}: {n} task{}",
                        py_str(&c["id"]),
                        details.join(" · "),
                        if n == 1 { "" } else { "s" }
                    );
                }
            }
        }
        ComponentCommand::Add {
            component_id,
            label,
            path,
            review,
            from_label,
            assign,
        } => {
            if ctx.dry_run {
                let options = [
                    ("label", some(label.as_ref()).into_iter().collect()),
                    ("path", path.iter().map(String::as_str).collect()),
                    ("review", some(review.as_ref()).into_iter().collect()),
                    (
                        "from-label",
                        some(from_label.as_ref()).into_iter().collect(),
                    ),
                ];
                println!("{}", dry_line("add", &component_id, &options, assign));
                return Ok(());
            }
            let (component, moved) = add_component(
                &mut Project::open(cwd)?,
                &component_id,
                label.as_deref(),
                &path,
                review.as_deref(),
                from_label.as_deref(),
                assign,
            )?;
            if ctx.json {
                print_json(
                    &json!({"component": component, "from_label": moved.from_label, "by_files": moved.by_files}),
                );
                return Ok(());
            }
            let text = moved_text(from_label.as_deref(), &moved, assign);
            if text.is_empty() {
                println!("Added the component {component_id}.");
            } else {
                println!("Added the component {component_id}: {text}.");
            }
        }
        ComponentCommand::Edit {
            component_id,
            label,
            add_path,
            review,
            from_label,
            assign,
        } => {
            if ctx.dry_run {
                let options = [
                    ("label", label.as_deref().into_iter().collect()),
                    ("add-path", add_path.iter().map(String::as_str).collect()),
                    ("review", review.as_deref().into_iter().collect()),
                    (
                        "from-label",
                        some(from_label.as_ref()).into_iter().collect(),
                    ),
                ];
                println!("{}", dry_line("edit", &component_id, &options, assign));
                return Ok(());
            }
            let edited = edit_component(
                &mut Project::open(cwd)?,
                &component_id,
                label.as_deref(),
                &add_path,
                review.as_deref(),
                from_label.as_deref(),
                assign,
            )?;
            if ctx.json {
                print_json(&json!({
                    "component": edited.component,
                    "changed": edited.changed,
                    "from_label": edited.moved.from_label,
                    "by_files": edited.moved.by_files,
                }));
                return Ok(());
            }
            let mut parts = Vec::new();
            if !edited.changed.is_empty() {
                parts.push(format!("changed {}", edited.changed.join(", ")));
            }
            let moved = moved_text(from_label.as_deref(), &edited.moved, assign);
            if !moved.is_empty() {
                parts.push(moved);
            }
            let text = if parts.is_empty() {
                "nothing changed".to_string()
            } else {
                parts.join(", ")
            };
            println!("{component_id}: {text}");
        }
        ComponentCommand::Rename { component_id, new } => {
            if ctx.dry_run {
                println!("[dry-run] would rename component {component_id} to {new}");
                return Ok(());
            }
            let renamed = rename_component(&mut Project::open(cwd)?, &component_id, &new)?;
            if ctx.json {
                print_json(&json!({
                    "from": renamed.from,
                    "to": renamed.to,
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
            println!(
                "Renamed the component {} to {}: {n} task{}{archived} follow it.",
                renamed.from,
                renamed.to,
                if n == 1 { "" } else { "s" }
            );
        }
    }
    Ok(())
}
