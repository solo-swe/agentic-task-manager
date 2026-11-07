//! A project's vocabularies (docs/v4/data-model.md §2–3): its statuses with their kinds, types and priorities, and the
//! lists and settings of `meta` every command checks before it does anything (sprints, releases, components, the
//! workflow), with the defaults of `atm-cli/atm/common/schema/vocabularies.json` for what a project doesn't define.
//! As the Python atm's `common/model.py` `vocabulary()`: the same checks, in the same order, with the same messages.

use std::sync::LazyLock;

use serde_json::{Map, Value};

use crate::error::{Error, Result};
use crate::ids::{valid_branch, valid_short_id};
use crate::py::{capitalize, py_repr, py_repr_opt, repr_str, strip, truthy};

/// The vocabularies' one source, shared with the dashboard.
const VOCABULARIES_JSON: &str =
    include_str!("../../../atm-cli/atm/common/schema/vocabularies.json");

/// What a status means: every rule acts on a status's kind, never its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// Waiting to be started.
    Todo,
    /// Being worked on.
    Active,
    /// Waiting for a review's decision.
    Review,
    /// Waiting on a decision (a person's or an agent's).
    Blocked,
    /// Finished with its work done.
    Done,
    /// Finished without its work done.
    Skipped,
}

impl Kind {
    /// Every kind, in the order `vocabularies.json` lists them.
    pub const ALL: [Kind; 6] = [
        Kind::Todo,
        Kind::Active,
        Kind::Review,
        Kind::Blocked,
        Kind::Done,
        Kind::Skipped,
    ];

    /// The kind's name, as `tasks.json` and `--json` write it.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Todo => "todo",
            Kind::Active => "active",
            Kind::Review => "review",
            Kind::Blocked => "blocked",
            Kind::Done => "done",
            Kind::Skipped => "skipped",
        }
    }

    /// The kind with this name, if there is one.
    #[must_use]
    pub fn parse(name: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.as_str() == name)
    }
}

/// An entry of a vocabulary: a type or a priority.
#[derive(Debug, Clone, PartialEq)]
pub struct Term {
    /// Its ID, as tasks name it.
    pub id: String,
    /// Its label: the one `meta` gives, else the ID in words. Any value: `meta` may hold a label that isn't text.
    pub label: Value,
}

/// A status: a term with its kind.
#[derive(Debug, Clone, PartialEq)]
pub struct Status {
    /// Its ID, as tasks name it.
    pub id: String,
    /// Its label (see [`Term::label`]).
    pub label: Value,
    /// What it means.
    pub kind: Kind,
}

/// A project's vocabularies, checked: what `meta` defines, the defaults for the rest.
#[derive(Debug, Clone, PartialEq)]
pub struct Vocabulary {
    /// The statuses, in order; at least one each of the kinds todo, active and done.
    pub statuses: Vec<Status>,
    /// The task types.
    pub types: Vec<Term>,
    /// The priorities, most urgent first.
    pub priorities: Vec<Term>,
    /// The priority of a task without one (or with an unknown one); one of `priorities`.
    pub default_priority: String,
    /// The sprints, in order, each with its `state` filled in.
    pub sprints: Vec<Map<String, Value>>,
    /// The releases, in order, each with its `state` filled in.
    pub releases: Vec<Map<String, Value>>,
    /// How new sprints are named: `people` or `time`.
    pub sprint_names: String,
    /// The project's review mode: `blocker`, `async`, `batch` or `none`.
    pub review: String,
    /// The components, in order, each with its `paths` filled in.
    pub components: Vec<Map<String, Value>>,
}

struct Defaults {
    statuses: Vec<Status>,
    types: Vec<Term>,
    priorities: Vec<Term>,
    default_priority: String,
}

static DEFAULTS: LazyLock<Defaults> = LazyLock::new(|| {
    let data: Value =
        serde_json::from_str(VOCABULARIES_JSON).expect("vocabularies.json is valid JSON");
    let terms = |key: &str| -> Vec<Term> {
        data[key]
            .as_array()
            .expect("vocabularies.json lists each vocabulary")
            .iter()
            .map(|t| Term {
                id: t["id"].as_str().expect("each term has an id").to_string(),
                label: t["label"].clone(),
            })
            .collect()
    };
    let statuses = data["statuses"]
        .as_array()
        .expect("vocabularies.json lists the statuses")
        .iter()
        .map(|s| Status {
            id: s["id"].as_str().expect("each status has an id").to_string(),
            label: s["label"].clone(),
            kind: Kind::parse(s["kind"].as_str().expect("each status has a kind"))
                .expect("each status's kind is a kind"),
        })
        .collect();
    Defaults {
        statuses,
        types: terms("types"),
        priorities: terms("priorities"),
        default_priority: data["default_priority"]
            .as_str()
            .expect("vocabularies.json has a default priority")
            .to_string(),
    }
});

/// The statuses every project has unless `meta.statuses` names its own.
#[must_use]
pub fn default_statuses() -> &'static [Status] {
    &DEFAULTS.statuses
}

const WORKFLOW_TYPES: [&str; 2] = ["direct", "pull-request"];
const SPRINT_STATES: [&str; 3] = ["planned", "active", "closed"];
const SPRINT_NAMING: [&str; 2] = ["people", "time"];
/// The review modes (SR-3.8): a task's, a component's or the project's.
pub const REVIEW_MODES: [&str; 4] = ["blocker", "async", "batch", "none"];
const RELEASE_STATES: [&str; 3] = ["planned", "released", "archived"];
/// The longest sprint name, in characters.
pub const MAX_SPRINT_NAME: usize = 48;

fn fail(message: impl Into<String>) -> Error {
    Error::failed(message)
}

fn one_of(value: &Value, choices: &[&str]) -> bool {
    value.as_str().is_some_and(|s| choices.contains(&s))
}

/// `YYYY-MM-DD`, digits and dashes only (not a check of the calendar).
/// Whether the value is a date, `YYYY-MM-DD`.
#[must_use]
pub fn is_date(value: &Value) -> bool {
    value.as_str().is_some_and(|s| {
        let bytes = s.as_bytes();
        bytes.len() == 10
            && bytes.iter().enumerate().all(|(i, b)| {
                if i == 4 || i == 7 {
                    *b == b'-'
                } else {
                    b.is_ascii_digit()
                }
            })
    })
}

/// A vocabulary's entries as `meta` lists them, each with its kind when the vocabulary has kinds.
type Entries = Vec<(Term, Option<Kind>)>;

/// One of a workflow's settings: its key, the check its value passes, and what the error says it must be.
type WorkflowCheck = (&'static str, fn(&Value) -> bool, &'static str);

/// `meta[key]` as a list of terms, each with its kind when `need_kind`, or None when `meta` doesn't have it (the
/// defaults then). Checked entry by entry, as Python does, so the first problem is the one reported.
fn terms(meta: &Map<String, Value>, key: &str, need_kind: bool) -> Result<Option<Entries>> {
    let Some(value) = meta.get(key) else {
        return Ok(None);
    };
    let items = value
        .as_array()
        .filter(|a| !a.is_empty())
        .ok_or_else(|| fail(format!("meta.{key} must be a non-empty list")))?;
    let mut seen: Vec<&str> = Vec::new();
    let mut found = Vec::new();
    for item in items {
        let id = item
            .as_object()
            .and_then(|o| o.get("id"))
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or_else(|| {
                fail(format!(
                    "meta.{key}: every entry needs an \"id\" (a non-empty string)"
                ))
            })?;
        if seen.contains(&id) {
            return Err(fail(format!("meta.{key}: \"{id}\" is listed twice")));
        }
        seen.push(id);
        let label = item
            .get("label")
            .filter(|l| truthy(l))
            .cloned()
            .unwrap_or_else(|| Value::String(capitalize(&id.replace('_', " "))));
        let kind = if need_kind {
            let given = item.get("kind");
            let parsed = given
                .and_then(Value::as_str)
                .and_then(Kind::parse)
                .ok_or_else(|| {
                    let kinds: Vec<&str> = Kind::ALL.iter().map(|k| k.as_str()).collect();
                    fail(format!(
                        "meta.{key}: \"{id}\" has the kind {}; use one of {}",
                        py_repr_opt(given),
                        kinds.join(", ")
                    ))
                })?;
            Some(parsed)
        } else {
            None
        };
        found.push((
            Term {
                id: id.to_string(),
                label,
            },
            kind,
        ));
    }
    Ok(Some(found))
}

fn statuses(meta: &Map<String, Value>) -> Result<Vec<Status>> {
    let Some(found) = terms(meta, "statuses", true)? else {
        return Ok(DEFAULTS.statuses.clone());
    };
    Ok(found
        .into_iter()
        .map(|(term, kind)| Status {
            id: term.id,
            label: term.label,
            kind: kind.expect("statuses are read with their kinds"),
        })
        .collect())
}

fn plain_terms(meta: &Map<String, Value>, key: &str, defaults: &[Term]) -> Result<Vec<Term>> {
    Ok(terms(meta, key, false)?.map_or_else(
        || defaults.to_vec(),
        |found| found.into_iter().map(|(t, _)| t).collect(),
    ))
}

/// `meta.workflow`, when set (docs/v4/skills-and-workflows.md S-1.1): an error naming what's wrong.
///
/// # Errors
/// When the workflow's type isn't one atm knows or one of its settings isn't what it must be.
pub fn check_workflow(workflow: Option<&Value>) -> Result<()> {
    let Some(workflow) = workflow.filter(|w| !w.is_null()) else {
        return Ok(());
    };
    let Some(w) = workflow
        .as_object()
        .filter(|w| w.get("type").is_some_and(|t| one_of(t, &WORKFLOW_TYPES)))
    else {
        return Err(fail(format!(
            "meta.workflow.type must be one of {}",
            WORKFLOW_TYPES.join(", ")
        )));
    };
    let checks: [WorkflowCheck; 5] = [
        (
            "base",
            |v| v.as_str().is_some_and(valid_branch),
            "a branch name (letters, digits, `.`, `_`, `/`, `-`; not starting with `-`)",
        ),
        ("verify", Value::is_boolean, "true or false"),
        (
            "max_rounds",
            |v| v.as_u64().is_some_and(|n| n > 0),
            "a positive whole number",
        ),
        (
            "commit_template",
            |v| v.as_str().is_some_and(|s| !s.is_empty()),
            "a message template like \"feat({id}): {name}\"",
        ),
        (
            "trailers",
            |v| {
                v.as_str() == Some("forbid")
                    || v.get("require")
                        .and_then(Value::as_array)
                        .is_some_and(|r| r.iter().all(Value::is_string))
            },
            "\"forbid\" or {\"require\": [\"Co-Authored-By: ...\"]}",
        ),
    ];
    for (key, valid, expected) in checks {
        if w.get(key).is_some_and(|v| !valid(v)) {
            return Err(fail(format!("meta.workflow.{key} must be {expected}")));
        }
    }
    Ok(())
}

fn with_state(item: &Map<String, Value>) -> Map<String, Value> {
    let mut filled = item.clone();
    if !filled.contains_key("state") {
        filled.insert("state".into(), Value::String("planned".into()));
    }
    filled
}

fn check_sprints(meta: &Map<String, Value>) -> Result<Vec<Map<String, Value>>> {
    let Some(value) = meta.get("sprints") else {
        return Ok(Vec::new());
    };
    let sprints = value
        .as_array()
        .ok_or_else(|| fail("meta.sprints must be a list of sprints"))?;
    let mut seen: Vec<String> = Vec::new();
    let mut active: Vec<&str> = Vec::new();
    let mut checked = Vec::new();
    for sprint in sprints {
        let (s, name) = sprint
            .as_object()
            .and_then(|s| {
                s.get("id")
                    .and_then(Value::as_str)
                    .filter(|id| !strip(id).is_empty())
                    .map(|id| (s, id))
            })
            .ok_or_else(|| {
                fail("meta.sprints: every sprint needs an \"id\", its name (a non-empty string)")
            })?;
        if name != strip(name) || name.chars().count() > MAX_SPRINT_NAME {
            return Err(fail(format!(
                "meta.sprints: {} is not a sprint name (at most {MAX_SPRINT_NAME} characters, no space at either end)",
                repr_str(name)
            )));
        }
        let folded = name.to_lowercase();
        if seen.contains(&folded) {
            return Err(fail(format!(
                "meta.sprints: {} is listed twice",
                repr_str(name)
            )));
        }
        seen.push(folded);
        let state = s
            .get("state")
            .cloned()
            .unwrap_or_else(|| Value::String("planned".into()));
        if !one_of(&state, &SPRINT_STATES) {
            return Err(fail(format!(
                "meta.sprints: {} has the state {}; use one of {}",
                repr_str(name),
                py_repr(&state),
                SPRINT_STATES.join(", ")
            )));
        }
        if state == "active" {
            active.push(name);
        }
        if let Some(due) = s.get("due").filter(|d| !is_date(d)) {
            return Err(fail(format!(
                "meta.sprints: {} has the due date {}; use YYYY-MM-DD",
                repr_str(name),
                py_repr(due)
            )));
        }
        for key in ["goal", "release", "start", "end"] {
            if s.get(key).is_some_and(|v| !v.is_string()) {
                return Err(fail(format!(
                    "meta.sprints: {}'s {key} must be a string",
                    repr_str(name)
                )));
            }
        }
        checked.push(with_state(s));
    }
    if active.len() > 1 {
        return Err(fail(format!(
            "meta.sprints: only one sprint can be active, but {} are",
            active.join(" and ")
        )));
    }
    Ok(checked)
}

fn check_releases(meta: &Map<String, Value>) -> Result<Vec<Map<String, Value>>> {
    let Some(value) = meta.get("releases") else {
        return Ok(Vec::new());
    };
    let releases = value
        .as_array()
        .ok_or_else(|| fail("meta.releases must be a list of releases"))?;
    let mut seen: Vec<&str> = Vec::new();
    let mut checked = Vec::new();
    for release in releases {
        let (r, id) = release
            .as_object()
            .and_then(|r| r.get("id").and_then(Value::as_str).filter(|id| valid_short_id(id)).map(|id| (r, id)))
            .ok_or_else(|| {
                fail("meta.releases: every release needs an \"id\" of letters, digits, \".\", \"-\" and \"_\" (at most 64)")
            })?;
        if seen.contains(&id) {
            return Err(fail(format!(
                "meta.releases: {} is listed twice",
                repr_str(id)
            )));
        }
        seen.push(id);
        let state = r
            .get("state")
            .cloned()
            .unwrap_or_else(|| Value::String("planned".into()));
        if !one_of(&state, &RELEASE_STATES) {
            return Err(fail(format!(
                "meta.releases: {} has the state {}; use one of {}",
                repr_str(id),
                py_repr(&state),
                RELEASE_STATES.join(", ")
            )));
        }
        if let Some(target) = r.get("target").filter(|t| !is_date(t)) {
            return Err(fail(format!(
                "meta.releases: {} has the target {}; use YYYY-MM-DD",
                repr_str(id),
                py_repr(target)
            )));
        }
        for key in ["name", "released_at"] {
            if r.get(key).is_some_and(|v| !v.is_string()) {
                return Err(fail(format!(
                    "meta.releases: {}'s {key} must be a string",
                    repr_str(id)
                )));
            }
        }
        checked.push(with_state(r));
    }
    Ok(checked)
}

fn check_components(meta: &Map<String, Value>) -> Result<Vec<Map<String, Value>>> {
    let Some(value) = meta.get("components") else {
        return Ok(Vec::new());
    };
    let components = value
        .as_array()
        .ok_or_else(|| fail("meta.components must be a list of components"))?;
    let mut seen: Vec<&str> = Vec::new();
    let mut checked = Vec::new();
    for component in components {
        let (c, name) = component
            .as_object()
            .and_then(|c| c.get("id").and_then(Value::as_str).filter(|id| valid_short_id(id)).map(|id| (c, id)))
            .ok_or_else(|| {
                fail("meta.components: every component needs an \"id\" of letters, digits, \".\", \"-\" and \"_\" (at most 64)")
            })?;
        if seen.contains(&name) {
            return Err(fail(format!(
                "meta.components: {} is listed twice",
                repr_str(name)
            )));
        }
        seen.push(name);
        let paths_ok = c.get("paths").is_none_or(|p| {
            p.as_array().is_some_and(|paths| {
                paths
                    .iter()
                    .all(|p| p.as_str().is_some_and(|s| !s.is_empty()))
            })
        });
        if !paths_ok {
            return Err(fail(format!(
                "meta.components: {}'s paths must be a list of globs (strings)",
                repr_str(name)
            )));
        }
        if let Some(review) = c.get("review").filter(|r| !one_of(r, &REVIEW_MODES)) {
            return Err(fail(format!(
                "meta.components: {} has the review {}; use one of {}",
                repr_str(name),
                py_repr(review),
                REVIEW_MODES.join(", ")
            )));
        }
        if c.get("label").is_some_and(|l| !l.is_string()) {
            return Err(fail(format!(
                "meta.components: {}'s label must be a string",
                repr_str(name)
            )));
        }
        let mut filled = c.clone();
        if !filled.contains_key("paths") {
            filled.insert("paths".into(), Value::Array(Vec::new()));
        }
        checked.push(filled);
    }
    Ok(checked)
}

fn check_setting(
    meta: &Map<String, Value>,
    key: &str,
    choices: &[&str],
    default: &str,
) -> Result<String> {
    let value = meta
        .get(key)
        .cloned()
        .unwrap_or_else(|| Value::String(default.into()));
    match value.as_str() {
        Some(s) if choices.contains(&s) => Ok(s.to_string()),
        _ => Err(fail(format!(
            "meta.{key} must be one of {}",
            choices.join(", ")
        ))),
    }
}

impl Vocabulary {
    /// The vocabularies of a task file's document (`meta`, or none when it isn't an object).
    ///
    /// # Errors
    /// When `meta` defines something that can't work; the message names it.
    pub fn of_document(document: &Value) -> Result<Vocabulary> {
        let empty = Map::new();
        Vocabulary::of_meta(
            document
                .get("meta")
                .and_then(Value::as_object)
                .unwrap_or(&empty),
        )
    }

    /// The vocabularies `meta` defines, with the defaults for the rest.
    ///
    /// # Errors
    /// When `meta` defines something that can't work; the message names it.
    pub fn of_meta(meta: &Map<String, Value>) -> Result<Vocabulary> {
        let statuses = statuses(meta)?;
        for kind in [Kind::Todo, Kind::Active, Kind::Done] {
            if !statuses.iter().any(|s| s.kind == kind) {
                return Err(fail(format!(
                    "meta.statuses needs at least one status of the kind {}",
                    repr_str(kind.as_str())
                )));
            }
        }
        let priorities = plain_terms(meta, "priorities", &DEFAULTS.priorities)?;
        let ids: Vec<&str> = priorities.iter().map(|p| p.id.as_str()).collect();
        // Without a default: P2 if the project has it, else the middle priority (as the dashboard does)
        let fallback = if ids.contains(&DEFAULTS.default_priority.as_str()) {
            DEFAULTS.default_priority.clone()
        } else {
            ids[(ids.len() - 1) / 2].to_string()
        };
        let default_priority = match meta.get("default_priority") {
            None => fallback,
            Some(Value::String(p)) if ids.contains(&p.as_str()) => p.clone(),
            Some(other) => {
                return Err(fail(format!(
                    "meta.default_priority {} is not one of meta.priorities",
                    py_repr(other)
                )));
            }
        };
        // meta.phases, from before sprints, isn't read: atm migrate turns it into sprints (SR-8.1, SR-8.3)
        check_workflow(meta.get("workflow"))?;
        Ok(Vocabulary {
            statuses,
            types: plain_terms(meta, "types", &DEFAULTS.types)?,
            priorities,
            default_priority,
            sprints: check_sprints(meta)?,
            releases: check_releases(meta)?,
            sprint_names: check_setting(meta, "sprint_names", &SPRINT_NAMING, "people")?,
            review: check_setting(meta, "review", &REVIEW_MODES, "none")?,
            components: check_components(meta)?,
        })
    }

    /// The kind of a status. One the project doesn't list keeps its default kind if it is a default status; any
    /// other has none.
    #[must_use]
    pub fn kind_of(&self, status: Option<&Value>) -> Option<Kind> {
        let status = status?.as_str()?;
        self.statuses
            .iter()
            .chain(DEFAULTS.statuses.iter())
            .find(|s| s.id == status)
            .map(|s| s.kind)
    }

    /// A task's priority: its own when the project has it, else the default.
    #[must_use]
    pub fn priority_of(&self, task: &Map<String, Value>) -> String {
        match task.get("priority").and_then(Value::as_str) {
            Some(p) if self.priorities.iter().any(|t| t.id == p) => p.to_string(),
            _ => self.default_priority.clone(),
        }
    }
}

#[cfg(test)]
mod tests;
