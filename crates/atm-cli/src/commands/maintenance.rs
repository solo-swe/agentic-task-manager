//! The single maintenance commands (the Python atm's `cli/commands/maintenance.py` and `cli/serve.py`): `atm doctor`,
//! and `atm service`, the CLI's side of the dashboard server: one per user, a detached process serving every
//! registered project (docs/multi-project-api.md). The CLI starts, stops, checks and refreshes it as a process (its
//! record in the user's atm folder, its pid); the server itself is atm-api's, run as `atm _serve <home> <port>`.

use std::path::{Path, PathBuf};

use atm_common::Shown;
use std::process::Stdio;
use std::time::{Duration, Instant};

use atm_lib::Project;

use crate::process::{Running, is_atm_server, pid_exists};
use atm_common::dashboard::{
    PAGE, SCAN_PORTS, SERVER_LOG, SERVER_RECORD, home_id, page_tags, parse_version, whoami,
};
use atm_common::home::{ensure_home, legacy_folder_of, state_dir};
use atm_common::py::py_str_opt;
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

/// Only one `atm service` at a time looks for a running server and starts one: the OS's lock on a file of the state
/// folder (ATM-673, audit C7). The OS lets it go when its holder ends, however it ends, so there is never a stale lock
/// to break, nor two runs breaking one and both starting a server (as the folder lock let them).
struct ServerLock(#[allow(dead_code)] std::fs::File); // reason: held for its lock, let go when dropped

impl ServerLock {
    fn take(home: &Path) -> Result<Self> {
        ensure_home(home).map_err(|e| Error::failed(format!("{}: {e}", home.shown())))?;
        let path = home.join(SERVER_LOCK);
        let failed = |e: &dyn std::fmt::Display| Error::failed(format!("{}: {e}", path.shown()));
        // A folder an older atm's lock left there: no one holds it now that this lock is a file
        if path.is_dir() {
            let _ = std::fs::remove_dir_all(&path);
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .map_err(|e| failed(&e))?;
        let deadline = Instant::now() + SERVER_LOCK_WAIT;
        loop {
            match file.try_lock() {
                Ok(()) => return Ok(Self(file)),
                Err(std::fs::TryLockError::WouldBlock) if Instant::now() < deadline => {
                    std::thread::sleep(POLL);
                }
                Err(std::fs::TryLockError::WouldBlock) => {
                    return Err(failed(
                        &"another `atm service` has been starting the server for too long; try again",
                    ));
                }
                Err(std::fs::TryLockError::Error(e)) => return Err(failed(&e)),
            }
        }
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

/// What a service command did (ATM-679, audit C11): its lines for people, and the same as one JSON object for
/// `--json`.
struct Said {
    lines: Vec<String>,
    json: Map<String, Value>,
}

impl Said {
    fn print(&self, json: bool) {
        if json {
            print_json(&Value::Object(self.json.clone()));
        } else {
            for line in &self.lines {
                println!("{line}");
            }
        }
    }
}

/// Start (or reuse) the server; the URL to show: the project's (at or above `cwd`), or the overview's. `--json`:
/// `{url, started, listed, migrated}`.
fn serve_start(cwd: &Path, port: u16, overview: bool) -> Result<(String, Said)> {
    let mut project = None;
    let mut lines = Vec::new();
    let mut migrated = Vec::new();
    let mut listed = Value::Null;
    if !overview {
        let migration = super::db::maintenance(cwd)?.migrate(false)?;
        for line in &migration.lines {
            if line != "Nothing to migrate." {
                lines.push(line.clone());
                migrated.push(line.clone());
            }
        }
        if !migration.tasks_exists {
            return Err(Error::failed(
                "no tasks.json to serve. Run /atm-create-tasks first.",
            ));
        }
        let (id, shown) = Project::open(cwd)?.join_default_workspace()?;
        listed = Value::Bool(shown);
        if shown {
            project = Some(id);
        } else {
            lines.push(
                "This project isn't listed on the dashboard (meta.dashboard.listed is false)."
                    .into(),
            );
        }
    }
    let home = state_dir();
    let (record, started) = ensure_server(&home, u64::from(port))?;
    let url = server_url(
        record.get("port").unwrap_or(&Value::Null),
        project.as_deref(),
    );
    lines.push(format!(
        "Dashboard server {}: {url}",
        if started {
            "started"
        } else {
            "already running"
        }
    ));
    let json = json!({ "url": url, "started": started, "listed": listed, "migrated": migrated });
    let json = json.as_object().cloned().unwrap_or_default();
    Ok((url, Said { lines, json }))
}

/// How many checks in a row must miss a process before it counts as gone: a busy machine can make one look miss it.
const GONE_CHECKS: u32 = 3;

/// Whether process `pid` ends within `wait`, sending it `signal` again (SIGKILL) each time it is still seen. One that
/// ended is given a moment more to be reaped by its parent, so that whoever started it sees it gone too; one nobody
/// reaps (a container without an init) counts as gone then.
fn wait_gone(server: Running, wait: Duration, kill: bool) -> bool {
    let deadline = Instant::now() + wait;
    let mut missed = 0;
    while missed < GONE_CHECKS {
        // The process found, by its start time too (ATM-673, audit C14): a later process given its pid isn't it, and is
        // never sent a SIGKILL
        if server.alive() {
            missed = 0;
            if Instant::now() >= deadline {
                return false;
            }
            if kill {
                let _ = server.signal(true);
            }
        } else {
            missed += 1;
        }
        std::thread::sleep(POLL);
    }
    let reaped = Instant::now() + REAP_WAIT;
    while Instant::now() < reaped
        && pid_exists(server.pid)
        && !Running::find(server.pid).is_some_and(|now| now != server)
    {
        std::thread::sleep(POLL);
    }
    true
}

/// How long before a server's record says it started its process may have started: the server writes the record once
/// it listens, a few seconds at most after its start on a busy machine.
const RECORD_SLACK: i64 = 30;

/// Whether `server` is the process `record` names: the record's server, started when it says (ATM-673, audit C14: its
/// pid alone may be a later process's)
fn started_as_recorded(server: Running, record: &Map<String, Value>) -> bool {
    // To the second: a record written elsewhere may carry fractions of one (`…:05.123Z`)
    let at = record
        .get("started_at")
        .and_then(Value::as_str)
        .and_then(|text| text.get(..19))
        .and_then(|seconds| parse_iso(&format!("{seconds}Z")));
    let Some(at) = at else {
        return false;
    };
    let started = i64::try_from(server.started()).unwrap_or(i64::MAX);
    (at - RECORD_SLACK..=at + RECORD_SLACK).contains(&started)
}

/// Stop a server of the user's (whose pid whoami confirmed): SIGTERM, then SIGKILL if it outlives the wait. Only a
/// process known to be the server (ATM-673): the one its `record` names, started when the record says (atm wrote it,
/// in a folder only its owner reads); or, found by asking the ports (no record), an atm server, `atm _serve` (audit
/// C6: anyone may answer whoami on a local port, naming any pid). Its record goes only once it is gone, and only if it
/// is that server's: a server still running is never forgotten.
fn stop_server(home: &Path, pid: u32, record: Option<&Map<String, Value>>) -> Result<()> {
    let Some(server) = Running::find(pid) else {
        // Gone already
        if read_record(home).is_some_and(|r| pid_of(&r) == Some(pid)) {
            let _ = std::fs::remove_file(record_path(home));
        }
        return Ok(());
    };
    let known = match record {
        Some(record) => started_as_recorded(server, record),
        None => is_atm_server(pid),
    };
    if !known {
        let why = if record.is_some() {
            "isn't the server its record names (it started at another time)"
        } else {
            "answered as a dashboard server but isn't one (not `atm _serve`)"
        };
        return Err(Error::failed(format!(
            "pid {pid} {why}: it is left running"
        )));
    }
    // Where there is no SIGTERM to send (Windows), straight to ending it, rather than waiting out STOP_WAIT for nothing
    let asked = server.signal(false);
    if !asked || !wait_gone(server, STOP_WAIT, false) {
        let _ = server.signal(true);
        if !wait_gone(server, KILL_WAIT, true) {
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

/// Stop the server, and any atm server the record doesn't name. `--json`: `{stopped: [{pid, port, recorded}],
/// stale_record}` (the pid of a record whose server was gone, removed).
fn serve_stop(home: &Path, port: u16) -> Result<Said> {
    let record = read_record(home);
    let live = record
        .as_ref()
        .is_some_and(|r| ask(port_of(r)).is_some_and(|i| answers_for(&i, r, home)));
    let recorded = record.as_ref().filter(|_| live).and_then(|r| r.get("pid"));
    let strays = unrecorded_servers(home, &servers_on(port), recorded);
    let mut said = Said {
        lines: Vec::new(),
        json: Map::new(),
    };
    let mut stopped = Vec::new();
    let mut stale = Value::Null;
    match &record {
        None if strays.is_empty() => said.lines.push("Dashboard server is not running.".into()),
        None => {}
        Some(record) if !live => {
            let _ = std::fs::remove_file(record_path(home));
            stale = record.get("pid").cloned().unwrap_or(Value::Null);
            if strays.is_empty() {
                said.lines.push(format!(
                    "Dashboard server is not running (removed a stale record for pid {}).",
                    py_str_opt(record.get("pid"))
                ));
            }
        }
        Some(record) => {
            if let Some(pid) = pid_of(record) {
                stop_server(home, pid, Some(record))?;
                said.lines
                    .push(format!("Dashboard server stopped (pid {pid})."));
                stopped.push(json!({ "pid": pid, "port": record.get("port"), "recorded": true }));
            }
        }
    }
    let failed = stop_strays(home, &strays, &mut said.lines, &mut stopped);
    said.json.insert("stopped".into(), Value::Array(stopped));
    said.json.insert("stale_record".into(), stale);
    failed.map_or(Ok(said), Err)
}

/// Stop the servers the record doesn't name: each one that is an atm server; any other answering as one is only
/// reported (ATM-673, audit C6), and the others are still stopped. The last refusal, if any.
fn stop_strays(
    home: &Path,
    strays: &[Map<String, Value>],
    lines: &mut Vec<String>,
    stopped: &mut Vec<Value>,
) -> Option<Error> {
    let mut failed = None;
    for info in strays {
        let Some(pid) = pid_of(info) else { continue };
        match stop_server(home, pid, None) {
            Ok(()) => {
                lines.push(format!(
                    "Stopped a dashboard server the record didn't name (pid {pid}, port {}).",
                    py_str_opt(info.get("port"))
                ));
                stopped.push(json!({ "pid": pid, "port": info.get("port"), "recorded": false }));
            }
            Err(e) => {
                eprintln!("atm: port {}: {e}", py_str_opt(info.get("port")));
                failed = Some(e);
            }
        }
    }
    failed
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
        stop_server(home, pid, Some(&live))?;
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

/// Open `url` in the user's browser (`open` on macOS, `explorer.exe` on Windows, `xdg-open` elsewhere; ATM-673, audit
/// C15: Windows has no `xdg-open`): atm's own command, with the URL it made, never through a shell.
fn open_browser(url: &str) {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer.exe"
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
            return Ok(());
        }
        let restarted = refresh_server(&home)?;
        let url = restarted
            .as_ref()
            .map(|r| server_url(r.get("port").unwrap_or(&Value::Null), None));
        if ctx.json {
            let version = restarted.as_ref().and_then(|r| r.get("version"));
            print_json(
                &json!({ "restarted": restarted.is_some(), "version": version, "url": url }),
            );
        } else if let (Some(record), Some(url)) = (&restarted, &url) {
            println!(
                "Dashboard server restarted with dashboard {}: {url}",
                py_str_opt(record.get("version"))
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
            let (url, said) = serve_start(&cwd, args.port, args.all)?;
            said.print(ctx.json);
            if args.open {
                open_browser(&url);
            }
        }
        "stop" => {
            if ctx.dry_run {
                println!("[dry-run] would stop the dashboard server");
                return Ok(());
            }
            serve_stop(&home, args.port)?.print(ctx.json);
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

#[cfg(all(test, unix))]
mod stop_tests {
    use super::*;

    /// Not a test: what [`stubborn`] runs, this test binary sleeping. Run only by it (`--ignored`, by name)
    #[test]
    #[ignore = "run by `stubborn`, as its process"]
    fn sleeper() {
        std::thread::sleep(Duration::from_secs(60));
    }

    /// A process to stop: this test binary, sleeping. No shell (I5): atm starts only git and itself (I24). (A server
    /// that ignores SIGTERM, ended with SIGKILL, is `serve.test.mjs`'s, T-390: no signal can be ignored from here
    /// without a library for it.)
    fn sleeping() -> std::process::Child {
        let own_binary = std::env::current_exe().expect("the test binary");
        std::process::Command::new(own_binary)
            .args([
                "--ignored",
                "--exact",
                "commands::maintenance::stop_tests::sleeper",
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("the test binary")
    }

    /// The process found is ended by its SIGTERM, and counts as gone once it is
    #[test]
    fn a_process_asked_to_stop_is_gone() {
        let mut child = sleeping();
        let found = Running::find(child.id()).expect("it runs");
        assert!(found.signal(false));
        assert!(wait_gone(found, STOP_WAIT, false));
        let _ = child.wait();
        assert!(!found.alive());
    }

    /// A process found earlier and gone since: a later process given its pid isn't it, so it counts as gone and gets no
    /// signal
    #[test]
    fn a_process_gone_counts_as_gone_without_any_signal() {
        let mut child = sleeping();
        let found = Running::find(child.id()).expect("it runs");
        child.kill().unwrap();
        let _ = child.wait();
        assert!(wait_gone(found, Duration::from_millis(500), true));
        assert!(!found.signal(true));
    }
}
