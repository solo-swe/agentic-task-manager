//! `atm workspace`: the workspaces, named lists of projects (ADR-003 §1, §3), through atm-lib's `Workspaces`. Each
//! command that works on one takes it first and defaults to `default`; `add-project` adds the current folder's project
//! by default. Every other atm command works on the project in the current folder and never takes a workspace.

use std::path::Path;
use std::sync::Arc;

use atm_common::py::py_str;
use atm_common::{Error, Result};
use atm_lib::Workspaces;
use atm_lib::workspaces::Workspace;
use clap::Subcommand;
use serde_json::Value;

use super::{Context, print_json};

const DEFAULT: &str = "default";

/// The workspace commands.
#[derive(Debug, Subcommand)]
pub enum WorkspaceCommand {
    /// List the workspaces, with how many projects each has and which was opened last.
    List,
    /// Make a workspace, with no projects.
    Create {
        /// Its ID (letters, digits, '.', '_' and '-')
        workspace: String,
    },
    /// Delete a workspace (its projects stay where they are, and in the other workspaces). Not `default`.
    Delete {
        /// The workspace
        workspace: String,
    },
    /// Rename a workspace. Not `default`.
    Rename {
        /// The workspace
        workspace: String,
        /// Its new ID
        new: String,
    },
    /// List a workspace's projects, the missing ones marked.
    Projects {
        /// The workspace (default: `default`)
        workspace: Option<String>,
    },
    /// Add a project to a workspace.
    AddProject {
        /// The workspace (default: `default`)
        workspace: Option<String>,
        /// The project's folder (default: here)
        folder: Option<String>,
    },
    /// Remove a project from a workspace: `remove-project [WORKSPACE] PROJECT`.
    RemoveProject {
        /// The workspace, then the project; or the project alone, in `default`
        first: String,
        /// The project's ID or path, after a workspace
        second: Option<String>,
    },
    /// Remove a workspace's projects whose folder is gone.
    Prune {
        /// The workspace (default: `default`)
        workspace: Option<String>,
    },
}

/// With `--dry-run`, say what would be done; whether it was a dry run.
fn dry(ctx: Context, what: &str) -> bool {
    if ctx.dry_run {
        println!("[dry-run] would {what}");
    }
    ctx.dry_run
}

fn workspace(workspaces: &Workspaces, id: Option<&str>) -> Result<Arc<Workspace>> {
    let id = id.unwrap_or(DEFAULT);
    workspaces
        .get(id)
        .ok_or_else(|| Error::NotFound(format!("no workspace {id} (see `atm workspace list`)")))
}

fn describe_entry(entry: &Value) -> String {
    format!(
        "{}  {}  {}",
        py_str(&entry["id"]),
        py_str(&entry["name"]),
        py_str(&entry["path"])
    )
}

fn damaged_notice(workspace: &str, result: &Value) {
    if let Some(damaged) = result["damaged"].as_str() {
        println!(
            "The workspace {workspace} (workspaces/{workspace}.json) was unreadable: moved it to {damaged} and started an empty one."
        );
    }
}

fn plural(n: usize, one: &str) -> String {
    format!("{n} {one}{}", if n == 1 { "" } else { "s" })
}

/// Run a workspace command in `cwd`.
///
/// # Errors
/// What atm-lib refuses: no such workspace or project, `default` deleted or renamed, an ID that isn't one or is taken,
/// a folder without a task file or one that opted out, the state folder's lock or files.
pub fn run(command: WorkspaceCommand, ctx: Context, cwd: &Path) -> Result<()> {
    let workspaces = Workspaces::open();
    match command {
        WorkspaceCommand::List => {
            list(&workspaces, ctx);
            Ok(())
        }
        WorkspaceCommand::Create { workspace } => {
            if dry(ctx, &format!("make the workspace {workspace}")) {
                return Ok(());
            }
            let made = workspaces.create(&workspace)?;
            if ctx.json {
                print_json(&serde_json::json!({"workspace": workspace, "created": made}));
            } else if made {
                println!(
                    "Made the workspace {workspace}. `atm workspace add-project {workspace} [FOLDER]` adds a project."
                );
            } else {
                println!("There is a workspace {workspace} already.");
            }
            Ok(())
        }
        WorkspaceCommand::Delete { workspace } => {
            if dry(ctx, &format!("delete the workspace {workspace}")) {
                return Ok(());
            }
            workspaces.delete(&workspace)?;
            if ctx.json {
                print_json(&serde_json::json!({"workspace": workspace, "deleted": true}));
            } else {
                println!("Deleted the workspace {workspace}; its projects are where they were.");
            }
            Ok(())
        }
        WorkspaceCommand::Rename { workspace, new } => {
            if dry(ctx, &format!("rename the workspace {workspace} to {new}")) {
                return Ok(());
            }
            workspaces.rename(&workspace, &new)?;
            if ctx.json {
                print_json(&serde_json::json!({"workspace": new, "was": workspace}));
            } else {
                println!("Renamed the workspace {workspace} to {new}.");
            }
            Ok(())
        }
        WorkspaceCommand::Projects { workspace: id } => projects(&workspaces, id.as_deref(), ctx),
        WorkspaceCommand::AddProject {
            workspace: id,
            folder,
        } => add_project(&workspaces, id.as_deref(), folder.as_deref(), ctx, cwd),
        WorkspaceCommand::RemoveProject { first, second } => {
            let (id, target) = match second {
                Some(project) => (Some(first), project),
                None => (None, first),
            };
            remove_project(&workspaces, id.as_deref(), &target, ctx, cwd)
        }
        WorkspaceCommand::Prune { workspace: id } => prune(&workspaces, id.as_deref(), ctx),
    }
}

/// `atm workspace add-project [WORKSPACE] [FOLDER]`.
fn add_project(
    workspaces: &Workspaces,
    id: Option<&str>,
    folder: Option<&str>,
    ctx: Context,
    cwd: &Path,
) -> Result<()> {
    let found = workspace(workspaces, id)?;
    let what = format!(
        "add {} to the workspace {}",
        folder.unwrap_or("this folder"),
        found.id()
    );
    if dry(ctx, &what) {
        return Ok(());
    }
    let result = found.add_project(folder, cwd)?;
    if ctx.json {
        print_json(&result);
        return Ok(());
    }
    damaged_notice(found.id(), &result);
    let verb = if result["result"] == "added" {
        "Added to"
    } else {
        "Already in"
    };
    println!(
        "{verb} {}: {}",
        found.id(),
        describe_entry(&result["project"])
    );
    Ok(())
}

/// `atm workspace remove-project [WORKSPACE] PROJECT`.
fn remove_project(
    workspaces: &Workspaces,
    id: Option<&str>,
    target: &str,
    ctx: Context,
    cwd: &Path,
) -> Result<()> {
    let found = workspace(workspaces, id)?;
    if dry(
        ctx,
        &format!("remove {target} from the workspace {}", found.id()),
    ) {
        return Ok(());
    }
    let result = found.remove_project(target, cwd)?;
    if ctx.json {
        print_json(&result);
    } else {
        println!(
            "Removed from {}: {}",
            found.id(),
            describe_entry(&result["removed"])
        );
    }
    Ok(())
}

/// `atm workspace prune [WORKSPACE]`.
fn prune(workspaces: &Workspaces, id: Option<&str>, ctx: Context) -> Result<()> {
    let found = workspace(workspaces, id)?;
    if dry(
        ctx,
        &format!(
            "remove the projects whose folder is gone from the workspace {}",
            found.id()
        ),
    ) {
        return Ok(());
    }
    let result = found.prune()?;
    if ctx.json {
        print_json(&result);
        return Ok(());
    }
    damaged_notice(found.id(), &result);
    let removed = result["removed"].as_array().cloned().unwrap_or_default();
    if removed.is_empty() {
        println!(
            "Nothing to prune: every project in {} is there.",
            found.id()
        );
    }
    for e in &removed {
        println!("Removed from {}: {}", found.id(), describe_entry(e));
    }
    Ok(())
}

/// `atm workspace list`.
fn list(workspaces: &Workspaces, ctx: Context) {
    let summary = workspaces.summary();
    if ctx.json {
        print_json(&summary);
        return;
    }
    let recent = summary["recent"].as_str().unwrap_or(DEFAULT);
    let all = summary["workspaces"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let width = all
        .iter()
        .map(|w| w["id"].as_str().unwrap_or_default().chars().count())
        .max()
        .unwrap_or(0);
    for w in &all {
        let id = w["id"].as_str().unwrap_or_default();
        let count = usize::try_from(w["projects"].as_u64().unwrap_or(0)).unwrap_or(usize::MAX);
        let opened = if id == recent && all.len() > 1 {
            "  (opened last)"
        } else {
            ""
        };
        println!("{id:<width$}  {}{opened}", plural(count, "project"));
    }
}

/// `atm workspace projects [WORKSPACE]`.
fn projects(workspaces: &Workspaces, id: Option<&str>, ctx: Context) -> Result<()> {
    let found = workspace(workspaces, id)?;
    let result = found.list_projects()?;
    if ctx.json {
        print_json(&result);
        return Ok(());
    }
    damaged_notice(found.id(), &result);
    let listed = result["projects"].as_array().cloned().unwrap_or_default();
    if listed.is_empty() {
        if found.id() == DEFAULT {
            println!(
                "No projects in default. `atm workspace add-project` adds this folder's; atm adds a project when it runs there."
            );
        } else {
            println!(
                "No projects in {0}. `atm workspace add-project {0} [FOLDER]` adds one.",
                found.id()
            );
        }
    }
    for e in &listed {
        let missing = if e["missing"] == true {
            "  (not found)"
        } else {
            ""
        };
        println!("{}{missing}", describe_entry(e));
    }
    Ok(())
}
