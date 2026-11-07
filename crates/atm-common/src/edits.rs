//! What a change writes on a task (the Python atm's `common/model.py`): notes, who changed it and when, the way back
//! to to do, the intervals of blocks and reviews, and the status a task moving to a kind gets. Each keeps the key
//! order the Python atm writes, so a task file written by either diffs cleanly.

use serde_json::{Map, Value, json};

use crate::Task;
use crate::error::{Error, Result};
use crate::vocabulary::{Kind, Vocabulary};

/// The longest note: anything longer belongs in a file in the repository.
pub const MAX_NOTE: usize = 500;

/// A note's text, checked: at most 500 characters.
///
/// # Errors
/// A longer one (exit code 2), with how many characters it has.
pub fn check_note(text: &str) -> Result<&str> {
    let n = text.chars().count();
    if n > MAX_NOTE {
        return Err(Error::Failed {
            message: format!(
                "a note is at most {MAX_NOTE} characters (this one has {n}); put longer text in a file in the repository and mention it in the note"
            ),
            code: 2,
        });
    }
    Ok(text)
}

/// Append a note (DM-4): `{at, kind, text[, by]}`. `checked` false for a note built from parts already checked one by
/// one (an unblock's questions and answers).
///
/// # Errors
/// A note too long, when `checked`.
pub fn add_note(
    task: &mut Task,
    kind: &str,
    text: &str,
    by: Option<&str>,
    stamp: &str,
    checked: bool,
) -> Result<()> {
    let text = if checked { check_note(text)? } else { text };
    let mut note = Map::new();
    note.insert("at".into(), json!(stamp));
    note.insert("kind".into(), json!(kind));
    note.insert("text".into(), json!(text));
    if let Some(by) = by.filter(|b| !b.is_empty()) {
        note.insert("by".into(), json!(by));
    }
    let notes = task.entry("notes").or_insert_with(|| json!([]));
    if !notes.is_array() {
        *notes = json!([]);
    }
    if let Value::Array(list) = notes {
        list.push(Value::Object(note));
    }
    Ok(())
}

/// Every change records when and by whom (CLI-3.5).
pub fn touch(task: &mut Task, actor: Option<&str>, stamp: &str) {
    task.insert("updated_at".into(), json!(stamp));
    if let Some(actor) = actor.filter(|a| !a.is_empty()) {
        task.insert("updated_by".into(), json!(actor));
    }
}

/// The statuses of a kind, in the project's order.
#[must_use]
pub fn statuses_of(vocab: &Vocabulary, kind: Kind) -> Vec<&str> {
    vocab
        .statuses
        .iter()
        .filter(|s| s.kind == kind)
        .map(|s| s.id.as_str())
        .collect()
}

/// The status a task moving to `kind` gets: `chosen` (`--status`), which must be of that kind, else the first status
/// of that kind the project lists.
///
/// # Errors
/// A chosen status of another kind, or none the project has (exit code 2).
pub fn status_for(vocab: &Vocabulary, kind: Kind, chosen: Option<&str>) -> Result<String> {
    let options = statuses_of(vocab, kind);
    let Some(chosen) = chosen else {
        return Ok(options
            .first()
            .map(|s| (*s).to_string())
            .unwrap_or_default());
    };
    if options.contains(&chosen) {
        return Ok(chosen.to_string());
    }
    let what = match vocab.kind_of(Some(&json!(chosen))) {
        Some(known) => format!("its kind is {}", known.as_str()),
        None => "the project has no such status".into(),
    };
    Err(Error::Failed {
        message: format!(
            "--status {chosen} is not one of the {} statuses ({what}); use {}",
            kind.as_str(),
            options.join(" or ")
        ),
        code: 2,
    })
}

/// The fields an attempt at a task leaves, which going back to to do clears (DM-4.4).
const ATTEMPT_FIELDS: [&str; 13] = [
    "started_at",
    "completed_at",
    "skipped_at",
    "started_by",
    "completed_by",
    "agent",
    "review_requested_at",
    "waiting_on",
    "questions",
    "blocked_at",
    "sprint_review",
    "resolution",
    "duplicate_of",
];

/// The task returns to the first todo-kind status, and its attempt's timestamps and actors go. The commit goes too,
/// unless `keep_commit`: an expired lease or a release leaves the work in git, to be finished later.
pub fn back_to_todo(
    task: &mut Task,
    vocab: &Vocabulary,
    actor: Option<&str>,
    stamp: &str,
    keep_commit: bool,
) {
    let todo = statuses_of(vocab, Kind::Todo)
        .first()
        .map_or_else(|| "todo".into(), |s| (*s).to_string());
    task.insert("status".into(), json!(todo));
    touch(task, actor, stamp);
    for field in ATTEMPT_FIELDS {
        task.shift_remove(field);
    }
    if !keep_commit {
        task.shift_remove("commit");
    }
}

/// Append `{from, to[, on][, outcome]}` to the task's list `field`: `blocks` (with who it waited on) or `reviews`
/// (with what the review decided).
pub fn add_interval(
    task: &mut Task,
    field: &str,
    start: &Value,
    end: &Value,
    on: Option<&Value>,
    outcome: Option<&str>,
) {
    let mut interval = Map::new();
    interval.insert("from".into(), start.clone());
    interval.insert("to".into(), end.clone());
    if let Some(on) = on.filter(|v| crate::py::truthy(v)) {
        interval.insert("on".into(), on.clone());
    }
    if let Some(outcome) = outcome {
        interval.insert("outcome".into(), json!(outcome));
    }
    let list = task.entry(field).or_insert_with(|| json!([]));
    if !list.is_array() {
        *list = json!([]);
    }
    if let Value::Array(items) = list {
        items.push(Value::Object(interval));
    }
}

/// One note for an unblocked task: each question with its answer.
#[must_use]
pub fn answer_note(questions: &[String], answers: &[String]) -> String {
    (0..questions.len().max(answers.len()))
        .map(|i| {
            match (
                questions.get(i).filter(|q| !q.is_empty()),
                answers.get(i).filter(|a| !a.is_empty()),
            ) {
                (Some(q), Some(a)) => format!("{q} → {a}"),
                (Some(q), None) => format!("{q} → (no answer)"),
                (None, Some(a)) => a.clone(),
                (None, None) => answers.get(i).cloned().unwrap_or_default(),
            }
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// Whether an actor is an agent (a Claude Code session, `claude:…`); anyone else is a person.
#[must_use]
pub fn is_agent(actor: Option<&str>) -> bool {
    actor.is_some_and(|a| a.starts_with("claude:"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(v: Value) -> Task {
        match v {
            Value::Object(m) => m,
            other => panic!("not a task: {other}"),
        }
    }

    #[test]
    fn notes_are_short_and_say_who_wrote_them() {
        let mut t = task(json!({"id": "T-1"}));
        add_note(
            &mut t,
            "outcome",
            "Shipped",
            Some("alice"),
            "2026-01-01T00:00:00Z",
            true,
        )
        .unwrap();
        add_note(&mut t, "note", "x", None, "2026-01-01T00:00:01Z", true).unwrap();
        assert_eq!(
            t["notes"],
            json!([
                {"at": "2026-01-01T00:00:00Z", "kind": "outcome", "text": "Shipped", "by": "alice"},
                {"at": "2026-01-01T00:00:01Z", "kind": "note", "text": "x"}
            ])
        );
        let long = "a".repeat(501);
        let error = add_note(&mut t, "note", &long, None, "x", true).unwrap_err();
        assert_eq!(error.code(), 2);
        assert!(
            error
                .to_string()
                .starts_with("a note is at most 500 characters (this one has 501)")
        );
        assert!(add_note(&mut t, "answer", &long, None, "x", false).is_ok());
    }

    #[test]
    fn back_to_todo_clears_the_attempt_and_keeps_the_commit_when_asked() {
        let vocab = Vocabulary::of_meta(&Map::new()).unwrap();
        let mut t = task(
            json!({"id": "T-1", "status": "in_progress", "started_at": "x", "started_by": "a", "commit": "abc"}),
        );
        back_to_todo(&mut t, &vocab, Some("bob"), "now", true);
        assert_eq!(
            Value::Object(t.clone()),
            json!({"id": "T-1", "status": "todo", "commit": "abc", "updated_at": "now", "updated_by": "bob"})
        );
        back_to_todo(&mut t, &vocab, None, "later", false);
        assert!(t.get("commit").is_none());
    }

    #[test]
    fn the_status_of_a_kind_is_the_first_or_the_one_chosen() {
        let vocab = Vocabulary::of_meta(&Map::new()).unwrap();
        assert_eq!(
            status_for(&vocab, Kind::Active, None).unwrap(),
            "in_progress"
        );
        assert_eq!(
            status_for(&vocab, Kind::Done, Some("done")).unwrap(),
            "done"
        );
        assert_eq!(
            status_for(&vocab, Kind::Done, Some("todo"))
                .unwrap_err()
                .to_string(),
            "--status todo is not one of the done statuses (its kind is todo); use done"
        );
        assert_eq!(
            status_for(&vocab, Kind::Done, Some("x"))
                .unwrap_err()
                .to_string(),
            "--status x is not one of the done statuses (the project has no such status); use done"
        );
    }

    #[test]
    fn answers_pair_with_their_questions() {
        let q = vec!["Which?".to_string(), "When?".to_string()];
        assert_eq!(
            answer_note(&q, &["A".into()]),
            "Which? → A; When? → (no answer)"
        );
        assert_eq!(answer_note(&[], &["Just this".into()]), "Just this");
    }

    #[test]
    fn intervals_and_agents() {
        let mut t = task(json!({}));
        add_interval(
            &mut t,
            "blocks",
            &json!("a"),
            &json!("b"),
            Some(&json!("user")),
            None,
        );
        add_interval(
            &mut t,
            "reviews",
            &json!("c"),
            &json!("d"),
            None,
            Some("approved"),
        );
        assert_eq!(t["blocks"], json!([{"from": "a", "to": "b", "on": "user"}]));
        assert_eq!(
            t["reviews"],
            json!([{"from": "c", "to": "d", "outcome": "approved"}])
        );
        assert!(is_agent(Some("claude:1")) && !is_agent(Some("alice")) && !is_agent(None));
    }
}
