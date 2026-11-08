//! The rules of task data that reads need (the Python atm's `common/model.py`): finding a sprint or a release, a
//! task's effective release and review mode, the dependencies it still waits for, and the loop's ordering keys. Every
//! rule acts on a status's kind, never its name.

use std::cmp::Ordering;

use serde_json::{Map, Value};

use crate::Task;
use crate::error::{Error, Result};
use crate::py::{repr_str, truthy};
use crate::vocabulary::{Kind, REVIEW_MODES, Vocabulary};

/// Whether `id` is in a list-or-text field, as Python's `in` reads it: an element of a list, part of a text.
#[must_use]
pub fn contains(field: Option<&Value>, id: &str) -> bool {
    match field {
        Some(Value::Array(items)) => items.iter().any(|r| r == id),
        Some(Value::String(text)) => text.contains(id),
        _ => false,
    }
}

impl Vocabulary {
    /// The sprint with this name (compared case-insensitively), if there is one.
    #[must_use]
    pub fn find_sprint(&self, name: Option<&Value>) -> Option<&Map<String, Value>> {
        let name = name?.as_str()?.to_lowercase();
        self.sprints.iter().find(|s| {
            s.get("id")
                .and_then(Value::as_str)
                .is_some_and(|id| id.to_lowercase() == name)
        })
    }

    /// `release`, when `meta.releases` has it.
    ///
    /// # Errors
    /// An error naming the releases (exit code 2) when it doesn't.
    pub fn known_release(&self, release: &str) -> Result<()> {
        if self
            .releases
            .iter()
            .any(|r| r.get("id").and_then(Value::as_str) == Some(release))
        {
            return Ok(());
        }
        let ids: Vec<&str> = self
            .releases
            .iter()
            .filter_map(|r| r.get("id").and_then(Value::as_str))
            .collect();
        let known = if ids.is_empty() {
            "none yet: atm release add <R>".to_string()
        } else {
            ids.join(", ")
        };
        Err(Error::Failed {
            message: format!("no release {}; the releases: {known}", repr_str(release)),
            code: 2,
        })
    }

    /// A task's release: its own, else its sprint's (SR-2.6), if any.
    #[must_use]
    pub fn effective_release(&self, task: &Task) -> Option<Value> {
        if let Some(own) = task.get("release").filter(|r| truthy(r)) {
            return Some(own.clone());
        }
        self.find_sprint(task.get("sprint"))
            .and_then(|s| s.get("release"))
            .filter(|r| !r.is_null())
            .cloned()
    }

    /// A task's review mode: its own, else its first component's that has one, else the project's (SR-2.6).
    #[must_use]
    pub fn effective_review(&self, task: &Task) -> String {
        let mode = |v: Option<&Value>| {
            v.and_then(Value::as_str)
                .filter(|m| REVIEW_MODES.contains(m))
                .map(String::from)
        };
        if let Some(own) = mode(task.get("review")) {
            return own;
        }
        for component in task
            .get("components")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let found = self
                .components
                .iter()
                .find(|c| c.get("id") == Some(component));
            if let Some(m) = mode(found.and_then(|c| c.get("review"))) {
                return m;
            }
        }
        self.review.clone()
    }

    /// 0 for the most urgent priority; a missing or unknown one ranks as the default priority.
    #[must_use]
    pub fn priority_rank(&self, priority: Option<&Value>) -> usize {
        let rank = |p: &str| self.priorities.iter().position(|t| t.id == p);
        priority
            .and_then(Value::as_str)
            .and_then(rank)
            .or_else(|| rank(&self.default_priority))
            .unwrap_or(0)
    }
}

/// The dependencies a task still waits for: those that don't have a done-kind status yet, with their status (None
/// for one without), and unknown IDs (status "missing"). A dependency only in the archive is done (DM-3.1):
/// `archived(id)` says whether the archive has it.
///
/// # Errors
/// When `archived` can't read the archive.
pub fn unfinished_dependencies(
    vocab: &Vocabulary,
    tasks: &[&Task],
    task: &Task,
    archived: &dyn Fn(&str) -> Result<bool>,
) -> Result<Vec<(String, Option<Value>)>> {
    let mut found = Vec::new();
    for dep in task
        .get("dependencies")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        // The last task with the ID, as a dict of the statuses by ID has it
        match tasks
            .iter()
            .rev()
            .find(|t| t.get("id").and_then(Value::as_str) == Some(dep))
        {
            Some(live) => {
                if vocab.kind_of(live.get("status")) != Some(Kind::Done) {
                    found.push((dep.to_string(), live.get("status").cloned()));
                }
            }
            None => {
                if !archived(dep)? {
                    found.push((dep.to_string(), Some(Value::String("missing".into()))));
                }
            }
        }
    }
    Ok(found)
}

/// Sort `T-2` before `T-10`, as the dashboard's `localeCompare(…, { numeric: true })` does: runs of digits compare as
/// numbers, the rest case-insensitively.
#[must_use]
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let parts = |s: &str| -> Vec<(bool, String)> {
        let mut out: Vec<(bool, String)> = Vec::new();
        for c in s.chars() {
            let digit = c.is_ascii_digit();
            match out.last_mut() {
                Some((d, run)) if *d == digit => run.push(c),
                _ => out.push((digit, c.to_string())),
            }
        }
        out
    };
    let (pa, pb) = (parts(a), parts(b));
    for (x, y) in pa.iter().zip(pb.iter()) {
        let ordering = match (x.0, y.0) {
            (true, true) => {
                let (nx, ny) = (x.1.trim_start_matches('0'), y.1.trim_start_matches('0'));
                nx.len().cmp(&ny.len()).then_with(|| nx.cmp(ny))
            }
            // Python compares an int with a str by raising; atm's IDs never get there, so ints come first
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (false, false) => x.1.to_lowercase().cmp(&y.1.to_lowercase()),
        };
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    pa.len().cmp(&pb.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn task(value: Value) -> Task {
        match value {
            Value::Object(map) => map,
            other => panic!("not a task: {other}"),
        }
    }

    fn vocab(meta: Value) -> Vocabulary {
        Vocabulary::of_meta(&task(meta)).unwrap()
    }

    #[test]
    fn a_sprint_is_found_by_its_name_in_any_case() {
        let v = vocab(json!({"sprints": [{"id": "Alpha", "release": "1.0"}]}));
        assert!(v.find_sprint(Some(&json!("alpha"))).is_some());
        assert!(v.find_sprint(Some(&json!(3))).is_none());
        assert_eq!(
            v.effective_release(&task(json!({"sprint": "ALPHA"}))),
            Some(json!("1.0"))
        );
        assert_eq!(
            v.effective_release(&task(json!({"sprint": "Alpha", "release": "2.0"}))),
            Some(json!("2.0"))
        );
        assert_eq!(v.effective_release(&task(json!({}))), None);
    }

    #[test]
    fn an_unknown_release_names_the_releases() {
        assert_eq!(
            vocab(json!({}))
                .known_release("1.0")
                .unwrap_err()
                .to_string(),
            "no release '1.0'; the releases: none yet: atm release add <R>"
        );
        let v = vocab(json!({"releases": [{"id": "1.0"}, {"id": "2.0"}]}));
        assert!(v.known_release("2.0").is_ok());
        let e = v.known_release("3.0").unwrap_err();
        assert_eq!(
            (e.to_string(), e.code()),
            ("no release '3.0'; the releases: 1.0, 2.0".into(), 2)
        );
    }

    #[test]
    fn the_review_mode_is_the_tasks_its_components_or_the_projects() {
        let v = vocab(
            json!({"review": "batch", "components": [{"id": "a"}, {"id": "b", "review": "blocker"}]}),
        );
        assert_eq!(
            v.effective_review(&task(json!({"review": "async"}))),
            "async"
        );
        assert_eq!(
            v.effective_review(&task(json!({"components": ["a", "b"]}))),
            "blocker"
        );
        assert_eq!(v.effective_review(&task(json!({"review": "x"}))), "batch");
    }

    #[test]
    fn unfinished_dependencies_skip_done_and_archived_ones() {
        let v = vocab(json!({}));
        let (done, todo, me) = (
            task(json!({"id": "T-1", "status": "done"})),
            task(json!({"id": "T-2", "status": "todo"})),
            task(json!({"id": "T-4", "dependencies": ["T-1", "T-2", "T-3", "T-9"]})),
        );
        let archived = |id: &str| Ok(id == "T-3");
        let found = unfinished_dependencies(&v, &[&done, &todo, &me], &me, &archived).unwrap();
        assert_eq!(
            found,
            [
                ("T-2".into(), Some(json!("todo"))),
                ("T-9".into(), Some(json!("missing")))
            ]
        );
    }

    #[test]
    fn priorities_rank_most_urgent_first() {
        let v = vocab(json!({}));
        assert_eq!(v.priority_rank(Some(&json!("P0"))), 0);
        assert_eq!(v.priority_rank(Some(&json!("zz"))), 2);
        assert_eq!(v.priority_rank(None), 2);
    }

    #[test]
    fn ids_sort_naturally() {
        let mut ids = vec!["T-10", "T-2", "t-3", "T-1"];
        ids.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(ids, ["T-1", "T-2", "t-3", "T-10"]);
        assert_eq!(natural_cmp("T-01", "T-1"), Ordering::Equal);
    }

    #[test]
    fn contains_reads_a_list_or_a_text() {
        assert!(contains(Some(&json!(["a", "b"])), "b"));
        assert!(contains(Some(&json!("xab")), "ab"));
        assert!(!contains(Some(&json!(3)), "3"));
        assert!(!contains(None, "a"));
    }
}
