//! The JSON-in-git store's files (docs/v4/data-model.md §1), as the Python atm's `db/jsongit/files.py` reads them:
//! `tasks.json` strictly (every command stops on a file it can't trust) and the archive, `tasks-archive.json`.

use std::path::Path;

use atm_common::Shown;

use atm_common::ids::{
    KEYS_FORMAT, file_format, valid_project_key, valid_task_id, valid_task_uuid,
};
use atm_common::py::repr_str;
use atm_common::{Error, Result, Task, Vocabulary};
use serde_json::Value;

use super::paths::Paths;
use crate::interface::Problems;

/// The file's JSON, or None when it can't be read or parsed (atm treats both as "not valid").
#[must_use]
pub fn read_json(path: &Path) -> Option<Value> {
    let bytes = std::fs::read(path).ok()?;
    let text = std::str::from_utf8(&bytes).ok()?;
    serde_json::from_str(text).ok()
}

/// A task file read and checked: its document and vocabularies.
#[derive(Debug, Clone)]
pub struct Loaded {
    /// The whole file, as stored.
    pub document: Value,
    /// Its vocabularies, checked.
    pub vocabulary: Vocabulary,
}

impl Loaded {
    /// The tasks, in the file's order (every one an object: [`load_tasks`] checked them).
    pub fn tasks(&self) -> impl Iterator<Item = &Task> {
        self.document["tasks"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_object)
    }
}

fn read_task_file(paths: &Paths) -> Result<Value> {
    if !paths.tasks.is_file() {
        return Err(Error::failed(format!(
            "no tasks.json in {}. Run /atm-create-tasks first.",
            paths.dir.shown()
        )));
    }
    read_json(&paths.tasks)
        .filter(|d| d.get("tasks").is_some_and(Value::is_array) && d.is_object())
        .ok_or_else(|| {
            Error::failed(format!(
                "{} is not a valid task file (expected an object with a 'tasks' list)",
                paths.tasks.shown()
            ))
        })
}

/// The file's format (ADR-007 §6), checked: one this atm reads, or an error saying what to do. A file written by a
/// newer atm is never read, so this one can't half-understand and rewrite it.
///
/// # Errors
/// When `meta.format` isn't a whole number of at least 1, or is newer than [`KEYS_FORMAT`].
pub fn checked_format(document: &Value, file: &str) -> Result<u64> {
    let Some(format) = file_format(document) else {
        return Err(Error::failed(format!(
            "{file}: meta.format must be a whole number (1 or {KEYS_FORMAT}); run atm doctor"
        )));
    };
    if format > KEYS_FORMAT {
        return Err(Error::failed(format!(
            "{file} is format {format}, written by a newer atm (this one reads formats up to {KEYS_FORMAT}): upgrade \
             atm to work on this project"
        )));
    }
    Ok(format)
}

/// What a task of a keys-format file (ADR-007 §1) must hold beyond a legacy one's: a UUID `id`, a `number` of at least
/// 1, and, when present, `aliases` that are a list of keys. None when it holds them. (An old `key_final` is ignored,
/// whatever it holds: ADR-009, and `atm db migrate` drops it.)
fn keys_problem(task: &Value) -> Option<&'static str> {
    let id = task.get("id").and_then(Value::as_str).unwrap_or_default();
    if !valid_task_uuid(id) {
        return Some("its id isn't a UUID (a format-2 file stores tasks by UUID)");
    }
    if task
        .get("number")
        .and_then(Value::as_u64)
        .is_none_or(|n| n < 1)
    {
        return Some("it has no number (a whole number of at least 1)");
    }
    if task
        .get("aliases")
        .is_some_and(|a| !a.as_array().is_some_and(|a| a.iter().all(Value::is_string)))
    {
        return Some("its aliases aren't a list of keys");
    }
    None
}

/// `meta.key` of a keys-format file: the project's short name (3 to 6 letters and digits), which its keys need.
fn meta_key_problem(document: &Value) -> Option<String> {
    let key = document.get("meta").and_then(|m| m.get("key"));
    match key.and_then(Value::as_str) {
        Some(k) if valid_project_key(k) => None,
        Some(k) => Some(format!(
            "meta.key {} isn't a short name (3 to 6 letters and digits)",
            repr_str(k)
        )),
        None => {
            Some("meta.key, the project's short name its tasks' keys start with, is missing".into())
        }
    }
}

/// `tasks.json`, read strictly: an unusable vocabulary, a task that isn't an object with an ID, an ID with characters
/// atm doesn't allow or dependencies that aren't a list of IDs stop every command, before anything is written. A file
/// of a newer format than this atm's is refused (ADR-007 §6); in a keys-format file every task needs a UUID and a
/// number, and `meta` the project's short name.
///
/// # Errors
/// When there is no task file, it isn't one, or one of the checks above fails; the message says which.
pub fn load_tasks(paths: &Paths) -> Result<Loaded> {
    let document = read_task_file(paths)?;
    let file = paths.tasks.shown();
    let format = checked_format(&document, &file)?;
    let vocabulary = Vocabulary::of_document(&document)?;
    if format == KEYS_FORMAT
        && let Some(problem) = meta_key_problem(&document)
    {
        return Err(Error::failed(format!("{file}: {problem}; run atm doctor")));
    }
    for (i, task) in document["tasks"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        let Some(id) = task
            .as_object()
            .and_then(|t| t.get("id"))
            .and_then(Value::as_str)
        else {
            return Err(Error::failed(format!(
                "{file}: task #{} isn't a task (an object with an \"id\"); run atm doctor",
                i + 1
            )));
        };
        if !valid_task_id(id) {
            return Err(Error::failed(format!(
                "{file}: the task ID {} has characters atm doesn't allow (letters, digits, '.', '_' and '-' only): fix \
                 it in the file, then run atm doctor",
                repr_str(id)
            )));
        }
        let deps_ok = task
            .get("dependencies")
            .is_none_or(|d| d.as_array().is_some_and(|d| d.iter().all(Value::is_string)));
        if !deps_ok {
            return Err(Error::failed(format!(
                "{file}: {id}'s dependencies aren't a list of IDs; run atm doctor"
            )));
        }
        if format == KEYS_FORMAT
            && let Some(problem) = keys_problem(task)
        {
            return Err(Error::failed(format!(
                "{file}: task {id}: {problem}; run atm doctor"
            )));
        }
    }
    Ok(Loaded {
        document,
        vocabulary,
    })
}

/// `tasks.json` read as far as it can be, for showing it (the dashboard, T-377): what [`load_tasks`] refuses is kept
/// where it can be, and said. Entries that aren't tasks (no string `id`) are left out; a task with an ID atm doesn't
/// allow is kept as it is, one whose dependencies aren't a list of IDs with the IDs that can be read (so every task
/// read fits the task schema); the vocabularies that don't work are left out of `meta`. The data is for reading only.
///
/// # Errors
/// When there is no task file, or it isn't one at all.
pub fn load_tasks_tolerant(paths: &Paths) -> Result<(Loaded, Problems)> {
    let mut document = read_task_file(paths)?;
    // A newer format can't be shown as if it were this one's
    let format = checked_format(&document, &paths.tasks.shown())?;
    let mut problems = Problems::default();
    let mut kept = Vec::new();
    let tasks = document["tasks"].as_array().cloned().unwrap_or_default();
    for (i, mut task) in tasks.into_iter().enumerate() {
        let Some(id) = task.get("id").and_then(Value::as_str).map(String::from) else {
            problems.file.push(format!(
                "task #{} isn't a task (an object with an \"id\"): left out",
                i + 1
            ));
            continue;
        };
        let mut own = Vec::new();
        if !valid_task_id(&id) {
            own.push(Value::String(format!(
                "the ID {} has characters atm doesn't allow (letters, digits, '.', '_' and '-')",
                repr_str(&id)
            )));
        }
        let deps = task.get("dependencies").cloned();
        let deps_ok = deps
            .as_ref()
            .is_none_or(|d| d.as_array().is_some_and(|d| d.iter().all(Value::is_string)));
        if !deps_ok {
            // Shown with the IDs that can be read, so what reads it gets a task of the schema's shape
            let readable: Vec<Value> = deps
                .as_ref()
                .and_then(Value::as_array)
                .map(|d| d.iter().filter(|x| x.is_string()).cloned().collect())
                .unwrap_or_default();
            task["dependencies"] = Value::Array(readable);
            own.push(Value::String(
                "its dependencies aren't a list of task IDs (shown with the IDs that can be read)"
                    .into(),
            ));
        }
        if format == KEYS_FORMAT
            && let Some(problem) = keys_problem(&task)
        {
            own.push(Value::String(problem.into()));
        }
        if !own.is_empty() {
            problems.tasks.insert(id, Value::Array(own));
        }
        kept.push(task);
    }
    document["tasks"] = Value::Array(kept);
    if format == KEYS_FORMAT
        && let Some(problem) = meta_key_problem(&document)
    {
        problems.file.push(problem);
    }
    // The rules read the vocabularies from meta: what doesn't work is left out of it (the problems say what)
    let (vocabulary, vocabulary_problems, meta) = Vocabulary::tolerant(document.get("meta"));
    problems.file.extend(vocabulary_problems);
    document["meta"] = Value::Object(meta);
    Ok((
        Loaded {
            document,
            vocabulary,
        },
        problems,
    ))
}

/// The archived tasks, in the archive's order (its entries that are objects); empty without an archive. Looking one
/// up by ID takes the last with that ID, as in the Python atm.
///
/// # Errors
/// When the archive exists but isn't an object with a `tasks` list.
pub fn load_archive(paths: &Paths) -> Result<Vec<Task>> {
    let data = archive_document(paths)?;
    Ok(data["tasks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
        .cloned()
        .collect())
}

/// The archive document as stored (`{meta, tasks}`), or an empty one when there is no archive (DM-1.2).
///
/// # Errors
/// When the archive exists but isn't an object with a `tasks` list.
pub fn archive_document(paths: &Paths) -> Result<Value> {
    let path = paths.archive();
    if !path.is_file() {
        return Ok(serde_json::json!({"meta": {}, "tasks": []}));
    }
    read_json(&path)
        .filter(|d| d.is_object() && d.get("tasks").is_some_and(Value::is_array))
        .ok_or_else(|| {
            Error::failed(format!(
                "{} is not a valid archive (expected an object with a 'tasks' list)",
                path.shown()
            ))
        })
}

/// The last task with this ID in a list of tasks (a later entry wins, as a dict of them by ID has it).
#[must_use]
pub fn last_with_id<'a>(tasks: &'a [Task], id: &str) -> Option<&'a Task> {
    tasks
        .iter()
        .rev()
        .find(|t| t.get("id").and_then(Value::as_str) == Some(id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jsongit::paths::find_paths;
    use crate::test_dir::TestDir;
    use atm_common::TASK_DIR;
    use serde_json::json;

    fn project(tasks: &str) -> (TestDir, Paths) {
        let dir = TestDir::new();
        std::fs::create_dir(dir.path().join(TASK_DIR)).unwrap();
        std::fs::write(dir.path().join(TASK_DIR).join("tasks.json"), tasks).unwrap();
        let paths = find_paths(dir.path()).unwrap();
        (dir, paths)
    }

    fn error(tasks: &str) -> String {
        let (_dir, paths) = project(tasks);
        load_tasks(&paths).unwrap_err().to_string()
    }

    #[test]
    fn a_task_file_is_read_with_its_tasks_in_order() {
        let (_dir, paths) = project(r#"{"tasks": [{"id": "T-2", "z": 1, "a": 2}, {"id": "T-1"}]}"#);
        let loaded = load_tasks(&paths).unwrap();
        let ids: Vec<&Value> = loaded.tasks().map(|t| &t["id"]).collect();
        assert_eq!(ids, [&json!("T-2"), &json!("T-1")]);
        assert_eq!(
            loaded.tasks().next().unwrap().keys().collect::<Vec<_>>(),
            ["id", "z", "a"]
        );
    }

    #[test]
    fn no_task_file_says_how_to_make_one() {
        let dir = TestDir::new();
        let paths = find_paths(dir.path()).unwrap();
        assert!(
            load_tasks(&paths)
                .unwrap_err()
                .to_string()
                .ends_with(". Run /atm-create-tasks first.")
        );
    }

    #[test]
    fn a_file_that_isnt_a_task_file_is_refused() {
        for text in ["[]", "{\"tasks\": {}}", "not json", "{\"meta\": {}}"] {
            assert!(
                error(text)
                    .ends_with("is not a valid task file (expected an object with a 'tasks' list)"),
                "{text}"
            );
        }
    }

    #[test]
    fn a_malformed_task_stops_every_command() {
        assert!(
            error(r#"{"tasks": [{"id": "T-1"}, {"name": "x"}]}"#)
                .ends_with(": task #2 isn't a task (an object with an \"id\"); run atm doctor")
        );
        assert!(error(r#"{"tasks": [{"id": "T 1"}]}"#).ends_with(
            ": the task ID 'T 1' has characters atm doesn't allow (letters, digits, '.', '_' and '-' only): fix it in \
             the file, then run atm doctor"
        ));
        assert!(
            error(r#"{"tasks": [{"id": "T-1", "dependencies": "T-2"}]}"#)
                .ends_with(": T-1's dependencies aren't a list of IDs; run atm doctor")
        );
    }

    #[test]
    fn a_newer_format_is_refused_saying_to_upgrade() {
        let message = error(r#"{"meta": {"format": 3}, "tasks": []}"#);
        assert!(
            message.ends_with(
                " is format 3, written by a newer atm (this one reads formats up to 2): upgrade atm to work on this \
                 project"
            ),
            "{message}"
        );
        assert!(
            error(r#"{"meta": {"format": "2"}, "tasks": []}"#)
                .ends_with(": meta.format must be a whole number (1 or 2); run atm doctor")
        );
        // Shown on the dashboard neither
        let (_dir, paths) = project(r#"{"meta": {"format": 3}, "tasks": []}"#);
        assert!(load_tasks_tolerant(&paths).is_err());
    }

    #[test]
    fn a_keys_format_file_holds_uuids_numbers_and_the_projects_short_name() {
        let uuid = "0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8091";
        let good = format!(
            r#"{{"meta": {{"format": 2, "key": "ATM"}}, "tasks": [{{"id": "{uuid}", "number": 527, "key_final": true, "aliases": ["T-527"]}}]}}"#
        );
        let (_dir, paths) = project(&good);
        let loaded = load_tasks(&paths).unwrap();
        assert_eq!(loaded.tasks().next().unwrap()["number"], json!(527));
        // An old key_final, whatever it holds, is ignored (ADR-009)
        let old = format!(
            r#"{{"meta": {{"format": 2, "key": "ATM"}}, "tasks": [{{"id": "{uuid}", "number": 1, "key_final": "yes"}}]}}"#
        );
        let (_old_dir, old_paths) = project(&old);
        assert!(load_tasks(&old_paths).is_ok());

        assert!(
            error(r#"{"meta": {"format": 2, "key": "ATM"}, "tasks": [{"id": "T-1", "number": 1}]}"#)
                .ends_with(": task T-1: its id isn't a UUID (a format-2 file stores tasks by UUID); run atm doctor")
        );
        for (task, problem) in [
            (
                format!(r#"{{"id": "{uuid}"}}"#),
                "it has no number (a whole number of at least 1)",
            ),
            (
                format!(r#"{{"id": "{uuid}", "number": 0}}"#),
                "it has no number (a whole number of at least 1)",
            ),
            (
                format!(r#"{{"id": "{uuid}", "number": 1, "aliases": "T-1"}}"#),
                "its aliases aren't a list of keys",
            ),
        ] {
            let message = error(&format!(
                r#"{{"meta": {{"format": 2, "key": "ATM"}}, "tasks": [{task}]}}"#
            ));
            assert!(
                message.ends_with(&format!(": task {uuid}: {problem}; run atm doctor")),
                "{message}"
            );
        }
        assert!(error(r#"{"meta": {"format": 2}, "tasks": []}"#).ends_with(
            ": meta.key, the project's short name its tasks' keys start with, is missing; run atm doctor"
        ));
        assert!(error(r#"{"meta": {"format": 2, "key": "TaskMan"}, "tasks": []}"#).ends_with(
            ": meta.key 'TaskMan' isn't a short name (3 to 6 letters and digits); run atm doctor"
        ));
        // A legacy file is read as before: its IDs needn't be UUIDs, nor have numbers
        let (_dir, paths) = project(r#"{"tasks": [{"id": "T-1"}]}"#);
        assert!(load_tasks(&paths).is_ok());
    }

    #[test]
    fn a_keys_format_files_problems_are_shown_not_refused_on_the_dashboard() {
        let (_dir, paths) = project(r#"{"meta": {"format": 2}, "tasks": [{"id": "T-1"}]}"#);
        let (loaded, problems) = load_tasks_tolerant(&paths).unwrap();
        assert_eq!(loaded.tasks().count(), 1);
        assert_eq!(
            problems.tasks["T-1"],
            json!(["its id isn't a UUID (a format-2 file stores tasks by UUID)"])
        );
        assert!(
            problems
                .file
                .iter()
                .any(|p| p.starts_with("meta.key, the project's short name"))
        );
    }

    #[test]
    fn an_unusable_vocabulary_stops_every_command() {
        assert_eq!(
            error(r#"{"meta": {"types": []}, "tasks": []}"#),
            "meta.types must be a non-empty list"
        );
    }

    #[test]
    fn the_archive_is_read_by_id_and_may_be_missing() {
        let (dir, paths) = project(r#"{"tasks": []}"#);
        assert_eq!(
            load_archive(&paths).unwrap(),
            [] as [serde_json::Map<std::string::String, serde_json::Value>; 0]
        );
        std::fs::write(
            dir.path().join(TASK_DIR).join("tasks-archive.json"),
            r#"{"tasks": [{"id": "T-1", "v": 1}, "junk", {"id": "T-1", "v": 2}]}"#,
        )
        .unwrap();
        let archive = load_archive(&paths).unwrap();
        assert_eq!(archive.len(), 2, "the entries that are objects, in order");
        assert_eq!(last_with_id(&archive, "T-1").unwrap()["v"], json!(2));
        std::fs::write(dir.path().join(TASK_DIR).join("tasks-archive.json"), "[]").unwrap();
        assert!(
            load_archive(&paths)
                .unwrap_err()
                .to_string()
                .ends_with("is not a valid archive (expected an object with a 'tasks' list)")
        );
    }
}
