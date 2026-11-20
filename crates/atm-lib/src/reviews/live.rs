//! Live reviews (§3.5): the working copy as a round, checkpoints, and an approval that covers a task.

use super::{
    Command, Error, Kind, NewRound, Options, Project, Result, Review, ReviewFilter, ReviewHeader,
    ReviewRef, ReviewStore, Reviews, RoundKind, Status, TASK_DIR, actor_of, change_task_status,
    key_of_id, kind_of, naming, rounds, store, vcs,
};

/// Whether an approved live review covers a task's change now (§3.5): the review named the task, and the working
/// tree's diff against the review's base is the approved checkpoint's. The review, when one does.
///
/// # Errors
/// The reviews can't be read, or git fails.
pub fn live_approval_for(project: &Project, task: &str) -> Result<Option<ReviewHeader>> {
    let id = project.resolve_task(task)?;
    let key = key_of_id(project, &id);
    let Some(vcs) = project.vcs() else {
        return Ok(None);
    };
    let reviews = store(project)?;
    // Every approved review, then those naming the task by any key it answers to (ATM-630): the store compares keys
    // as written, and a review written before the task's number moved names the old one
    let filter = ReviewFilter {
        statuses: vec![Status::Approved],
        task: None,
    };
    for header in naming(
        project,
        &reviews,
        reviews.list_reviews(&filter)?,
        Some(&key),
    ) {
        let review = reviews.get_review(&header.reference)?;
        let Some(last) = review.rounds.last() else {
            continue;
        };
        let RoundKind::Working { base } = &last.kind else {
            continue;
        };
        let approved = reviews.round_diff(&header.id, last.number)?;
        let now = rounds::from_working(&vcs, base, &actor_of(project))?;
        if now.patch == approved {
            return Ok(Some(header));
        }
    }
    Ok(None)
}

/// A live review's working copy now (§3.5, ATM-576): the working tree against its base (its last checkpoint's), as a
/// round, when it differs from that checkpoint. None when it doesn't, or the review isn't live.
///
/// # Errors
/// The project isn't in git, the base is gone from this clone, or the store's.
pub fn working_now(project: &Project, review: &Review) -> Result<Option<NewRound>> {
    if review.header.status != Status::Live {
        return Ok(None);
    }
    let Some(last) = review.rounds.last() else {
        return Ok(None);
    };
    let RoundKind::Working { base } = &last.kind else {
        return Ok(None);
    };
    let vcs = rounds::repository(&vcs(project)?, last)?;
    let now = rounds::from_working(&vcs, base, &actor_of(project))?;
    if now.patch == store(project)?.round_diff(&review.header.id, last.number)? {
        return Ok(None);
    }
    Ok(Some(now))
}

/// Before a person's comment or approval on a live review (§3.5, ATM-576): the working copy as it is now made a
/// checkpoint, so what they saw is what it is anchored to and what is approved. The round to anchor to: the new
/// checkpoint, else the last round (nothing changed since it, or the review isn't live).
///
/// # Errors
/// No such review, the working copy can't be read, or the review's rules refuse the checkpoint.
pub fn checkpoint_now(project: &Project, reference: &ReviewRef) -> Result<u32> {
    let mut reviews = store(project)?;
    let review = reviews.get_review(reference)?;
    let last = review.rounds.last().map_or(1, |r| r.number);
    match working_now(project, &review)? {
        Some(now) => Reviews::new(&mut reviews).checkpoint(reference, now),
        None => Ok(last),
    }
}

/// Be told when the project's working copy changes in a way a live review shows (ATM-607), until the subscription is
/// dropped: a settled burst of changes in its folder that touches a file git doesn't ignore, outside `.git/` and atm's
/// task folder (never part of a review). `on_change` runs on the watcher's thread.
///
/// # Errors
/// The project isn't in git (reviews are of changes in git), or its folder can't be watched.
pub fn subscribe_working_copy(
    project: &Project,
    on_change: atm_db::subscription::OnChange,
) -> Result<atm_db::subscription::Subscription> {
    let vcs = vcs(project)?;
    let root = project
        .root()
        .ok_or_else(|| Error::failed("this project has no folder to watch"))?;
    let root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let inside = root.clone();
    let keep: atm_db::subscription::Keep =
        std::sync::Arc::new(move |paths: &[std::path::PathBuf]| {
            let relative: Vec<String> = paths
                .iter()
                .filter_map(|path| {
                    let rel = path.strip_prefix(&inside).ok()?;
                    let first = rel.components().next()?.as_os_str();
                    (first != ".git" && first != TASK_DIR)
                        .then(|| rel.to_string_lossy().replace('\\', "/"))
                })
                .collect();
            vcs.any_not_ignored(&relative)
        });
    atm_db::subscription::watch_tree_where(&[root], keep, on_change)
}

/// A task in progress whose change an approved live review covers: put in review and approved, its note naming the
/// review, so it can be done with no other review (§3.5). None when no live review covers it as it is now.
///
/// # Errors
/// The reviews can't be read, git fails, or the task can't change status.
pub fn apply_live_approval(project: &mut Project, task: &str) -> Result<Option<ReviewHeader>> {
    let Some(header) = live_approval_for(project, task)? else {
        return Ok(None);
    };
    let id = project.resolve_task(task)?;
    if kind_of(project, &id)?.is_some_and(|(kind, _)| kind == Kind::Active) {
        let note = format!("approved in the live review {}", header.reference);
        let options = Options {
            note: Some(note),
            ..Options::default()
        };
        change_task_status(project, Command::Review, &id, &options)?;
        change_task_status(project, Command::Approve, &id, &options)?;
    }
    Ok(Some(header))
}
