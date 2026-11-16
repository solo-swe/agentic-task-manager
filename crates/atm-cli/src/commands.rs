//! One module per top-level command (docs/porting-to-rust.md §2): each holds its clap subcommands, its handlers and
//! its area's text. The groups come with their slices (§5); the Python atm's `cli/commands/` has them all.

pub mod agents;
pub mod claim;
pub mod completion;
pub mod component;
pub mod db;
pub mod detach;
pub mod label;
pub mod loop_;
pub mod maintenance;
pub mod meta;
pub mod project;
pub mod release;
pub mod sprint;
pub mod task;
pub mod vcs;
pub mod workspace;

use atm_common::py::to_json;
use serde_json::Value;

/// What every command gets: how to print its result, and whether to only say what it would do.
#[derive(Debug, Clone, Copy)]
pub struct Context {
    /// `--json`: print the result as JSON.
    pub json: bool,
    /// `--dry-run`: say what would be done, and do nothing.
    pub dry_run: bool,
}

/// Print a JSON result, as atm prints JSON.
pub fn print_json(value: &Value) {
    println!("{}", to_json(value));
}

/// The stored IDs of the tasks these references name (ADR-007 §5, `Project::resolve_task`): in a keys-format project a
/// key (`ATM-527`), an earlier prefix (`T-527`), an alias or a UUID; before task keys, the IDs as given. A project that
/// can't be opened gives them back as given, for the command to report.
///
/// # Errors
/// A reference that names no task, or several, in a keys-format project.
pub fn resolved(cwd: &std::path::Path, references: &[String]) -> atm_common::Result<Vec<String>> {
    match atm_lib::Project::open(cwd) {
        Ok(project) => project.resolve_tasks(references),
        Err(_) => Ok(references.to_vec()),
    }
}

/// [`resolved`] for one.
///
/// # Errors
/// As [`resolved`].
pub fn resolved_one(cwd: &std::path::Path, reference: &str) -> atm_common::Result<String> {
    Ok(resolved(cwd, &[reference.to_string()])?.remove(0))
}
