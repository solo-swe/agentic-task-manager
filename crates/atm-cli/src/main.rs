//! The `atm` binary: the command tree of docs/improving-atm-cli-ergonomics.md, parsed with clap, rendered as text or
//! `--json`, with atm's exit codes. Its commands call atm-lib through atm-boot, never the store.
//!
//! Part of atm's Rust port (T-415): docs/porting-to-rust.md (§5 lists the slices in the order they come), the Python
//! atm being the spec until the switch (tests/scripts, run with `ATM_IMPL=rust`). This file holds only the root of
//! the tree, the options of the whole tree, the dispatch and the exit codes (§2); the commands are in `commands/`.

mod commands;

use std::ffi::OsString;
use std::path::Path;
use std::process::ExitCode;

use atm_common::Error;
use atm_common::py::to_json;
use clap::error::ErrorKind;
use clap::{Parser, Subcommand};
use commands::{Context, task::TaskCommand};
use serde_json::json;

/// atm, the Agentic Task Manager's command line: its commands, by group. `atm <command> --help` describes one.
///
/// Every command takes --json (-j) and --dry-run (-n), anywhere on the line. atm --dir PATH <command> runs as if
/// started in PATH.
#[derive(Debug, Parser)]
#[command(name = "atm", disable_version_flag = true)]
struct Cli {
    /// Print result as JSON
    #[arg(short = 'j', long = "json", global = true)]
    json: bool,

    /// Don't execute, just show what would be done
    #[arg(short = 'n', long = "dry-run", global = true)]
    dry_run: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Task lifecycle commands
    #[command(subcommand)]
    Task(TaskCommand),
    /// Print the plugin version.
    Version,
}

/// An error as atm reports it: `atm: <message>` on stderr, or `{"error", "code"}` with `--json`.
fn report(message: &str, code: u8, as_json: bool) {
    if as_json {
        eprintln!("{}", to_json(&json!({"error": message, "code": code})));
    } else {
        eprintln!("atm: {message}");
    }
}

/// `atm --dir PATH <command> …` (or `--dir=PATH`, T-351): the folder to run in, and the rest. Only before the
/// command: `atm service --dir PATH` keeps its own meaning.
fn leading_dir(args: &[OsString]) -> (Option<OsString>, &[OsString]) {
    match args.first() {
        Some(first) if first == "--dir" => (
            Some(args.get(1).cloned().unwrap_or_default()),
            args.get(2..).unwrap_or(&[]),
        ),
        Some(first) if first.to_string_lossy().starts_with("--dir=") => (
            Some(OsString::from(&first.to_string_lossy()["--dir=".len()..])),
            &args[1..],
        ),
        _ => (None, args),
    }
}

/// Whether the line asks for JSON (`--json` or `-j` before any `--`): errors are reported as JSON then, even those
/// clap finds before it knows the options.
fn wants_json(args: &[OsString]) -> bool {
    args.iter()
        .take_while(|a| *a != "--")
        .any(|a| a == "--json" || a == "-j")
}

fn run(args: &[OsString]) -> u8 {
    let (target, args) = leading_dir(args);
    let as_json = wants_json(args);
    if let Some(target) = target {
        // As if started in that folder: the project is found from there, and relative paths count from there
        let folder = Path::new(&target);
        if target.is_empty() || !folder.is_dir() || std::env::set_current_dir(folder).is_err() {
            report(
                &format!("--dir: no folder '{}'", target.to_string_lossy()),
                2,
                as_json,
            );
            return 2;
        }
    }
    let cli = match Cli::try_parse_from(
        std::iter::once(OsString::from("atm")).chain(args.iter().cloned()),
    ) {
        Ok(cli) => cli,
        Err(e) => {
            if matches!(e.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) {
                let _ = e.print();
                return 0;
            }
            if as_json {
                let rendered = e.render().to_string();
                let first = rendered.lines().next().unwrap_or_default();
                report(first.strip_prefix("error: ").unwrap_or(first), 2, true);
            } else {
                let _ = e.print();
            }
            return 2;
        }
    };
    let ctx = Context { json: cli.json };
    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(e) => {
            report(&format!("can't read the current folder: {e}"), 1, as_json);
            return 1;
        }
    };
    let result: Result<(), Error> = match cli.command {
        Command::Task(command) => commands::task::run(command, ctx, &cwd),
        Command::Version => {
            commands::meta::version(ctx);
            Ok(())
        }
    };
    match result {
        Ok(()) => 0,
        Err(error) => {
            report(&error.to_string(), error.code(), as_json);
            error.code()
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    ExitCode::from(run(&args))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<OsString> {
        list.iter().map(OsString::from).collect()
    }

    #[test]
    fn dir_is_read_only_before_the_command() {
        let line = args(&["--dir", "x", "task", "details"]);
        let (dir, rest) = leading_dir(&line);
        assert_eq!(dir, Some(OsString::from("x")));
        assert_eq!(rest, &args(&["task", "details"])[..]);
        let line = args(&["--dir=y", "version"]);
        assert_eq!(leading_dir(&line).0, Some(OsString::from("y")));
        let line = args(&["--dir"]);
        assert_eq!(leading_dir(&line), (Some(OsString::new()), &[][..]));
        let line = args(&["service", "--dir", "x"]);
        assert_eq!(leading_dir(&line).0, None);
    }

    #[test]
    fn json_is_wanted_anywhere_before_a_double_dash() {
        assert!(wants_json(&args(&["task", "details", "T-1", "-j"])));
        assert!(!wants_json(&args(&["x", "--", "--json"])));
    }

    #[test]
    fn global_options_go_anywhere_on_the_line() {
        let cli = Cli::try_parse_from(["atm", "task", "details", "T-1", "--json", "-n"]).unwrap();
        assert!(cli.json && cli.dry_run);
        let cli = Cli::try_parse_from(["atm", "-j", "version"]).unwrap();
        assert!(cli.json);
    }

    #[test]
    fn a_missing_folder_for_dir_is_a_usage_error() {
        assert_eq!(run(&args(&["--dir", "/no/such/folder", "version"])), 2);
        assert_eq!(run(&args(&["--dir", "", "version"])), 2);
    }

    #[test]
    fn an_unknown_command_is_a_usage_error() {
        assert_eq!(run(&args(&["no-such-command"])), 2);
        assert_eq!(run(&args(&["no-such-command", "--json"])), 2);
    }

    #[test]
    fn help_exits_zero() {
        assert_eq!(run(&args(&["--help"])), 0);
    }
}
