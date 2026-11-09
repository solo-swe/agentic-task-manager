//! `atm vcs`: version control (the Python atm's `cli/commands/vcs.py`): commit, pr, pushed, repository.

use std::path::Path;

use atm_common::py::py_str;
use atm_common::{Error, Result};
use atm_lib::Project;
use atm_lib::actor::agent_identity;
use atm_lib::code::{
    commit_task, folder_changes, get_repository, record_commit, record_merge, record_pr,
    repository_paths, set_repository,
};
use clap::Subcommand;
use serde_json::json;

use super::{Context, print_json};

/// The version-control commands.
#[derive(Debug, Subcommand)]
pub enum VcsCommand {
    /// Commit exactly the named files plus tasks.json, and record the commit.
    Commit {
        /// The task ID
        task_id: String,
        /// A file the task changed (repeatable)
        #[arg(long)]
        file: Vec<String>,
        /// Every change under a folder (repeatable)
        #[arg(long = "dir")]
        dir: Vec<String>,
        /// The commit message
        #[arg(long)]
        message: Option<String>,
        /// Record this existing commit instead
        #[arg(long)]
        record: Option<String>,
        /// Who you are
        #[arg(long)]
        agent: Option<String>,
    },
    /// Record the PR that contains the tasks.
    Pr {
        /// Task IDs
        #[arg(required = true, value_name = "IDS")]
        ids: Vec<String>,
        /// The PR number
        #[arg(long)]
        number: Option<i64>,
        /// The PR's URL
        #[arg(long)]
        url: Option<String>,
        /// Who you are
        #[arg(long)]
        agent: Option<String>,
    },
    /// Record that a PR was merged, on the tasks it contains.
    Pushed {
        /// The PR number
        #[arg(long)]
        pr: Option<i64>,
        /// When it was merged (ISO 8601 UTC)
        #[arg(long)]
        at: Option<String>,
        /// Who you are
        #[arg(long)]
        agent: Option<String>,
    },
    /// Print or set the repository's web address.
    Repository {
        /// An http(s) URL or git remote
        url: Option<String>,
        /// Move recorded PR links from this address
        #[arg(long = "from")]
        old: Option<String>,
        /// Who you are
        #[arg(long)]
        agent: Option<String>,
    },
}

/// A required option, missing: as Click words it.
fn missing(option: &str) -> Error {
    Error::Invalid(format!("Missing option '{option}'."))
}

fn dry_run(parts: &[String]) {
    println!("[dry-run] would run: {}", parts.join(" "));
}

/// `--flag value` for each value given.
fn flags(parts: &mut Vec<String>, flag: &str, values: &[&String]) {
    parts.extend(
        values
            .iter()
            .filter(|v| !v.is_empty())
            .map(|v| format!("--{flag} {v}")),
    );
}

/// Run a version-control command in `cwd`.
///
/// # Errors
/// What atm-lib or git refuses.
#[allow(clippy::too_many_lines)] // reason: one arm per command, as the Python atm's vcs_app has them
pub fn run(command: VcsCommand, ctx: Context, cwd: &Path) -> Result<()> {
    match command {
        VcsCommand::Commit {
            task_id,
            file,
            dir,
            message,
            record,
            agent,
        } => {
            if ctx.dry_run {
                let mut parts = vec![format!("atm vcs commit {task_id}")];
                flags(&mut parts, "file", &file.iter().collect::<Vec<_>>());
                flags(&mut parts, "dir", &dir.iter().collect::<Vec<_>>());
                flags(&mut parts, "message", &message.iter().collect::<Vec<_>>());
                flags(&mut parts, "record", &record.iter().collect::<Vec<_>>());
                flags(&mut parts, "agent", &agent.iter().collect::<Vec<_>>());
                dry_run(&parts);
                return Ok(());
            }
            let mut project =
                Project::open(cwd)?.with_actor(Some(agent_identity(agent.as_deref(), cwd)));
            let vcs = project.vcs();
            let vcs = vcs.as_ref();
            if let Some(rev) = record {
                if !file.is_empty() || !dir.is_empty() || message.is_some() {
                    return Err(Error::Invalid(
                        "--record records an existing commit: no --file, --dir or --message".into(),
                    ));
                }
                let result = record_commit(&mut project, vcs, &task_id, &rev)?;
                if ctx.json {
                    let mut shown = json!({"task": result["task"], "commit": result["commit"], "recorded": true});
                    if !result["hint"].is_null() {
                        shown["hint"] = result["hint"].clone();
                    }
                    print_json(&shown);
                    return Ok(());
                }
                let sha = py_str(&result["commit"]);
                let line = format!(
                    "{}: recorded {} {}",
                    py_str(&result["task"]),
                    &sha[..7.min(sha.len())],
                    py_str(&result["summary"])
                );
                println!("{}", line.trim_end());
                if let Some(hint) = result["hint"].as_str() {
                    println!("  {hint}");
                }
                return Ok(());
            }
            let mut files = repository_paths(vcs, &file, cwd)?;
            if !dir.is_empty() {
                for f in folder_changes(&project, vcs, &task_id, &dir, cwd)? {
                    if !files.contains(&f) {
                        files.push(f);
                    }
                }
            }
            let result = commit_task(&mut project, vcs, &task_id, &files, message.as_deref())?;
            if ctx.json {
                print_json(&result);
                return Ok(());
            }
            let sha = py_str(&result["commit"]);
            let first = py_str(&result["message"])
                .lines()
                .next()
                .unwrap_or_default()
                .to_string();
            println!(
                "{}: committed {} {first}",
                py_str(&result["task"]),
                &sha[..7.min(sha.len())]
            );
            let listed: Vec<String> = result["files"]
                .as_array()
                .into_iter()
                .flatten()
                .map(py_str)
                .collect();
            println!("  {}", listed.join(", "));
        }
        VcsCommand::Pr {
            ids,
            number,
            url,
            agent,
        } => {
            let number = number.ok_or_else(|| missing("--number"))?;
            let url = url.ok_or_else(|| missing("--url"))?;
            if ctx.dry_run {
                let mut parts = vec![format!(
                    "atm vcs pr {} --number {number} --url {url}",
                    ids.join(" ")
                )];
                flags(&mut parts, "agent", &agent.iter().collect::<Vec<_>>());
                dry_run(&parts);
                return Ok(());
            }
            let mut project =
                Project::open(cwd)?.with_actor(Some(agent_identity(agent.as_deref(), cwd)));
            let recorded = record_pr(&mut project, &ids, number, &url)?;
            if ctx.json {
                print_json(&json!({"tasks": recorded, "pr": {"number": number, "url": url}}));
            } else {
                println!("{}: in PR #{number} ({url})", recorded.join(", "));
            }
        }
        VcsCommand::Pushed { pr, at, agent } => {
            let pr = pr.ok_or_else(|| missing("--pr"))?;
            if ctx.dry_run {
                let mut parts = vec![format!("atm vcs pushed --pr {pr}")];
                flags(&mut parts, "at", &at.iter().collect::<Vec<_>>());
                flags(&mut parts, "agent", &agent.iter().collect::<Vec<_>>());
                dry_run(&parts);
                return Ok(());
            }
            let mut project =
                Project::open(cwd)?.with_actor(Some(agent_identity(agent.as_deref(), cwd)));
            let result = record_merge(&mut project, pr, at.as_deref())?;
            if ctx.json {
                print_json(&result);
                return Ok(());
            }
            let ids: Vec<String> = result["tasks"]
                .as_array()
                .into_iter()
                .flatten()
                .map(py_str)
                .collect();
            if ids.is_empty() {
                println!("PR #{pr}: already recorded as merged");
            } else {
                println!("PR #{pr} merged: {}", ids.join(", "));
            }
            for release in result["released"].as_array().into_iter().flatten() {
                println!("Released {}: all its tasks are merged.", py_str(release));
            }
            for (release, waiting) in result["pending"].as_object().into_iter().flatten() {
                let waiting: Vec<String> = waiting
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(py_str)
                    .collect();
                let verb = if waiting.len() == 1 { "is" } else { "are" };
                println!(
                    "{release} stays planned: {} {verb} not merged yet.",
                    waiting.join(", ")
                );
            }
        }
        VcsCommand::Repository { url, old, agent } => {
            if ctx.dry_run {
                let mut parts = vec!["atm vcs repository".to_string()];
                parts.extend(url.iter().filter(|u| !u.is_empty()).cloned());
                flags(&mut parts, "from", &old.iter().collect::<Vec<_>>());
                flags(&mut parts, "agent", &agent.iter().collect::<Vec<_>>());
                dry_run(&parts);
                return Ok(());
            }
            let mut project =
                Project::open(cwd)?.with_actor(Some(agent_identity(agent.as_deref(), cwd)));
            let Some(url) = url else {
                if let Some(old) = old {
                    return Err(Error::Invalid(format!(
                        "unexpected argument '{old}' (atm vcs repository URL)"
                    )));
                }
                let current = get_repository(&project)?;
                if ctx.json {
                    print_json(&json!({"repository": current}));
                } else if let Some(current) = current {
                    println!("{current}");
                } else {
                    println!(
                        "No repository address yet. Set it with `atm vcs repository <url>` (a git remote works too)."
                    );
                }
                return Ok(());
            };
            let result = set_repository(&mut project, &url, old.as_deref())?;
            let moved = result["moved_links"].as_u64().unwrap_or(0);
            if ctx.json {
                print_json(&json!({"repository": result["repository"], "moved_links": moved}));
            } else {
                let links = if moved > 0 {
                    format!(
                        " ({moved} PR link{} moved from {})",
                        if moved == 1 { "" } else { "s" },
                        py_str(&result["previous"])
                    )
                } else {
                    String::new()
                };
                println!(
                    "Repository address set: {}{links}",
                    py_str(&result["repository"])
                );
            }
        }
    }
    Ok(())
}
