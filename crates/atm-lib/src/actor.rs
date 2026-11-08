//! Who works on a project (the actor, DM-4.5): the name changes are recorded as.

use std::path::Path;

use atm_db::jsongit::paths::{main_worktree, project_root};

/// A Claude Code session's identity: `claude:` and 8 characters of its ID; in a linked git worktree, also
/// `@<worktree folder>`, so agents in separate worktrees have identities of their own.
#[must_use]
pub fn session_identity(session: &str, cwd: &Path) -> String {
    let mut identity = format!("claude:{}", session.chars().take(8).collect::<String>());
    let root = project_root(cwd);
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

/// Who runs a command: `agent` (`--agent`), else `ATM_AGENT`, else this Claude Code session (per linked worktree),
/// else git's user name (a person in a terminal), else `user@host`.
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
    let root = project_root(cwd);
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
