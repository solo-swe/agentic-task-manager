//! The `atm` binary: the command tree of docs/improving-atm-cli-ergonomics.md, parsed with clap, rendered as text or
//! `--json`, with atm's exit codes. Its commands call atm-lib through atm-boot, never the store.
//!
//! Part of atm's Rust port (T-415): docs/porting-to-rust.md (§5 lists the slices in the order they come), the Python
//! atm being the spec until the switch (tests/scripts, run with `ATM_IMPL=rust`).

use clap::{Parser, Subcommand};

/// atm, the Agentic Task Manager's command line.
#[derive(Parser)]
#[command(name = "atm", disable_version_flag = true)]
struct Cli {
    /// Print the result as JSON
    #[arg(short = 'j', long = "json", global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print atm's version
    Version,
}

/// `{"version": …}` as atm prints JSON: two-space indent, as Python's `json.dumps(…, indent=2)`
fn version_json(version: &str) -> String {
    format!("{{\n  \"version\": \"{version}\"\n}}")
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::Version => {
            let version = atm_common::VERSION;
            println!(
                "{}",
                if cli.json {
                    version_json(version)
                } else {
                    version.to_string()
                }
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::version_json;

    #[test]
    fn version_json_is_indented_as_python_prints_it() {
        assert_eq!(version_json("1.2.3"), "{\n  \"version\": \"1.2.3\"\n}");
    }
}
