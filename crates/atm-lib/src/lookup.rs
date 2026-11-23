//! Finding a task in a change (ATM-681): its position among the working tasks, or the error every command gives for
//! one that isn't there, saying when it is archived.

use atm_common::{Error, Result};
use atm_db::working::Working;
use serde_json::Value;

/// The error for a task that isn't among the working tasks; `archived`, when the archive has it.
pub(crate) fn no_task(task_id: &str, archived: bool) -> Error {
    let hint = if archived {
        " (it is archived: atm db unarchive brings it back)"
    } else {
        ""
    };
    Error::NotFound(format!("no task {task_id}{hint}"))
}

fn has_id(task: &Value, task_id: &str) -> bool {
    task.get("id").and_then(Value::as_str) == Some(task_id)
}

/// The position of the task with this ID among the change's working tasks.
///
/// # Errors
/// [`no_task`] when it isn't there (saying when the archive has it); an archive that can't be read.
pub(crate) fn task_at(work: &mut Working, task_id: &str) -> Result<usize> {
    if let Some(at) = work.document["tasks"]
        .as_array()
        .and_then(|list| list.iter().position(|t| has_id(t, task_id)))
    {
        return Ok(at);
    }
    let archived = work.archive()?["tasks"]
        .as_array()
        .is_some_and(|list| list.iter().any(|t| has_id(t, task_id)));
    Err(no_task(task_id, archived))
}

#[cfg(test)]
mod tests;
