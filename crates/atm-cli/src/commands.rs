//! One module per top-level command (docs/porting-to-rust.md §2): each holds its clap subcommands, its handlers and
//! its area's text. The groups come with their slices (§5); the Python atm's `cli/commands/` has them all.

pub mod agents;
pub mod claim;
pub mod completion;
pub mod component;
pub mod db;
pub mod detach;
pub mod loop_;
pub mod maintenance;
pub mod meta;
pub mod project;
pub mod release;
pub mod sprint;
pub mod task;
pub mod vcs;

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
