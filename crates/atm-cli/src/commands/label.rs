//! `atm label`: the tasks' labels (T-499). A label lives on each task that has it; `rename` changes it on all of them.

use std::path::Path;

use atm_common::Result;
use atm_lib::Project;
use atm_lib::edit::relabel;
use clap::Subcommand;
use serde_json::json;

use super::{Context, print_json};

/// The label commands.
#[derive(Debug, Subcommand)]
pub enum LabelCommand {
    /// Rename a label on every task that has it, archived ones too.
    Rename {
        /// The label
        label: String,
        /// Its new name (lowercase letters, digits, '-', '_', at most 32)
        new: String,
    },
}

/// Run a label command in `cwd`.
///
/// # Errors
/// What atm-lib refuses, and a task file that can't be read.
pub fn run(command: LabelCommand, ctx: Context, cwd: &Path) -> Result<()> {
    match command {
        LabelCommand::Rename { label, new } => {
            if ctx.dry_run {
                println!("[dry-run] would rename the label {label} to {new}");
                return Ok(());
            }
            let renamed = relabel(&mut Project::open(cwd)?, &label, &new)?;
            if ctx.json {
                print_json(&json!({
                    "from": label,
                    "to": new,
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
                "Renamed the label {label} to {new} on {n} task{}{archived}.",
                if n == 1 { "" } else { "s" }
            );
        }
    }
    Ok(())
}
