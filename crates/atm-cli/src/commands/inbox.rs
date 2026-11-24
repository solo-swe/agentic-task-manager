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
    },
    /// An item in full: its text, state, and what became of it
    Show {
        /// The item (IN-3)
        item: String,
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

/// One line per item: its ID, its state, its title, and what became of it
fn line(item: &Value) -> String {
    let state = field(item, "state");
    let outcome = match state {
        "discarded" => format!(" · {}", field(item, "reason")),
        "injected" => format!(
            " → {}",
            item.get("tasks")
                .and_then(Value::as_array)
                .map(|t| t
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(", "))
                .unwrap_or_default()
        ),
        _ => String::new(),
    };
    format!("{}  {state}  {}{outcome}", field(item, "id"), title(item))
}

/// What a move did, as one line, or the item as JSON
fn report(ctx: Context, item: &Value, did: &str) {
    if ctx.json {
        print_json(item);
    } else {
        println!("{}: {did}", field(item, "id"));
    }
}

/// `atm inbox list`: the items `state` names (the open ones without it), of the inbox or its archive
fn list(ctx: Context, cwd: &Path, state: Option<&str>, archived: bool) -> Result<()> {
    let listed = inbox::list(&Project::open(cwd)?, inbox::listing_of(state)?, archived)?;
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

/// `atm inbox show`: an item in full
fn show(ctx: Context, cwd: &Path, id: &str) -> Result<()> {
    let item = inbox::show(&Project::open(cwd)?, id)?;
    if ctx.json {
        print_json(&item);
        return Ok(());
    }
    println!("{}", line(&item));
    println!(
        "  created {} by {}",
        field(&item, "created_at"),
        field(&item, "created_by")
    );
    if !field(&item, "decided_at").is_empty() {
        println!(
            "  decided {} by {}",
            field(&item, "decided_at"),
            field(&item, "decided_by")
        );
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
        InboxCommand::List { state, archived } => list(ctx, cwd, state.as_deref(), archived)?,
        InboxCommand::Show { item } => show(ctx, cwd, &item)?,
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
