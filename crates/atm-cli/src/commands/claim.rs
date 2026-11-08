//! `atm claim`: claims' lifecycle (the Python atm's `cli/commands/claim.py`): list, heartbeat (with `--auto`, the
//! plugin's hook), and hold (T-437), whose watcher is this binary started detached (`atm _claim-watch`).

use std::io::{IsTerminal, Read};
use std::path::Path;
use std::process::Stdio;

use atm_boot::{agent_identity, hook_project, open_project, process_started, session_identity};
use atm_common::py::{py_str, py_str_opt, truthy};
use atm_common::time::{format_iso, now, parse_iso, real_now};
use atm_common::{Error, Result};
use atm_lib::claims::{expire_claims, renew_due_claims, renew_lease};
use clap::{Args, Subcommand};
use serde_json::{Value, json};

use super::{Context, print_json};

/// `atm claim hold`: how long a watcher may hold a claim, by default and at most, in minutes.
const HOLD_MAX_MINUTES: f64 = 240.0;
const HOLD_LIMIT_MINUTES: f64 = 720.0;

/// The claim commands.
#[derive(Debug, Subcommand)]
pub enum ClaimCommand {
    /// List live claims, shared by all worktrees of this clone.
    List,
    /// Extend your claim's lease.
    Heartbeat {
        /// The task ID
        task_id: Option<String>,
        /// Renew your claims if their last renewal is ten minutes old; silent (the plugin's hook)
        #[arg(long)]
        auto: bool,
        /// Who you are
        #[arg(long)]
        agent: Option<String>,
    },
    /// Keep your claim alive while the shell that runs this lives: `atm claim hold T-1 && npm test`. atm runs nothing:
    /// a watcher renews the claim until that shell ends, the claim is released or finished, or --max is up.
    Hold(HoldArgs),
}

/// `atm claim hold`'s options.
#[derive(Debug, Args)]
pub struct HoldArgs {
    /// The task ID
    task_id: String,
    /// Stop after this long at most (up to 720)
    #[arg(long = "max", value_name = "MINUTES", default_value_t = HOLD_MAX_MINUTES)]
    max_minutes: f64,
    /// Renew this often (default: three times per lease, at least once a minute)
    #[arg(long, value_name = "SECONDS")]
    every: Option<f64>,
    /// Who you are
    #[arg(long)]
    agent: Option<String>,
}

/// `atm _claim-watch`: what `claim hold` leaves behind, as its arguments.
#[derive(Debug, Args)]
pub struct WatchArgs {
    /// The task ID
    task_id: String,
    /// The shell to follow
    #[arg(long)]
    shell: u32,
    /// When that shell started (seconds since 1970)
    #[arg(long)]
    started: u64,
    /// When to stop at the latest (seconds since 1970)
    #[arg(long)]
    until: f64,
    /// Renew this often, in seconds
    #[arg(long)]
    every: f64,
    /// Whose claim it is
    #[arg(long)]
    agent: String,
}

/// Run a claim command in `cwd`.
///
/// # Errors
/// What atm-lib refuses (no live claim, someone else's), bad options.
pub fn run(command: ClaimCommand, ctx: Context, cwd: &Path) -> Result<()> {
    match command {
        ClaimCommand::List => list(ctx, cwd),
        ClaimCommand::Heartbeat {
            task_id,
            auto,
            agent,
        } => heartbeat(task_id.as_deref(), auto, agent.as_deref(), ctx, cwd),
        ClaimCommand::Hold(args) => hold(&args, ctx, cwd),
    }
}

/// `3s`, `5m`, `2h 10m`, as the Python atm's `format_duration`.
pub(crate) fn format_duration(seconds: i64) -> String {
    let seconds = seconds.max(0);
    if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3600 {
        format!("{}m", seconds / 60)
    } else {
        format!("{}h {}m", seconds / 3600, seconds % 3600 / 60)
    }
}

fn list(ctx: Context, cwd: &Path) -> Result<()> {
    let live = expire_claims(&mut open_project(cwd)?)?.claims;
    if ctx.json {
        print_json(&Value::Array(live));
        return Ok(());
    }
    if live.is_empty() {
        println!("No live claims.");
        return Ok(());
    }
    let current = now();
    for c in &live {
        let until = c
            .get("lease_expires_at")
            .and_then(Value::as_str)
            .and_then(parse_iso)
            .unwrap_or(current);
        let branch = c
            .get("branch")
            .filter(|b| truthy(b))
            .map_or_else(|| "-".to_string(), py_str);
        println!(
            "{}  {}  {branch}  {} left  ({})",
            py_str_opt(c.get("task")),
            py_str_opt(c.get("agent")),
            format_duration(until - current),
            py_str_opt(c.get("worktree"))
        );
    }
    Ok(())
}

/// The JSON a Claude Code hook gets on stdin (`{session_id, cwd, …}`), or an empty object.
fn hook_input() -> serde_json::Map<String, Value> {
    let mut stdin = std::io::stdin();
    if stdin.is_terminal() {
        return serde_json::Map::new();
    }
    let mut text = String::new();
    if stdin.read_to_string(&mut text).is_err() {
        return serde_json::Map::new();
    }
    let text = if text.trim().is_empty() { "{}" } else { &text };
    match serde_json::from_str(text) {
        Ok(Value::Object(map)) => map,
        _ => serde_json::Map::new(),
    }
}

/// `heartbeat --auto`, run by the plugin's hook after every tool call (CLI-6.2): renew this agent's live claims once
/// their last renewal is ten minutes old. It never prints and never fails, so a tool call is never disturbed.
fn auto_heartbeat(agent: Option<&str>, cwd: &Path) {
    let payload = hook_input();
    let folder = payload
        .get("cwd")
        .and_then(Value::as_str)
        .map(Path::new)
        .filter(|p| p.is_dir())
        .unwrap_or(cwd);
    let session = payload.get("session_id").filter(|s| truthy(s)).map(py_str);
    let env_agent = std::env::var("ATM_AGENT").is_ok_and(|a| !a.is_empty());
    let actor = match session {
        // The same identity atm commands get in that folder: a subagent in a linked worktree renews its own claims
        Some(session) if agent.is_none_or(str::is_empty) && !env_agent => {
            session_identity(&session, folder)
        }
        _ => agent_identity(agent, cwd),
    };
    if let Some(mut project) = hook_project(folder, actor) {
        let _ = renew_due_claims(&mut project);
    }
}

fn heartbeat(
    task_id: Option<&str>,
    auto: bool,
    agent: Option<&str>,
    ctx: Context,
    cwd: &Path,
) -> Result<()> {
    if ctx.dry_run {
        if auto {
            println!("[dry-run] would renew claims due for it");
        } else {
            println!(
                "[dry-run] would extend the lease on {}",
                task_id.unwrap_or("None")
            );
        }
        return Ok(());
    }
    if auto {
        auto_heartbeat(agent, cwd);
        return Ok(());
    }
    let Some(task_id) = task_id.filter(|t| !t.is_empty()) else {
        return Err(Error::Invalid(
            "atm claim heartbeat needs a task ID (or --auto)".into(),
        ));
    };
    let actor = agent_identity(agent, cwd);
    let mut project = open_project(cwd)?.with_actor(Some(actor));
    let claim = renew_lease(&mut project, task_id, true)?.claim;
    if ctx.json {
        print_json(&claim);
    } else {
        println!(
            "{task_id}: lease extended to {}",
            py_str_opt(claim.get("lease_expires_at"))
        );
    }
    Ok(())
}

/// Start the watcher: this binary again, as `atm _claim-watch`, detached (`detach::spawn_detached`; stdin, stdout and
/// stderr on the null device, so the caller's pipes close when atm exits). Returns its pid.
fn detach(args: &[String], cwd: &Path) -> Result<u32> {
    let own_binary = std::env::current_exe()
        .map_err(|e| Error::failed(format!("can't find atm's own binary: {e}")))?;
    let mut command = std::process::Command::new(own_binary);
    command
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let child = crate::commands::detach::spawn_detached(&mut command)
        .map_err(|e| Error::failed(format!("can't start the claim's watcher: {e}")))?;
    Ok(child.id())
}

fn hold(args: &HoldArgs, ctx: Context, cwd: &Path) -> Result<()> {
    if !(args.max_minutes > 0.0 && args.max_minutes <= HOLD_LIMIT_MINUTES) {
        return Err(Error::Invalid(format!(
            "--max must be more than 0 and at most {HOLD_LIMIT_MINUTES:.1} minutes"
        )));
    }
    if args.every.is_some_and(|e| e <= 0.0) {
        return Err(Error::Invalid("--every must be more than 0 seconds".into()));
    }
    if ctx.dry_run {
        println!(
            "[dry-run] would hold the claim on {} while this shell runs",
            args.task_id
        );
        return Ok(());
    }
    let actor = agent_identity(args.agent.as_deref(), cwd);
    let mut project = open_project(cwd)?.with_actor(Some(actor.clone()));
    // Renewed now, refused before anything else when it isn't the actor's
    let renewed = renew_lease(&mut project, &args.task_id, true)?;
    let interval = args
        .every
        .unwrap_or((renewed.minutes * 60.0 / 3.0).max(60.0));
    // The shell that ran atm: atm's parent (the launchers exec)
    #[cfg(unix)]
    let shell = std::os::unix::process::parent_id();
    #[cfg(not(unix))]
    let shell = 0;
    #[allow(clippy::cast_precision_loss)] // reason: seconds since 1970 fit an f64 exactly
    let deadline = real_now() as f64 + args.max_minutes * 60.0;
    let started = process_started(shell).unwrap_or(0);
    let watcher = detach(
        &[
            "_claim-watch".into(),
            args.task_id.clone(),
            "--shell".into(),
            shell.to_string(),
            "--started".into(),
            started.to_string(),
            "--until".into(),
            deadline.to_string(),
            "--every".into(),
            interval.to_string(),
            "--agent".into(),
            actor,
        ],
        cwd,
    )?;
    #[allow(clippy::cast_possible_truncation)] // reason: seconds since 1970 fit an i64
    let until = format_iso(deadline as i64);
    let held = json!({
        "task": args.task_id,
        "claim": renewed.claim,
        "watcher": watcher,
        "watching": shell,
        "until": until,
    });
    if ctx.json {
        print_json(&held);
    } else {
        println!(
            "{}: holding the claim while this shell runs (pid {shell}), until {until} at most; lease now to {}",
            args.task_id,
            py_str_opt(renewed.claim.get("lease_expires_at"))
        );
    }
    Ok(())
}

/// `atm _claim-watch`: renew the claim every `--every` seconds while the shell lives (the same pid, started at the same
/// time), until `--until`; stop at the first renewal that fails (the claim was released, finished or taken over).
pub fn watch(args: &WatchArgs, cwd: &Path) {
    #[allow(clippy::cast_precision_loss)] // reason: seconds since 1970 fit an f64 exactly
    let clock = || {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(real_now() as f64, |d| d.as_secs_f64())
    };
    loop {
        let left = args.until - clock();
        if left <= 0.0 {
            return;
        }
        std::thread::sleep(std::time::Duration::from_secs_f64(args.every.min(left)));
        if clock() >= args.until || process_started(args.shell) != Some(args.started) {
            return;
        }
        let renewed = open_project(cwd).and_then(|project| {
            renew_lease(
                &mut project.with_actor(Some(args.agent.clone())),
                &args.task_id,
                false,
            )
        });
        if renewed.is_err() {
            return; // released, finished, taken over, or the project is gone: the claim lapses as usual
        }
    }
}

#[cfg(test)]
mod tests {
    use super::format_duration;

    #[test]
    fn durations_read_as_the_python_atm_writes_them() {
        assert_eq!(format_duration(-5), "0s");
        assert_eq!(format_duration(59), "59s");
        assert_eq!(format_duration(600), "10m");
        assert_eq!(format_duration(7800), "2h 10m");
    }
}
