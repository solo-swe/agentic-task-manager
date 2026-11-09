//! Shell completion (T-451; the Python atm's `cli/main.py` completers): `clap_complete`'s dynamic completion, with
//! atm's own values (task IDs with their names, sprints, releases, components, statuses, types, priorities, the targets
//! of `task state`, docs topics, harnesses), read through atm-lib in the folder the shell is in. Completion never fails
//! loudly: outside a project, or with a task file atm can't read, it completes nothing. `atm completion <shell>`
//! prints the line that turns it on.

use atm_common::Vocabulary;
use atm_common::py::py_str;
use atm_lib::Project;
use atm_lib::query::{Filters, task_views};
use clap_complete::engine::{ArgValueCandidates, CompletionCandidate};
use serde_json::Value;

/// The targets of `atm task state`, with what each runs.
const STATE_TARGETS: [(&str, &str); 6] = [
    ("in_progress", "start"),
    ("in_review", "review"),
    ("done", "done"),
    (
        "todo",
        "back to to do: fail, release, drop (rejected), unblock or reopen",
    ),
    ("blocked", "block"),
    ("skipped", "skip"),
];

/// What a value completes to: (value, its help).
type Values = fn() -> Vec<(String, String)>;

fn here() -> Option<atm_lib::Project> {
    Project::open(&std::env::current_dir().ok()?).ok()
}

fn with_vocabulary(pick: fn(&Vocabulary) -> Vec<(String, String)>) -> Vec<(String, String)> {
    here()
        .and_then(|p| p.store().vocabulary().ok().map(pick))
        .unwrap_or_default()
}

fn text(value: Option<&Value>) -> String {
    value
        .filter(|v| atm_common::py::truthy(v))
        .map(py_str)
        .unwrap_or_default()
}

fn task_ids() -> Vec<(String, String)> {
    let Some(project) = here() else {
        return Vec::new();
    };
    task_views(&project, &Filters::default())
        .unwrap_or_default()
        .iter()
        .filter_map(|v| {
            let id = v["task"].get("id")?.as_str()?.to_string();
            Some((id, text(v["task"].get("name"))))
        })
        .collect()
}

fn sprints() -> Vec<(String, String)> {
    with_vocabulary(|v| {
        v.sprints
            .iter()
            .filter(|s| s.get("state").and_then(Value::as_str) != Some("closed"))
            .map(|s| {
                let help = text(s.get("goal"));
                let help = if help.is_empty() {
                    text(s.get("state"))
                } else {
                    help
                };
                (text(s.get("id")), help)
            })
            .collect()
    })
}

fn releases() -> Vec<(String, String)> {
    with_vocabulary(|v| {
        v.releases
            .iter()
            .map(|r| {
                let help = text(r.get("name"));
                let help = if help.is_empty() {
                    text(r.get("state"))
                } else {
                    help
                };
                (text(r.get("id")), help)
            })
            .collect()
    })
}

fn components() -> Vec<(String, String)> {
    with_vocabulary(|v| {
        v.components
            .iter()
            .map(|c| (text(c.get("id")), text(c.get("label"))))
            .collect()
    })
}

fn statuses() -> Vec<(String, String)> {
    with_vocabulary(|v| {
        v.statuses
            .iter()
            .map(|s| (s.id.clone(), text(Some(&s.label))))
            .collect()
    })
}

fn types() -> Vec<(String, String)> {
    with_vocabulary(|v| {
        v.types
            .iter()
            .map(|t| (t.id.clone(), text(Some(&t.label))))
            .collect()
    })
}

fn priorities() -> Vec<(String, String)> {
    with_vocabulary(|v| {
        v.priorities
            .iter()
            .map(|p| (p.id.clone(), text(Some(&p.label))))
            .collect()
    })
}

fn state_targets() -> Vec<(String, String)> {
    STATE_TARGETS
        .iter()
        .map(|(v, h)| ((*v).to_string(), (*h).to_string()))
        .collect()
}

fn topics() -> Vec<(String, String)> {
    super::meta::AGENT_DOCS
        .iter()
        .map(|(name, summary, _)| ((*name).to_string(), (*summary).to_string()))
        .collect()
}

fn harnesses() -> Vec<(String, String)> {
    atm_lib::agents::HARNESSES
        .iter()
        .map(|h| (h.id.to_string(), h.label.to_string()))
        .collect()
}

/// The completer of an argument of the command at `path` (`"task state"`), if atm completes it: by the command and
/// the argument, else by the argument wherever it is.
fn values_for(path: &str, arg: &str) -> Option<Values> {
    let specific: Option<Values> = match (path, arg) {
        ("task state", "next_state") => Some(state_targets),
        // Archived tasks, which the task list doesn't hold
        ("db unarchive", "ids") => return None,
        ("sprint edit" | "sprint start" | "sprint close" | "sprint delete", "name")
        | ("sprint close", "carry_to")
        | ("sprint delete", "move_to")
        | ("sprint move", "to") => Some(sprints),
        ("release edit" | "release archive" | "release notes", "release_id") => Some(releases),
        ("component edit", "component_id") => Some(components),
        ("docs", "topic") => Some(topics),
        ("agents install" | "agents uninstall", "harness") => Some(harnesses),
        _ => None,
    };
    specific.or(match arg {
        "task_id" | "task_ids" | "ids" => Some(task_ids),
        "sprint" => Some(sprints),
        "release" => Some(releases),
        "component" => Some(components),
        "status" => Some(statuses),
        "type_" => Some(types),
        "priority" => Some(priorities),
        _ => None,
    })
}

/// `values` as completion candidates, nothing on any error (completion must never break the shell).
fn candidates(values: Values) -> Vec<CompletionCandidate> {
    std::panic::catch_unwind(values)
        .unwrap_or_default()
        .into_iter()
        .map(|(value, help)| {
            let candidate = CompletionCandidate::new(value);
            if help.is_empty() {
                candidate
            } else {
                candidate.help(Some(help.into()))
            }
        })
        .collect()
}

/// The command at `path` and its subcommands, with atm's completers on their arguments, and no `help` subcommand in
/// any group (`atm help` is atm's own).
#[must_use]
pub fn settle(command: clap::Command, path: &str) -> clap::Command {
    let ids: Vec<String> = command
        .get_arguments()
        .map(|a| a.get_id().to_string())
        .collect();
    // `mut_arg` moves the argument it changes to the end: positionals keep their places by an explicit index
    let positionals: Vec<String> = command
        .get_positionals()
        .map(|a| a.get_id().to_string())
        .collect();
    let mut command = command.disable_help_subcommand(true);
    for (at, id) in positionals.into_iter().enumerate() {
        command = command.mut_arg(id, |a| a.index(at + 1));
    }
    for id in ids {
        if let Some(values) = values_for(path, &id) {
            command = command.mut_arg(id, |a| {
                a.add(ArgValueCandidates::new(move || candidates(values)))
            });
        }
    }
    let names: Vec<String> = command
        .get_subcommands()
        .map(|s| s.get_name().to_string())
        .collect();
    for name in names {
        let sub_path = if path.is_empty() {
            name.clone()
        } else {
            format!("{path} {name}")
        };
        command = command.mut_subcommand(name, |s| settle(s, &sub_path));
    }
    command
}

/// `atm completion <shell>`: the line to add to the shell's startup file.
#[must_use]
pub fn activation(shell: &str) -> &'static str {
    match shell {
        "zsh" => "source <(COMPLETE=zsh atm)",
        "fish" => "COMPLETE=fish atm | source",
        "powershell" => {
            "$env:COMPLETE = \"powershell\"; atm | Out-String | Invoke-Expression; Remove-Item Env:\\COMPLETE"
        }
        _ => "source <(COMPLETE=bash atm)",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_are_found_by_command_and_argument() {
        assert!(values_for("task state", "next_state").is_some());
        assert!(values_for("db unarchive", "ids").is_none());
        assert!(values_for("task details", "task_ids").is_some());
        assert!(values_for("release add", "name").is_none());
        assert_eq!(state_targets()[0], ("in_progress".into(), "start".into()));
        assert_eq!(activation("fish"), "COMPLETE=fish atm | source");
    }
}
