//! `atm version` (and, with their slices, `atm help` and `atm docs`): the commands about atm itself.

use serde_json::json;

use super::{Context, print_json};

/// `atm version`: atm's version, as text or `{"version": …}`.
pub fn version(ctx: Context) {
    if ctx.json {
        print_json(&json!({"version": atm_common::VERSION}));
    } else {
        println!("{}", atm_common::VERSION);
    }
}
