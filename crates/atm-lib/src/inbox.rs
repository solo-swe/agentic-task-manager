//! The inbox's rules (docs/inbox-design.md §3–§4, ATM-759): a project's raw input, `IN-<n>` items, each created by a
//! person, then discarded with a reason or injected into the tasks made from it. Who may move an item, from which
//! state, and what each move records; the front ends (`atm inbox`, the REST routes, the app's commands) only ask.
//!
//! | Move | Who | From |
//! |---|---|---|
//! | add | a person; from another project, an agent too (ATM-772: recorded as `from_project`) | — |
//! | edit | a person | created |
//! | discard (a reason) | anyone (an agent on the user's word: the triage skill) | created |
//! | inject (tasks that exist) | anyone | created; final |
//! | restore | a person | discarded |
//!
//! Each move runs in one change of the store, under its lock: an injection records the item's tasks and each task's
//! `from` together.

use atm_common::archive::recent_since;
use atm_common::edits::is_agent;
use atm_common::inbox::{
    InboxState, checked_reason, checked_text, item_id, item_number, next_number, state_of,
};
use atm_common::time::{now_iso, parse_iso};
use atm_common::{Error, Result};
use atm_db::working::Working;
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::project::Project;

/// Which items a listing gives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Listing {
    /// The open ones (created): the default.
    Open,
    /// Those in one state.
    In(InboxState),
    /// All of them.
    All,
}

/// An inbox's items as one listing gives them, newest first, and how many there are in each state (of the whole
/// inbox, or archive, whatever the listing).
#[derive(Debug, Clone, PartialEq)]
pub struct Listed {
    pub items: Vec<Value>,
    pub created: usize,
    pub discarded: usize,
    pub injected: usize,
}

fn items(doc: &Value) -> Vec<Value> {
    doc.get("items")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

/// A document's items, to change: made a list first when they aren't one
fn items_mut(doc: &mut Value) -> &mut Vec<Value> {
    if !doc["items"].is_array() {
        doc["items"] = json!([]);
    }
    doc["items"].as_array_mut().expect("made a list just above")
}

fn number_of(item: &Value) -> u64 {
    item.get("id")
        .and_then(Value::as_str)
        .and_then(item_number)
        .unwrap_or(0)
}

/// The project's inbox (or its archive, `archived`) as `which` lists it.
///
/// # Errors
/// When the inbox can't be read.
pub fn list(project: &Project, which: Listing, archived: bool) -> Result<Listed> {
    let mut all = items(&project.store().inbox(archived)?);
    all.sort_by_key(|i| std::cmp::Reverse(number_of(i)));
    let count = |state| all.iter().filter(|i| state_of(i) == Some(state)).count();
    let (created, discarded, injected) = (
        count(InboxState::Created),
        count(InboxState::Discarded),
        count(InboxState::Injected),
    );
    let items = all
        .into_iter()
        .filter(|i| match which {
            Listing::All => true,
            Listing::Open => state_of(i) == Some(InboxState::Created),
            Listing::In(state) => state_of(i) == Some(state),
        })
        .collect();
    Ok(Listed {
        items,
        created,
        discarded,
        injected,
    })
}

/// The item `id` names (`IN-3`, any case), in the inbox or else its archive.
///
/// # Errors
/// `Invalid` for no item ID, `NotFound` for no such item; an inbox that can't be read.
pub fn show(project: &Project, id: &str) -> Result<Value> {
    let number = parsed(id)?;
    for archived in [false, true] {
        if let Some(item) = items(&project.store().inbox(archived)?)
            .into_iter()
            .find(|i| number_of(i) == number)
        {
            return Ok(item);
        }
    }
    Err(no_item(number))
}

fn parsed(id: &str) -> Result<u64> {
    item_number(id.trim())
        .ok_or_else(|| Error::Invalid(format!("{id:?} is no inbox item (IN-<n>)")))
}

fn no_item(number: u64) -> Error {
    Error::NotFound(format!("no inbox item {}", item_id(number)))
}

/// Who is acting, refused if it is an agent, for the moves that are a person's
fn person(project: &Project, doing: &str) -> Result<String> {
    let actor = project.actor();
    if is_agent(actor) {
        return Err(Error::Refused(format!(
            "only a person {doing} (the inbox is the user's input; {} is an agent)",
            actor.unwrap_or_default()
        )));
    }
    Ok(actor.unwrap_or("someone").to_string())
}

fn actor(project: &Project) -> String {
    project.actor().unwrap_or("someone").to_string()
}

/// The item numbered `number` in the change's inbox, in state `from` (one of them), to change in place
fn item_in<'w>(
    work: &'w mut Working,
    number: u64,
    from: &[InboxState],
    doing: &str,
) -> Result<&'w mut Value> {
    let inbox = work.inbox()?;
    let item = inbox
        .get_mut("items")
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
        .find(|i| number_of(i) == number)
        .ok_or_else(|| no_item(number))?;
    let state = state_of(item);
    if !state.is_some_and(|s| from.contains(&s)) {
        let is = state.map_or("of no known state", InboxState::as_str);
        return Err(Error::Refused(format!(
            "{} is {is}: only a{} {} item can be {doing}",
            item_id(number),
            if from[0] == InboxState::Created {
                "n"
            } else {
                ""
            },
            from.iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(" or "),
        )));
    }
    Ok(item)
}

/// How another project names `project` in an item it gets from it (`from_project`): its key, else its `meta.id`.
///
/// # Errors
/// `Invalid` for a project with neither; task data that can't be read.
pub fn project_name(project: &Project) -> Result<String> {
    let document = project.store().document()?;
    atm_common::keys::project_key(document)
        .filter(|k| !k.is_empty())
        .or_else(|| {
            document
                .get("meta")
                .and_then(|m| m.get("id"))
                .and_then(Value::as_str)
        })
        .map(String::from)
        .ok_or_else(|| Error::Invalid("this project has no key or ID to name it by".into()))
}

/// Add an item with `text` to the inbox, as the one acting. `from`: the project it comes from (its key, else its
/// ID), when it comes from another project's agent or person (ATM-772); none for the project's own.
///
/// To its own project's inbox only a person adds (the inbox is the user's input there); from another project, an agent
/// may too: a finding for this project's user to triage, recorded as `from_project`.
///
/// # Errors
/// `Refused` for an agent adding to its own project's inbox; `Invalid` text or origin; the store's.
pub fn add(project: &mut Project, text: &str, from: Option<&str>) -> Result<Value> {
    let from = from.map(str::trim);
    let by = match from {
        None => person(project, "adds to its project's inbox")?,
        Some("") => {
            return Err(Error::Invalid(
                "the project an item comes from has no name".into(),
            ));
        }
        Some(_) => actor(project),
    };
    let text = checked_text(text)?;
    project.change(|work| {
        // Past every item ever numbered, the archived ones too: a number is never given twice
        let past_archive = next_number(work.inbox_archive()?);
        let inbox = work.inbox()?;
        let number = next_number(inbox).max(past_archive);
        let now = now_iso();
        let mut item = json!({
            "id": item_id(number), "text": text, "state": InboxState::Created.as_str(),
            "created_at": now, "created_by": by, "updated_at": now,
        });
        if let Some(from) = from {
            item["from_project"] = json!(from);
        }
        inbox["version"] = json!(atm_common::inbox::INBOX_VERSION);
        inbox["next"] = json!(number + 1);
        items_mut(inbox).push(item.clone());
        Ok(item)
    })
}

/// Change an open item's text, as the person acting.
///
/// # Errors
/// `Refused` for an agent or an item no longer open; `Invalid` text or ID; `NotFound`; the store's.
pub fn edit(project: &mut Project, id: &str, text: &str) -> Result<Value> {
    let number = parsed(id)?;
    let by = person(project, "edits an inbox item")?;
    let text = checked_text(text)?;
    project.change(|work| {
        let item = item_in(work, number, &[InboxState::Created], "edited")?;
        item["text"] = json!(text);
        item["updated_at"] = json!(now_iso());
        item["updated_by"] = json!(by);
        Ok(item.clone())
    })
}

/// Discard an open item, saying why.
///
/// # Errors
/// `Refused` for an item no longer open; `Invalid` reason or ID; `NotFound`; the store's.
pub fn discard(project: &mut Project, id: &str, reason: &str) -> Result<Value> {
    let number = parsed(id)?;
    let reason = checked_reason(reason)?;
    let by = actor(project);
    project.change(|work| {
        let item = item_in(work, number, &[InboxState::Created], "discarded")?;
        let now = now_iso();
        item["state"] = json!(InboxState::Discarded.as_str());
        item["reason"] = json!(reason);
        item["decided_at"] = json!(now);
        item["decided_by"] = json!(by);
        item["updated_at"] = json!(now);
        Ok(item.clone())
    })
}

/// Bring a discarded item back to open, as the person acting: its reason and decision go.
///
/// # Errors
/// `Refused` for an agent or an item that isn't discarded; `Invalid` ID; `NotFound`; the store's.
pub fn restore(project: &mut Project, id: &str) -> Result<Value> {
    let number = parsed(id)?;
    let by = person(project, "restores an inbox item")?;
    project.change(|work| {
        let item = item_in(work, number, &[InboxState::Discarded], "restored")?;
        if let Some(fields) = item.as_object_mut() {
            for key in ["reason", "decided_at", "decided_by"] {
                fields.shift_remove(key);
            }
        }
        item["state"] = json!(InboxState::Created.as_str());
        item["updated_at"] = json!(now_iso());
        item["updated_by"] = json!(by);
        Ok(item.clone())
    })
}

/// Record that an open item became `tasks` (at least one, each a working task of this project): the item injected
/// with their keys, and each task's `from` naming the item. Final.
///
/// # Errors
/// `Invalid` with no task; `NotFound` for a task that isn't a working one; `Refused` for an item no longer open; the
/// store's.
pub fn inject(project: &mut Project, id: &str, tasks: &[String]) -> Result<Value> {
    let number = parsed(id)?;
    if tasks.is_empty() {
        return Err(Error::Invalid(format!(
            "an injection names the tasks {} became (--task, at least one)",
            item_id(number)
        )));
    }
    let mut ids: Vec<String> = Vec::new();
    for reference in tasks {
        let task = project.working_task(reference)?;
        if !ids.contains(&task) {
            ids.push(task);
        }
    }
    let keys: Vec<String> = ids
        .iter()
        .map(|id| {
            project
                .store()
                .get_task(id)
                .ok()
                .flatten()
                .map_or_else(|| id.clone(), |t| project.task_key(t))
        })
        .collect();
    let by = actor(project);
    let item_name = item_id(number);
    project.change(|work| {
        // The item first: a refusal (not open) changes no task
        let item = item_in(work, number, &[InboxState::Created], "injected")?;
        let now = now_iso();
        item["state"] = json!(InboxState::Injected.as_str());
        item["tasks"] = json!(keys);
        item["decided_at"] = json!(now);
        item["decided_by"] = json!(by);
        item["updated_at"] = json!(now);
        let item = item.clone();
        for task in work.tasks().iter_mut().filter_map(Value::as_object_mut) {
            if task
                .get("id")
                .and_then(Value::as_str)
                .is_some_and(|t| ids.iter().any(|i| i == t))
            {
                add_from(task, &item_name);
            }
        }
        Ok(item)
    })
}

fn add_from(task: &mut Map<String, Value>, item: &str) {
    let from = task.entry("from").or_insert_with(|| json!([]));
    if !from.is_array() {
        *from = json!([]);
    }
    let list = from.as_array_mut().expect("made a list just above");
    if !list.iter().any(|v| v.as_str() == Some(item)) {
        list.push(json!(item));
    }
}

/// Move the decided items (discarded or injected) decided more than `keep_days` ago (`meta.archive.keep_days`
/// without it, as `atm db archive` keeps finished tasks) into the inbox's archive; open items stay. The IDs moved.
///
/// # Errors
/// The store's.
pub fn archive(project: &mut Project, keep_days: Option<f64>) -> Result<Vec<String>> {
    let before = recent_since(project.store().document()?, keep_days);
    project.change(|work| {
        let inbox = work.inbox()?;
        let (moving, staying): (Vec<Value>, Vec<Value>) = items(inbox).into_iter().partition(|i| {
            state_of(i).is_some_and(InboxState::decided)
                && i.get("decided_at")
                    .and_then(Value::as_str)
                    .and_then(parse_iso)
                    .is_some_and(|at| at <= before)
        });
        if moving.is_empty() {
            return Ok(Vec::new());
        }
        inbox["items"] = Value::Array(staying);
        let moved: Vec<String> = moving
            .iter()
            .filter_map(|i| i.get("id").and_then(Value::as_str).map(String::from))
            .collect();
        work.moving_into_inbox_archive();
        let archive = work.inbox_archive()?;
        archive["version"] = json!(atm_common::inbox::INBOX_VERSION);
        items_mut(archive).extend(moving);
        Ok(moved)
    })
}

/// Bring archived items back into the inbox, as they were. The IDs brought back.
///
/// # Errors
/// `Invalid` for no item ID; `NotFound` for one the archive doesn't hold; the store's.
pub fn unarchive(project: &mut Project, ids: &[String]) -> Result<Vec<String>> {
    let numbers: Vec<u64> = ids.iter().map(|id| parsed(id)).collect::<Result<_>>()?;
    project.change(|work| {
        let archive = work.inbox_archive()?;
        let (back, staying): (Vec<Value>, Vec<Value>) = items(archive)
            .into_iter()
            .partition(|i| numbers.contains(&number_of(i)));
        if let Some(missing) = numbers
            .iter()
            .find(|n| !back.iter().any(|i| number_of(i) == **n))
        {
            return Err(Error::NotFound(format!(
                "no archived inbox item {}",
                item_id(*missing)
            )));
        }
        // The numbers given so far, the archive's included, before the items move
        let past_archive = next_number(archive);
        archive["items"] = Value::Array(staying);
        let names = back
            .iter()
            .filter_map(|i| i.get("id").and_then(Value::as_str).map(String::from))
            .collect();
        let inbox = work.inbox()?;
        items_mut(inbox).extend(back);
        let next = next_number(inbox).max(past_archive);
        inbox["next"] = json!(next);
        Ok(names)
    })
}

/// `POST …/inbox` (ATM-761): an item's text.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NewItem {
    pub text: String,
}

/// `PATCH …/inbox/<IN>`: an open item's new text.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ItemEdit {
    pub text: String,
}

/// What a person decides about an item on the page: discard it (with why), or bring a discarded one back. Inject isn't
/// one: an agent records it, with the tasks it made (docs/inbox-design.md §7).
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Decided {
    Discarded,
    Restored,
}

/// `POST …/inbox/<IN>/decisions`: a decision, and for a discard its reason.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ItemDecision {
    pub decision: Decided,
    #[serde(default)]
    pub reason: Option<String>,
}

/// A person's decision on item `id`: discarded with its reason, or restored (no reason).
///
/// # Errors
/// `Invalid` for a discard without a reason or a restore with one; what [`discard`] and [`restore`] refuse.
pub fn decide(project: &mut Project, id: &str, decision: &ItemDecision) -> Result<Value> {
    match (decision.decision, decision.reason.as_deref()) {
        (Decided::Discarded, Some(reason)) => discard(project, id, reason),
        (Decided::Discarded, None) => Err(Error::Invalid("a discard says why (reason)".into())),
        (Decided::Restored, None) => restore(project, id),
        (Decided::Restored, Some(_)) => Err(Error::Invalid("a restore takes no reason".into())),
    }
}

/// The listing a `state` names (`created`, the open ones, when none; `discarded`, `injected`, `all`).
///
/// # Errors
/// `Invalid` for a state that isn't one.
pub fn listing_of(state: Option<&str>) -> Result<Listing> {
    match state {
        None | Some("created") => Ok(Listing::Open),
        Some("all") => Ok(Listing::All),
        Some(other) => InboxState::parse(other)
            .map(Listing::In)
            .ok_or_else(|| Error::Invalid(format!("no inbox state {other:?}"))),
    }
}

/// How many items wait in the inbox (created): the status line's and the dashboard's badge.
///
/// # Errors
/// When the inbox can't be read.
pub fn open_count(project: &Project) -> Result<usize> {
    Ok(items(&project.store().inbox(false)?)
        .iter()
        .filter(|i| state_of(i) == Some(InboxState::Created))
        .count())
}

#[cfg(test)]
mod tests;
