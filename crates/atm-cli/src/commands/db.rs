//! `atm db`: the task files' maintenance (the Python atm's `cli/commands/db.py`). The Rust atm has `db merge-driver`
//! so far; migrate, init, archive and unarchive come with T-476.

use std::path::Path;

use atm_boot::merge_versions;
use atm_common::Result;
use clap::Subcommand;
use serde_json::json;

use super::{Context, print_json};

/// The task files' maintenance commands.
#[derive(Debug, Subcommand)]
pub enum DbCommand {
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
}

/// Run a `db` command in `cwd`; its exit code (1 when a merge kept conflicting edits).
///
/// # Errors
/// What the store refuses.
pub fn run(command: DbCommand, ctx: Context, cwd: &Path) -> Result<u8> {
    match command {
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
    }
}
