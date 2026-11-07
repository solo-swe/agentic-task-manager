//! `atm task add` and `atm task note` (the Python atm's `task_add` and `task_note`): what they read beyond atm-lib (the
//! file `--json-file` names, a description file) and what they print (the components a new task's files suggest).

use std::path::Path;

use atm_boot::{agent_identity, open_project};
use atm_common::components::suggest_components;
use atm_common::py::py_str;
use atm_common::{Error, Result, Task, Vocabulary};
use atm_lib::changes::add_task_note;
use atm_lib::edit::add_new_tasks;
use clap::Args;
use serde_json::{Map, Value, json};

use crate::commands::{Context, print_json};

/// `atm task add`'s options.
#[derive(Debug, Args)]
pub struct AddArgs {
    /// A short name (about 60 characters)
    #[arg(long)]
    name: Option<String>,
    /// The task type
    #[arg(long = "type")]
    type_: Option<String>,
    /// The sprint it goes into
    #[arg(long)]
    sprint: Option<String>,
    /// The release it ships in
    #[arg(long)]
    release: Option<String>,
    /// The priority
    #[arg(long)]
    priority: Option<String>,
    /// A label (repeatable)
    #[arg(long)]
    label: Vec<String>,
    /// What exactly to do
    #[arg(long)]
    description: Option<String>,
    /// Read the description from a file
    #[arg(long)]
    description_file: Option<String>,
    /// A task it depends on (repeatable)
    #[arg(long)]
    dep: Vec<String>,
    /// A file it creates or changes (repeatable)
    #[arg(long)]
    file: Vec<String>,
    /// A requirement (repeatable)
    #[arg(long)]
    requirement: Vec<String>,
    /// A component (repeatable)
    #[arg(long)]
    component: Vec<String>,
    /// auto: the components its files are in (otherwise only suggested)
    #[arg(long = "components", value_parser = ["auto"])]
    components_auto: Option<String>,
    /// How it is reviewed
    #[arg(long)]
    review: Option<String>,
    /// A related task (repeatable)
    #[arg(long)]
    related: Vec<String>,
    /// user: done together with the user
    #[arg(long = "with")]
    with_user: Option<String>,
    /// Add many from a JSON file
    #[arg(long)]
    json_file: Option<String>,
    /// Who adds them
    #[arg(long)]
    agent: Option<String>,
}

/// `atm task add` / `task edit --add-file` (SR-2.7): the components the task's files suggest that it doesn't have yet,
/// as a line with the command that sets them; none when there are none (or the project has no components).
pub fn suggestion_line(vocab: &Vocabulary, task: &Task) -> Option<String> {
    let files = task
        .get("files")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let has = |c: &str| {
        task.get("components")
            .and_then(Value::as_array)
            .is_some_and(|list| list.iter().any(|v| v == c))
    };
    let missing: Vec<String> = suggest_components(vocab, &files)
        .into_iter()
        .filter(|c| !has(c))
        .collect();
    if missing.is_empty() {
        return None;
    }
    let id = py_str(&task["id"]);
    let adds: Vec<String> = missing
        .iter()
        .map(|c| format!("--add-component {c}"))
        .collect();
    Some(format!(
        "  {id}: its files suggest the component{} {} (atm task edit {id} {})",
        if missing.len() > 1 { "s" } else { "" },
        missing.join(", "),
        adds.join(" ")
    ))
}

/// The tasks of an `atm task add --json-file`: a list of task objects, or `{"tasks": [...]}`.
fn read_entries(path: &str, cwd: &Path) -> Result<Vec<Task>> {
    let refused = || {
        Error::Invalid(format!(
            "{path}: expected a JSON list of tasks (or {{\"tasks\": [...]}})"
        ))
    };
    let value: Value = std::fs::read_to_string(cwd.join(path))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .ok_or_else(refused)?;
    let value = match value {
        Value::Object(mut map) => map.shift_remove("tasks").unwrap_or(Value::Null),
        other => other,
    };
    match value {
        Value::Array(items) => items
            .into_iter()
            .map(|e| match e {
                Value::Object(task) => Ok(task),
                _ => Err(refused()),
            })
            .collect(),
        _ => Err(refused()),
    }
}

/// The one entry the options describe.
fn entry(args: AddArgs, description: Option<String>) -> Task {
    let list = |items: Vec<String>| json!(items);
    let mut fields = Map::new();
    fields.insert("name".into(), json!(args.name));
    fields.insert("type".into(), json!(args.type_));
    fields.insert("priority".into(), json!(args.priority));
    fields.insert("labels".into(), list(args.label));
    fields.insert("description".into(), json!(description.unwrap_or_default()));
    fields.insert("dependencies".into(), list(args.dep));
    fields.insert("files".into(), list(args.file));
    fields.insert("requirements".into(), list(args.requirement));
    fields.insert("sprint".into(), json!(args.sprint));
    fields.insert("release".into(), json!(args.release));
    let components = if args.components_auto.is_some() {
        json!("auto")
    } else {
        list(args.component)
    };
    fields.insert("components".into(), components);
    fields.insert("review".into(), json!(args.review));
    fields.insert("with".into(), json!(args.with_user));
    fields.insert("related".into(), list(args.related));
    fields
}

/// Run `atm task add` in `cwd`.
///
/// # Errors
/// Both `--description` and `--description-file`, a file that can't be read, and what atm-lib refuses.
pub fn add(args: AddArgs, ctx: Context, cwd: &Path) -> Result<()> {
    let given = |o: &Option<String>| o.as_deref().is_some_and(|v| !v.is_empty());
    if given(&args.description) && given(&args.description_file) {
        return Err(Error::Invalid(
            "--description and --description-file can't both be given".into(),
        ));
    }
    if ctx.dry_run {
        match &args.json_file {
            Some(file) if !file.is_empty() => println!("[dry-run] would add tasks from {file}"),
            _ => println!(
                "[dry-run] would add task {}",
                args.name.as_deref().unwrap_or("None")
            ),
        }
        return Ok(());
    }
    let actor = agent_identity(args.agent.as_deref(), cwd);
    let entries = if let Some(file) = args.json_file.as_deref().filter(|f| !f.is_empty()) {
        read_entries(file, cwd)?
    } else {
        let description = match args.description_file.as_deref().filter(|f| !f.is_empty()) {
            Some(file) => Some(std::fs::read_to_string(cwd.join(file)).map_err(|e| {
                Error::failed(format!("can't read the description file {file}: {e}"))
            })?),
            None => args.description.clone(),
        };
        vec![entry(args, description)]
    };
    let mut project = open_project(cwd)?.with_actor(Some(actor));
    let added = add_new_tasks(&mut project, &entries)?;
    if ctx.json {
        print_json(&Value::Array(
            added.tasks.into_iter().map(Value::Object).collect(),
        ));
        return Ok(());
    }
    for task in &added.tasks {
        println!("Added {} {}", py_str(&task["id"]), py_str(&task["name"]));
        if let Some(line) = suggestion_line(&added.vocab, task) {
            println!("{line}");
        }
    }
    Ok(())
}

/// `atm task note`'s arguments.
#[derive(Debug, Args)]
pub struct NoteArgs {
    /// The task ID
    task_id: String,
    /// The note text
    text: String,
    /// Who writes it
    #[arg(long)]
    agent: Option<String>,
}

/// Run `atm task note` in `cwd`.
///
/// # Errors
/// An unknown task, a note too long, a store error.
pub fn note(args: &NoteArgs, ctx: Context, cwd: &Path) -> Result<()> {
    if ctx.dry_run {
        println!("[dry-run] would add a note to {}", args.task_id);
        return Ok(());
    }
    let actor = agent_identity(args.agent.as_deref(), cwd);
    let mut project = open_project(cwd)?.with_actor(Some(actor));
    let task = add_task_note(&mut project, &args.task_id, &args.text)?;
    let notes = task
        .get("notes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if ctx.json {
        print_json(notes.last().unwrap_or(&Value::Null));
    } else {
        println!(
            "{}: note added ({} in all)",
            py_str(&task["id"]),
            notes.len()
        );
    }
    Ok(())
}
