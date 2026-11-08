//! `atm db`: the task files' maintenance (the Python atm's `cli/commands/db.py`): migrate, the merge driver, archive,
//! unarchive and init.

use std::path::Path;

use atm_common::py::py_str;
use atm_common::{Error, Result};
use atm_lib::Project;
use atm_lib::archive::{ArchiveRequest, archive_finished, unarchive_tasks};
use atm_lib::code::ensure_repository;
use atm_lib::maintenance::{StoreMaintenance, merge_versions};
use atm_lib::settings::{ensure_project_id, init_meta};
use clap::Subcommand;
use serde_json::{Value, json};

use super::{Context, print_json};

/// The task files' maintenance commands.
#[derive(Debug, Subcommand)]
pub enum DbCommand {
    /// Move tasks.json into .agentic-task-manager/ and set up the folder.
    Migrate,
    /// git's merge driver for tasks.json (git runs it; `db init` registers it).
    #[command(hide = true)]
    MergeDriver {
        /// The common ancestor's tasks.json
        base: String,
        /// This branch's tasks.json (written with the result)
        ours: String,
        /// The other branch's tasks.json
        theirs: String,
    },
    /// Move finished tasks to tasks-archive.json (with --dry-run: say which, and move nothing).
    Archive {
        /// Only this sprint's finished tasks (repeatable)
        #[arg(long)]
        sprint: Vec<String>,
        /// Only those finished before this date (YYYY-MM-DD)
        #[arg(long)]
        before: Option<String>,
        /// Keep those finished in the last this many days
        #[arg(long = "keep-days")]
        keep_days: Option<f64>,
    },
    /// Bring archived tasks back into tasks.json.
    Unarchive {
        /// Task IDs
        #[arg(required = true)]
        ids: Vec<String>,
    },
    /// Give tasks.json a meta.id and register the merge driver.
    Init {
        /// Set tasks.json meta from a JSON object
        #[arg(long = "meta-file")]
        meta_file: Option<String>,
    },
}

/// The store operations of the project at or above `cwd` (`project.store_maintenance()`).
///
/// # Errors
/// The project can't be opened, or its store has no such operations.
pub(crate) fn maintenance(cwd: &Path) -> Result<StoreMaintenance> {
    Project::open(cwd)?
        .store_maintenance()
        .ok_or_else(|| Error::Invalid("this project's store has no task files to maintain".into()))
}

/// "T-1, T-2 and 3 more": the first `limit` IDs.
fn describe_ids(ids: &[Value], limit: usize) -> String {
    let shown: Vec<String> = ids.iter().take(limit).map(py_str).collect();
    let mut text = shown.join(", ");
    if ids.len() > limit {
        text = format!("{text} and {} more", ids.len() - limit);
    }
    text
}

/// "T-010 (in open PR #1); T-011, T-012 (finished in the last 14 days)": grouped by reason, in first-seen order.
fn describe_kept(kept: &[Value]) -> String {
    let mut groups: Vec<(String, Vec<Value>)> = Vec::new();
    for k in kept {
        let reason = py_str(&k["reason"]);
        let id = k["id"].clone();
        match groups.iter_mut().find(|(r, _)| *r == reason) {
            Some((_, ids)) => ids.push(id),
            None => groups.push((reason, vec![id])),
        }
    }
    groups
        .iter()
        .map(|(reason, ids)| format!("{} ({reason})", describe_ids(ids, 12)))
        .collect::<Vec<_>>()
        .join("; ")
}

fn archive(
    ctx: Context,
    cwd: &Path,
    sprints: Vec<String>,
    before: Option<String>,
    keep_days: Option<f64>,
) -> Result<()> {
    let request = ArchiveRequest {
        sprints,
        before,
        keep_days,
        dry_run: ctx.dry_run,
    };
    let result = archive_finished(&mut Project::open(cwd)?, &request)?;
    let (ids, dropped, kept) = (&result.moved, &result.duplicates_dropped, &result.kept);
    let (tasks_kb, archive_kb) = (result.sizes.0 / 1024, result.sizes.1 / 1024);
    if ctx.json {
        print_json(&json!({
            "moved": ids,
            "dry_run": ctx.dry_run,
            "tasks_kb": tasks_kb,
            "archive_kb": archive_kb,
            "duplicates_dropped": dropped,
            "kept": kept,
        }));
        return Ok(());
    }
    if !dropped.is_empty() && !ctx.dry_run {
        println!(
            "Kept {} in tasks.json and dropped the archive's copy (an interrupted move).",
            describe_ids(dropped, 12)
        );
    }
    if ids.is_empty() {
        println!("Nothing to archive.");
    } else if ctx.dry_run {
        println!(
            "Would archive {} task(s): {}.",
            ids.len(),
            describe_ids(ids, 12)
        );
    } else {
        println!(
            "Archived {} task(s): {}. tasks.json is now {tasks_kb} KB, tasks-archive.json {archive_kb} KB.",
            ids.len(),
            describe_ids(ids, 12)
        );
    }
    if !kept.is_empty() {
        println!(
            "{} {} finished: {}.",
            if ctx.dry_run { "Would keep" } else { "Kept" },
            kept.len(),
            describe_kept(kept)
        );
    }
    Ok(())
}

fn unarchive(ctx: Context, cwd: &Path, ids: &[String]) -> Result<()> {
    if ctx.dry_run {
        println!("[dry-run] would unarchive {}", ids.join(", "));
        return Ok(());
    }
    let result = unarchive_tasks(&mut Project::open(cwd)?, ids)?;
    if ctx.json {
        print_json(&json!({"restored": result.restored, "moves_back": result.moves_back}));
        return Ok(());
    }
    println!("Brought back {} from the archive.", ids.join(", "));
    let again = &result.moves_back;
    if !again.is_empty() {
        let one = again.len() == 1;
        println!(
            "{} {} finished: the next `atm db archive` moves {} back, unless reopened (`atm task state <id> todo --reason …`).",
            describe_ids(again, 12),
            if one { "is" } else { "are" },
            if one { "it" } else { "them" }
        );
    }
    Ok(())
}

fn init(ctx: Context, cwd: &Path, meta_file: Option<&str>) -> Result<()> {
    if ctx.dry_run {
        println!("[dry-run] would give tasks.json a meta.id and register the merge driver");
        return Ok(());
    }
    let mut project = Project::open(cwd)?;
    if let Some(meta_file) = meta_file.filter(|m| !m.is_empty()) {
        let new = std::fs::read_to_string(cwd.join(meta_file))
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok());
        let Some(Value::Object(new)) = new else {
            return Err(Error::Invalid(format!(
                "{meta_file} must hold a JSON object: the meta keys to set"
            )));
        };
        let (created, keys) = init_meta(&mut project, &new)?;
        // The task data as it is now
        project = Project::open(cwd)?;
        if !ctx.json {
            let keys = if keys.is_empty() {
                "-".to_string()
            } else {
                keys.join(", ")
            };
            let start = if created {
                "Created tasks.json; meta: "
            } else {
                "Updated tasks.json meta: "
            };
            println!("{start}{keys}");
        }
    }
    let project_id = ensure_project_id(&mut project)?;
    let vcs = project.vcs();
    let repository = ensure_repository(&mut project, vcs.as_ref())?;
    // The dashboard lists every project atm runs in
    project.join_default_workspace()?;
    let setup = maintenance(cwd)?.set_up()?;
    if ctx.json {
        print_json(&json!({
            "project_id": project_id,
            "repository": repository,
            "git": setup.git,
            "gitattributes_added": setup.gitattributes_added,
        }));
        return Ok(());
    }
    if let Some(id) = project_id {
        println!(
            "Gave this project an id ({id}) in tasks.json meta, for the dashboard's saved view."
        );
    }
    if let Some(repository) = repository {
        println!(
            "Recorded the repository address ({repository}), for commit and PR links on the dashboard."
        );
    }
    if !setup.git {
        println!("Not in a git repository: there is no merge driver to set up.");
        return Ok(());
    }
    println!(
        "Registered the tasks.json merge driver (git config merge.atm){}",
        if setup.gitattributes_added {
            " and added .agentic-task-manager/.gitattributes (staged)."
        } else {
            "; .gitattributes already set."
        }
    );
    Ok(())
}

/// Run a `db` command in `cwd`; its exit code (1 when a merge kept conflicting edits).
///
/// # Errors
/// What the store refuses.
pub fn run(command: DbCommand, ctx: Context, cwd: &Path) -> Result<u8> {
    match command {
        DbCommand::Migrate => {
            if ctx.dry_run {
                println!(
                    "[dry-run] would move tasks.json into .agentic-task-manager/ and set up the folder"
                );
                return Ok(0);
            }
            let migration = maintenance(cwd)?.migrate(true)?;
            if ctx.json {
                print_json(&Value::Object(migration.result));
            } else {
                for line in &migration.lines {
                    println!("{line}");
                }
            }
            Ok(0)
        }
        DbCommand::MergeDriver { base, ours, theirs } => {
            let conflicts = merge_versions(&cwd.join(base), &cwd.join(ours), &cwd.join(theirs))?;
            if ctx.json {
                print_json(&json!({"conflicts": conflicts}));
            }
            if conflicts.is_empty() {
                return Ok(0);
            }
            eprintln!("atm: tasks.json has conflicting edits (this branch's version was kept):");
            for conflict in &conflicts {
                eprintln!("  {conflict}");
            }
            Ok(1)
        }
        DbCommand::Archive {
            sprint,
            before,
            keep_days,
        } => archive(ctx, cwd, sprint, before, keep_days).map(|()| 0),
        DbCommand::Unarchive { ids } => unarchive(ctx, cwd, &ids).map(|()| 0),
        DbCommand::Init { meta_file } => init(ctx, cwd, meta_file.as_deref()).map(|()| 0),
    }
}
