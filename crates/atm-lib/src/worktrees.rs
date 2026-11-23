//! A project's working copies (git worktrees, for the JSON-in-git store; the Python atm's `lib/worktrees.py`): the
//! dashboard shows them on one board, each task from the working copy where it got furthest.

use std::collections::HashMap;

use serde_json::{Map, Value, json};

use crate::project::Project;
use crate::query::{Filters, task_views};

/// The project's working copies, main first (`[{path, branch, main}]`); none for a store without them.
#[must_use]
pub fn working_copies(project: &Project) -> Vec<Value> {
    project.store().working_copies()
}

/// The tasks of the working copy at `folder` as views (`{task, derived}`), each worked out on that copy's own data:
/// what `atm loop next` would say there. By task ID; none when the copy can't be read.
fn copy_views(project: &Project, folder: &str) -> HashMap<String, Value> {
    let Ok(store) = project.store().copy_store(folder) else {
        return HashMap::new();
    };
    let copy = Project::new(store);
    let Ok(views) = task_views(&copy, &Filters::default()) else {
        return HashMap::new();
    };
    views
        .into_iter()
        .filter_map(|v| {
            let id = v["task"].get("id").and_then(Value::as_str)?.to_string();
            Some((id, v))
        })
        .collect()
}

/// The project's working copies, main first (`[{path, branch, main}]`), and for each other one the tasks whose copy
/// there differs from the main one's (or that only it has), under `tasks`, as views worked out in that working copy.
/// A copy of a task decided in another working copy (the claims store's `resolved`) is left out: it is stale until
/// `atm loop next` runs there, and would otherwise outrank the decision.
///
/// Each other copy also says which of its tasks the board shows from it, under `shown` (ATM-724): a task's copy that
/// got further than every other, by the merge driver's ranking (`kind_rank`), the first listed on a tie (main first);
/// a task only linked copies have, from the first of them.
#[must_use]
pub fn worktree_copies(project: &Project) -> Vec<Value> {
    let mut listed = copies(project);
    let vocab = project.store().vocabulary().ok();
    let rank = |task: &Value| {
        let kind = vocab.and_then(|v| v.kind_of(task.get("status")));
        atm_db::jsongit::merge::kind_rank(kind)
    };
    // The best copy of each task so far: its rank and the working copy it is from (none: main)
    let mut best: HashMap<String, (u8, Option<usize>)> = HashMap::new();
    for (at, tree) in listed.iter().enumerate() {
        if tree["main"] == json!(true) {
            for t in project
                .store()
                .copy_tasks(tree["path"].as_str().unwrap_or_default())
            {
                if let Some(id) = t["id"].as_str() {
                    best.insert(id.to_string(), (rank(&t), None));
                }
            }
            continue;
        }
        for view in tree["tasks"].as_array().into_iter().flatten() {
            let Some(id) = view["task"]["id"].as_str() else {
                continue;
            };
            let here = rank(&view["task"]);
            match best.get(id) {
                Some((there, _)) if here <= *there => {}
                _ => {
                    best.insert(id.to_string(), (here, Some(at)));
                }
            }
        }
    }
    for (at, tree) in listed.iter_mut().enumerate() {
        if tree["main"] == json!(true) {
            continue;
        }
        let mut shown: Vec<&String> = best
            .iter()
            .filter(|(_, (_, from))| *from == Some(at))
            .map(|(id, _)| id)
            .collect();
        shown.sort();
        tree["shown"] = json!(shown);
    }
    listed
}

/// The working copies with the tasks that differ in each other one, as views.
fn copies(project: &Project) -> Vec<Value> {
    let store = project.store();
    let claims = store.read_claims();
    let stale: Vec<(Option<&Value>, &str)> = claims
        .get("resolved")
        .and_then(Value::as_object)
        .map(|r| {
            r.iter()
                .filter(|(_, v)| v.is_object())
                .map(|(id, v)| (v.get("worktree"), id.as_str()))
                .collect()
        })
        .unwrap_or_default();
    let mut listed = Vec::new();
    let mut main: Map<String, Value> = Map::new();
    for tree in store.working_copies() {
        let path = tree["path"].as_str().unwrap_or_default().to_string();
        let tasks = store.copy_tasks(&path);
        if tree["main"] == json!(true) {
            for t in tasks {
                if let Some(id) = t["id"].as_str() {
                    main.insert(id.to_string(), t.clone());
                }
            }
            listed.push(tree);
            continue;
        }
        let differing: Vec<Value> = tasks
            .into_iter()
            .filter(|t| {
                let id = t["id"].as_str().unwrap_or_default();
                main.get(id) != Some(t)
                    && !stale
                        .iter()
                        .any(|(w, s)| *s == id && w.and_then(Value::as_str) == Some(path.as_str()))
            })
            .collect();
        let views = if differing.is_empty() {
            HashMap::new()
        } else {
            copy_views(project, &path)
        };
        let shown: Vec<Value> = differing
            .into_iter()
            .map(|t| {
                let id = t["id"].as_str().unwrap_or_default().to_string();
                views
                    .get(&id)
                    .cloned()
                    .unwrap_or_else(|| json!({"task": t, "derived": {}}))
            })
            .collect();
        let mut entry = tree;
        entry["tasks"] = Value::Array(shown);
        listed.push(entry);
    }
    listed
}
