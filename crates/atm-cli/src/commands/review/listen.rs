//! Listening to the reviews: `wait`, `watch` and `inbox` (with Claude Code's hook).

use clap::Subcommand;

use super::{
    Context, Error, Path, Result, Value, actor, json, opened, print_json, reference, summary_line,
};

/// The commands that listen to the reviews (in `atm review`'s list, ATM-711).
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Wait until a reviewer submits on a review (or decides it): what happened, and the comments added meanwhile.
    /// Exit code 3 when the time is up first.
    Wait {
        /// The review (CR-7)
        review: String,
        /// What to wait for: submitted (a decision, or a comment on the whole) or decision
        #[arg(long = "for", default_value = "submitted", value_parser = ["submitted", "decision"])]
        until: String,
        /// How long at most: 90s, 30m, 2h (seconds without a unit)
        #[arg(long, default_value = "2h")]
        timeout: String,
        /// Who waits (their own comments aren't news)
        #[arg(long)]
        agent: Option<String>,
    },
    /// The agent's one watch for its session: each person's comment about the agent's work, from any review, as it is
    /// added (a JSON object a line with --json), until the time is up. Names no review and no task.
    Watch {
        /// How long at most: 90s, 30m, 8h (seconds without a unit)
        #[arg(long, default_value = "8h")]
        timeout: String,
        /// Stop once something was told (exit code 3 if nothing was, at the time up)
        #[arg(long)]
        once: bool,
        /// Tell what came since this time (2026-10-24T18:00:00Z), whatever the last watch told
        #[arg(long)]
        since: Option<String>,
        /// Who watches
        #[arg(long)]
        agent: Option<String>,
    },
    /// What you haven't read about your work in the reviews (ATM-708): each person's comment and decision since you
    /// last read them, as a watch prints them (a JSON object a line with --json), oldest first; read from then on. A
    /// watch with --once counts as read; one that runs on doesn't (its lines may never be read).
    Inbox {
        /// Only look: leave them unread
        #[arg(long)]
        peek: bool,
        /// Run as Claude Code's hook (`atm agents hooks install`): its input on stdin, its answer on stdout
        #[arg(long, conflicts_with = "peek")]
        hook: bool,
        /// Whose inbox
        #[arg(long)]
        agent: Option<String>,
    },
}

/// Run a command that listens to the reviews, in `cwd`.
///
/// # Errors
/// What the review's rules or atm-lib refuse, and a project or review that can't be read.
#[allow(clippy::too_many_lines)] // reason: one arm per command
pub(super) fn run(command: Command, ctx: Context, cwd: &Path) -> Result<u8> {
    match command {
        Command::Wait {
            review,
            until,
            timeout,
            agent,
        } => {
            let project = opened(cwd, agent.as_deref())?;
            let me = actor(&project);
            let until = if until == "decision" {
                atm_lib::review_listen::WaitFor::Decision
            } else {
                atm_lib::review_listen::WaitFor::Submitted
            };
            let waited = atm_lib::review_listen::wait(
                &project,
                &reference(&review),
                until,
                duration_of(&timeout)?,
                &me.0,
            )?;
            let Some(waited) = waited else {
                if ctx.json {
                    print_json(&json!({ "review": review, "timed_out": true }));
                } else {
                    println!("{review}: nothing yet (the time is up)");
                }
                return Ok(3);
            };
            if ctx.json {
                print_json(&waited);
            } else {
                let header: atm_lib::reviews::ReviewHeader =
                    serde_json::from_value(waited["review"].clone())
                        .map_err(|e| Error::failed(e.to_string()))?;
                println!("{}", summary_line(&header));
                for heard in waited["comments"].as_array().into_iter().flatten() {
                    println!("  {}", heard_line(heard));
                }
            }
            Ok(0)
        }
        Command::Watch {
            timeout,
            once,
            since,
            agent,
        } => {
            let project = opened(cwd, agent.as_deref())?;
            let me = actor(&project);
            let mut ready = || eprintln!("Watching the reviews for {}'s work.", me.0);
            let mut emit = |heard: Value| {
                if ctx.json {
                    // One object a line, exactly, as print_json prints (I30): a reader takes each as it comes
                    ::std::println!("{heard}");
                } else {
                    println!("{}", heard_line(&heard));
                }
            };
            let told = atm_lib::review_listen::watch(
                &project,
                &me.0,
                duration_of(&timeout)?,
                &atm_lib::review_listen::WatchOptions { once, since },
                &mut ready,
                &mut emit,
            )?;
            // --once with nothing told by the time up: exit code 3, as `atm review wait` (ATM-624)
            Ok(if once && told == 0 { 3 } else { 0 })
        }
        Command::Inbox {
            peek: _,
            hook: true,
            agent,
        } => Ok(hook_inbox(cwd, agent.as_deref())),
        Command::Inbox {
            peek,
            hook: false,
            agent,
        } => {
            let project = opened(cwd, agent.as_deref())?;
            let me = actor(&project);
            let unread = atm_lib::review_listen::inbox(&project, &me.0, !peek)?;
            if ctx.json {
                for event in &unread {
                    // One object a line, as the watch prints them (I30)
                    ::std::println!("{event}");
                }
            } else if unread.is_empty() {
                println!("Nothing unread about your work.");
            } else {
                for event in &unread {
                    match event["decision"].as_str() {
                        Some(decision) => println!(
                            "{}{}: {}",
                            event["review"].as_str().unwrap_or_default(),
                            event["task"]
                                .as_str()
                                .map(|t| format!(" ({t})"))
                                .unwrap_or_default(),
                            decision.replace('_', " ")
                        ),
                        None => println!("{}", heard_line(event)),
                    }
                }
            }
            Ok(0)
        }
    }
}

/// The most of Claude Code's hook input read from stdin.
pub(super) const HOOK_INPUT_BYTES: u64 = 1024 * 1024;

/// `atm review inbox --hook` (ATM-708): Claude Code's hook. Its input on stdin names the session, its folder and the
/// event; the session's unread review events are its answer on stdout, read from then on: context for the agent
/// (`UserPromptSubmit`, `PostToolUse`), or its reply held (`Stop`). Always exit code 0 and nothing printed when there
/// is nothing to say, the input isn't Claude Code's, or the folder holds no project: a hook never stands in the way.
pub(super) fn hook_inbox(cwd: &Path, agent: Option<&str>) -> u8 {
    use std::io::Read;
    let mut text = String::new();
    if std::io::stdin()
        .take(HOOK_INPUT_BYTES)
        .read_to_string(&mut text)
        .is_err()
    {
        return 0;
    }
    let input: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    let Some(event) = input["hook_event_name"].as_str() else {
        return 0;
    };
    if !atm_lib::agent_hooks::HOOK_EVENTS.contains(&event) {
        return 0;
    }
    let folder = input["cwd"]
        .as_str()
        .map_or_else(|| cwd.to_path_buf(), std::path::PathBuf::from);
    let Ok(project) = atm_lib::Project::open(&folder) else {
        return 0;
    };
    // The session's identity, as its own commands have it: from the input, as a hook's environment may not name it
    let me = match (agent, input["session_id"].as_str()) {
        (Some(agent), _) if !agent.is_empty() => agent.to_string(),
        (_, Some(session)) if std::env::var("ATM_AGENT").map_or(true, |a| a.is_empty()) => {
            atm_lib::actor::session_identity(session, &folder)
        }
        _ => atm_lib::actor::agent_identity(None, &folder),
    };
    let Ok(unread) = atm_lib::review_listen::inbox(&project, &me, true) else {
        return 0;
    };
    if let Some(answer) = atm_lib::agent_hooks::hook_answer(event, &unread) {
        ::std::println!("{answer}");
    }
    0
}

/// A comment heard, for people: where, who, and its text.
pub(super) fn heard_line(heard: &Value) -> String {
    let text = |key: &str| {
        heard
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let task = heard
        .get("task")
        .and_then(Value::as_str)
        .map_or_else(String::new, |t| format!(" ({t})"));
    let place = match (
        heard.get("path").and_then(Value::as_str),
        heard.get("lines").and_then(Value::as_array),
    ) {
        (Some(path), Some(lines)) => format!(" on {path}:{}-{}", lines[0], lines[1]),
        (Some(path), None) => format!(" on {path}"),
        _ => String::new(),
    };
    format!(
        "{} {}{task}{place}, {}: {}",
        text("review"),
        text("comment"),
        text("author"),
        text("body").replace('\n', " ")
    )
}

/// `90s`, `30m`, `2h`, or seconds without a unit.
pub(super) fn duration_of(text: &str) -> Result<std::time::Duration> {
    let bad = || {
        Error::Invalid(format!(
            "--timeout takes 90s, 30m, 2h or seconds, not {text}"
        ))
    };
    let (number, unit) = match text.char_indices().last() {
        Some((at, 's' | 'm' | 'h')) => (&text[..at], &text[at..]),
        _ => (text, "s"),
    };
    let n: f64 = number.parse().map_err(|_| bad())?;
    if !n.is_finite() || n <= 0.0 {
        return Err(bad());
    }
    let seconds = n * match unit {
        "m" => 60.0,
        "h" => 3600.0,
        _ => 1.0,
    };
    // A number too large for a Duration (1e20) is refused, never a panic (ATM-666, audit C5)
    std::time::Duration::try_from_secs_f64(seconds).map_err(|_| bad())
}
