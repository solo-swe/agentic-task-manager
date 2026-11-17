//! What the app and the browser ask of a project's reviews (docs/code-review-design.md §3.12, ATM-572): one function
//! per operation, each the same atm-lib call and review rules as its `atm review …` command, acting as the project's
//! actor, which the front ends open **as the user** (`Member::project_as_user`), never as an agent. A review's
//! decisions stay the user's (I23): these are the user's own actions in their page.
//!
//! The request bodies are the JSON the page sends (each with `deny_unknown_fields`), checked as the CLI checks its
//! arguments: texts not empty and at most [`MAX_TEXT`] characters, files inside the repository (`GitVcs::paths`),
//! revisions only as commits (atm-vcs).

use atm_common::{Error, Result};
use serde::Deserialize;

use crate::project::Project;
use crate::reviews::{
    self, Actor, Anchor, CommentId, NewReview, NewRound, ReviewHeader, ReviewRef, ReviewStore,
    Reviews, Side, Status, Verdict, rounds,
};

/// The longest text a request takes (a comment, a summary, a reason): a page's, not a file's.
pub const MAX_TEXT: usize = 100_000;

/// What a new review's first round is made from.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ChangeToReview {
    /// A commit range, `BASE..HEAD`.
    #[serde(default)]
    pub range: Option<String>,
    /// The working copy's changes, against `base` (else `HEAD`).
    #[serde(default)]
    pub uncommitted: bool,
    #[serde(default)]
    pub base: Option<String>,
    /// Files as they are, from the project's root.
    #[serde(default)]
    pub files: Vec<String>,
}

/// How a new review starts.
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStart {
    /// Waiting on its reviewers (its tasks in progress go in review).
    #[default]
    Open,
    /// The author's, not sent yet.
    Draft,
    /// Reviewed as it is made, on the working copy.
    Live,
}

/// `POST …/reviews`: a new review.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReviewOpening {
    pub title: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub tasks: Vec<String>,
    #[serde(default)]
    pub reviewers: Vec<String>,
    #[serde(default)]
    pub start: ReviewStart,
    /// What it reviews; a live review is always the working copy (its `base` only)
    #[serde(default)]
    pub change: ChangeToReview,
}

/// `POST …/reviews/<CR>/comments`: a new chain, on a file's lines (or the whole file), or the review as a whole.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NewComment {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub lines: Option<(u32, u32)>,
    #[serde(default)]
    pub side: Side,
    #[serde(default)]
    pub quote: Option<String>,
    /// The round it is on; the latest when none is named
    #[serde(default)]
    pub round: Option<u32>,
    pub body: String,
}

/// `POST …/comments/<c>/replies`: a reply.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NewReply {
    pub body: String,
}

/// `PATCH …/comments/<c>`: a comment's text (one's own), or a top-level comment's resolved box; one of them.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CommentUpdate {
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub resolved: Option<bool>,
}

/// A reviewer's decision, as the page names it.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DecisionKind {
    Approve,
    RequestChanges,
    Comment,
}

/// `POST …/reviews/<CR>/decisions`: a reviewer's decision, with a comment on the whole review.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Decision {
    pub decision: DecisionKind,
    #[serde(default)]
    pub body: Option<String>,
}

/// `PATCH …/reviews/<CR>`: the review closed without approval (the one change of a review's own fields a page makes).
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReviewClosing {
    /// Always `closed`.
    pub status: Status,
    pub reason: String,
    /// Its tasks rejected too
    #[serde(default)]
    pub reject_tasks: bool,
}

/// `PUT …/reviews/<CR>/tasks`: the tasks it reviews, as a whole.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReviewTasks {
    pub tasks: Vec<String>,
}

/// A page's JSON as the request it is.
///
/// # Errors
/// `Invalid` naming what doesn't fit (an unknown field, a missing one).
pub fn request_from<T: for<'de> Deserialize<'de>>(body: serde_json::Value) -> Result<T> {
    serde_json::from_value(body).map_err(|e| Error::Invalid(format!("not this request: {e}")))
}

fn text<'t>(what: &str, text: &'t str) -> Result<&'t str> {
    if text.trim().is_empty() {
        return Err(Error::Invalid(format!("{what} is empty")));
    }
    if text.chars().count() > MAX_TEXT {
        return Err(Error::TooLarge(format!(
            "{what} is longer than {MAX_TEXT} characters"
        )));
    }
    Ok(text)
}

fn actor_of(project: &Project) -> Actor {
    Actor(project.actor().unwrap_or("someone").to_string())
}

fn first_round(project: &Project, change: &ChangeToReview, by: &Actor) -> Result<NewRound> {
    let vcs = reviews::vcs(project)?;
    if let Some(range) = &change.range {
        let (base, head) = range
            .split_once("..")
            .ok_or_else(|| Error::Invalid(format!("a range is BASE..HEAD, not {range}")))?;
        return rounds::from_range(&vcs, base, if head.is_empty() { "HEAD" } else { head }, by);
    }
    if !change.files.is_empty() {
        // From the project's root, never outside the repository
        let paths = vcs.paths(&change.files, None)?;
        return rounds::from_files(&vcs, &paths, by);
    }
    if change.uncommitted {
        return rounds::from_working(&vcs, change.base.as_deref().unwrap_or("HEAD"), by);
    }
    Err(Error::Invalid(
        "say what to review: a range, the uncommitted changes, or files".into(),
    ))
}

/// A new review (`atm review create`, or `live`), its tasks in progress put in review when it is open.
///
/// # Errors
/// No change named, one git can't diff, an empty title, or what the review's rules refuse.
pub fn open_review(project: &mut Project, opening: ReviewOpening) -> Result<ReviewHeader> {
    let by = actor_of(project);
    let change = if opening.start == ReviewStart::Live {
        ChangeToReview {
            uncommitted: true,
            base: opening.change.base,
            ..ChangeToReview::default()
        }
    } else {
        opening.change
    };
    let first = first_round(project, &change, &by)?;
    reviews::create(
        project,
        NewReview {
            title: text("the title", &opening.title)?.to_string(),
            summary: opening.summary,
            tasks: opening.tasks,
            author: by,
            reviewers: opening.reviewers.into_iter().map(Actor).collect(),
            status: match opening.start {
                ReviewStart::Open => Status::Open,
                ReviewStart::Draft => Status::Draft,
                ReviewStart::Live => Status::Live,
            },
            first,
        },
    )
}

/// A new chain on `review` (`atm review comment`): its first comment's ID.
///
/// # Errors
/// No such review, an empty text, or what the review's rules refuse.
pub fn add_comment(
    project: &Project,
    review: &ReviewRef,
    comment: NewComment,
) -> Result<CommentId> {
    let mut store = reviews::store(project)?;
    let found = store.get_review(review)?;
    let anchor = Anchor {
        round: comment
            .round
            .unwrap_or_else(|| found.rounds.last().map_or(1, |r| r.number)),
        path: comment.path,
        side: comment.side,
        lines: comment.lines,
        quote: comment.quote,
    };
    Reviews::new(&mut store).comment(
        &found.header.reference,
        anchor,
        text("the comment", &comment.body)?,
        &actor_of(project),
    )
}

/// A reply to `comment` (`atm review reply`): the reply's ID.
///
/// # Errors
/// No such review or comment, an empty text.
pub fn reply_to_comment(
    project: &Project,
    review: &ReviewRef,
    comment: &CommentId,
    reply: &NewReply,
) -> Result<CommentId> {
    let mut store = reviews::store(project)?;
    Reviews::new(&mut store).reply(
        review,
        comment,
        text("the reply", &reply.body)?,
        &actor_of(project),
    )
}

/// A comment's text changed (one's own, `atm review edit`) or its resolved box (`atm review resolve`).
///
/// # Errors
/// Neither or both asked for, no such comment, or what the review's rules refuse (another's text, a reply's box).
pub fn update_comment(
    project: &Project,
    review: &ReviewRef,
    comment: &CommentId,
    update: CommentUpdate,
) -> Result<()> {
    let mut store = reviews::store(project)?;
    let by = actor_of(project);
    match (update.body, update.resolved) {
        (Some(body), None) => {
            Reviews::new(&mut store).edit(review, comment, text("the comment", &body)?, &by)
        }
        (None, Some(resolved)) => Reviews::new(&mut store).resolve(review, comment, resolved, &by),
        _ => Err(Error::Invalid(
            "change the comment's body or its resolved box, one of them".into(),
        )),
    }
}

/// A reviewer's decision (`atm review submit`), which becomes the review's tasks'.
///
/// # Errors
/// A comment without its text, or what the review's rules refuse (the author deciding, open comments on approval).
pub fn submit_decision(
    project: &mut Project,
    review: &ReviewRef,
    decision: &Decision,
) -> Result<ReviewHeader> {
    let verdict = match decision.decision {
        DecisionKind::Approve => Verdict::Approve,
        DecisionKind::RequestChanges => Verdict::RequestChanges,
        DecisionKind::Comment => Verdict::Comment,
    };
    let body = decision
        .body
        .as_deref()
        .map(|b| text("the comment", b))
        .transpose()?;
    if verdict == Verdict::Comment && body.is_none() {
        return Err(Error::Invalid(
            "a comment on the review needs its text".into(),
        ));
    }
    reviews::submit(project, review, verdict, body)?;
    Ok(reviews::store(project)?.get_review(review)?.header)
}

/// The review closed without approval (`atm review close`).
///
/// # Errors
/// A status other than `closed`, an empty reason, or what the review's rules refuse.
pub fn close_review(
    project: &mut Project,
    review: &ReviewRef,
    closing: &ReviewClosing,
) -> Result<ReviewHeader> {
    if closing.status != Status::Closed {
        return Err(Error::Invalid(
            "a review is changed here only to closed".into(),
        ));
    }
    reviews::close(
        project,
        review,
        text("the reason", &closing.reason)?,
        closing.reject_tasks,
    )?;
    Ok(reviews::store(project)?.get_review(review)?.header)
}

/// The tasks it reviews, as a whole (`atm review tasks`, with what to add and remove worked out).
///
/// # Errors
/// No such review or task, or what the review's rules refuse.
pub fn set_review_tasks(
    project: &mut Project,
    review: &ReviewRef,
    wanted: &ReviewTasks,
) -> Result<ReviewHeader> {
    let now = reviews::store(project)?.get_review(review)?.header.tasks;
    let add: Vec<String> = wanted
        .tasks
        .iter()
        .filter(|t| !now.contains(t))
        .cloned()
        .collect();
    let remove: Vec<String> = now
        .iter()
        .filter(|t| !wanted.tasks.contains(t))
        .cloned()
        .collect();
    reviews::set_tasks(project, review, &add, &remove)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn each_request_is_its_own_body_and_takes_nothing_else() {
        let comment: NewComment =
            request_from(json!({"path": "a.rs", "lines": [3, 4], "side": "old", "body": "Why?"}))
                .unwrap();
        assert_eq!((comment.lines, comment.side), (Some((3, 4)), Side::Old));
        let decision: Decision = request_from(json!({"decision": "request_changes"})).unwrap();
        assert_eq!(decision.decision, DecisionKind::RequestChanges);
        let opening: ReviewOpening =
            request_from(json!({"title": "Look", "change": {"files": ["a.rs"]}})).unwrap();
        assert_eq!(opening.start, ReviewStart::Open);
        // An unknown field (another author, an action), a missing one: refused, naming why
        assert!(matches!(
            request_from::<NewReply>(json!({"body": "x", "author": "claude:x"})),
            Err(Error::Invalid(_))
        ));
        assert!(matches!(
            request_from::<NewReply>(json!({})),
            Err(Error::Invalid(_))
        ));
        assert!(matches!(
            request_from::<Decision>(json!({"decision": "merge"})),
            Err(Error::Invalid(_))
        ));
    }

    #[test]
    fn texts_are_checked_as_the_cli_checks_them() {
        assert!(matches!(text("the reply", "  "), Err(Error::Invalid(_))));
        assert!(matches!(
            text("the reply", &"x".repeat(MAX_TEXT + 1)),
            Err(Error::TooLarge(_))
        ));
        assert_eq!(text("the reply", "ok").unwrap(), "ok");
    }
}
