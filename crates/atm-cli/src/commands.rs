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

/// Print a JSON result, as atm prints JSON.
pub fn print_json(value: &Value) {
    println!("{}", to_json(value));
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
#[must_use]
pub fn labelled(cwd: &std::path::Path, text: &str) -> String {
    if !has_uuid(text) {
        return text.to_string();
    }
    Labeller::open(cwd).text(text)
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
        Labeller { project }
    }

    /// A text with its task UUIDs as labels.
    #[must_use]
    pub fn text(&self, text: &str) -> String {
        if !has_uuid(text) {
            return text.to_string();
        }
        let Some((project, archive)) = &self.project else {
            return text.to_string();
        };
        let Ok(document) = project.store().document() else {
            return text.to_string();
        };
        let mut out = String::new();
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
            match atm_common::keys::resolve(document, archive, uuid) {
                Ok(f) => out.push_str(&project.task_label(f.task)),
                Err(_) => out.push_str(uuid),
            }
            rest = &rest[i + 36..];
        }
        out.push_str(rest);
        out
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
