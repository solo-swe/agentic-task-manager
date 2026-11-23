//! How a review names its tasks: by the key each had, found now by whatever key it has (ATM-630).

use super::{
    FileStore, Found, Project, Result, ReviewFilter, ReviewHeader, TASK_DIR, list_everywhere,
    review_keys,
};

/// How the review `header` (in `reviews`, when known) names the task it means by `key`, by its key now
/// ([`review_keys::task_of`]); a key the project doesn't have is kept, without a leading `~`.
#[must_use]
pub fn key_now(
    project: &Project,
    reviews: Option<&FileStore>,
    header: &ReviewHeader,
    key: &str,
) -> String {
    review_keys::task_of(project, reviews, header, key).map_or_else(
        |_| atm_common::keys::without_draft_mark(key).to_string(),
        |id| key_of_id(project, &id),
    )
}

/// The task a filter names (`--task`, `?task=`: a key with or without its `~`, an older ID, the UUID), as reviews name
/// it: its key (ATM-594). One the project doesn't have is kept as given, without a leading `~` (it fits no review).
#[must_use]
pub fn task_reference(project: &Project, reference: &str) -> String {
    project.resolve_task(reference).map_or_else(
        |_| atm_common::keys::without_draft_mark(reference).to_string(),
        |id| key_of_id(project, &id),
    )
}

/// Whether a review names the task `wanted` (its key now, as [`task_reference`] gives it), by whatever key the review
/// was written with (ATM-630): a task whose number moved at a merge (ADR-009) is named by the key it had, its alias
/// now, and an older review by `~KEY` or an earlier prefix's key. Each is resolved as every command resolves one,
/// except an old key another task holds now: the review in `reviews` says which it meant ([`key_now`]).
#[must_use]
pub fn names_task(
    project: &Project,
    reviews: Option<&FileStore>,
    header: &ReviewHeader,
    wanted: &str,
) -> bool {
    header
        .tasks
        .iter()
        .any(|key| key_now(project, reviews, header, key) == wanted)
}

/// The reviews among `headers` (in `reviews`) that name the task `wanted` (a key now), whatever key they were written
/// with
pub(super) fn naming(
    project: &Project,
    reviews: &FileStore,
    headers: Vec<ReviewHeader>,
    wanted: Option<&str>,
) -> Vec<ReviewHeader> {
    match wanted {
        None => headers,
        Some(key) => headers
            .into_iter()
            .filter(|h| names_task(project, Some(reviews), h, key))
            .collect(),
    }
}

/// The reviews of every working copy of the project at `root` (`list_everywhere`) that `filter` keeps, its task (a key
/// now, as [`task_reference`] gives it) found by whatever key each review was written with (ATM-630), not as written.
///
/// # Errors
/// The reviews can't be read.
pub fn list_naming(
    project: &Project,
    root: &std::path::Path,
    filter: &ReviewFilter,
) -> Result<Vec<Found>> {
    let wanted = filter.task.clone();
    let every = ReviewFilter {
        task: None,
        ..filter.clone()
    };
    Ok(list_everywhere(root, &every)?
        .into_iter()
        .filter(|f| {
            wanted.as_deref().is_none_or(|key| {
                // Read from the working copy it was found in: a review in another worktree is that copy's
                let reviews = FileStore::open(&f.working_copy.join(TASK_DIR)).ok();
                names_task(project, reviews.as_ref(), &f.header, key)
            })
        })
        .collect())
}

/// How a review names the task `id`: its key, never with the `~` people read (ATM-594), else its ID.
pub(super) fn key_of_id(project: &Project, id: &str) -> String {
    project
        .store()
        .get_task(id)
        .ok()
        .flatten()
        .map_or_else(|| id.to_string(), |t| project.task_key(t))
}
