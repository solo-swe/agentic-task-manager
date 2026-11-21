//! A review's page, and a live review's working copy as the round after the stored ones (ATM-576).

use super::{
    Error, FileStore, Patch, Project, Result, ReviewFilter, ReviewId, ReviewRef, ReviewStore,
    RoundDiffs, Status, Value, drafts_of, json, list_everywhere, me, needs, reviews, reviews_stamp,
    rounds, shown_with,
};

/// A store's rounds, and the working copy now as the round after them, not stored (ATM-576)
pub(super) struct WithNow<'a> {
    store: &'a FileStore,
    number: u32,
    patch: Patch,
}

impl RoundDiffs for WithNow<'_> {
    fn diff_of(&self, id: &ReviewId, round: u32) -> Result<Patch> {
        if round == self.number {
            Ok(self.patch.clone())
        } else {
            self.store.round_diff(id, round)
        }
    }
}

/// What a live review's page of its working copy now (`round` none) is made from, cheap to read (ATM-607): the
/// reviews' stamp, the working copy's state (`git status`, the changed files' sizes and times) and who asks. The same
/// while the page would be; a browser's poll gets a 304 from it without a diff being made. None for any other page,
/// which isn't tagged.
///
/// # Errors
/// No such review; the project isn't in git.
pub fn live_page_stamp(
    project: &Project,
    reference: &ReviewRef,
    round: Option<u32>,
    against: Option<u32>,
) -> Result<Option<Value>> {
    if round.is_some() {
        return Ok(None);
    }
    let review = reviews::store(project)?.get_review(reference)?;
    if review.header.status != Status::Live {
        return Ok(None);
    }
    let Some(state) = reviews::vcs(project)?.working_state() else {
        return Ok(None);
    };
    Ok(Some(json!([
        "live",
        review.header.id.0,
        against,
        reviews_stamp(project),
        state,
        project.actor(),
        // The user's drafts, which the page shows (ATM-612)
        serde_json::to_value(drafts_of(project, &review)).unwrap_or(Value::Null),
    ])))
}

/// A review's page for one view (`round`, the latest when none; `against`, an earlier round): `{review, summary,
/// branch, rounds, chains, view: {round, against, now, files}, needs_you, you}`. A live review shown with no round
/// is its working copy now (`now`, §3.5, ATM-576): when it changed since the last checkpoint, as the round after the
/// stored ones (`rounds` lists only those), which a comment on it makes a checkpoint.
///
/// # Errors
/// No such review, round, or the round against itself; the project isn't in git; a round's tree can't be had here.
pub fn review_page(
    project: &Project,
    reference: &ReviewRef,
    round: Option<u32>,
    against: Option<u32>,
) -> Result<Value> {
    let store = reviews::store(project)?;
    let mut review = store.get_review(reference)?;
    let stored = review.rounds.clone();
    let latest = review
        .rounds
        .last()
        .map(|r| r.number)
        .ok_or_else(|| Error::NotFound(format!("{} has no rounds", review.header.reference)))?;
    let vcs = reviews::vcs(project)?;
    // The working copy: asked with no round, or by the number the page shows it as, the round after the stored ones
    // (ATM-710: compared against an earlier round, the page names it, and it isn't stored)
    let now = review.header.status == Status::Live && round.is_none_or(|r| r == latest + 1);
    let working = if now {
        reviews::working_now(project, &review)?
    } else {
        None
    };
    // Unchanged since the last checkpoint, there is no round after it: that one is the working copy
    let round = round.filter(|r| working.is_some() || *r != latest + 1 || !now);
    let drafts = drafts_of(project, &review);
    // The reader's drafts sent with a decision count as open, as `submit_decision_with` counts them
    let open_drafts: Vec<String> = drafts
        .iter()
        .filter(|d| d.is_top_level())
        .map(|d| format!("{} (your draft)", d.id))
        .collect();
    let (mut view, chains, drafts) = match working {
        Some(new) => {
            let number = latest + 1;
            let (unstored, patch) = rounds::unstored(number, new, atm_common::time::now_iso());
            review.rounds.push(unstored);
            let diffs = WithNow {
                store: &store,
                number,
                patch,
            };
            shown_with(&vcs, &diffs, &review, number, against, &drafts)?
        }
        None => shown_with(
            &vcs,
            &store,
            &review,
            round.unwrap_or(latest),
            against,
            &drafts,
        )?,
    };
    if let Some(map) = view.as_object_mut() {
        map.insert("now".into(), json!(now));
    }
    let branch = project
        .root()
        .and_then(|root| list_everywhere(root, &ReviewFilter::default()).ok())
        .and_then(|all| all.into_iter().find(|f| f.header.id == review.header.id))
        .and_then(|f| f.branch);
    let me = me(project);
    // What this reader may do, and why not (ATM-633, docs/task-review-state-machine.md §5.2): the machine's answer,
    // so the page shows a button for what the server allows and never repeats the rules
    let can: serde_json::Map<String, Value> =
        crate::lifecycle::can(project, &review.header.reference, &me, &open_drafts)?
            .into_iter()
            .map(|(action, answer)| {
                let value = match answer {
                    Ok(()) => json!({"ok": true}),
                    Err(why) => json!({"ok": false, "why": why.to_string()}),
                };
                (action.to_string(), value)
            })
            .collect();
    Ok(json!({
        "review": review.header,
        "summary": review.summary,
        "branch": branch,
        "rounds": stored,
        "chains": chains,
        "drafts": drafts,
        "view": view,
        "needs_you": needs(&review.header, &me),
        "you": me.0,
        "can": can,
    }))
}
