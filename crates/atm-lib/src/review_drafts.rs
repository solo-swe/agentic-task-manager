//! A reviewer's drafts (ATM-612, from CR-2): comments and replies written for a review and sent together, with its
//! decision or on their own, as GitHub's *start a review*. Until then they are the reviewer's alone: kept in their own
//! atm state folder (`ATM_STATE_DIR`'s `drafts/`), never in the review's folder, which is committed to git, so no one
//! else sees them and an agent's review watch hears nothing of them. A file per project, review and person.
//!
//! A draft on a file is anchored as a comment is, on its round, so the page places it on later rounds as it places
//! that round's comments; on a live review, one about what is there now is anchored when it is sent, as a comment made
//! then would be (`add_comment`). A person drafts; an agent comments at once.

use std::path::{Path, PathBuf};

use atm_common::sha256::sha256_hex;
use atm_common::{Error, Result};
use serde::{Deserialize, Serialize};

use crate::project::Project;
use crate::review_requests::{self, NewComment, NewReply};
use crate::reviews::{self, CommentId, ReviewRef, ReviewStore, Side, Status};

/// `POST …/reviews/<CR>/drafts`: a draft comment, as a new comment is given (its file, lines, side, quote and round),
/// or a draft reply, with `reply_to`.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NewDraft {
    /// The comment it answers: a reply, then, and nothing of a place
    #[serde(default)]
    pub reply_to: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub lines: Option<(u32, u32)>,
    #[serde(default)]
    pub side: Side,
    #[serde(default)]
    pub quote: Option<String>,
    #[serde(default)]
    pub round: Option<u32>,
    pub body: String,
}

/// `PATCH …/reviews/<CR>/drafts/<d>`: a draft's text.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DraftUpdate {
    pub body: String,
}

/// Where a draft goes once sent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Place {
    /// A new chain: on a file's lines (or the whole file), or the review as a whole; `round` none only on a live
    /// review, for what is there when it is sent.
    Comment {
        path: Option<String>,
        lines: Option<(u32, u32)>,
        side: Side,
        quote: Option<String>,
        round: Option<u32>,
    },
    /// A reply to a comment.
    Reply { to: String },
}

/// A draft: `d1`, `d2`, … in its review, never reused there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Draft {
    pub id: String,
    /// When it was written (ISO 8601 UTC), or last changed
    pub at: String,
    pub body: String,
    #[serde(flatten)]
    pub place: Place,
}

impl Draft {
    /// It would start a chain, open once sent: what an approval can't be made with.
    #[must_use]
    pub fn is_top_level(&self) -> bool {
        matches!(self.place, Place::Comment { .. })
    }
}

/// A person's drafts on a review, as kept.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Kept {
    /// The last number given
    next: u32,
    drafts: Vec<Draft>,
}

/// Where a person's drafts are kept: theirs, in atm's state folder ([`Drafts::of_user`]), or a test's.
#[derive(Debug, Clone)]
pub struct Drafts {
    dir: PathBuf,
}

impl Drafts {
    /// The user's own, in atm's state folder.
    #[must_use]
    pub fn of_user() -> Self {
        Self::at(&atm_common::home::state_dir().join("drafts"))
    }

    /// In `dir`.
    #[must_use]
    pub fn at(dir: &Path) -> Self {
        Self {
            dir: dir.to_path_buf(),
        }
    }

    /// The file of `me`'s drafts on the review `id` of the project at `root`.
    fn file(&self, root: Option<&Path>, id: &str, me: &str) -> PathBuf {
        let key = format!(
            "{}\u{0}{id}\u{0}{me}",
            root.map(|r| r.display().to_string()).unwrap_or_default()
        );
        self.dir
            .join(format!("{}.json", &sha256_hex(key.as_bytes())[..16]))
    }

    fn read(file: &Path) -> Result<Kept> {
        match std::fs::read_to_string(file) {
            Ok(text) => serde_json::from_str(&text).map_err(|e| {
                Error::failed(format!(
                    "the drafts in {} can't be read: {e}",
                    file.display()
                ))
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Kept::default()),
            Err(e) => Err(Error::failed(format!(
                "the drafts in {} can't be read: {e}",
                file.display()
            ))),
        }
    }

    /// The drafts in `file` changed by `change` under the drafts folder's lock (ATM-674, audit R11): read, changed and
    /// written by one command at a time, so two at once (two tabs, the dashboard and a terminal) both keep theirs.
    fn changing<T>(file: &Path, change: impl FnOnce(&mut Kept) -> Result<T>) -> Result<T> {
        let dir = file.parent().unwrap_or(file);
        let _lock = atm_db::jsongit::lock::lock(dir, None)?;
        let mut kept = Self::read(file)?;
        change(&mut kept)
    }

    /// Kept even with none left, so the next draft's number is never one given before.
    fn write(file: &Path, kept: &Kept) -> Result<()> {
        let text = serde_json::to_string_pretty(kept)
            .map_err(|e| Error::failed(format!("the drafts can't be written: {e}")))?;
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| Error::failed(format!("{} can't be made: {e}", dir.display())))?;
        }
        atm_common::home::write_private_atomic(file, &text).map_err(|e| {
            Error::failed(format!(
                "the drafts in {} can't be written: {e}",
                file.display()
            ))
        })
    }
}

/// Who drafts: the project's actor, a person.
fn drafter(project: &Project) -> Result<String> {
    let me = project.actor().unwrap_or("someone").to_string();
    if atm_common::edits::is_agent(Some(&me)) {
        return Err(Error::Invalid(
            "a person drafts a review's comments: an agent comments at once (atm review comment)"
                .into(),
        ));
    }
    Ok(me)
}

/// The review and the file of the drafts on it.
fn opened(
    project: &Project,
    review: &ReviewRef,
    drafts: &Drafts,
) -> Result<(reviews::Review, PathBuf)> {
    let me = drafter(project)?;
    let found = reviews::store(project)?.get_review(review)?;
    let file = drafts.file(project.root(), &found.header.id.0, &me);
    Ok((found, file))
}

fn not_ended(review: &reviews::Review, what: &str) -> Result<()> {
    if review.header.status.ended() {
        return Err(Error::Invalid(format!(
            "{} is {}: it can't {what}",
            review.header.reference,
            review.header.status.name()
        )));
    }
    Ok(())
}

/// `review`'s drafts, the oldest first (`atm review drafts`).
///
/// # Errors
/// An agent asking, no such review, or drafts that can't be read.
pub fn list_drafts_in(
    project: &Project,
    review: &ReviewRef,
    drafts: &Drafts,
) -> Result<Vec<Draft>> {
    let (_, file) = opened(project, review, drafts)?;
    Ok(Drafts::read(&file)?.drafts)
}

/// The user's drafts on `review`.
///
/// # Errors
/// As [`list_drafts_in`].
pub fn list_drafts(project: &Project, review: &ReviewRef) -> Result<Vec<Draft>> {
    list_drafts_in(project, review, &Drafts::of_user())
}

/// A new draft on `review` (`atm review draft`).
///
/// # Errors
/// An agent drafting, an empty text, a reply with a place of its own, a comment it answers that the review hasn't, a
/// round it hasn't, an ended review, or drafts that can't be kept.
pub fn add_draft_in(
    project: &Project,
    review: &ReviewRef,
    new: NewDraft,
    drafts: &Drafts,
) -> Result<Draft> {
    let (found, file) = opened(project, review, drafts)?;
    not_ended(&found, "take drafts")?;
    let body = review_requests::checked_text("the draft", &new.body)?.to_string();
    let place = if let Some(to) = new.reply_to {
        if new.path.is_some() || new.lines.is_some() || new.quote.is_some() || new.round.is_some() {
            return Err(Error::Invalid(
                "a reply is placed by the comment it answers: give no file, lines, quote or round"
                    .into(),
            ));
        }
        if !found.comments().any(|c| c.id.0 == to) {
            return Err(Error::NotFound(format!(
                "{} has no comment {to}",
                found.header.reference
            )));
        }
        Place::Reply { to }
    } else {
        let last = found.rounds.last().map_or(1, |r| r.number);
        // On the round it was written on (the latest when none is named), so it stays with that round's lines; on a
        // live review, what is there now is anchored when it is sent
        let now = new.round.is_none_or(|r| r > last);
        let round = if found.header.status == Status::Live && now {
            None
        } else {
            let round = new.round.unwrap_or(last);
            if !found.rounds.iter().any(|r| r.number == round) {
                return Err(Error::Invalid(format!(
                    "{} has no round {round}",
                    found.header.reference
                )));
            }
            Some(round)
        };
        Place::Comment {
            path: new.path,
            lines: new.lines,
            side: new.side,
            quote: new.quote,
            round,
        }
    };
    Drafts::changing(&file, |kept| {
        kept.next += 1;
        let draft = Draft {
            id: format!("d{}", kept.next),
            at: atm_common::time::now_iso(),
            body,
            place,
        };
        kept.drafts.push(draft.clone());
        Drafts::write(&file, kept)?;
        Ok(draft)
    })
}

/// A new draft among the user's.
///
/// # Errors
/// As [`add_draft_in`].
pub fn add_draft(project: &Project, review: &ReviewRef, new: NewDraft) -> Result<Draft> {
    add_draft_in(project, review, new, &Drafts::of_user())
}

/// A draft's text changed (`atm review draft --edit`).
///
/// # Errors
/// An agent asking, no such review or draft, an empty text, or drafts that can't be kept.
pub fn update_draft_in(
    project: &Project,
    review: &ReviewRef,
    id: &str,
    update: &DraftUpdate,
    drafts: &Drafts,
) -> Result<Draft> {
    let (found, file) = opened(project, review, drafts)?;
    let body = review_requests::checked_text("the draft", &update.body)?.to_string();
    Drafts::changing(&file, |kept| {
        let draft = kept
            .drafts
            .iter_mut()
            .find(|d| d.id == id)
            .ok_or_else(|| no_draft(&found, id))?;
        draft.body = body;
        draft.at = atm_common::time::now_iso();
        let changed = draft.clone();
        Drafts::write(&file, kept)?;
        Ok(changed)
    })
}

/// A draft of the user's changed.
///
/// # Errors
/// As [`update_draft_in`].
pub fn update_draft(
    project: &Project,
    review: &ReviewRef,
    id: &str,
    update: &DraftUpdate,
) -> Result<Draft> {
    update_draft_in(project, review, id, update, &Drafts::of_user())
}

/// A draft thrown away (`atm review draft --remove`).
///
/// # Errors
/// An agent asking, no such review or draft, or drafts that can't be kept.
pub fn remove_draft_in(
    project: &Project,
    review: &ReviewRef,
    id: &str,
    drafts: &Drafts,
) -> Result<()> {
    let (found, file) = opened(project, review, drafts)?;
    Drafts::changing(&file, |kept| {
        let before = kept.drafts.len();
        kept.drafts.retain(|d| d.id != id);
        if kept.drafts.len() == before {
            return Err(no_draft(&found, id));
        }
        Drafts::write(&file, kept)
    })
}

/// A draft of the user's thrown away.
///
/// # Errors
/// As [`remove_draft_in`].
pub fn remove_draft(project: &Project, review: &ReviewRef, id: &str) -> Result<()> {
    remove_draft_in(project, review, id, &Drafts::of_user())
}

fn no_draft(review: &reviews::Review, id: &str) -> Error {
    Error::NotFound(format!(
        "you have no draft {id} on {}",
        review.header.reference
    ))
}

/// Every draft on `review` sent, in the order written, each as the comment or reply it is: their IDs. A draft sent
/// is gone from the drafts at once, so one that can't be sent (its comment gone, a refusal) leaves it and those after
/// it to send again, the error naming it.
///
/// # Errors
/// What sending a comment or a reply refuses, naming the draft, or drafts that can't be kept.
pub fn send_drafts_in(
    project: &Project,
    review: &ReviewRef,
    drafts: &Drafts,
) -> Result<Vec<CommentId>> {
    let (_, file) = opened(project, review, drafts)?;
    // Under the lock throughout: two sends at once never send a draft twice
    Drafts::changing(&file, |kept| send_kept(project, review, &file, kept))
}

/// [`send_drafts_in`]'s sending, the drafts locked: each draft removed from the file once sent, before the next.
fn send_kept(
    project: &Project,
    review: &ReviewRef,
    file: &Path,
    kept: &mut Kept,
) -> Result<Vec<CommentId>> {
    let mut sent = Vec::new();
    while let Some(draft) = kept.drafts.first().cloned() {
        let result = match draft.place {
            Place::Comment {
                path,
                lines,
                side,
                quote,
                round,
            } => review_requests::add_comment(
                project,
                review,
                NewComment {
                    path,
                    lines,
                    side,
                    quote,
                    round,
                    body: draft.body,
                },
            ),
            Place::Reply { to } => review_requests::reply_to_comment(
                project,
                review,
                &CommentId(to),
                &NewReply { body: draft.body },
            ),
        };
        let id = result.map_err(|e| {
            Error::failed(format!(
                "your draft {} wasn't sent ({} sent before it): {e}",
                draft.id,
                sent.len()
            ))
        })?;
        sent.push(id);
        kept.drafts.remove(0);
        Drafts::write(file, kept)?;
    }
    Ok(sent)
}

#[cfg(test)]
mod tests;
