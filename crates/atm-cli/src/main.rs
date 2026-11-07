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
use clap::error::{ContextKind, ContextValue, ErrorKind};
use clap::{Parser, Subcommand};
use commands::{
    Context,
    agents::AgentsCommand,
    claim::{ClaimCommand, WatchArgs},
    component::ComponentCommand,
    db::DbCommand,
    loop_::LoopCommand,
    project::ProjectCommand,
    release::ReleaseCommand,
    sprint::SprintCommand,
    task::TaskCommand,
    vcs::VcsCommand,
};
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
#[allow(clippy::large_enum_variant)] // reason: the command line is parsed once per process, so its size doesn't matter
enum Command {
    /// Task lifecycle commands
    #[command(subcommand)]
    Task(TaskCommand),
    /// Project-level config
    #[command(subcommand)]
    Project(ProjectCommand),
    /// Sprint management
    #[command(subcommand)]
    Sprint(SprintCommand),
    /// Release management
    #[command(subcommand)]
    Release(ReleaseCommand),
    /// Component management
    #[command(subcommand)]
    Component(ComponentCommand),
    /// The work loop
    #[command(subcommand)]
    Loop(LoopCommand),
    /// Claim lifecycle
    #[command(subcommand)]
    Claim(ClaimCommand),
    /// Version control integration
    #[command(subcommand)]
    Vcs(VcsCommand),
    /// The task files' maintenance
    #[command(subcommand)]
    Db(DbCommand),
    /// Install atm's skills where your agent harnesses read them (Claude Code for now)
    #[command(subcommand)]
    Agents(AgentsCommand),
    /// Reference for agents, for this version of atm: the rules, the commands, the states, claims, the task fields.
    Docs {
        /// The topic to print; without one, the list of topics
        topic: Option<String>,
    },
    /// Print the plugin version.
    Version,
    /// Hold the task folder's lock for a while (for the lock's tests).
    #[command(name = "_lock-test", hide = true)]
    LockTest {
        /// How long to hold it, in seconds
        #[arg(default_value_t = 0.0)]
        seconds: f64,
    },
    /// Renew a claim while a shell lives (what `atm claim hold` leaves behind).
    #[command(name = "_claim-watch", hide = true)]
    ClaimWatch(WatchArgs),
}

impl Command {
    /// Whether running it renews the caller's own claims (T-439): not the commands that work on no project, nor the
    /// heartbeat and the watcher, which renew their own.
    fn renews_claims(&self) -> bool {
        !matches!(
            self,
            Command::Docs { .. }
                | Command::Version
                | Command::LockTest { .. }
                | Command::ClaimWatch(_)
                | Command::Db(DbCommand::MergeDriver { .. })
                | Command::Claim(ClaimCommand::Heartbeat { .. })
        )
    }
}

/// Every atm command an agent runs renews that agent's live claims, so they last while it works, in any harness, with
/// no hook (T-439, docs/porting-to-rust.md §4). As `claim heartbeat --auto`: only claims whose last renewal is ten
/// minutes old, so most commands only read. Not for a command run as someone else (`--agent`), and never in the way of
/// the command itself.
fn renew_own_claims(args: &[OsString], cwd: &Path) {
    let as_someone_else = args.iter().take_while(|a| *a != "--").any(|a| {
        let a = a.to_string_lossy();
        a == "--agent" || a.starts_with("--agent=")
    });
    if as_someone_else {
        return;
    }
    if let Ok(project) = atm_boot::open_project(cwd) {
        let actor = atm_boot::agent_identity(None, cwd);
        // No project here, or one atm can't read: the command says so itself
        let _ = atm_lib::claims::renew_due_claims(&mut project.with_actor(Some(actor)));
    }
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

/// What clap found wrong with the line, reported as the Python atm (Click) words it where agents and the tests read
/// it: an unknown option, a missing argument or option, with the command's usage line (not in JSON). Returns the exit
/// code: 0 for help, else 2.
fn parse_error(e: &clap::Error, as_json: bool) -> u8 {
    if matches!(e.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) {
        let _ = e.print();
        return 0;
    }
    let message = match (e.kind(), e.get(ContextKind::InvalidArg)) {
        (ErrorKind::UnknownArgument, Some(ContextValue::String(option)))
            if option.starts_with('-') =>
        {
            Some(format!(
                "No such option: {}",
                option.split('=').next().unwrap_or(option)
            ))
        }
        (ErrorKind::MissingRequiredArgument, Some(ContextValue::Strings(missing))) => {
            missing.first().map(|first| {
                let name = first.split(' ').next().unwrap_or(first);
                if name.starts_with('-') {
                    format!("Missing option '{name}'.")
                } else {
                    format!("Missing argument '{}'.", name.trim_matches(['<', '>']))
                }
            })
        }
        _ => None,
    };
    let rendered = e.render().to_string();
    match message {
        Some(message) => match rendered
            .lines()
            .find(|l| l.starts_with("Usage:"))
            .filter(|_| !as_json)
        {
            Some(usage) => report(&format!("{message}\n{usage}"), 2, false),
            None => report(&message, 2, as_json),
        },
        None if as_json => {
            let first = rendered.lines().next().unwrap_or_default();
            report(first.strip_prefix("error: ").unwrap_or(first), 2, true);
        }
        None => {
            let _ = e.print();
        }
    }
    2
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
        Err(e) => return parse_error(&e, as_json),
    };
    let ctx = Context {
        json: cli.json,
        dry_run: cli.dry_run,
    };
    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(e) => {
            report(&format!("can't read the current folder: {e}"), 1, as_json);
            return 1;
        }
    };
    if cli.command.renews_claims() {
        renew_own_claims(args, &cwd);
    }
    // Each command's exit code when it ends without an error (`agents install` ends with 1 when it left a folder alone,
    // `loop next` with 3 when no task is available)
    let result: Result<u8, Error> = match cli.command {
        Command::Task(command) => commands::task::run(command, ctx, &cwd).map(|()| 0),
        Command::Project(command) => commands::project::run(command, ctx, &cwd).map(|()| 0),
        Command::Sprint(command) => commands::sprint::run(command, ctx, &cwd).map(|()| 0),
        Command::Release(command) => commands::release::run(command, ctx, &cwd).map(|()| 0),
        Command::Component(command) => commands::component::run(command, ctx, &cwd).map(|()| 0),
        Command::Loop(command) => commands::loop_::run(command, ctx, &cwd),
        Command::Claim(command) => commands::claim::run(command, ctx, &cwd).map(|()| 0),
        Command::Vcs(command) => commands::vcs::run(command, ctx, &cwd).map(|()| 0),
        Command::Db(command) => commands::db::run(command, ctx, &cwd),
        Command::Agents(command) => commands::agents::run(command, ctx, &cwd),
        Command::Docs { topic } => commands::meta::docs(topic.as_deref(), ctx).map(|()| 0),
        Command::Version => {
            commands::meta::version(ctx);
            Ok(0)
        }
        Command::LockTest { seconds } => atm_boot::hold_lock_for_test(&cwd, seconds).map(|()| 0),
        Command::ClaimWatch(args) => {
            commands::claim::watch(&args, &cwd);
            Ok(0)
        }
    };
    match result {
        Ok(code) => code,
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
