//! The `atm` binary: the command tree of docs/improving-atm-cli-ergonomics.md, parsed with clap, rendered as text or
//! `--json`, with atm's exit codes. Its commands call atm-lib (which opens the project), never the store.
//!
//! Part of atm's Rust port (T-415): docs/porting-to-rust.md (§5 lists the slices in the order they come), the Python
//! atm being the spec until the switch (tests/scripts, run with `ATM_IMPL=rust`). This file holds only the root of
//! the tree, the options of the whole tree, the dispatch and the exit codes (§2); the commands are in `commands/`.

// The doc comments on the commands, their arguments and options are their help text, shown as written: the lint's
// backticks would show too (`in_progress`, `ATM_AGENT`)
#![allow(clippy::doc_markdown)]

// Everything atm prints for people goes through `output::for_people`: no control character of task text reaches the
// terminal (`clean`, I30), and a task named by its UUID shows by its key (ATM-557). These shadow std's macros in the
// whole crate (defined before its modules, as macros are scoped by their place); JSON is printed with std's own.
macro_rules! println {
    () => { ::std::println!() };
    ($($arg:tt)*) => { ::std::println!("{}", crate::output::for_people(&::std::format!($($arg)*))) };
}
macro_rules! print {
    ($($arg:tt)*) => { ::std::print!("{}", crate::output::for_people(&::std::format!($($arg)*))) };
}
macro_rules! eprintln {
    () => { ::std::eprintln!() };
    ($($arg:tt)*) => { ::std::eprintln!("{}", crate::output::for_people(&::std::format!($($arg)*))) };
}
macro_rules! eprint {
    ($($arg:tt)*) => { ::std::eprint!("{}", crate::output::for_people(&::std::format!($($arg)*))) };
}

mod commands;
mod option_file;
mod output;
mod process;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use atm_common::Error;
use atm_common::py::to_json;
use clap::error::ErrorKind;
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use commands::{
    Context,
    agents::AgentsCommand,
    claim::{ClaimCommand, WatchArgs},
    component::ComponentCommand,
    db::DbCommand,
    inbox::InboxCommand,
    label::LabelCommand,
    loop_::LoopCommand,
    project::ProjectCommand,
    release::ReleaseCommand,
    review::ReviewCommand,
    sprint::SprintCommand,
    task::TaskCommand,
    vcs::VcsCommand,
    workspace::WorkspaceCommand,
};
use serde_json::json;

/// atm, the Agentic Task Manager's command line: its commands, by group. `atm <command> --help` describes one.
///
/// Every command takes --json (-j) and --dry-run (-n), anywhere on the line. atm --dir PATH <command> runs as if
/// started in PATH.
#[derive(Debug, Parser)]
#[command(
    name = "atm",
    disable_version_flag = true,
    disable_help_subcommand = true
)]
struct Cli {
    /// Print result as JSON
    #[arg(short = 'j', long = "json", global = true)]
    json: bool,

    /// Don't execute, just show what would be done
    #[arg(short = 'n', long = "dry-run", global = true)]
    dry_run: bool,

    /// When to style the output: auto (on a terminal, without NO_COLOR), always or never
    #[arg(
        long = "color",
        global = true,
        value_enum,
        default_value_t,
        value_name = "WHEN",
        hide_possible_values = true,
        hide_default_value = true
    )]
    color: output::ColorWhen,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
#[allow(clippy::large_enum_variant)] // reason: the command line is parsed once per process, so its size doesn't matter
enum Command {
    /// Task lifecycle commands
    #[command(subcommand)]
    Task(TaskCommand),
    /// The work loop
    #[command(subcommand)]
    Loop(LoopCommand),
    /// Claim lifecycle
    #[command(subcommand)]
    Claim(ClaimCommand),
    /// Code reviews recorded in the project (CR-1, CR-2, …): open, comment, reply, decide
    #[command(subcommand)]
    Review(ReviewCommand),
    /// Version control integration
    #[command(subcommand)]
    Vcs(VcsCommand),
    /// Release management
    #[command(subcommand)]
    Release(ReleaseCommand),
    /// Component management
    #[command(subcommand)]
    Component(ComponentCommand),
    /// The tasks' labels
    #[command(subcommand)]
    Label(LabelCommand),
    /// The project's inbox: what a person notes for later, triaged into tasks (IN-1, IN-2, …)
    #[command(subcommand)]
    Inbox(InboxCommand),
    /// Project-level config
    #[command(subcommand)]
    Project(ProjectCommand),
    /// Sprint management
    #[command(subcommand)]
    Sprint(SprintCommand),
    /// The workspaces: named lists of projects the dashboard shows
    #[command(subcommand)]
    Workspace(WorkspaceCommand),
    /// The task files' maintenance
    #[command(subcommand)]
    Db(DbCommand),
    /// Validate tasks.json: schema, vocabularies, dependencies.
    Doctor {
        /// Also warn about missing files
        #[arg(long)]
        files: bool,
    },
    /// Run the dashboard server in the background.
    Service {
        /// start, stop or status
        #[arg(default_value = "start", value_parser = ["start", "stop", "status"])]
        action: String,
        /// Also open the dashboard in the browser
        #[arg(long)]
        open: bool,
        /// Show all projects overview
        #[arg(long)]
        all: bool,
        /// The port to try first
        #[arg(long, default_value_t = atm_common::dashboard::DEFAULT_PORT)]
        port: u16,
        /// The project to show
        #[arg(long)]
        dir: Option<String>,
        /// Only restart if dashboard is older
        #[arg(long)]
        refresh: bool,
    },
    /// The dashboard server itself (what `atm service` starts).
    #[command(name = "_serve", hide = true)]
    Serve {
        /// The user's atm folder
        home: PathBuf,
        /// The first port to try
        port: u16,
    },
    /// Print the line that turns on shell completion (add it to your shell's startup file).
    Completion {
        /// bash, zsh, fish or powershell
        #[arg(value_parser = ["bash", "zsh", "fish", "powershell"])]
        shell: String,
    },
    /// Install atm's skills where your agent harnesses read them (Claude Code, Codex, Copilot, Cursor, OpenCode,
    /// Gemini CLI, Antigravity)
    #[command(subcommand)]
    Agents(AgentsCommand),
    /// Show the list of commands.
    Help,
    /// Reference for agents, for this version of atm: the rules, the commands, the states, claims, the task fields.
    Docs {
        /// The topic to print; without one, the list of topics
        topic: Option<String>,
    },
    /// Print atm's version.
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
    /// Whether its `--agent` is a filter, not who runs it (ATM-679, audit C13): `task list --agent`, the old name of
    /// `--started-by`. Its caller is the one running it, whose claims it renews.
    fn agent_is_a_filter(&self) -> bool {
        matches!(self, Command::Task(commands::task::TaskCommand::List(_)))
    }

    /// Whether running it renews the caller's own claims (T-439): not the commands that work on no project, nor the
    /// heartbeat and the watcher, which renew their own.
    fn renews_claims(&self) -> bool {
        self.refreshes_server() && !matches!(self, Command::Claim(ClaimCommand::Heartbeat { .. }))
    }

    /// Whether an agent running it is reminded that it isn't watching its reviews (ATM-642): the commands it works
    /// with, not the watch or a wait on a review (they are the watching), nor the hook's heartbeat.
    fn reminds_to_watch(&self) -> bool {
        self.renews_claims() && !matches!(self, Command::Review(ReviewCommand::Listen(_)))
    }

    /// Whether running it first brings an outdated dashboard server along (after an update, the first atm command
    /// restarts it on the new dashboard): every command that works on a project, the hook's heartbeat too.
    fn refreshes_server(&self) -> bool {
        !matches!(
            self,
            Command::Docs { .. }
                | Command::Version
                | Command::Help
                | Command::Completion { .. }
                | Command::LockTest { .. }
                | Command::ClaimWatch(_)
                | Command::Service { .. }
                | Command::Serve { .. }
                | Command::Db(DbCommand::MergeDriver { .. })
        )
    }
}

/// Every atm command an agent runs renews that agent's live claims, so they last while it works, in any harness, with
/// no hook (T-439, docs/porting-to-rust.md §4). As `claim heartbeat --auto`: only claims whose last renewal is ten
/// minutes old, so most commands only read. Not for a command run as someone else (`--agent`), and never in the way of
/// the command itself.
fn renew_own_claims(cwd: &Path) {
    if let Ok(project) = atm_lib::Project::open(cwd) {
        let actor = atm_lib::actor::agent_identity(None, cwd);
        // No project here, or one atm can't read: the command says so itself
        let _ = atm_lib::claims::renew_due_claims(&mut project.with_actor(Some(actor)));
    }
}

/// An error as atm reports it: `atm: <message>` on stderr, or `{"error", "code"}` with `--json`.
fn report(message: &str, code: u8, as_json: bool) {
    if as_json {
        // Exactly, as print_json prints
        ::std::eprintln!("{}", to_json(&json!({"error": message, "code": code})));
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

/// What clap found wrong with the line, in clap's words, as atm reports errors: `atm: <message>` and clap's tip and
/// usage line on stderr, or `{"error", "code"}` with `--json`. Help (`--help`, or a command or group given without
/// what it needs, `atm` alone) goes to stdout. Returns the exit code: 0 for help, else 2.
fn parse_error(e: &clap::Error, as_json: bool) -> u8 {
    match e.kind() {
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
            let _ = e.print();
            return 0;
        }
        ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => {
            print!("{}", e.render());
            return 0;
        }
        _ => {}
    }
    let rendered = e.render().to_string();
    let text = rendered.strip_prefix("error: ").unwrap_or(&rendered);
    if as_json {
        // The message without the tip and the usage: its first paragraph, on one line
        let first = text.split("\n\n").next().unwrap_or_default();
        let message: Vec<&str> = first
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        report(&message.join(" "), 2, true);
    } else {
        eprint!("atm: {text}");
    }
    2
}

/// The command tree, as clap parses it and shell completion reads it: the derived tree, with no `help` subcommand in
/// any group (`atm help` is atm's own) and atm's values completed (task IDs, sprints, statuses, …).
fn cli_command() -> clap::Command {
    commands::completion::settle(Cli::command(), "")
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
    let parsed = cli_command()
        .try_get_matches_from(std::iter::once(OsString::from("atm")).chain(args.iter().cloned()))
        .and_then(|matches| Cli::from_arg_matches(&matches));
    let cli = match parsed {
        Ok(cli) => cli,
        Err(e) => return parse_error(&e, as_json),
    };
    output::init(cli.color, cli.json);
    let ctx = Context {
        json: cli.json,
        dry_run: cli.dry_run,
    };
    // On Windows the real, long form (no 8.3 short names), as the project's root is: atm-common's `pathtext::real`
    let cwd = match std::env::current_dir() {
        Ok(cwd) => atm_common::pathtext::real(&cwd),
        Err(e) => {
            report(&format!("can't read the current folder: {e}"), 1, as_json);
            return 1;
        }
    };
    // The project whose task keys the printed lines use for its tasks' UUIDs (ATM-557)
    output::set_project_dir(&cwd);
    if cli.command.refreshes_server() {
        // Never in the way of the command itself
        let _ = commands::maintenance::refresh_server(&atm_common::home::state_dir());
    }
    // Run as someone else: `--agent`, unless the command's `--agent` is a filter (ATM-679)
    let other = as_someone_else(args) && !cli.command.agent_is_a_filter();
    // An agent harness's session never acts as a person (ATM-660): `--agent` (unless it is a filter) or ATM_AGENT
    let named = agent_arg(args).filter(|_| !cli.command.agent_is_a_filter());
    if let Err(error) = atm_lib::actor::check_named(named.as_deref(), &cwd) {
        report(&error.to_string(), error.code(), as_json);
        return error.code();
    }
    if cli.command.renews_claims() && !other {
        renew_own_claims(&cwd);
    }
    let reminds = cli.command.reminds_to_watch() && !other;
    let code = match dispatch(cli.command, ctx, &cwd) {
        Ok(code) => code,
        Err(error) => {
            report(&labelled(&error.to_string(), &cwd), error.code(), as_json);
            error.code()
        }
    };
    if reminds {
        remind_to_watch(&cwd);
    }
    code
}

/// An agent that runs a command with a review of its work open and no watch of its reviews running is told so, once
/// in a while, on stderr (ATM-642, `review_listen::reminder`): comments, pokes and decisions would go unseen. After
/// the command, so its own output comes first; never with `--agent` (run as someone else), never in its way.
fn remind_to_watch(cwd: &Path) {
    if let Ok(project) = atm_lib::Project::open(cwd) {
        let me = atm_lib::actor::agent_identity(None, cwd);
        if let Some(line) = atm_lib::review_listen::reminder(&project, &me) {
            eprintln!("atm: {line}");
        }
        // What it was told and hasn't read, on every command until it reads it (ATM-708): a watch's lines that went
        // to a file it never opened are still in front of it
        if let Some(line) = atm_lib::review_listen::unread_line(&project, &me) {
            eprintln!("atm: {line}");
        }
    }
}

/// Whether a command is run as someone else (`--agent`, before any `--`); the caller decides from the parsed command
/// whether its `--agent` is a filter instead ([`Command::agent_is_a_filter`]).
fn as_someone_else(args: &[OsString]) -> bool {
    agent_arg(args).is_some()
}

/// The name `--agent` gives (before any `--`): `--agent NAME` or `--agent=NAME`; an empty one when it has none.
fn agent_arg(args: &[OsString]) -> Option<String> {
    let args: Vec<String> = args
        .iter()
        .take_while(|a| *a != "--")
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    args.iter().enumerate().find_map(|(i, a)| {
        if a == "--agent" {
            Some(args.get(i + 1).cloned().unwrap_or_default())
        } else {
            a.strip_prefix("--agent=").map(String::from)
        }
    })
}

/// Run one command in `cwd`: its exit code when it ends without an error (`agents install` ends with 1 when it left a
/// folder alone, `loop next` with 3 when no task is available).
fn dispatch(command: Command, ctx: Context, cwd: &Path) -> Result<u8, Error> {
    let cwd = cwd.to_path_buf();
    match command {
        Command::Task(command) => commands::task::run(command, ctx, &cwd).map(|()| 0),
        Command::Project(command) => commands::project::run(command, ctx, &cwd).map(|()| 0),
        Command::Sprint(command) => commands::sprint::run(command, ctx, &cwd).map(|()| 0),
        Command::Workspace(command) => commands::workspace::run(command, ctx, &cwd).map(|()| 0),
        Command::Release(command) => commands::release::run(command, ctx, &cwd).map(|()| 0),
        Command::Component(command) => commands::component::run(command, ctx, &cwd).map(|()| 0),
        Command::Label(command) => commands::label::run(command, ctx, &cwd).map(|()| 0),
        Command::Inbox(command) => commands::inbox::run(command, ctx, &cwd).map(|()| 0),
        Command::Loop(command) => commands::loop_::run(command, ctx, &cwd),
        Command::Claim(command) => commands::claim::run(command, ctx, &cwd).map(|()| 0),
        Command::Review(command) => commands::review::run(command, ctx, &cwd),
        Command::Vcs(command) => commands::vcs::run(command, ctx, &cwd).map(|()| 0),
        Command::Db(command) => commands::db::run(command, ctx, &cwd),
        Command::Doctor { files } => commands::maintenance::doctor(files, ctx, &cwd),
        Command::Service {
            action,
            open,
            all,
            port,
            dir,
            refresh,
        } => {
            let args = commands::maintenance::ServiceArgs {
                action,
                open,
                all,
                port,
                dir,
                refresh,
            };
            commands::maintenance::service(&args, ctx, &cwd).map(|()| 0)
        }
        Command::Serve { home, port } => Ok(commands::maintenance::serve(&home, port)),
        Command::Agents(command) => commands::agents::run(command, ctx, &cwd),
        Command::Help => {
            print!("{}", cli_command().render_long_help());
            Ok(0)
        }
        Command::Completion { shell } => {
            println!("{}", commands::completion::activation(&shell));
            Ok(0)
        }
        Command::Docs { topic } => commands::meta::docs(topic.as_deref(), ctx).map(|()| 0),
        Command::Version => {
            commands::meta::version(ctx);
            Ok(0)
        }
        Command::LockTest { seconds } => commands::db::maintenance(&cwd)
            .and_then(|m| m.hold_lock_for_test(seconds))
            .map(|()| 0),
        Command::ClaimWatch(args) => {
            commands::claim::watch(&args, &cwd);
            Ok(0)
        }
    }
}

/// A message as people read it: its task UUIDs as labels (`commands::labelled`).
fn labelled(message: &str, cwd: &Path) -> String {
    commands::labelled(cwd, message)
}

/// atm's own stack: 8 MiB, what Linux and macOS give a main thread. Windows gives one 1 MiB, which a debug build's
/// commands outgrew in 0.41.0 (every command overflowed it, in CI's Windows suite); the same everywhere, so no platform
/// runs out first.
const STACK: usize = 8 * 1024 * 1024;

fn main() -> ExitCode {
    let work = std::thread::Builder::new()
        .name("atm".into())
        .stack_size(STACK)
        .spawn(|| {
            // `COMPLETE=<shell> atm …`: the shell asks for completions (`atm completion <shell>` prints the line that
            // does)
            clap_complete::CompleteEnv::with_factory(cli_command).complete();
            let args: Vec<OsString> = std::env::args_os().skip(1).collect();
            run(&args)
        });
    // A panic was told on stderr by the hook, as before; it fails the command
    match work.map(std::thread::JoinHandle::join) {
        Ok(Ok(code)) => ExitCode::from(code),
        _ => ExitCode::FAILURE,
    }
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
