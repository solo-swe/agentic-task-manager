//! Opening a project (and, as later slices come, the registry of projects): the one place that picks the store and the version control, and the actor.
//!
//! Part of atm's Rust port (T-415): docs/porting-to-rust.md, docs/layered-architecture.md, docs/guides/rust.md.

use std::path::Path;

use atm_common::Result;
use atm_db::JsonGitStore;
use atm_db::jsongit::paths::{check_task_folder, find_paths};
use atm_lib::Project;

/// The project at or above `cwd`, on the JSON-in-git store. Nothing is read until the project's data is asked for.
///
/// # Errors
/// When the project's task folder fails its checks (a symlink, or one leading outside the project).
pub fn open_project(cwd: &Path) -> Result<Project> {
    let paths = find_paths(cwd)?;
    // The folder's checks run on every open, so a file linked after the paths were found is refused too
    check_task_folder(&paths.root, &paths.dir)?;
    Ok(Project::new(Box::new(JsonGitStore::new(paths))))
}

/// The main worktree's folder for a project root: itself, unless it is a linked git worktree.
fn main_worktree(root: &Path) -> std::path::PathBuf {
    if let Some(common) = atm_db::git::git(&["rev-parse", "--git-common-dir"], root) {
        let common = atm_common::home::absolute(&root.join(common));
        // `..` worked out without reading links, as Python's normpath does
        let mut normal = std::path::PathBuf::new();
        for part in common.components() {
            match part {
                std::path::Component::ParentDir => {
                    normal.pop();
                }
                std::path::Component::CurDir => {}
                other => normal.push(other),
            }
        }
        if normal.file_name().is_some_and(|n| n == ".git") {
            return normal
                .parent()
                .map_or_else(|| root.to_path_buf(), Path::to_path_buf);
        }
    }
    root.to_path_buf()
}

/// A Claude Code session's identity: `claude:` and 8 characters of its ID; in a linked git worktree, also
/// `@<worktree folder>`, so agents in separate worktrees have identities of their own.
fn session_identity(session: &str, cwd: &Path) -> String {
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
    use std::io::Write;
    let dir = find_paths(cwd)?.dir;
    let log = dir.join(".lock-test.log");
    let append = |line: String| -> Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log)
            .map_err(|e| atm_common::Error::failed(format!("{}: {e}", log.display())))?;
        writeln!(file, "{line}")
            .map_err(|e| atm_common::Error::failed(format!("{}: {e}", log.display())))
    };
    let held = atm_db::jsongit::lock::lock(&dir, None)?;
    append(format!("acquire {}", std::process::id()))?;
    std::thread::sleep(std::time::Duration::from_secs_f64(seconds.max(0.0)));
    append(format!("release {}", std::process::id()))?;
    drop(held);
    Ok(())
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
