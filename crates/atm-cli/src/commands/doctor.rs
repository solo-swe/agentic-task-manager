//! `atm doctor` (the Python atm's `cli/commands/maintenance.py`): check the task data and say what is wrong.

use std::path::Path;

use atm_boot::open_project;
use atm_common::Result;
use atm_lib::check::check_project;
use serde_json::json;

use super::{Context, print_json};

/// `atm doctor [--files]`: the errors and warnings in the task data; exit code 1 when there are errors.
///
/// # Errors
/// When there is no task data at all.
pub fn run(files: bool, ctx: Context, cwd: &Path) -> Result<u8> {
    let result = check_project(&open_project(cwd)?, files)?;
    let (errors, warnings) = (&result.errors, &result.warnings);
    if ctx.json {
        print_json(&json!({"errors": errors, "warnings": warnings}));
    } else {
        for e in errors {
            println!("error: {e}");
        }
        for w in warnings {
            println!("warning: {w}");
        }
        if errors.is_empty() {
            let count = result
                .count
                .map_or_else(|| "None".to_string(), |c| c.to_string());
            let more = if warnings.is_empty() {
                String::new()
            } else {
                format!(", {} warnings", warnings.len())
            };
            println!("tasks.json is fine ({count} tasks{more})");
        }
    }
    Ok(u8::from(!errors.is_empty()))
}
