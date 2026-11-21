//! A review's tasks: linked to it, and what opening a review does to each (ATM-631).

use super::{
    Actor, Command, Error, FileStore, Kind, Options, Project, Result, ReviewHeader, ReviewRef,
    ReviewStore, Status, Task, Value, Vocabulary, change_task_status, is_agent, json, review_keys,
    store,
};

pub(super) fn actor_of(project: &Project) -> Actor {
    Actor(project.actor().unwrap_or("someone").to_string())
}

pub(super) fn id_of(task: &Value) -> Option<&str> {
    task.get("id").and_then(Value::as_str)
}

/// A task's kind and review state now, none when it is gone.
pub(crate) fn kind_of(project: &Project, id: &str) -> Result<Option<(Kind, Option<String>)>> {
    let vocab: &Vocabulary = project.store().vocabulary()?;
    Ok(project.store().get_task(id)?.map(|task: &Task| {
        (
            vocab.kind_of(task.get("status")).unwrap_or(Kind::Todo),
            task.get("review_state")
                .and_then(Value::as_str)
                .map(String::from),
        )
    }))
}

/// Add (or, with `add` false, remove) the link to `header`'s review on the tasks `ids`.
/// The agent holding a claim in this working copy (a live review's author, §3.5), if one does.
pub(super) fn agent_at_work(project: &Project) -> Option<Actor> {
    let root = project.root()?;
    let here = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    crate::query::list_claims(project)
        .into_iter()
        .find_map(|claim| {
            let worktree = claim.get("worktree").and_then(Value::as_str)?;
            let there = std::fs::canonicalize(worktree).unwrap_or_else(|_| worktree.into());
            let agent = claim.get("agent").and_then(Value::as_str)?;
            (there == here && is_agent(Some(agent))).then(|| Actor(agent.to_string()))
        })
}

/// The agent that worked the first of these tasks (who started it), if an agent did.
pub(super) fn agent_of_tasks(project: &Project, ids: &[String]) -> Option<Actor> {
    ids.iter().find_map(|id| {
        let task = project.store().get_task(id).ok().flatten()?;
        let agent = task.get("started_by").and_then(Value::as_str)?;
        is_agent(Some(agent)).then(|| Actor(agent.to_string()))
    })
}

pub(super) fn link(
    project: &mut Project,
    header: &ReviewHeader,
    ids: &[String],
    add: bool,
) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    let entry = json!({ "id": header.id.0, "ref": header.reference.0 });
    project.store_mut().change(&mut |work| {
        for task in work.tasks().iter_mut() {
            if !id_of(task).is_some_and(|id| ids.iter().any(|i| i == id)) {
                continue;
            }
            let Some(task) = task.as_object_mut() else {
                continue;
            };
            let mut links: Vec<Value> = task
                .get("code_reviews")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            links.retain(|l| l.get("id") != entry.get("id"));
            if add {
                links.push(entry.clone());
            }
            if links.is_empty() {
                task.shift_remove("code_reviews");
            } else {
                task.insert("code_reviews".into(), Value::Array(links));
            }
        }
        Ok(())
    })
}

/// The task IDs of `keys` (keys, old IDs or UUIDs).
pub(super) fn resolve(project: &Project, keys: &[String]) -> Result<Vec<String>> {
    keys.iter().map(|k| project.resolve_task(k)).collect()
}

/// The task IDs a review in `reviews` names: each key as the review meant it (ATM-630, [`review_keys::task_of`]), so
/// a decision reaches the task it was about even when a merge gave its old number to another.
pub(crate) fn named(
    project: &Project,
    reviews: &FileStore,
    header: &ReviewHeader,
) -> Result<Vec<String>> {
    header
        .tasks
        .iter()
        .map(|key| review_keys::task_of(project, Some(reviews), header, key))
        .collect()
}

/// Put the in-progress tasks among `ids` in review, with the review's link in their note (§3.8).
pub(super) fn ask_review(
    project: &mut Project,
    ids: &[String],
    header: &ReviewHeader,
    what: &str,
) -> Result<()> {
    for id in ids {
        if kind_of(project, id)?.is_some_and(|(kind, _)| kind == Kind::Active) {
            let options = Options {
                note: Some(format!("{}: {what}", header.reference)),
                ..Options::default()
            };
            change_task_status(project, Command::Review, id, &options)?;
        }
    }
    Ok(())
}

/// What opening a review that waits on its reviewers does to one of its tasks (ATM-631).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Opening {
    /// In progress, held by the author or by no one: into review.
    Review,
    /// The author's own claim lapsed (the task back in to do, or about to be): started again, then into review.
    TakeBack,
    /// Already in review, with no other review waiting on it: linked, as it is.
    Link,
}

use crate::leases::holder;

/// The open review, other than a draft or a live one, that already carries `task`'s review, if one does.
pub(super) fn waiting_review(project: &Project, task: &Task) -> Result<Option<String>> {
    let links = task
        .get("code_reviews")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let reviews = store(project)?;
    for reference in links
        .iter()
        .filter_map(|l| l.get("ref").and_then(Value::as_str))
    {
        let Ok(found) = reviews.get_review(&ReviewRef(reference.to_string())) else {
            continue;
        };
        if matches!(
            found.header.status,
            Status::Waiting | Status::ChangesRequested
        ) {
            return Ok(Some(reference.to_string()));
        }
    }
    Ok(None)
}

/// What opening a review waiting on its reviewers does to each of `ids`, decided before anything is written, so the
/// review is never kept beside a task that didn't follow it (ATM-631). Exit code 4 when another agent holds a task.
///
/// # Errors
/// A task another agent holds, one blocked, done or skipped, one in to do that the author didn't start, or one
/// another review already waits on: each says what to do instead.
pub(crate) fn openings(project: &Project, ids: &[String]) -> Result<Vec<Opening>> {
    let me = project.actor().map(String::from);
    let now = atm_common::time::now();
    let claims = crate::query::list_claims(project);
    let vocab: &Vocabulary = project.store().vocabulary()?;
    let mut planned = Vec::with_capacity(ids.len());
    for id in ids {
        let Some(task) = project.store().get_task(id)? else {
            return Err(Error::NotFound(format!("no task {id}")));
        };
        let key = project.task_key(task);
        let claim = claims
            .iter()
            .find(|c| c.get("task").and_then(Value::as_str) == Some(id.as_str()));
        let held_by = claim.and_then(|c| holder(c, now));
        if let Some(other) = held_by.filter(|agent| Some(*agent) != me.as_deref()) {
            return Err(Error::Conflict(format!(
                "can't open a review of {key}: it is claimed by {other}, who opens its review"
            )));
        }
        // The author's own claim, run out but not yet dropped: the task is about to go back to to do
        let own_lapsed = claim.is_some_and(|c| {
            held_by.is_none() && c.get("agent").and_then(Value::as_str) == me.as_deref()
        });
        // Its lapse cleared `started_by`: the lapse's note says whose it was (ATM-712)
        let started_by_me = me
            .as_deref()
            .is_some_and(|me| crate::lifecycle::lapsed_for(task, me));
        let kind = vocab.kind_of(task.get("status")).unwrap_or(Kind::Todo);
        planned.push(match kind {
            Kind::Active if own_lapsed => Opening::TakeBack,
            Kind::Active => Opening::Review,
            Kind::Todo if started_by_me && me.is_some() => Opening::TakeBack,
            Kind::Todo => {
                return Err(Error::failed(format!(
                    "can't open a review of {key}: it is to do, and you didn't start it. Start it first \
                     (atm task state {key} in_progress), then open its review"
                )));
            }
            Kind::Review => match waiting_review(project, task)? {
                Some(other) => {
                    return Err(Error::failed(format!(
                        "can't open a review of {key}: {other} already waits on it. Add a round to it instead \
                         (atm review round {other})"
                    )));
                }
                None => Opening::Link,
            },
            Kind::Blocked => {
                return Err(Error::failed(format!(
                    "can't open a review of {key}: it is blocked. Its question is answered first \
                     (atm task state {key} todo --answer '…'), then it is started again"
                )));
            }
            Kind::Done | Kind::Skipped => {
                return Err(Error::failed(format!(
                    "can't open a review of {key}: it is {}. Reopen it (atm task state {key} todo) to review new \
                     work on it",
                    if kind == Kind::Done { "done" } else { "skipped" }
                )));
            }
        });
    }
    Ok(planned)
}
