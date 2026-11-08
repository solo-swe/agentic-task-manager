//! Opening a project (and, as later slices come, the registry of projects): the one place that picks the store and the version control, and the actor.
//!
//! Part of atm's Rust port (T-415): docs/porting-to-rust.md, docs/layered-architecture.md, docs/guides/rust.md.

use std::path::Path;

use atm_common::Result;
use atm_db::JsonGitStore;
use atm_db::jsongit::paths::{check_task_folder, find_paths};
use atm_lib::Project;
use atm_lib::maintenance::StoreMaintenance;

/// The project at or above `cwd`, on the JSON-in-git store. Nothing is read until the project's data is asked for.
///
/// # Errors
/// When the project's task folder fails its checks (a symlink, or one leading outside the project).
pub fn open_project(cwd: &Path) -> Result<Project> {
    Project::open(cwd)
}

/// The project at or above `cwd`, its task file read as far as it can be read, saying what is wrong instead of
/// refusing it (T-377): for showing it (the REST API, `atm task list --derived --tolerant`).
///
/// # Errors
/// When the project's task folder fails its checks (a symlink, or one leading outside the project).
pub fn open_project_tolerant(cwd: &Path) -> Result<Project> {
    Project::open_tolerant(cwd)
}

/// `atm project config`: where the project's task files are (the JSON-in-git store's own locations): root, dir,
/// tasks (and whether it exists), the old dashboard's place, and a task file left in a legacy location.
///
/// # Errors
/// When the project's task folder fails its checks (a symlink, or one leading outside the project).
pub fn task_paths(cwd: &Path) -> Result<serde_json::Value> {
    StoreMaintenance::at(cwd).task_paths()
}

/// The remote's default branch (`origin/HEAD`), else `main`: the base of a workflow that names none.
#[must_use]
pub fn default_base(cwd: &Path) -> String {
    let root = atm_db::jsongit::paths::project_root(cwd);
    atm_db::git::git(
        &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
        &root,
    )
    .and_then(|r| r.split_once('/').map(|(_, branch)| branch.to_string()))
    .unwrap_or_else(|| "main".into())
}

/// `atm db merge-driver` (git runs it): merge the three versions of a task file task by task, writing the result over
/// `ours`. Returns the conflicts (this branch's version was kept for each).
///
/// # Errors
/// A version that isn't task data, or a result that can't be written.
pub fn merge_versions(base: &Path, ours: &Path, theirs: &Path) -> Result<Vec<String>> {
    atm_lib::maintenance::merge_versions(base, ours, theirs)
}

/// List the project at or above `cwd` on the dashboard, by its folder (every command that sets a project up does).
///
/// # Errors
/// The task folder's checks, the registry's lock or file.
pub fn register_project(cwd: &Path) -> Result<()> {
    atm_db::registry::register(&find_paths(cwd)?, true)?;
    Ok(())
}

/// (id, listed) of the project at or above `cwd` on the dashboard, after registering it: the ID the registry gave it
/// (`meta.id`, or another when that one is taken) and whether it is listed (`meta.dashboard.listed`).
///
/// # Errors
/// The task folder's checks, the registry's lock or file.
pub fn dashboard_id(cwd: &Path) -> Result<(String, bool)> {
    let paths = find_paths(cwd)?;
    atm_db::registry::register(&paths, true)?;
    Ok(atm_db::registry::registered_id(&paths))
}

pub use atm_lib::maintenance::{Migration, SetUp};

/// `atm db init`: set the store up in its working copy: in git, the merge driver for the task files (registered in
/// the clone, `.gitattributes` staged).
///
/// # Errors
/// The task folder's checks, a `.gitattributes` that can't be written.
pub fn set_up_store(cwd: &Path) -> Result<SetUp> {
    StoreMaintenance::at(cwd).set_up()
}

/// `atm db migrate` (and `atm service`, before it shows a project): bring the task folder at or above `cwd` up to
/// date. With `register_it`, the project is listed on the dashboard too (every skill runs migrate first).
///
/// # Errors
/// The task folder's checks, a file that can't be moved, read or written.
pub fn migrate_folder(cwd: &Path, register_it: bool) -> Result<Migration> {
    StoreMaintenance::at(cwd).migrate(register_it)
}

/// The version control of the project at or above `cwd`: git, at the project's root.
#[must_use]
pub fn open_vcs(cwd: &Path) -> atm_vcs::GitVcs {
    atm_vcs::GitVcs::new(&atm_db::jsongit::paths::project_root(cwd))
}

/// The user's registry of projects (the dashboard's list).
pub use atm_db::registry::FileRegistry as Registry;

/// The user's registry of projects (the dashboard's list), in atm's state folder.
#[must_use]
pub fn open_registry() -> Registry {
    Registry::new()
}

/// The registry of the atm folder `home` (the server's).
#[must_use]
pub fn open_registry_at(home: &Path) -> Registry {
    Registry::at(home)
}

/// The project the plugin's hook (`claim heartbeat --auto`) works on in `cwd`, as `actor`, after listing it on the
/// dashboard (one read of the registry when it is listed already); none when there are no task files there.
#[must_use]
pub fn hook_project(cwd: &Path, actor: String) -> Option<Project> {
    let paths = find_paths(cwd).ok()?;
    if !paths.tasks.is_file() {
        return None;
    }
    check_task_folder(&paths.root, &paths.dir).ok()?;
    // The hook never fails on the registry
    let _ = atm_db::registry::register_if_needed(&paths);
    Some(Project::new(Box::new(JsonGitStore::new(paths))).with_actor(Some(actor)))
}

/// When process `pid` started, in seconds since 1970, or none when there is no such process (or it has ended): with
/// its pid, it tells a process from a later one that got the same pid.
#[must_use]
pub fn process_started(pid: u32) -> Option<u64> {
    let pid = sysinfo::Pid::from_u32(pid);
    let mut system = sysinfo::System::new().ok()?;
    system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
    system
        .process(pid)
        .filter(|p| p.status() != sysinfo::ProcessStatus::Zombie)
        .map(sysinfo::Process::start_time)
}

/// Whether process `pid` runs (an ended one nobody reaped yet doesn't).
#[must_use]
pub fn pid_alive(pid: u32) -> bool {
    process_started(pid).is_some()
}

/// Whether there is a process `pid` at all, one that ended but isn't reaped yet included.
#[must_use]
pub fn pid_exists(pid: u32) -> bool {
    let pid = sysinfo::Pid::from_u32(pid);
    let Ok(mut system) = sysinfo::System::new() else {
        return false;
    };
    system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
    system.process(pid).is_some()
}

/// Ask process `pid` to stop (SIGTERM), or with `kill`, end it (SIGKILL). Whether the signal was sent.
#[must_use]
pub fn signal_process(pid: u32, kill: bool) -> bool {
    let pid = sysinfo::Pid::from_u32(pid);
    let Ok(mut system) = sysinfo::System::new() else {
        return false;
    };
    system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
    let signal = if kill {
        sysinfo::Signal::Kill
    } else {
        sysinfo::Signal::Term
    };
    system
        .process(pid)
        .and_then(|p| p.kill_with(signal))
        .unwrap_or(false)
}

use atm_db::jsongit::paths::main_worktree;

/// A Claude Code session's identity: `claude:` and 8 characters of its ID; in a linked git worktree, also
/// `@<worktree folder>`, so agents in separate worktrees have identities of their own.
#[must_use]
pub fn session_identity(session: &str, cwd: &Path) -> String {
    let mut identity = format!("claude:{}", session.chars().take(8).collect::<String>());
    let root = atm_db::jsongit::paths::project_root(cwd);
    if main_worktree(&root) != root {
        identity.push('@');
        identity.push_str(
            &root
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
        );
    }
    identity
}

/// Who runs a command (the actor, DM-4.5): `agent` (`--agent`), else `ATM_AGENT`, else this Claude Code session (per
/// linked worktree), else git's user name (a person in a terminal), else `user@host`.
#[must_use]
pub fn agent_identity(agent: Option<&str>, cwd: &Path) -> String {
    if let Some(agent) = agent.filter(|a| !a.is_empty()) {
        return agent.to_string();
    }
    let env = |key: &str| std::env::var(key).ok().filter(|v| !v.is_empty());
    if let Some(agent) = env("ATM_AGENT") {
        return agent;
    }
    if let Some(session) = env("CLAUDE_CODE_SESSION_ID") {
        return session_identity(&session, cwd);
    }
    let root = atm_db::jsongit::paths::project_root(cwd);
    if let Some(name) = atm_db::git::git(&["config", "user.name"], &root).filter(|n| !n.is_empty())
    {
        return name;
    }
    let user = env("USER")
        .or_else(|| env("USERNAME"))
        .unwrap_or_else(|| "user".into());
    format!(
        "{user}@{}",
        sysinfo::System::host_name().unwrap_or_default()
    )
}

/// `atm _lock-test <seconds>`: hold the task folder's lock for `seconds`, logging `acquire <pid>` and `release <pid>`
/// to `.lock-test.log` in it, so tests can check that two processes never hold it at once.
///
/// # Errors
/// When the lock can't be taken in time, or the log can't be written.
pub fn hold_lock_for_test(cwd: &Path, seconds: f64) -> Result<()> {
    StoreMaintenance::at(cwd).hold_lock_for_test(seconds)
}

#[cfg(test)]
mod tests {
    use super::open_project;

    #[test]
    fn a_folder_without_a_project_opens_and_says_so_when_read() {
        let project = open_project(&std::env::temp_dir()).unwrap();
        let error = project.store().list_tasks().unwrap_err();
        assert!(
            error.to_string().starts_with("no tasks.json in "),
            "{error}"
        );
    }
}
