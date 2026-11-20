//! The single maintenance commands (the Python atm's `cli/commands/maintenance.py` and `cli/serve.py`): `atm doctor`,
//! and `atm service`, the CLI's side of the dashboard server: one per user, a detached process serving every
//! registered project (docs/multi-project-api.md). The CLI starts, stops, checks and refreshes it as a process (its
//! record in the user's atm folder, its pid); the server itself is atm-api's, run as `atm _serve <home> <port>`.

use std::path::{Path, PathBuf};

use atm_common::Shown;
use std::process::Stdio;
use std::time::{Duration, Instant};

use atm_lib::Project;

use crate::process::{pid_alive, pid_exists, signal_process};
use atm_common::dashboard::{
    PAGE, SCAN_PORTS, SERVER_LOG, SERVER_RECORD, home_id, page_tags, parse_version, whoami,
};
use atm_common::home::{ensure_home, legacy_folder_of, state_dir};
use atm_common::py::{py_str_opt, to_json};
use atm_common::time::{parse_iso, real_now};
use atm_common::{Error, Result};
use atm_lib::check::check_project;
use serde_json::{Map, Value, json};

use super::{Context, print_json};

/// Held while one `atm service` checks for a server and starts one, so two at once start one.
const SERVER_LOCK: &str = "server.lock";
const SERVER_LOCK_WAIT: Duration = Duration::from_secs(15);
const STOP_WAIT: Duration = Duration::from_secs(5);
const KILL_WAIT: Duration = Duration::from_secs(2);
const START_WAIT: Duration = Duration::from_secs(10);
const POLL: Duration = Duration::from_millis(50);
const REAP_WAIT: Duration = Duration::from_secs(2);

/// `atm doctor [--files]`: the errors and warnings in the task data; exit code 1 when there are errors.
///
/// # Errors
/// When there is no task data at all.
pub fn doctor(files: bool, ctx: Context, cwd: &Path) -> Result<u8> {
    let result = check_project(&Project::open(cwd)?, files)?;
    // Messages name tasks as people know them, by key (ATM-557), in --json too (they are text, not data); before
    // grouping, so the lines of keys are packed by the keys' width
    let labeller = super::Labeller::open(cwd);
    let label = |messages: &[String]| -> Vec<String> {
        messages.iter().map(|m| labeller.text(m)).collect()
    };
    let (errors, warnings) = (&label(&result.errors), &label(&result.warnings));
    if ctx.json {
        print_json(&json!({"errors": errors, "warnings": warnings}));
    } else {
        for e in errors {
            crate::output::emit(&format!("{} {e}", crate::output::error_prefix()));
        }
        for line in warning_lines(warnings, crate::output::width()) {
            crate::output::emit(&line);
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

/// The task IDs a line of grouped warnings holds, in a pipe.
const IDS_PER_LINE: usize = 16;

/// `doctor`'s warnings (docs/cli-output-design.md §4): those about one task each that say the same thing grouped,
/// the message once with the number of tasks, then their IDs; the rest one per line, in the order they came.
#[must_use]
pub fn warning_lines(warnings: &[String], width: Option<usize>) -> Vec<String> {
    // `T-235: its files match …`: the task and what is said of it
    let split = |w: &str| {
        w.split_once(": ")
            .filter(|(id, _)| is_task_reference(id))
            .map(|(id, said)| (id.to_string(), said.to_string()))
    };
    let mut groups: Vec<(String, Vec<String>)> = Vec::new();
    // A group (by its place in `groups`), or a warning on its own
    let mut order: Vec<std::result::Result<usize, String>> = Vec::new();
    for w in warnings {
        match split(w) {
            Some((id, said)) => {
                if let Some(g) = groups.iter().position(|(s, _)| *s == said) {
                    groups[g].1.push(id);
                } else {
                    groups.push((said, vec![id]));
                    order.push(Ok(groups.len() - 1));
                }
            }
            None => order.push(Err(w.clone())),
        }
    }
    let prefix = crate::output::warning_prefix();
    let mut lines = Vec::new();
    for item in order {
        match item {
            Err(w) => lines.push(format!("{prefix} {w}")),
            Ok(g) => {
                let (said, ids) = &groups[g];
                if let [id] = ids.as_slice() {
                    lines.push(format!("{prefix} {id}: {said}"));
                    continue;
                }
                lines.push(format!("{prefix} {said}: {} tasks", ids.len()));
                let per_line = width.map_or(IDS_PER_LINE, |w| {
                    let widest = ids.iter().map(|i| i.chars().count()).max().unwrap_or(1);
                    (w.saturating_sub(2) / (widest + 1)).max(1)
                });
                for chunk in ids.chunks(per_line) {
                    lines.push(format!("  {}", chunk.join(" ")));
                }
            }
        }
    }
    lines
}

/// Whether a warning's first word names a task: an ID or key (`T-235`, `ATM-12`, `~ATM-13`) or a UUID.
fn is_task_reference(word: &str) -> bool {
    if atm_common::ids::valid_task_uuid(word) {
        return true;
    }
    let word = word.strip_prefix('~').unwrap_or(word);
    word.rsplit_once('-').is_some_and(|(prefix, number)| {
        prefix.starts_with(|c: char| c.is_ascii_alphabetic())
            && prefix
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-')
            && !number.is_empty()
            && number.chars().all(|c| c.is_ascii_digit())
    })
}

#[cfg(test)]
mod warning_tests {
    use super::{is_task_reference, warning_lines};

    #[test]
    fn warnings_saying_the_same_of_several_tasks_are_grouped() {
        let w: Vec<String> = [
            "T-1: its files match no component",
            "meta.x: something",
            "T-2: its files match no component",
            "T-3: another thing",
            "T-4: its files match no component",
        ]
        .map(String::from)
        .to_vec();
        assert_eq!(
            warning_lines(&w, None),
            [
                "warning: its files match no component: 3 tasks",
                "  T-1 T-2 T-4",
                "warning: meta.x: something",
                "warning: T-3: another thing",
            ]
        );
        assert_eq!(warning_lines(&w, Some(10))[1..3], ["  T-1 T-2", "  T-4"]);
    }

    #[test]
    fn task_references_are_ids_keys_and_uuids() {
        for yes in [
            "T-1",
            "ATM-538",
            "~ATM-540",
            "T-P1-03",
            "0195f0a4-1b2c-7d3e-8f40-0123456789ab",
        ] {
            assert!(is_task_reference(yes), "{yes}");
        }
        for no in ["meta.x", "tasks.json", "-1", "T-", "a b-1"] {
            assert!(!is_task_reference(no), "{no}");
        }
    }
}

/// What `atm service` is asked to do.
#[derive(Debug, Clone)]
pub struct ServiceArgs {
    /// `start`, `stop` or `status`.
    pub action: String,
    /// Also open the dashboard in the browser.
    pub open: bool,
    /// Show the overview of all projects.
    pub all: bool,
    /// The port to try first.
    pub port: u16,
    /// The project to show.
    pub dir: Option<String>,
    /// Only restart if the dashboard is older.
    pub refresh: bool,
}

/// The dashboard's address: the project's page (`/projects/<id>`, T-429), or the overview.
fn server_url(port: &Value, project: Option<&str>) -> String {
    let port = py_str_opt(Some(port));
    match project {
        Some(project) => format!("http://localhost:{port}/projects/{project}"),
        None => format!("http://localhost:{port}/"),
    }
}

/// Whether whoami's answer is the server `record` names, of this atm folder: the same pid, and `home` this folder's
/// (T-501: a record moved from `~/.agentic-task-manager` names a server an older atm runs for that folder).
fn answers_for(info: &Map<String, Value>, record: &Map<String, Value>, home: &Path) -> bool {
    info.get("pid") == record.get("pid") && info.get("home") == Some(&json!(home_id(home)))
}

fn record_path(home: &Path) -> PathBuf {
    home.join(SERVER_RECORD)
}

fn read_record(home: &Path) -> Option<Map<String, Value>> {
    let text = std::fs::read_to_string(record_path(home)).ok()?;
    serde_json::from_str::<Value>(&text)
        .ok()?
        .as_object()
        .cloned()
}

fn port_of(record: &Map<String, Value>) -> u64 {
    record.get("port").and_then(Value::as_u64).unwrap_or(0)
}

fn pid_of(record: &Map<String, Value>) -> Option<u32> {
    record
        .get("pid")
        .and_then(Value::as_u64)
        .and_then(|p| u32::try_from(p).ok())
}

fn ask(port: u64) -> Option<Map<String, Value>> {
    whoami(port, Duration::from_secs(1))
}

/// The record of the user's running server, or none. A stale record is removed, and so is one naming another atm
/// folder's server.
fn live_server(home: &Path) -> Option<Map<String, Value>> {
    let record = read_record(home)?;
    let info = ask(port_of(&record));
    if info.is_some_and(|i| answers_for(&i, &record, home)) {
        return Some(record);
    }
    let _ = std::fs::remove_file(record_path(home));
    None
}

/// Only one `atm service` at a time looks for a running server and starts one: a folder, which `mkdir` creates for
/// exactly one process. A lock whose holder is gone (its pid isn't running) or that outlived the wait is broken.
struct ServerLock(PathBuf);

impl ServerLock {
    fn take(home: &Path) -> Result<Self> {
        ensure_home(home).map_err(|e| Error::failed(format!("{}: {e}", home.shown())))?;
        let lock = home.join(SERVER_LOCK);
        let deadline = Instant::now() + SERVER_LOCK_WAIT;
        loop {
            match std::fs::create_dir(&lock) {
                Ok(()) => break,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    let holder = std::fs::read_to_string(lock.join("pid.json"))
                        .ok()
                        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                        .and_then(|h| h.get("pid").and_then(Value::as_u64))
                        .and_then(|p| u32::try_from(p).ok());
                    let gone = holder.is_some_and(|pid| !pid_alive(pid));
                    if gone || Instant::now() >= deadline {
                        let _ = std::fs::remove_dir_all(&lock);
                        continue;
                    }
                    std::thread::sleep(POLL);
                }
                Err(e) => return Err(Error::failed(format!("{}: {e}", lock.shown()))),
            }
        }
        let _ = std::fs::write(
            lock.join("pid.json"),
            to_json(&json!({"pid": std::process::id()})),
        );
        Ok(Self(lock))
    }
}

impl Drop for ServerLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Start the server process (this binary, as `atm _serve <home> <port>`, detached) and wait for its record.
fn start_server(home: &Path, port: u64) -> Result<Map<String, Value>> {
    let log_path = home.join(SERVER_LOG);
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|e| Error::failed(format!("{}: {e}", log_path.shown())))?;
    let errors = log
        .try_clone()
        .map_err(|e| Error::failed(format!("{}: {e}", log_path.shown())))?;
    let own_binary = std::env::current_exe()
        .map_err(|e| Error::failed(format!("can't find atm's own binary: {e}")))?;
    let mut command = std::process::Command::new(own_binary);
    command
        .arg("_serve")
        .arg(home)
        .arg(port.to_string())
        .current_dir(home)
        .stdin(Stdio::null())
        .stdout(log)
        .stderr(errors);
    let mut child = crate::commands::detach::spawn_detached(&mut command)
        .map_err(|e| Error::failed(format!("can't start the dashboard server: {e}")))?;
    let deadline = Instant::now() + START_WAIT;
    while Instant::now() < deadline {
        if let Some(record) = read_record(home)
            && pid_of(&record) == Some(child.id())
            && ask(port_of(&record)).is_some()
        {
            return Ok(record);
        }
        if child.try_wait().ok().flatten().is_some() {
            break;
        }
        std::thread::sleep(POLL);
    }
    Err(Error::failed(format!(
        "the dashboard server did not start; see {}",
        log_path.shown()
    )))
}

/// The running server's record, starting the server if none runs: (record, started). Under the server lock, so two
/// at once start one server, and the second gets the first's.
fn ensure_server(home: &Path, port: u64) -> Result<(Map<String, Value>, bool)> {
    let _lock = ServerLock::take(home)?;
    if let Some(record) = live_server(home) {
        return Ok((record, false));
    }
    Ok((start_server(home, port)?, true))
}

/// Start (or reuse) the server; the URL to show: the project's (at or above `cwd`), or the overview's.
fn serve_start(cwd: &Path, port: u16, overview: bool) -> Result<String> {
    let mut project = None;
    if !overview {
        let migration = super::db::maintenance(cwd)?.migrate(false)?;
        for line in &migration.lines {
            if line != "Nothing to migrate." {
                println!("{line}");
            }
        }
        if !migration.tasks_exists {
            return Err(Error::failed(
                "no tasks.json to serve. Run /atm-create-tasks first.",
            ));
        }
        let (id, shown) = Project::open(cwd)?.join_default_workspace()?;
        if shown {
            project = Some(id);
        } else {
            println!(
                "This project isn't listed on the dashboard (meta.dashboard.listed is false)."
            );
        }
    }
    let home = state_dir();
    let (record, started) = ensure_server(&home, u64::from(port))?;
    let url = server_url(
        record.get("port").unwrap_or(&Value::Null),
        project.as_deref(),
    );
    println!(
        "Dashboard server {}: {url}",
        if started {
            "started"
        } else {
            "already running"
        }
    );
    Ok(url)
}

/// How many checks in a row must miss a process before it counts as gone: a busy machine can make one look miss it.
const GONE_CHECKS: u32 = 3;

/// Whether process `pid` ends within `wait`, sending it `signal` again (SIGKILL) each time it is still seen. One that
/// ended is given a moment more to be reaped by its parent, so that whoever started it sees it gone too; one nobody
/// reaps (a container without an init) counts as gone then.
fn wait_gone(pid: u32, wait: Duration, kill: bool) -> bool {
    let deadline = Instant::now() + wait;
    let mut missed = 0;
    while missed < GONE_CHECKS {
        if pid_alive(pid) {
            missed = 0;
            if Instant::now() >= deadline {
                return false;
            }
            if kill {
                let _ = signal_process(pid, true);
            }
        } else {
            missed += 1;
        }
        std::thread::sleep(POLL);
    }
    let reaped = Instant::now() + REAP_WAIT;
    while Instant::now() < reaped && pid_exists(pid) {
        std::thread::sleep(POLL);
    }
    true
}

/// Stop a server of the user's (whose pid whoami confirmed): SIGTERM, then SIGKILL if it outlives the wait. Its record
/// goes only once it is gone, and only if it is that server's: a server still running is never forgotten.
fn stop_server(home: &Path, pid: u32) -> Result<()> {
    // Where there is no SIGTERM to send (Windows), straight to ending it, rather than waiting out STOP_WAIT for nothing
    let asked = signal_process(pid, false);
    if !asked || !wait_gone(pid, STOP_WAIT, false) {
        let _ = signal_process(pid, true);
        if !wait_gone(pid, KILL_WAIT, true) {
            return Err(Error::failed(format!(
                "the dashboard server (pid {pid}) didn't stop; its record is kept"
            )));
        }
    }
    if read_record(home).is_some_and(|r| pid_of(&r) == Some(pid)) {
        let _ = std::fs::remove_file(record_path(home));
    }
    Ok(())
}

/// The atm servers answering on the usual ports (from `first_port`), asked all at once: a port nothing listens on can
/// take the whole timeout to answer (Windows: ATM-482), so one after the other they would add up to seconds.
fn servers_on(first_port: u16) -> Vec<Map<String, Value>> {
    std::thread::scope(|scope| {
        let asks: Vec<_> = (first_port..first_port.saturating_add(SCAN_PORTS))
            .map(|port| scope.spawn(move || whoami(u64::from(port), Duration::from_millis(500))))
            .collect();
        asks.into_iter()
            .filter_map(|ask| ask.join().ok().flatten())
            .collect()
    })
}

/// The servers of this atm folder among `servers` that the record doesn't name: a server whose record was lost or
/// overwritten. A server of another folder isn't this folder's to stop.
fn unrecorded_servers(
    home: &Path,
    servers: &[Map<String, Value>],
    recorded: Option<&Value>,
) -> Vec<Map<String, Value>> {
    let mine = json!(home_id(home));
    servers
        .iter()
        .filter(|info| info.get("home") == Some(&mine) && info.get("pid") != recorded)
        .cloned()
        .collect()
}

/// The servers among `servers` an older atm runs for `~/.agentic-task-manager`, after this state folder took its
/// content (T-501): reported, never stopped. None for a state folder set with `ATM_STATE_DIR` (`legacy_folder_of`),
/// and servers of any other folder aren't this atm's business.
fn older_atms_servers(home: &Path, servers: &[Map<String, Value>]) -> Vec<Map<String, Value>> {
    let Some(legacy) = legacy_folder_of(home) else {
        return Vec::new();
    };
    let theirs = json!(home_id(&legacy));
    servers
        .iter()
        .filter(|info| info.get("home") == Some(&theirs))
        .cloned()
        .collect()
}

fn serve_stop(home: &Path, port: u16) -> Result<()> {
    let record = read_record(home);
    let live = record
        .as_ref()
        .is_some_and(|r| ask(port_of(r)).is_some_and(|i| answers_for(&i, r, home)));
    let recorded = record.as_ref().filter(|_| live).and_then(|r| r.get("pid"));
    let strays = unrecorded_servers(home, &servers_on(port), recorded);
    let Some(record) = record else {
        if strays.is_empty() {
            println!("Dashboard server is not running.");
            return Ok(());
        }
        return stop_strays(home, &strays);
    };
    if !live {
        let _ = std::fs::remove_file(record_path(home));
        if strays.is_empty() {
            println!(
                "Dashboard server is not running (removed a stale record for pid {}).",
                py_str_opt(record.get("pid"))
            );
            return Ok(());
        }
    } else if let Some(pid) = pid_of(&record) {
        stop_server(home, pid)?;
        println!("Dashboard server stopped (pid {pid}).");
    }
    stop_strays(home, &strays)
}

fn stop_strays(home: &Path, strays: &[Map<String, Value>]) -> Result<()> {
    for info in strays {
        if let Some(pid) = pid_of(info) {
            stop_server(home, pid)?;
            println!(
                "Stopped a dashboard server the record didn't name (pid {pid}, port {}).",
                py_str_opt(info.get("port"))
            );
        }
    }
    Ok(())
}

fn real_path(path: &str) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| PathBuf::from(path))
}

/// Whether the server runs an older dashboard than this atm's: an older version, or the same version but another
/// build of this atm's own dashboard (it was rebuilt since the server started). A newer server is left alone, so a
/// session still on an older atm never downgrades it; so is another install of the same version (its record names
/// another `plugin_root`), so installs don't restart the server in turn.
fn outdated(record: &Map<String, Value>) -> bool {
    let (mine, mine_build) = page_tags(PAGE);
    let text = |key: &str| {
        record
            .get(key)
            .filter(|v| !v.is_null())
            .map(|v| py_str_opt(Some(v)))
            .unwrap_or_default()
    };
    let (theirs, theirs_build) = (text("version"), text("build"));
    let (a, b) = (parse_version(&mine), parse_version(&theirs));
    if a != b {
        return a > b;
    }
    let theirs_root = text("plugin_root");
    if !theirs_root.is_empty() {
        let own = std::env::current_exe()
            .map(|p| p.shown())
            .unwrap_or_default();
        if real_path(&theirs_root) != real_path(&own) {
            return false;
        }
    }
    !mine_build.is_empty() && mine_build != theirs_build
}

/// Restart the user's server on the same port if it runs an older dashboard than this atm's. Cheap when there is
/// nothing to do: one read of the record, and no request unless it is outdated. Never starts a server that isn't
/// running. The new record, if it restarted one.
///
/// # Errors
/// The server lock, or a server that wouldn't stop or start.
pub fn refresh_server(home: &Path) -> Result<Option<Map<String, Value>>> {
    let Some(record) = read_record(home) else {
        return Ok(None);
    };
    if !outdated(&record) {
        return Ok(None);
    }
    let _lock = ServerLock::take(home)?;
    let Some(live) = live_server(home) else {
        return Ok(None);
    };
    if let Some(pid) = pid_of(&live) {
        stop_server(home, pid)?;
    }
    start_server(home, port_of(&live)).map(Some)
}

fn serve_status(home: &Path, as_json: bool, port: u16) {
    let record = live_server(home);
    let servers = servers_on(port);
    let strays = unrecorded_servers(home, &servers, record.as_ref().and_then(|r| r.get("pid")));
    let older = older_atms_servers(home, &servers);
    if as_json {
        let mut status = Map::new();
        status.insert("running".into(), json!(record.is_some()));
        if let Some(record) = &record {
            status.extend(record.clone());
        }
        status.insert(
            "url".into(),
            record.as_ref().map_or(Value::Null, |r| {
                json!(server_url(r.get("port").unwrap_or(&Value::Null), None))
            }),
        );
        let unrecorded: Vec<Value> = strays
            .iter()
            .map(|s| json!({"pid": s.get("pid"), "port": s.get("port"), "version": s.get("version")}))
            .collect();
        status.insert("unrecorded".into(), Value::Array(unrecorded));
        let older_atms: Vec<Value> = older
            .iter()
            .map(|s| json!({"pid": s.get("pid"), "port": s.get("port"), "version": s.get("version")}))
            .collect();
        status.insert("older_atms".into(), Value::Array(older_atms));
        print_json(&Value::Object(status));
        return;
    }
    match &record {
        None => println!("Dashboard server is not running."),
        Some(record) => {
            let started = record
                .get("started_at")
                .and_then(Value::as_str)
                .and_then(parse_iso)
                .unwrap_or_else(real_now);
            println!(
                "Dashboard server running: {} (pid {}, dashboard {}, up {})",
                server_url(record.get("port").unwrap_or(&Value::Null), None),
                py_str_opt(record.get("pid")),
                py_str_opt(record.get("version")),
                super::claim::format_duration(real_now() - started)
            );
        }
    }
    for info in &strays {
        println!(
            "Also running, without a record: pid {} on port {} (dashboard {}); `atm service stop` stops it.",
            py_str_opt(info.get("pid")),
            py_str_opt(info.get("port")),
            py_str_opt(info.get("version"))
        );
    }
    for info in &older {
        println!(
            "Also running: an older atm's server, still on ~/.agentic-task-manager: pid {} on port {} (dashboard {}). \
             It shows only the projects that atm listed; upgrade it, or stop it with its own `atm service stop`.",
            py_str_opt(info.get("pid")),
            py_str_opt(info.get("port")),
            py_str_opt(info.get("version"))
        );
    }
}

/// Open `url` in the user's browser (`open` on macOS, `xdg-open` elsewhere): atm's own command, with the URL it made.
fn open_browser(url: &str) {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let started = std::process::Command::new(opener)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    if started.is_err() {
        println!("Could not open a browser; open {url} yourself.");
    }
}

/// `atm service [start|stop|status]`: run the dashboard server in the background.
///
/// # Errors
/// A `--dir` that isn't a folder, no task file to serve, a server that wouldn't start or stop.
pub fn service(args: &ServiceArgs, ctx: Context, cwd: &Path) -> Result<()> {
    let cwd = match &args.dir {
        Some(dir) => {
            let folder = atm_common::home::absolute(&cwd.join(dir));
            if !folder.is_dir() {
                return Err(Error::Invalid(format!("--dir {dir}: not a directory")));
            }
            folder
        }
        None => cwd.to_path_buf(),
    };
    let home = state_dir();
    if args.refresh {
        if ctx.dry_run {
            println!("[dry-run] would refresh the dashboard server if outdated");
        } else if let Some(record) = refresh_server(&home)? {
            println!(
                "Dashboard server restarted with dashboard {}: {}",
                py_str_opt(record.get("version")),
                server_url(record.get("port").unwrap_or(&Value::Null), None)
            );
        }
        return Ok(());
    }
    match args.action.as_str() {
        "start" => {
            if ctx.dry_run {
                println!("[dry-run] would start the dashboard server");
                return Ok(());
            }
            refresh_server(&home)?;
            let url = serve_start(&cwd, args.port, args.all)?;
            if args.open {
                open_browser(&url);
            }
        }
        "stop" => {
            if ctx.dry_run {
                println!("[dry-run] would stop the dashboard server");
                return Ok(());
            }
            serve_stop(&home, args.port)?;
        }
        _ => serve_status(&home, ctx.json, args.port),
    }
    Ok(())
}

/// `atm _serve <home> <port>`: the server process itself (atm-api), until it is stopped.
pub fn serve(home: &Path, port: u16) -> u8 {
    match atm_api::serve(&atm_common::home::absolute(home), port) {
        Ok(()) => 0,
        Err(message) => {
            eprintln!("atm: {message}");
            1
        }
    }
}
