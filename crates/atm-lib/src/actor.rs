//! Who works on a project (the actor, DM-4.5): the name changes are recorded as.

use std::path::Path;

use atm_db::jsongit::paths::{main_worktree, project_root};

/// A Claude Code session's identity: `claude:` and 8 characters of its ID; in a linked git worktree, also
/// `@<worktree folder>`, so agents in separate worktrees have identities of their own.
#[must_use]
pub fn session_identity(session: &str, cwd: &Path) -> String {
    harness_session("claude", session, cwd)
}

/// A harness's session identity: `<harness>:` and 8 characters of `session` (letters, digits, `-` and `_` only), and
/// `@<worktree folder>` in a linked git worktree.
fn harness_session(harness: &str, session: &str, cwd: &Path) -> String {
    let id: String = session
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        .take(8)
        .collect();
    let mut identity = format!("{harness}:{}", if id.is_empty() { "session" } else { &id });
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

/// The identity of the agent harness atm runs in, from the markers each puts in the environment of the commands its
/// agent runs (ATM-660): Claude Code (`CLAUDE_CODE_SESSION_ID`, else `CLAUDECODE`), Codex (`CODEX_THREAD_ID`, else
/// `CODEX_SANDBOX`), Gemini CLI (`GEMINI_CLI`, with `GEMINI_SESSION_ID` when it has one), `OpenCode` (`OPENCODE`) and
/// any harness naming itself in `AI_AGENT`. None outside one. Each is `<harness>:<session>`, per linked worktree.
#[must_use]
pub fn harness_identity(cwd: &Path) -> Option<String> {
    let env = |key: &str| std::env::var(key).ok().filter(|v| !v.is_empty());
    if let Some(session) = env("CLAUDE_CODE_SESSION_ID") {
        return Some(session_identity(&session, cwd));
    }
    if env("CLAUDECODE").is_some() {
        return Some(harness_session("claude", "", cwd));
    }
    if let Some(thread) = env("CODEX_THREAD_ID") {
        return Some(harness_session("codex", &thread, cwd));
    }
    if env("CODEX_SANDBOX").is_some() {
        return Some(harness_session("codex", "", cwd));
    }
    if env("GEMINI_CLI").is_some() {
        return Some(harness_session(
            "gemini",
            &env("GEMINI_SESSION_ID").unwrap_or_default(),
            cwd,
        ));
    }
    if env("OPENCODE").is_some() {
        return Some(harness_session("opencode", "", cwd));
    }
    env("AI_AGENT").map(|name| harness_session("agent", &name, cwd))
}

/// Who runs a command: `agent` (`--agent`), else `ATM_AGENT`, else the agent harness's session ([`harness_identity`],
/// per linked worktree), else git's user name (a person in a terminal), else `user@host`.
#[must_use]
pub fn agent_identity(agent: Option<&str>, cwd: &Path) -> String {
    if let Some(agent) = agent.filter(|a| !a.is_empty()) {
        return agent.to_string();
    }
    let env = |key: &str| std::env::var(key).ok().filter(|v| !v.is_empty());
    if let Some(agent) = env("ATM_AGENT") {
        return agent;
    }
    harness_identity(cwd).unwrap_or_else(|| user_identity(cwd))
}

/// Inside an agent harness's session ([`harness_identity`]), a name given with `--agent` or `ATM_AGENT` must be an
/// agent's (ATM-660): an agent can't run a command as a person, which would let it finish a task a review must decide.
/// Outside one, any name is taken.
///
/// # Errors
/// `Invalid`, naming the person and the session, when a harness's agent names a person.
pub fn check_named(agent: Option<&str>, cwd: &Path) -> atm_common::Result<()> {
    let env = |key: &str| std::env::var(key).ok().filter(|v| !v.is_empty());
    let Some(session) = harness_identity(cwd) else {
        return Ok(());
    };
    let named = agent
        .filter(|a| !a.is_empty())
        .map(String::from)
        .or_else(|| env("ATM_AGENT"));
    match named {
        Some(name) if !atm_common::edits::is_agent(Some(&name)) => {
            Err(atm_common::Error::Invalid(format!(
                "{session} is an agent's session: it can't act as {name}, a person (--agent or ATM_AGENT); \
             leave it out, or name an agent ({})",
                atm_common::edits::AGENT_PREFIXES.join(" ")
            )))
        }
        _ => Ok(()),
    }
}

/// The person at the machine, whatever agent's environment atm runs in: their git name, else `user@host`. What the
/// app and the browser act as (docs/code-review-design.md §3.12): a server an agent's shell started must still write
/// as the user, never as that agent.
#[must_use]
pub fn user_identity(cwd: &Path) -> String {
    let env = |key: &str| std::env::var(key).ok().filter(|v| !v.is_empty());
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
