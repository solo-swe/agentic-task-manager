//! One module per top-level command (docs/porting-to-rust.md §2): each holds its clap subcommands, its handlers and
//! its area's text. The groups come with their slices (§5); the Python atm's `cli/commands/` has them all.

pub mod agents;
pub mod claim;
pub mod completion;
pub mod component;
pub mod db;
pub mod detach;
pub mod label;
pub mod loop_;
pub mod maintenance;
pub mod meta;
pub mod project;
pub mod release;
pub mod review;
pub mod sprint;
pub mod task;
pub mod vcs;
pub mod workspace;

use atm_common::py::to_json;
use serde_json::Value;

/// What every command gets: how to print its result, and whether to only say what it would do.
#[derive(Debug, Clone, Copy)]
pub struct Context {
    /// `--json`: print the result as JSON.
    pub json: bool,
    /// `--dry-run`: say what would be done, and do nothing.
    pub dry_run: bool,
}

/// Print a JSON result, as atm prints JSON: exactly (std's `println!`, not the crate's, which drops control
/// characters; JSON escapes those it must, and a program reads it, not a terminal).
pub fn print_json(value: &Value) {
    ::std::println!("{}", to_json(value));
}

/// The stored IDs of the tasks these references name (ADR-007 §5, `Project::resolve_task`): in a keys-format project a
/// key (`ATM-527`), an earlier prefix (`T-527`), an alias or a UUID; before task keys, the IDs as given. A project that
/// can't be opened gives them back as given, for the command to report.
///
/// # Errors
/// A reference that names no task, or several, in a keys-format project.
pub fn resolved(cwd: &std::path::Path, references: &[String]) -> atm_common::Result<Vec<String>> {
    match atm_lib::Project::open(cwd) {
        Ok(project) => project.resolve_tasks(references),
        Err(_) => Ok(references.to_vec()),
    }
}

/// A text with every task UUID in it replaced by the task's label (its key, `~` while provisional: ADR-007 §1), as
/// people know the task: for the lines atm prints. Text without one, or a project without task keys, unchanged.
///
/// One [`Labeller`] a process (ATM-679, audit C8): each printed line used to open the project and read its archive
/// again. It is opened again when the task file changed since it read it (the command wrote it: a key changed, a task
/// added), or when a line names a task it doesn't know.
#[must_use]
pub fn labelled(cwd: &std::path::Path, text: &str) -> String {
    if !has_uuid(text) {
        return text.to_string();
    }
    LABELLER.with(|cell| {
        let mut shared = cell.borrow_mut();
        if shared
            .as_ref()
            .is_none_or(|(dir, labeller)| dir != cwd || labeller.stale())
        {
            *shared = Some((cwd.to_path_buf(), Labeller::open(cwd)));
        }
        let (out, missed) = shared.as_ref().map_or_else(
            || (text.to_string(), false),
            |(_, labeller)| labeller.labels(text),
        );
        if !missed {
            return out;
        }
        let fresh = Labeller::open(cwd);
        let out = fresh.text(text);
        *shared = Some((cwd.to_path_buf(), fresh));
        out
    })
}

thread_local! {
    /// The process's labeller, and the folder it is of ([`labelled`])
    static LABELLER: std::cell::RefCell<Option<(std::path::PathBuf, Labeller)>> =
        const { std::cell::RefCell::new(None) };
}

fn is_uuid(s: &str) -> bool {
    atm_common::ids::valid_task_uuid(s)
}

fn has_uuid(s: &str) -> bool {
    s.as_bytes()
        .windows(36)
        .any(|w| std::str::from_utf8(w).is_ok_and(is_uuid))
}

/// [`labelled`] for many texts of one project: the project opened once.
pub struct Labeller {
    project: Option<(atm_lib::Project, Vec<atm_common::Task>)>,
    /// The task file's size and time when it was read: a later write (a key changed, a task added) makes it stale
    read: Option<(u64, std::time::SystemTime)>,
}

/// The task file's size and modification time, of the project `project` is.
fn task_file_stamp(project: &atm_lib::Project) -> Option<(u64, std::time::SystemTime)> {
    let file = project
        .root()?
        .join(atm_common::TASK_DIR)
        .join("tasks.json");
    let meta = std::fs::metadata(file).ok()?;
    Some((meta.len(), meta.modified().ok()?))
}

impl Labeller {
    /// The labeller of the project in `cwd` (one that can't be opened leaves texts as they are).
    #[must_use]
    pub fn open(cwd: &std::path::Path) -> Self {
        let project = atm_lib::Project::open(cwd).ok().map(|project| {
            let archive: Vec<atm_common::Task> = project
                .store()
                .list_archived()
                .map(|a| a.into_iter().cloned().collect())
                .unwrap_or_default();
            (project, archive)
        });
        let read = project.as_ref().and_then(|(p, _)| task_file_stamp(p));
        Labeller { project, read }
    }

    /// Whether the task file changed since it was read (or it can't be told).
    fn stale(&self) -> bool {
        match &self.project {
            Some((project, _)) => self.read.is_none() || task_file_stamp(project) != self.read,
            None => true,
        }
    }

    /// A text with its task UUIDs as labels.
    #[must_use]
    pub fn text(&self, text: &str) -> String {
        self.labels(text).0
    }

    /// A text with its task UUIDs as labels, and whether a UUID in it named no task this labeller knows (one the
    /// command added since it was opened, or none at all).
    fn labels(&self, text: &str) -> (String, bool) {
        if !has_uuid(text) {
            return (text.to_string(), false);
        }
        let Some((project, archive)) = &self.project else {
            return (text.to_string(), true);
        };
        let Ok(document) = project.store().document() else {
            return (text.to_string(), true);
        };
        let mut out = String::new();
        let mut missed = false;
        let mut rest = text;
        while rest.len() >= 36 {
            let found = (0..=rest.len() - 36)
                .filter(|&i| rest.is_char_boundary(i) && rest.is_char_boundary(i + 36))
                .find(|&i| is_uuid(&rest[i..i + 36]));
            let Some(i) = found else {
                break;
            };
            out.push_str(&rest[..i]);
            let uuid = &rest[i..i + 36];
            if let Ok(f) = atm_common::keys::resolve(document, archive, uuid) {
                out.push_str(&project.task_label(f.task));
            } else {
                missed = true;
                out.push_str(uuid);
            }
            rest = &rest[i + 36..];
        }
        out.push_str(rest);
        (out, missed)
    }

    /// The project's palette (its statuses and priorities, for their styles).
    #[must_use]
    pub fn palette(&self) -> crate::output::Palette {
        self.project
            .as_ref()
            .and_then(|(p, _)| p.store().document().ok())
            .map(crate::output::Palette::of_document)
            .unwrap_or_default()
    }
}

/// `println!` of a line as people read it ([`labelled`]).
pub fn say(cwd: &std::path::Path, line: &str) {
    crate::output::emit(&labelled(cwd, line));
}

/// [`resolved`] for one.
///
/// # Errors
/// As [`resolved`].
pub fn resolved_one(cwd: &std::path::Path, reference: &str) -> atm_common::Result<String> {
    Ok(resolved(cwd, &[reference.to_string()])?.remove(0))
}
