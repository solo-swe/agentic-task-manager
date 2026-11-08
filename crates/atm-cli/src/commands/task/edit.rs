//! `atm task edit <id>` (the Python atm's `task_edit`): a task's definition, never its status; what changed, in words.

use std::path::Path;

use atm_boot::{agent_identity, open_project};
use atm_common::py::py_str;
use atm_common::{Error, Result};
use atm_lib::edit::{TaskEdit, edit_existing_task};
use clap::Args;
use serde_json::Value;

use super::add::suggestion_line;
use crate::commands::{Context, print_json};

/// `atm task edit`'s arguments and options.
#[derive(Debug, Args)]
#[allow(clippy::struct_excessive_bools)] // reason: each is one of the command's flags, as the Python atm has them
pub struct EditArgs {
    /// The task ID
    task_id: String,
    #[arg(long)]
    name: Option<String>,
    #[arg(long)]
    description: Option<String>,
    #[arg(long)]
    description_file: Option<String>,
    #[arg(long)]
    sprint: Option<String>,
    #[arg(long)]
    backlog: bool,
    #[arg(long = "type")]
    type_: Option<String>,
    #[arg(long)]
    priority: Option<String>,
    #[arg(long)]
    release: Option<String>,
    #[arg(long)]
    review: Option<String>,
    #[arg(long)]
    add_file: Vec<String>,
    #[arg(long)]
    remove_file: Vec<String>,
    #[arg(long)]
    add_dep: Vec<String>,
    #[arg(long)]
    remove_dep: Vec<String>,
    #[arg(long)]
    add_label: Vec<String>,
    #[arg(long)]
    remove_label: Vec<String>,
    #[arg(long)]
    add_component: Vec<String>,
    #[arg(long)]
    remove_component: Vec<String>,
    #[arg(long)]
    add_related: Vec<String>,
    #[arg(long)]
    remove_related: Vec<String>,
    #[arg(long)]
    add_requirement: Vec<String>,
    #[arg(long)]
    remove_requirement: Vec<String>,
    #[arg(long)]
    add_use_case: Vec<String>,
    #[arg(long)]
    remove_use_case: Vec<String>,
    /// auto: add the components its files are in (otherwise only suggested)
    #[arg(long = "components", value_parser = ["auto"])]
    components_auto: Option<String>,
    #[arg(long = "with")]
    with_user: Option<String>,
    #[arg(long)]
    agent: Option<String>,
}

/// Run `atm task edit` in `cwd`.
///
/// # Errors
/// Options that can't go together, a description file that can't be read, and what atm-lib refuses.
pub fn edit(args: EditArgs, ctx: Context, cwd: &Path) -> Result<()> {
    let file = args.description_file.clone().filter(|f| !f.is_empty());
    if args.description.is_some() && file.is_some() {
        return Err(Error::Invalid(
            "--description and --description-file can't both be given".into(),
        ));
    }
    if args.sprint.is_some() && args.backlog {
        return Err(Error::Invalid(
            "--sprint and --backlog can't both be given".into(),
        ));
    }
    if ctx.dry_run {
        println!("[dry-run] would edit task {}", args.task_id);
        return Ok(());
    }
    let description =
        match &file {
            Some(file) => Some(std::fs::read_to_string(cwd.join(file)).map_err(|e| {
                Error::failed(format!("can't read the description file {file}: {e}"))
            })?),
            None => args.description,
        };
    let added_files = !args.add_file.is_empty();
    let change = TaskEdit {
        name: args.name,
        description,
        sprint: args.sprint,
        backlog: args.backlog,
        type_: args.type_,
        priority: args.priority,
        release: args.release,
        review: args.review,
        with_: args.with_user,
        components_auto: args.components_auto.is_some(),
        add_files: args.add_file,
        remove_files: args.remove_file,
        add_dependencies: args.add_dep,
        remove_dependencies: args.remove_dep,
        add_labels: args.add_label,
        remove_labels: args.remove_label,
        add_components: args.add_component,
        remove_components: args.remove_component,
        add_related: args.add_related,
        remove_related: args.remove_related,
        add_requirements: args.add_requirement,
        remove_requirements: args.remove_requirement,
        add_use_cases: args.add_use_case,
        remove_use_cases: args.remove_use_case,
    };
    let actor = agent_identity(args.agent.as_deref(), cwd);
    let mut project = open_project(cwd)?.with_actor(Some(actor));
    let edited = edit_existing_task(&mut project, &args.task_id, &change)?;
    if ctx.json {
        print_json(&Value::Object(edited.task));
        return Ok(());
    }
    let id = py_str(&edited.task["id"]);
    if edited.changed.is_empty() {
        println!("{id}: nothing changed");
    } else {
        println!("{id}: changed {}", edited.changed.join(", "));
    }
    // New files may put the task in components it isn't in yet (SR-2.7): suggested, not set
    if added_files && let Some(line) = suggestion_line(&edited.vocab, &edited.task) {
        println!("{line}");
    }
    Ok(())
}
