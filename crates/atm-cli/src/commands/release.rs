//! `atm release`: releases (the Python atm's `cli/commands/release.py`): list, add, edit, rename, archive, notes, and
//! how they print.

use std::fmt::Write;
use std::path::Path;

use atm_common::Result;
use atm_common::py::{py_str, truthy};
use atm_lib::Project;
use atm_lib::releases::{
    Mark, add_release, archive_release, edit_release, list_releases, mark_release, release_notes,
    rename_release,
};
use clap::Subcommand;
use serde_json::{Value, json};

use super::{Context, print_json};

/// The release commands.
#[derive(Debug, Subcommand)]
pub enum ReleaseCommand {
    /// List releases with their tasks by kind and stage.
    List {
        /// List archived releases too
        #[arg(long)]
        all: bool,
    },
    /// Add a release.
    Add {
        /// The release's ID (letters, digits, '.', '-', '_')
        release_id: String,
        /// Add, edit: a name besides its ID (empty: remove it)
        #[arg(long)]
        name: Option<String>,
        /// Add, edit: the planned date, YYYY-MM-DD
        #[arg(long, value_name = "DATE")]
        target: Option<String>,
        /// Add: give it the tasks of pull request N (repeatable)
        #[arg(long, value_name = "N")]
        pr: Vec<i64>,
        /// Add: it shipped already, at this time (e.g. its PR's merge)
        #[arg(long, value_name = "TIME")]
        released_at: Option<String>,
    },
    /// Edit a release.
    Edit {
        /// The release's ID (letters, digits, '.', '-', '_')
        release_id: String,
        /// Add, edit: a name besides its ID (empty: remove it)
        #[arg(long)]
        name: Option<String>,
        /// Add, edit: the planned date, YYYY-MM-DD
        #[arg(long, value_name = "DATE")]
        target: Option<String>,
        /// It shipped: mark it released, on DATE (YYYY-MM-DD) or today
        #[arg(long, value_name = "DATE", num_args = 0..=1, default_missing_value = "", conflicts_with = "planned")]
        released: Option<String>,
        /// Mark it planned again
        #[arg(long)]
        planned: bool,
        /// With --released: even with tasks still open in it
        #[arg(long)]
        force: bool,
    },
    /// Rename a release's ID: its tasks (archived ones too) and the sprints delivering to it follow.
    Rename {
        /// The release's ID
        release_id: String,
        /// Its new ID (letters, digits, '.', '-', '_')
        new: String,
    },
    /// Archive a release.
    Archive {
        /// The release's ID (letters, digits, '.', '-', '_')
        release_id: String,
    },
    /// Print a Markdown draft of a release's changes.
    Notes {
        /// The release's ID (letters, digits, '.', '-', '_')
        release_id: String,
    },
}

fn text(value: &Value, key: &str) -> Option<String> {
    value.get(key).filter(|v| truthy(v)).map(py_str)
}

fn titled(r: &Value) -> String {
    let mut title = py_str(&r["id"]);
    if let Some(name) = text(r, "name") {
        let _ = write!(title, " “{name}”");
    }
    title
}

fn describe_release(r: &Value) -> String {
    let mut details = vec![py_str(&r["state"])];
    if let Some(target) = text(r, "target") {
        details.push(format!("target {target}"));
    }
    if let Some(at) = text(r, "released_at") {
        details.push(format!(
            "released {}",
            at.chars().take(10).collect::<String>()
        ));
    }
    if let Some(number) = r
        .get("pr")
        .filter(|p| p.is_object())
        .and_then(|p| text(p, "number"))
    {
        details.push(format!("PR #{number}"));
    }
    let counts = |key: &str, words: fn(&str) -> String| -> Vec<String> {
        r[key]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(k, n)| format!("{} {}", words(k), py_str(n)))
            .collect()
    };
    let kinds = counts("by_kind", str::to_string);
    let stages = counts("by_stage", |s| s.replace('_', " "));
    let tasks = r["tasks"].as_u64().unwrap_or(0);
    let mut line = format!(
        "{} · {}: {tasks} task{}",
        titled(r),
        details.join(" · "),
        if tasks == 1 { "" } else { "s" }
    );
    if !kinds.is_empty() {
        let _ = write!(line, " ({})", kinds.join(", "));
    }
    if !stages.is_empty() {
        let _ = write!(line, "; {}", stages.join(", "));
    }
    line
}

/// The draft as Markdown: a heading per type, a line per task; what's not done yet at the end.
fn describe_notes(notes: &Value) -> Vec<String> {
    let mut lines = vec![format!("# {}", titled(&notes["release"]))];
    let groups = notes["groups"].as_array().cloned().unwrap_or_default();
    if groups.is_empty() {
        lines.extend([String::new(), "Nothing done in it yet.".into()]);
    }
    for group in &groups {
        lines.extend([
            String::new(),
            format!("## {}", py_str(&group["label"])),
            String::new(),
        ]);
        for t in group["tasks"].as_array().into_iter().flatten() {
            let mut line = format!("- **{}** {}", py_str(&t["id"]), py_str(&t["name"]));
            if let Some(outcome) = text(t, "outcome") {
                let _ = write!(line, ": {outcome}");
            }
            lines.push(line);
        }
    }
    let open = notes["open"].as_array().cloned().unwrap_or_default();
    if !open.is_empty() {
        lines.extend([String::new(), "## Not done yet".into(), String::new()]);
        lines.extend(open.iter().map(|t| {
            format!(
                "- {} {} ({})",
                py_str(&t["id"]),
                py_str(&t["name"]),
                py_str(&t["status"])
            )
        }));
    }
    lines
}

/// Run a release command in `cwd`.
///
/// # Errors
/// What atm-lib refuses, and a task file that can't be read.
#[allow(clippy::too_many_lines)] // reason: one arm per command, as the Python atm's release_app has them
pub fn run(command: ReleaseCommand, ctx: Context, cwd: &Path) -> Result<()> {
    match command {
        ReleaseCommand::List { all } => {
            let (releases, any) = list_releases(&Project::open(cwd)?, all)?;
            if ctx.json {
                print_json(&Value::Array(releases));
            } else if releases.is_empty() {
                println!(
                    "{}",
                    if any {
                        "No open releases (--all lists the archived ones)."
                    } else {
                        "No releases yet: atm release add <R>."
                    }
                );
            } else {
                for r in &releases {
                    println!("{}", describe_release(r));
                }
            }
        }
        ReleaseCommand::Add {
            release_id,
            name,
            target,
            pr,
            released_at,
        } => {
            if ctx.dry_run {
                let mut parts = vec![release_id.clone()];
                if let Some(name) = name.as_deref().filter(|n| !n.is_empty()) {
                    parts.push(format!("--name {name}"));
                }
                if let Some(target) = target.as_deref().filter(|t| !t.is_empty()) {
                    parts.push(format!("--target {target}"));
                }
                parts.extend(pr.iter().map(|n| format!("--pr {n}")));
                if let Some(at) = released_at.as_deref().filter(|t| !t.is_empty()) {
                    parts.push(format!("--released-at {at}"));
                }
                println!("[dry-run] would add release {}", parts.join(" "));
                return Ok(());
            }
            let added = add_release(
                &mut Project::open(cwd)?,
                &release_id,
                name.as_deref(),
                target.as_deref(),
                &pr,
                released_at.as_deref(),
            )?;
            if ctx.json {
                print_json(&json!({"release": added.release, "tasks": added.tasks}));
                return Ok(());
            }
            let mut line = format!(
                "Added the release “{release_id}” ({})",
                py_str(&added.release["state"])
            );
            if !added.tasks.is_empty() {
                let numbers: Vec<String> = pr.iter().map(|n| format!("#{n}")).collect();
                let n = added.tasks.len();
                let _ = write!(
                    line,
                    ", with the {n} task{} of PR{} {}",
                    if n == 1 { "" } else { "s" },
                    if pr.len() > 1 { "s" } else { "" },
                    numbers.join(", ")
                );
            }
            println!("{line}.");
        }
        ReleaseCommand::Edit {
            release_id,
            name,
            target,
            released,
            planned,
            force,
        } => {
            let mark = match (&released, planned) {
                (Some(date), _) => Some(Mark::Released(
                    Some(date.as_str()).filter(|d| !d.is_empty()),
                )),
                (None, true) => Some(Mark::Planned),
                (None, false) => None,
            };
            if force && !matches!(mark, Some(Mark::Released(_))) {
                return Err(atm_common::Error::Invalid(
                    "--force goes with --released".into(),
                ));
            }
            if ctx.dry_run {
                let mut parts = vec![release_id.clone()];
                if let Some(name) = &name {
                    parts.push(format!("--name {name}"));
                }
                if let Some(target) = &target {
                    parts.push(format!("--target {target}"));
                }
                match &mark {
                    Some(Mark::Released(date)) => {
                        parts.push(format!("--released {}", date.unwrap_or("(today)")));
                    }
                    Some(Mark::Planned) => parts.push("--planned".into()),
                    None => {}
                }
                println!("[dry-run] would edit release {}", parts.join(" "));
                return Ok(());
            }
            let mut project = Project::open(cwd)?;
            let fields = name.is_some() || target.is_some() || mark.is_none();
            let (mut release, changed) = if fields {
                edit_release(
                    &mut project,
                    &release_id,
                    name.as_deref(),
                    target.as_deref(),
                )?
            } else {
                (Value::Null, Vec::new())
            };
            let marked = match &mark {
                Some(mark) => Some(mark_release(&mut project, &release_id, mark, force)?),
                None => None,
            };
            if let Some(marked) = &marked {
                release = marked.release.clone();
            }
            if ctx.json {
                match &marked {
                    Some(m) => print_json(
                        &json!({"release": release, "open": m.open, "unmerged": m.unmerged}),
                    ),
                    None => print_json(&release),
                }
                return Ok(());
            }
            let id = py_str(&release["id"]);
            if fields {
                if changed.is_empty() && marked.is_none() {
                    println!("{id}: nothing changed");
                } else if !changed.is_empty() {
                    println!("{id}: changed {}", changed.join(", "));
                }
            }
            if let Some(marked) = &marked {
                if matches!(mark, Some(Mark::Planned)) {
                    println!("{id}: planned");
                } else {
                    let day = text(&release, "released_at")
                        .map(|at| at.chars().take(10).collect::<String>())
                        .unwrap_or_default();
                    println!("{id}: released {day}");
                    if !marked.unmerged.is_empty() {
                        println!(
                            "  done but not merged in a PR: {}",
                            marked.unmerged.join(", ")
                        );
                    }
                    if !marked.open.is_empty() {
                        println!("  still open (--force): {}", marked.open.join(", "));
                    }
                }
            }
        }
        ReleaseCommand::Rename { release_id, new } => {
            if ctx.dry_run {
                println!("[dry-run] would rename release {release_id} to {new}");
                return Ok(());
            }
            let renamed = rename_release(&mut Project::open(cwd)?, &release_id, &new)?;
            if ctx.json {
                print_json(&json!({
                    "from": renamed.from,
                    "to": renamed.to,
                    "tasks": renamed.tasks,
                    "archived": renamed.archived,
                    "sprints": renamed.sprints,
                }));
                return Ok(());
            }
            let n = renamed.tasks + renamed.archived;
            let mut line = format!(
                "Renamed the release “{}” to “{}”: {n} task{}",
                renamed.from,
                renamed.to,
                if n == 1 { "" } else { "s" }
            );
            if renamed.archived > 0 {
                let _ = write!(line, " ({} archived)", renamed.archived);
            }
            if renamed.sprints > 0 {
                let s = renamed.sprints;
                let _ = write!(line, " and {s} sprint{}", if s == 1 { "" } else { "s" });
            }
            println!("{line} follow it.");
        }
        ReleaseCommand::Archive { release_id } => {
            if ctx.dry_run {
                println!("[dry-run] would archive release {release_id}");
                return Ok(());
            }
            let release = archive_release(&mut Project::open(cwd)?, &release_id)?;
            if ctx.json {
                print_json(&release);
            } else {
                println!("Archived the release “{}”.", py_str(&release["id"]));
            }
        }
        ReleaseCommand::Notes { release_id } => {
            let notes = release_notes(&Project::open(cwd)?, &release_id)?;
            if ctx.json {
                print_json(&notes);
            } else {
                for line in describe_notes(&notes) {
                    println!("{line}");
                }
            }
        }
    }
    Ok(())
}
