//! `atm inbox`: the project's inbox (docs/inbox-design.md §3, ATM-760), raw input a person notes for later: `IN-<n>`
//! items, each discarded with a reason or injected into the tasks made from it. atm-lib's `inbox` holds the rules (who
//! may move an item, from which state); these commands only ask, and print.

use std::path::Path;

use atm_common::{Error, Result};
use atm_lib::Project;
use atm_lib::actor::agent_identity;
use atm_lib::inbox;
use clap::{Args, Subcommand};
use serde_json::{Value, json};

use super::{Context, print_json};

/// An item's text: given, or from a file.
#[derive(Debug, Args, Default)]
pub struct Text {
    /// The text (Markdown)
    #[arg(conflicts_with = "text_file")]
    text: Option<String>,
    /// The text, from a file
    #[arg(long, value_name = "FILE")]
    text_file: Option<String>,
}

/// The inbox commands.
#[derive(Debug, Subcommand)]
pub enum InboxCommand {
    /// Note something for later: a new open item (to this project's, a person only; to another's, an agent too)
    Add {
        #[command(flatten)]
        text: Text,
        /// Who adds it (by default the git user, or the agent's session)
        #[arg(long)]
        agent: Option<String>,
        /// Another registered project's inbox instead (its key or ID): a finding for its user to triage, recorded as
        /// coming from this project
        #[arg(long, value_name = "KEY")]
        project: Option<String>,
    },
    /// Change an open item's text (a person only)
    Edit {
        /// The item (IN-3)
        item: String,
        #[command(flatten)]
        text: Text,
        #[arg(long)]
        agent: Option<String>,
    },
    /// The items: the open ones, newest first; --state for others, --archived for the archive's
    List {
        /// created (the open ones, the default), discarded, injected or all
        #[arg(long, value_parser = ["created", "discarded", "injected", "all"])]
        state: Option<String>,
        /// The archive's items instead
        #[arg(long)]
        archived: bool,
        /// Another registered project's inbox (its key or ID): read only
        #[arg(long, value_name = "KEY")]
        project: Option<String>,
        /// Only the items you added (with --project, those you left there from this project)
        #[arg(long)]
        mine: bool,
        #[arg(long)]
        agent: Option<String>,
    },
    /// An item in full: its text, state, and what became of it (its tasks' statuses once injected)
    Show {
        /// The item (IN-3; KEY:IN-3 for another registered project's)
        item: String,
        /// Another registered project's inbox (its key or ID): read only
        #[arg(long, value_name = "KEY")]
        project: Option<String>,
    },
    /// Discard an open item, saying why (on the user's word, for an agent)
    Discard {
        /// The item (IN-3)
        item: String,
        /// Why (at most 500 characters)
        #[arg(long)]
        reason: String,
        #[arg(long)]
        agent: Option<String>,
    },
    /// Record the tasks an open item became: the item injected, each task's `from` naming it. Final
    Inject {
        /// The item (IN-3)
        item: String,
        /// A task made from it (its key); repeat for each
        #[arg(long = "task", value_name = "KEY", required = true)]
        tasks: Vec<String>,
        #[arg(long)]
        agent: Option<String>,
    },
    /// Bring a discarded item back to open (a person only)
    Restore {
        /// The item (IN-3)
        item: String,
        #[arg(long)]
        agent: Option<String>,
    },
}

fn opened(cwd: &Path, agent: Option<&str>) -> Result<Project> {
    Ok(Project::open(cwd)?.with_actor(Some(agent_identity(agent, cwd))))
}

fn text_of(text: Text, cwd: &Path) -> Result<String> {
    match (text.text, text.text_file) {
        (Some(text), _) => Ok(text),
        (None, Some(file)) => crate::option_file::read_option_file(cwd, "text-file", &file),
        (None, None) => Err(Error::Invalid(
            "give the text, or --text-file with it".into(),
        )),
    }
}

fn field<'v>(item: &'v Value, key: &str) -> &'v str {
    item.get(key).and_then(Value::as_str).unwrap_or_default()
}

/// The text's first line, as the item's title: cut at 80 characters
fn title(item: &Value) -> String {
    let first = field(item, "text")
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or_default();
    let mut title: String = first.chars().take(80).collect();
    if first.chars().count() > 80 {
        title.push('…');
    }
    title
}

/// What an injected item's tasks are now, as one line's end: `ATM-760 (done), ATM-761 (in_progress)`; a task not
/// found says so (ATM-773). Without their states, their keys.
fn tasks_line(item: &Value) -> String {
    if let Some(states) = item.get("task_states").and_then(Value::as_array) {
        return states
            .iter()
            .map(|s| {
                let key = field(s, "key");
                if s.get("found") == Some(&json!(true)) {
                    format!("{key} ({})", field(s, "status"))
                } else {
                    format!("{key} (not found)")
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
    }
    item.get("tasks")
        .and_then(Value::as_array)
        .map(|t| {
            t.iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default()
}

/// One line per item: its ID, its state, its title, and what became of it
fn line(item: &Value) -> String {
    let state = field(item, "state");
    let outcome = match state {
        "discarded" => format!(" · {}", field(item, "reason")),
        "injected" => format!(" → {}", tasks_line(item)),
        _ => String::new(),
    };
    format!("{}  {state}  {}{outcome}", field(item, "id"), title(item))
}

/// The project an inbox is read from (ATM-773): this one, or the user's registered one `project` names (its key or
/// ID, never a path), read only.
fn inbox_project(cwd: &Path, project: Option<&str>) -> Result<Project> {
    let here = Project::open(cwd)?;
    match project {
        None => Ok(here),
        Some(name) => Project::open(&atm_lib::foreign::registered_project(name, here.root())?),
    }
}

/// An item as `atm inbox show` takes it: `IN-3`, or `KEY:IN-3` for another registered project's (as `--project KEY`)
fn item_and_project<'a>(
    item: &'a str,
    project: Option<&'a str>,
) -> Result<(&'a str, Option<&'a str>)> {
    match item.split_once(':') {
        Some((key, id)) if !key.trim().is_empty() => {
            if project.is_some_and(|p| !p.eq_ignore_ascii_case(key.trim())) {
                return Err(Error::Invalid(format!(
                    "{item} names the project {key}, and --project another: give one"
                )));
            }
            Ok((id, Some(key.trim())))
        }
        _ => Ok((item, project)),
    }
}

/// What a move did, as one line, or the item as JSON
fn report(ctx: Context, item: &Value, did: &str) {
    if ctx.json {
        print_json(item);
    } else {
        println!("{}: {did}", field(item, "id"));
    }
}

/// What `atm inbox list` lists: of which state, the archive's, of which project, only whose
struct ListOf<'a> {
    state: Option<&'a str>,
    archived: bool,
    project: Option<&'a str>,
    mine: bool,
    agent: Option<&'a str>,
}

/// `atm inbox list`: the items `state` names (the open ones without it), of the inbox or its archive, of this project
/// or another registered one; `mine`, those the one asking added (from this project, to another's)
fn list(ctx: Context, cwd: &Path, of: &ListOf) -> Result<()> {
    let project = inbox_project(cwd, of.project)?;
    let mut listed = inbox::list(&project, inbox::listing_of(of.state)?, of.archived)?;
    if of.mine {
        let who = agent_identity(of.agent, cwd);
        let from = match of.project {
            Some(_) => Some(inbox::project_name(&Project::open(cwd)?)?),
            None => None,
        };
        listed = inbox::added_by(listed, &who, from.as_deref());
    }
    let archived = of.archived;
    if ctx.json {
        print_json(&json!({
            "items": listed.items,
            "created": listed.created,
            "discarded": listed.discarded,
            "injected": listed.injected,
            "archived": archived,
        }));
        return Ok(());
    }
    for item in &listed.items {
        println!("{}", line(item));
    }
    let total = listed.created + listed.discarded + listed.injected;
    let place = if archived { "archived" } else { "in the inbox" };
    println!(
        "{total} item{} {place}: {} open, {} discarded, {} injected",
        if total == 1 { "" } else { "s" },
        listed.created,
        listed.discarded,
        listed.injected,
    );
    Ok(())
}

/// `atm inbox show`: an item in full, of this project or another registered one (`--project`, `KEY:IN-3`)
fn show(ctx: Context, cwd: &Path, item: &str, project: Option<&str>) -> Result<()> {
    let (id, project) = item_and_project(item, project)?;
    let item = inbox::show(&inbox_project(cwd, project)?, id)?;
    if ctx.json {
        print_json(&item);
        return Ok(());
    }
    println!("{}", line(&item));
    println!(
        "  created {} by {}{}",
        field(&item, "created_at"),
        field(&item, "created_by"),
        match field(&item, "from_project") {
            "" => String::new(),
            from => format!(", from {from}"),
        }
    );
    if !field(&item, "decided_at").is_empty() {
        println!(
            "  decided {} by {}",
            field(&item, "decided_at"),
            field(&item, "decided_by")
        );
    }
    for state in item
        .get("task_states")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if state.get("found") == Some(&json!(true)) {
            println!(
                "  {}  {}  {}{}",
                field(state, "key"),
                field(state, "status"),
                field(state, "name"),
                if state.get("archived") == Some(&json!(true)) {
                    " (archived)"
                } else {
                    ""
                }
            );
        } else {
            println!("  {}  not found", field(state, "key"));
        }
    }
    println!();
    for text in field(&item, "text").lines() {
        println!("  {text}");
    }
    Ok(())
}

/// `atm inbox add --project`: an item in another registered project's inbox (ATM-772), through the same `inbox::add`,
/// recorded as coming from this project. The project is the user's registered one the key or ID names, never a path.
fn add_elsewhere(
    ctx: Context,
    cwd: &Path,
    name: &str,
    text: Text,
    agent: Option<&str>,
) -> Result<()> {
    let text = text_of(text, cwd)?;
    let here = Project::open(cwd)?;
    let from = inbox::project_name(&here)?;
    let folder = atm_lib::foreign::registered_project(name, here.root())?;
    let mut there = Project::open(&folder)?.with_actor(Some(agent_identity(agent, cwd)));
    let to = inbox::project_name(&there)?;
    if ctx.dry(&format!("add an item to {to}'s inbox, from {from}")) {
        return Ok(());
    }
    let item = inbox::add(&mut there, &text, Some(&from))?;
    if ctx.json {
        // With its project, so the agent can ask about it later (atm inbox show --project, ATM-773)
        let mut answer = item.clone();
        answer["project"] = json!(to);
        print_json(&answer);
    } else {
        println!(
            "{}: added to {to}'s inbox · {}",
            field(&item, "id"),
            title(&item)
        );
    }
    Ok(())
}

/// Run an inbox command in `cwd`.
///
/// # Errors
/// What atm-lib refuses (an agent adding, an item not open, a task that doesn't exist), and an inbox that can't be
/// read.
pub fn run(command: InboxCommand, ctx: Context, cwd: &Path) -> Result<()> {
    match command {
        InboxCommand::Add {
            text,
            agent,
            project: None,
        } => {
            let text = text_of(text, cwd)?;
            if ctx.dry("add an item to the inbox") {
                return Ok(());
            }
            let item = inbox::add(&mut opened(cwd, agent.as_deref())?, &text, None)?;
            report(ctx, &item, &format!("added · {}", title(&item)));
        }
        InboxCommand::Add {
            text,
            agent,
            project: Some(name),
        } => add_elsewhere(ctx, cwd, &name, text, agent.as_deref())?,
        InboxCommand::Edit { item, text, agent } => {
            let text = text_of(text, cwd)?;
            if ctx.dry(&format!("change {item}'s text")) {
                return Ok(());
            }
            let item = inbox::edit(&mut opened(cwd, agent.as_deref())?, &item, &text)?;
            report(ctx, &item, "text changed");
        }
        InboxCommand::List {
            state,
            archived,
            project,
            mine,
            agent,
        } => list(
            ctx,
            cwd,
            &ListOf {
                state: state.as_deref(),
                archived,
                project: project.as_deref(),
                mine,
                agent: agent.as_deref(),
            },
        )?,
        InboxCommand::Show { item, project } => show(ctx, cwd, &item, project.as_deref())?,
        InboxCommand::Discard {
            item,
            reason,
            agent,
        } => {
            if ctx.dry(&format!("discard {item}")) {
                return Ok(());
            }
            let item = inbox::discard(&mut opened(cwd, agent.as_deref())?, &item, &reason)?;
            report(ctx, &item, &format!("discarded · {reason}"));
        }
        InboxCommand::Inject { item, tasks, agent } => {
            if ctx.dry(&format!("record that {item} became {}", tasks.join(", "))) {
                return Ok(());
            }
            let item = inbox::inject(&mut opened(cwd, agent.as_deref())?, &item, &tasks)?;
            let keys = item
                .get("tasks")
                .and_then(Value::as_array)
                .map(|t| {
                    t.iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            report(ctx, &item, &format!("injected → {keys}"));
        }
        InboxCommand::Restore { item, agent } => {
            if ctx.dry(&format!("restore {item}")) {
                return Ok(());
            }
            let item = inbox::restore(&mut opened(cwd, agent.as_deref())?, &item)?;
            report(ctx, &item, "open again");
        }
    }
    Ok(())
}
