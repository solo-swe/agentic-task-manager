//! `atm project config|workflow|logo` and the dashboard's list of projects (`atm project list|add|remove|prune`), as
//! the Python atm's `cli/commands/project.py` has them.

use std::path::Path;

use atm_boot::{default_base, open_project, open_registry, task_paths};
use atm_common::py::{py_str, truthy};
use atm_common::{Error, Result};
use atm_lib::settings::{
    WorkflowChange, get_logo, get_workflow, set_logo, set_workflow, unset_logo,
};
use clap::Args;
use serde_json::{Map, Value, json};

use crate::commands::{Context, print_json};

/// A value that must be one of `choices` (or not given), refused as the Python atm refuses it (exit code 2).
fn choice(name: &str, value: Option<&str>, choices: &[&str]) -> Result<()> {
    match value {
        Some(v) if !choices.contains(&v) => Err(Error::Invalid(format!(
            "{name}: invalid choice '{v}' (choose from {})",
            choices.join(", ")
        ))),
        _ => Ok(()),
    }
}

/// With `--dry-run`, say what would be done; whether it was a dry run.
fn dry(ctx: Context, what: &str) -> bool {
    if ctx.dry_run {
        println!("[dry-run] would {what}");
    }
    ctx.dry_run
}

/// `atm project config`: where the task files are.
///
/// # Errors
/// A task folder that fails its checks.
pub fn config(ctx: Context, cwd: &Path) -> Result<()> {
    let paths = task_paths(cwd)?;
    if ctx.json {
        print_json(&paths);
        return Ok(());
    }
    for key in ["root", "dir", "tasks", "dashboard"] {
        println!("{key}: {}", py_str(&paths[key]));
    }
    if let Some(tasks) = paths["legacy"].get("tasks") {
        println!("legacy: {}", py_str(tasks));
    }
    Ok(())
}

/// `atm project workflow`'s arguments and options.
#[derive(Debug, Args)]
pub struct WorkflowArgs {
    /// set: change the workflow
    action: Option<String>,
    /// direct or pull-request
    wf_type: Option<String>,
    /// The branch PRs target, or that direct pushes to
    #[arg(long)]
    base: Option<String>,
    /// Run the project's verify before pushing (true|false)
    #[arg(long)]
    verify: Option<String>,
    /// CI fix rounds before /atm-push stops
    #[arg(long)]
    max_rounds: Option<i64>,
    /// Commit messages, e.g. "feat({id}): {name}"
    #[arg(long, value_name = "TEXT")]
    commit_template: Option<String>,
    /// forbid Co-Authored-By lines, require the --trailer lines, or none (the agent's)
    #[arg(long)]
    trailers: Option<String>,
    /// A required trailer line
    #[arg(long, value_name = "LINE")]
    trailer: Vec<String>,
}

fn describe_workflow(workflow: Option<&Map<String, Value>>, cwd: &Path) -> String {
    let Some(w) = workflow else {
        return "No workflow set: /atm-push asks for one, or run `atm project workflow set direct|pull-request`.".into();
    };
    let base = w.get("base").filter(|b| truthy(b)).map_or_else(
        || format!("{} (the remote's default branch)", default_base(cwd)),
        py_str,
    );
    let trailers = match w.get("trailers") {
        Some(Value::String(s)) if s == "forbid" => {
            "none (Co-Authored-By lines are removed)".to_string()
        }
        Some(Value::Object(t)) => {
            let lines: Vec<String> = t
                .get("require")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(py_str)
                .collect();
            format!("required: {}", lines.join("; "))
        }
        _ => "as the agent writes them".to_string(),
    };
    let verify = if w.get("verify").is_some_and(truthy) {
        "yes"
    } else {
        "no"
    };
    [
        format!(
            "workflow: {}",
            w.get("type").map(py_str).unwrap_or_default()
        ),
        format!("  base: {base}"),
        format!("  verify before pushing: {verify}"),
        format!(
            "  CI fix rounds: {}",
            w.get("max_rounds").map(py_str).unwrap_or_default()
        ),
        format!(
            "  commit message: {}",
            w.get("commit_template").map(py_str).unwrap_or_default()
        ),
        format!("  trailers: {trailers}"),
    ]
    .join("\n")
}

/// `atm project workflow [set TYPE]`: print the project's workflow, or set it.
///
/// # Errors
/// A bad choice, an unknown type, a workflow atm can't follow, task data that can't be read.
pub fn workflow(args: WorkflowArgs, ctx: Context, cwd: &Path) -> Result<()> {
    choice("action", args.action.as_deref(), &["set"])?;
    choice("--verify", args.verify.as_deref(), &["true", "false"])?;
    choice(
        "--trailers",
        args.trailers.as_deref(),
        &["forbid", "require", "none"],
    )?;
    let mut project = open_project(cwd)?;
    let current = if args.action.is_none() {
        get_workflow(&project)?
    } else {
        if dry(
            ctx,
            &format!(
                "set the workflow to {}",
                args.wf_type.as_deref().unwrap_or("None")
            ),
        ) {
            return Ok(());
        }
        let change = WorkflowChange {
            base: args.base,
            verify: args.verify.as_deref().map(|v| v == "true"),
            max_rounds: args.max_rounds,
            commit_template: args.commit_template,
            trailers: args.trailers,
            trailer: args.trailer,
        };
        Some(set_workflow(
            &mut project,
            args.wf_type.as_deref(),
            &change,
        )?)
    };
    if ctx.json {
        print_json(&current.map_or_else(|| json!({"type": null}), Value::Object));
    } else {
        println!("{}", describe_workflow(current.as_ref(), cwd));
    }
    Ok(())
}

/// `atm project logo [set PATH | unset]`: print, set or unset the project's logo (`meta.logo`).
///
/// # Errors
/// A bad choice or argument, an image the dashboard can't be given, task data that can't be read.
pub fn logo(action: Option<&str>, path: Option<&str>, ctx: Context, cwd: &Path) -> Result<()> {
    choice("action", action, &["set", "unset"])?;
    if let Some(action) = action
        && dry(ctx, &format!("{action} the logo"))
    {
        return Ok(());
    }
    let mut project = open_project(cwd)?;
    match action {
        None => {
            if let Some(path) = path {
                return Err(Error::Invalid(format!(
                    "unexpected argument '{path}' (atm project logo set PATH)"
                )));
            }
            let (found, chosen) = get_logo(&project)?;
            if ctx.json {
                let (logo, source) = found.as_ref().map_or((Value::Null, Value::Null), |l| {
                    (json!(l.path), json!(l.source))
                });
                print_json(&json!({"logo": logo, "source": source, "set": chosen}));
            } else if let Some(found) = found {
                let hint = if found.source == "found" {
                    " (found; set one with `atm project logo set <path>`)"
                } else {
                    ""
                };
                println!("{}{hint}", found.path);
            } else if truthy(&chosen) {
                println!(
                    "meta.logo is {}, which isn't a usable logo (missing, too large, or outside the project).",
                    py_str(&chosen)
                );
            } else {
                println!(
                    "No logo. Set one with `atm project logo set <path>` (SVG, PNG, WebP or ICO, inside the project)."
                );
            }
        }
        Some("unset") => {
            let removed = unset_logo(&mut project)?;
            if ctx.json {
                print_json(&json!({"logo": null}));
            } else {
                println!(
                    "{}",
                    if removed {
                        "Logo unset."
                    } else {
                        "No logo was set."
                    }
                );
            }
        }
        Some(_) => {
            let Some(path) = path else {
                return Err(Error::Invalid("atm project logo set needs a path".into()));
            };
            // The path is relative to where atm runs, like every path atm takes; atm-lib keeps it relative to the
            // project
            let relative = set_logo(&mut project, path, cwd)?;
            if ctx.json {
                print_json(&json!({"logo": relative}));
            } else {
                println!("Logo set: {relative}");
            }
        }
    }
    Ok(())
}

fn describe_entry(entry: &Value) -> String {
    format!(
        "{}  {}  {}",
        py_str(&entry["id"]),
        py_str(&entry["name"]),
        py_str(&entry["path"])
    )
}

fn damaged_notice(result: &Value) {
    if let Some(damaged) = result["damaged"].as_str() {
        println!(
            "The project list (workspaces/default.json) was unreadable: moved it to {damaged} and started an empty one."
        );
    }
}

/// `atm project list|add|remove|prune`: the projects the dashboard shows.
///
/// # Errors
/// What the registry refuses (no such project, a folder without tasks.json, one that opted out), its lock or file.
pub fn projects(action: &str, target: Option<&str>, ctx: Context, cwd: &Path) -> Result<()> {
    let registry = open_registry();
    match action {
        "add" => {
            if dry(
                ctx,
                &format!(
                    "add {} to the dashboard's projects",
                    target.unwrap_or("this folder")
                ),
            ) {
                return Ok(());
            }
            let result = registry.add_project(target, cwd)?;
            if ctx.json {
                print_json(&result);
                return Ok(());
            }
            damaged_notice(&result);
            let verb = if result["result"] == "added" {
                "Added"
            } else {
                "Already registered"
            };
            println!("{verb}: {}", describe_entry(&result["project"]));
        }
        "remove" => {
            let target = target.unwrap_or_default();
            if dry(
                ctx,
                &format!("remove {target} from the dashboard's projects"),
            ) {
                return Ok(());
            }
            let result = registry.remove_project(target, cwd)?;
            if ctx.json {
                print_json(&result);
            } else {
                println!("Removed: {}", describe_entry(&result["removed"]));
            }
        }
        "prune" => {
            if dry(ctx, "drop the projects whose folder is gone") {
                return Ok(());
            }
            let result = registry.prune()?;
            if ctx.json {
                print_json(&result);
                return Ok(());
            }
            damaged_notice(&result);
            let removed = result["removed"].as_array().cloned().unwrap_or_default();
            if removed.is_empty() {
                println!("Nothing to prune: every registered project is there.");
            }
            for e in &removed {
                println!("Removed: {}", describe_entry(e));
            }
        }
        _ => {
            let result = registry.list_projects()?;
            if ctx.json {
                print_json(&result);
                return Ok(());
            }
            damaged_notice(&result);
            let listed = result["projects"].as_array().cloned().unwrap_or_default();
            if listed.is_empty() {
                println!(
                    "No projects registered. `atm project add [PATH]` adds one; atm adds a project when it runs there."
                );
            }
            for e in &listed {
                let missing = if e["missing"] == true {
                    "  (not found)"
                } else {
                    ""
                };
                println!("{}{missing}", describe_entry(e));
            }
        }
    }
    Ok(())
}
