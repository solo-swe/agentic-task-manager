//! The review table (ATM-633, docs/task-review-state-machine.md §3.4, §5.4): what each event does to a review, or why
//! it may not, pure, for the commands to check before they write and a front end to ask (§5.2's `can`).

use atm_common::{Error, Result};

use super::{Verdict, decides, is_author, not_now, refused};
use crate::model::{Actor, Review, ReviewHeader, Status};

/// What happens to a review, the events whose rules are the review's own (ATM-633, docs/task-review-state-machine.md
/// §3.4, §5.4). The task table and their composition are atm-lib's (`lifecycle`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewEvent<'a> {
    /// A draft goes to its reviewers.
    AskForReview,
    /// The author's next round.
    Round,
    /// A live review's checkpoint (§3.5).
    Checkpoint,
    /// A reviewer's decision, or a comment on the whole; `overriding` a person's override of who decides, with why;
    /// `also_open` comments open by then besides the review's own (a reviewer's drafts sent with it, ATM-612).
    Submit {
        verdict: Verdict,
        overriding: Option<&'a str>,
        also_open: &'a [String],
    },
    /// Ended without approval.
    Close,
    /// The tasks it names changed.
    SetTasks,
    /// Its summary changed.
    EditSummary,
}

/// What an event does to a review, once its rules let it: its next status (none: it stays), the author it takes (a
/// round's maker, on a live review a person opened), and the override a decision was made under.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Next<'a> {
    pub status: Option<Status>,
    pub author: Option<Actor>,
    pub overridden: Option<&'a str>,
}

/// The review table (ATM-633): what `event` by `by` does to `review`, or why it may not, the refusal's words and kind
/// (its exit code) as each command has always said them. Pure: it reads the review given and writes nothing, so a
/// front end can ask it what a reader may do (§5.2's `can`) as the commands do before writing.
///
/// # Errors
/// The refusal: an ended review, someone who may not, a status the event isn't for, an approval with comments open, an
/// override without its reason.
pub fn review_next<'a>(review: &Review, event: &ReviewEvent<'a>, by: &Actor) -> Result<Next<'a>> {
    let header = &review.header;
    let status = header.status;
    let ended = |what: &str| -> Result<()> {
        if status.ended() {
            Err(not_now(review, what))
        } else {
            Ok(())
        }
    };
    let stays = Next::default();
    let to = |status: Status| Next {
        status: Some(status),
        ..Next::default()
    };
    let author_or_decider = |what: String| -> Result<()> {
        if is_author(header, by) || decides(header, by) {
            Ok(())
        } else {
            Err(refused(what))
        }
    };
    match *event {
        ReviewEvent::AskForReview => {
            if status != Status::Draft {
                return Err(not_now(review, "be asked for again"));
            }
            if !is_author(header, by) {
                return Err(refused(format!(
                    "only {}'s author asks for its review",
                    header.reference
                )));
            }
            Ok(to(Status::Waiting))
        }
        ReviewEvent::Round => {
            ended("get another round")?;
            round_next(review, by)
        }
        ReviewEvent::Checkpoint => {
            if status != Status::Live {
                return Err(not_now(
                    review,
                    "take a checkpoint (only a live review does)",
                ));
            }
            Ok(stays)
        }
        ReviewEvent::Submit {
            verdict,
            overriding,
            also_open,
        } => {
            ended("be submitted on")?;
            submit_next(review, verdict, overriding, also_open, by)
        }
        ReviewEvent::Close => {
            ended("be closed")?;
            author_or_decider(format!(
                "{} is closed by its author or a reviewer",
                header.reference
            ))?;
            Ok(to(Status::Closed))
        }
        ReviewEvent::SetTasks => {
            ended("change its tasks")?;
            author_or_decider(format!(
                "{}'s tasks are changed by its author or a reviewer",
                header.reference
            ))?;
            Ok(stays)
        }
        ReviewEvent::EditSummary => {
            ended("change its summary")?;
            if !is_author(header, by) {
                return Err(refused(format!(
                    "{}'s summary is its author's to change",
                    header.reference
                )));
            }
            Ok(stays)
        }
    }
}

/// A round on a review that isn't ended: its author's (one not known yet takes the round's maker, unless a reviewer);
/// a review sent back or live is waiting on its reviewers again.
pub(super) fn round_next<'a>(review: &Review, by: &Actor) -> Result<Next<'a>> {
    let header = &review.header;
    let author = match &header.author {
        Some(author) if author != by => {
            return Err(refused(format!(
                "only {}'s author adds its rounds",
                header.reference
            )));
        }
        None if header.reviewers.contains(by) => {
            return Err(refused(format!(
                "{}'s rounds are its author's, not a reviewer's",
                header.reference
            )));
        }
        None => Some(by.clone()),
        Some(_) => None,
    };
    let next =
        matches!(header.status, Status::ChangesRequested | Status::Live).then_some(Status::Waiting);
    Ok(Next {
        status: next,
        author,
        ..Next::default()
    })
}

/// A submission on a review that isn't ended: who decides (or a person's override, with why), then what the verdict
/// leads to.
pub(super) fn submit_next<'a>(
    review: &Review,
    verdict: Verdict,
    overriding: Option<&'a str>,
    also_open: &[String],
    by: &Actor,
) -> Result<Next<'a>> {
    let header = &review.header;
    let status = header.status;
    let overridden = match overriding {
        Some(reason) if reason.trim().is_empty() => {
            return Err(Error::Invalid(
                "overriding the review's rules needs the reason why".into(),
            ));
        }
        // One who may decide anyway: nothing to override
        _ if decides(header, by) => None,
        Some(reason) => Some(reason),
        None => {
            return Err(refused(format!(
                "{} is decided by its reviewers{}; a person may override that, saying why",
                header.reference,
                if header.reviewers.is_empty() {
                    ", not its author"
                } else {
                    ""
                }
            )));
        }
    };
    let next = match verdict {
        // Sent back too (ATM-609): a reviewer may change their mind, or settle what they asked for themselves
        Verdict::Approve => {
            if !status.in_play() {
                return Err(not_now(review, "be approved"));
            }
            let open: Vec<String> = review
                .open_comments()
                .map(|c| c.id.0.clone())
                .chain(also_open.iter().cloned())
                .collect();
            if !open.is_empty() {
                return Err(Error::failed(format!(
                    "{} can't be approved while comments are open: {}",
                    header.reference,
                    open.join(", ")
                )));
            }
            Some(Status::Approved)
        }
        // Sent back already, it isn't sent back again (CR-2): a reviewer's further words are a comment, which the page
        // calls *Publish comments*
        Verdict::RequestChanges => {
            if status != Status::Waiting {
                return Err(not_now(review, "be sent back"));
            }
            Some(Status::ChangesRequested)
        }
        Verdict::Comment => None,
    };
    Ok(Next {
        status: next,
        overridden,
        ..Next::default()
    })
}

/// Why `who` may not decide `header`'s review, said in the record of an override.
pub(super) fn not_a_decider(header: &ReviewHeader, who: &Actor) -> &'static str {
    if is_author(header, who) {
        "its author"
    } else {
        "not one of its reviewers"
    }
}
