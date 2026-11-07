//! One module per top-level command (docs/porting-to-rust.md §2): each holds its clap subcommands, its handlers and
//! its area's text. The groups come with their slices (§5); the Python atm's `cli/commands/` has them all.

pub mod meta;
pub mod task;

use atm_common::py::to_json;
use serde_json::Value;

/// What every command gets: how to print its result.
#[derive(Debug, Clone, Copy)]
pub struct Context {
    /// `--json`: print the result as JSON.
    pub json: bool,
}

/// Print a JSON result, as atm prints JSON.
pub fn print_json(value: &Value) {
    println!("{}", to_json(value));
}
