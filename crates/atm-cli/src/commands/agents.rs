//! `atm agents` (T-442): atm's skills, installed where the agent harnesses read them. The Python atm's
//! `cli/commands/agents.py`.

use std::path::{Path, PathBuf};

use atm_boot::open_project;
use atm_common::Result;
use atm_lib::agents::{install_skills, project_folder, skills_status, uninstall_skills};
use clap::{Args, Subcommand};
use serde_json::{Value, json};

use super::{Context, print_json};

/// Where the skills go: the user's folder, or this project's.
#[derive(Debug, Args)]
pub struct Scope {
    /// This project's folder (copied, to commit), not your own
    #[arg(long)]
    project: bool,
}

/// The `atm agents` commands.
#[derive(Debug, Subcommand)]
pub enum AgentsCommand {
    /// Put atm's skills, this version's, where the harness reads them: linked to atm's folder (copied with --copy or
    /// --project). Prints every path it wrote; never replaces a folder atm didn't write.
    Install {
        /// Only this harness (claude); all of them without it
        #[arg(long)]
        harness: Option<String>,
        #[command(flatten)]
        scope: Scope,
        /// Copy the skills instead of linking them
        #[arg(long)]
        copy: bool,
    },
    /// Where atm's skills are for each harness: linked, copied, missing, stale, or someone else's.
    Status {
        #[command(flatten)]
        scope: Scope,
    },
    /// Remove exactly what `atm agents install` wrote, and nothing else.
    Uninstall {
        /// Only this harness (claude); all of them without it
        #[arg(long)]
        harness: Option<String>,
        #[command(flatten)]
        scope: Scope,
    },
}

/// The project's folder for `--project`: where the task folder is (the working copy), else `cwd`.
fn project_dir(scope: &Scope, cwd: &Path) -> Result<Option<PathBuf>> {
    if !scope.project {
        return Ok(None);
    }
    Ok(Some(
        project_folder(&open_project(cwd)?).unwrap_or_else(|| cwd.to_path_buf()),
    ))
}

/// Run an `atm agents` command in `cwd`. Returns the exit code: 1 when install left a folder alone.
///
/// # Errors
/// An unknown harness; a folder or file that can't be written or removed.
pub fn run(command: AgentsCommand, ctx: Context, cwd: &Path) -> Result<u8> {
    let version = atm_common::VERSION;
    match command {
        AgentsCommand::Install {
            harness,
            scope,
            copy,
        } => {
            let project = project_dir(&scope, cwd)?;
            let result = install_skills(harness.as_deref(), project.as_deref(), copy, version)?;
            if ctx.json {
                print_json(&json!({"written": result.written, "refused": result.refused}));
            } else {
                let field = |w: &Value, key: &str| w[key].as_str().unwrap_or_default().to_string();
                let mut lines: Vec<String> = result
                    .written
                    .iter()
                    .map(|w| {
                        format!(
                            "{}: {} ({})",
                            field(w, "action"),
                            field(w, "path"),
                            field(w, "how")
                        )
                    })
                    .collect();
                let changed = result
                    .written
                    .iter()
                    .filter(|w| w["action"] != "unchanged")
                    .count();
                lines.push(format!(
                    "{changed} written, {} unchanged",
                    result.written.len() - changed
                ));
                println!("{}", lines.join("\n"));
            }
            for r in &result.refused {
                eprintln!(
                    "atm: left alone: {} {}",
                    r["path"].as_str().unwrap_or_default(),
                    r["reason"].as_str().unwrap_or_default()
                );
            }
            Ok(u8::from(!result.refused.is_empty()))
        }
        AgentsCommand::Status { scope } => {
            let report = skills_status(project_dir(&scope, cwd)?.as_deref(), version);
            if ctx.json {
                print_json(&Value::Array(report));
                return Ok(0);
            }
            let mut lines = Vec::new();
            for h in &report {
                let here = if h["detected"] == true {
                    "installed here"
                } else {
                    "not found here"
                };
                let text = |key: &str| h[key].as_str().unwrap_or_default().to_string();
                lines.push(format!(
                    "{} ({here}), {}: {}",
                    text("label"),
                    text("scope"),
                    text("folder")
                ));
                let skills = h["skills"].as_array().cloned().unwrap_or_default();
                let width = skills
                    .iter()
                    .map(|s| s["skill"].as_str().unwrap_or_default().chars().count())
                    .max()
                    .unwrap_or(0);
                for s in &skills {
                    let skill = s["skill"].as_str().unwrap_or_default();
                    lines.push(format!(
                        "  {skill:<width$}  {}",
                        s["state"].as_str().unwrap_or_default()
                    ));
                }
            }
            println!("{}", lines.join("\n"));
            Ok(0)
        }
        AgentsCommand::Uninstall { harness, scope } => {
            let removed =
                uninstall_skills(harness.as_deref(), project_dir(&scope, cwd)?.as_deref())?;
            if ctx.json {
                print_json(&json!({"removed": removed}));
            } else if removed.is_empty() {
                println!("nothing atm installed to remove");
            } else {
                let lines: Vec<String> = removed.iter().map(|p| format!("removed: {p}")).collect();
                println!("{}", lines.join("\n"));
            }
            Ok(0)
        }
    }
}
