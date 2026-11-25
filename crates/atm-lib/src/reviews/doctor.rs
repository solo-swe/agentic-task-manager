//! `atm doctor`'s checks of the reviews and the tasks they name, and what a task's move to review asks of them.

use atm_common::Vocabulary;

use super::{
    FileStore, Kind, Project, ReviewFilter, ReviewHeader, ReviewStore, Status, Value, id_of,
    kind_of, names_task, store, task_reference,
};

/// The states of a review a task waits on (ATM-770): waiting, live, or sent back (a draft puts no task in review)
const AWAITED: [Status; 3] = [Status::Waiting, Status::Live, Status::ChangesRequested];

/// The references of the reviews a task waits on (ATM-770): those waiting, live or sent back that name task `id` (as
/// stored), by whatever key they were written with. None when the project has no folder to keep reviews in (nothing
/// to say either way).
///
/// # Errors
/// The reviews can't be read.
pub fn awaited_reviews(project: &Project, id: &str) -> atm_common::Result<Option<Vec<String>>> {
    if project.root().is_none() {
        return Ok(None);
    }
    let reviews = store(project)?;
    let key = task_reference(project, id);
    Ok(Some(
        reviews
            .list_reviews(&ReviewFilter::default())?
            .into_iter()
            .filter(|h| AWAITED.contains(&h.status) && names_task(project, Some(&reviews), h, &key))
            .map(|h| h.reference.0)
            .collect(),
    ))
}

/// The tasks stranded in review (ATM-770): in review, their review not decided, and no review they wait on (`headers`
/// of `reviews`), so nothing in the dashboard decides them (an agent asked for one without a code review, before atm
/// refused that). Those in `warned` are named already.
fn stranded(
    project: &Project,
    document: &Value,
    reviews: &FileStore,
    headers: &[ReviewHeader],
    warned: &std::collections::HashSet<String>,
) -> Vec<String> {
    let Ok(vocab) = Vocabulary::of_document(document) else {
        return Vec::new();
    };
    let undecided = |task: &Value| {
        !matches!(
            task.get("review_state").and_then(Value::as_str),
            Some("approved" | "changes_requested" | "rejected")
        )
    };
    let mut found = Vec::new();
    for task in document["tasks"].as_array().into_iter().flatten() {
        let Some(id) = id_of(task) else { continue };
        if vocab.kind_of(task.get("status")) != Some(Kind::Review)
            || !undecided(task)
            || warned.contains(id)
        {
            continue;
        }
        let key = task_reference(project, id);
        let awaited = headers
            .iter()
            .any(|h| AWAITED.contains(&h.status) && names_task(project, Some(reviews), h, &key));
        if !awaited {
            found.push(format!(
                "{key}: in review with no code review waiting on it, so the dashboard can't decide it: \
                 `atm review create --task {key} …` opens one, or `atm task review approve|request-changes|reject {key}` decides it"
            ));
        }
    }
    found
}

/// `atm doctor`'s checks of the reviews (docs/code-review-design.md §4): errors, a review atm can't read; warnings,
/// lines of `comments.md` the comment format doesn't expect, numbers two reviews share (CR-8.1 and CR-8.2), and tasks
/// linked to reviews the project doesn't have. A project without reviews has nothing to say.
#[must_use]
pub fn check(project: &Project, document: &Value) -> (Vec<String>, Vec<String>) {
    let (mut errors, mut warnings) = (Vec::new(), Vec::new());
    let Ok(store) = store(project) else {
        return (errors, warnings);
    };
    match store.problems() {
        Ok(problems) => {
            for problem in problems {
                if problem.contains("comments.md line ") {
                    warnings.push(format!("review {problem}"));
                } else {
                    errors.push(format!("review {problem}"));
                }
            }
        }
        Err(e) => errors.push(format!("reviews: {e}")),
    }
    let headers = store
        .list_reviews(&ReviewFilter::default())
        .unwrap_or_default();
    let mut shared: Vec<u32> = headers
        .iter()
        .filter(|h| h.reference.0.contains('.'))
        .map(|h| h.number)
        .collect();
    shared.dedup();
    for number in shared {
        let refs: Vec<&str> = headers
            .iter()
            .filter(|h| h.number == number)
            .map(|h| h.reference.0.as_str())
            .collect();
        warnings.push(format!(
            "reviews {} share the number {number} (made in two clones; both are kept, by those names)",
            refs.join(" and ")
        ));
    }
    // A decision that didn't reach a task still waiting on it (an atm before ATM-594 named a provisional key with its
    // `~`): the review's decision, the user's, is recorded on the task with the command named (I23: never by atm). A
    // task follows its latest review (§3.8): the reviews are listed newest first, and only a task's first counts
    let mut followed = std::collections::HashSet::new();
    // The tasks warned of here: the stranded-task warning below doesn't name them again
    let mut warned = std::collections::HashSet::new();
    for header in &headers {
        let latest: Vec<&String> = header
            .tasks
            .iter()
            .filter(|k| followed.insert((*k).clone()))
            .collect();
        let (state, command) = match header.status {
            Status::Approved => ("approved", "approve"),
            Status::ChangesRequested => ("changes_requested", "request-changes"),
            _ => continue,
        };
        for key in latest {
            let Ok(id) = project.resolve_task(key) else {
                warnings.push(format!(
                    "review {}: it names {key}, which isn't one of the project's tasks",
                    header.reference
                ));
                continue;
            };
            if let Ok(Some((Kind::Review, now))) = kind_of(project, &id)
                && now.as_deref() != Some(state)
                && now.as_deref() != Some("approved")
            {
                let reason = if command == "approve" {
                    String::new()
                } else {
                    " --reason '…'".to_string()
                };
                warnings.push(format!(
                    "{key}: {} is {} but the task still waits for its review: `atm task review {command} {key}{reason}` records the decision",
                    header.reference,
                    header.status.name().replace('_', " ")
                ));
                warned.insert(id);
            }
        }
    }
    warnings.extend(stranded(project, document, &store, &headers, &warned));
    let known: std::collections::HashSet<&str> = headers.iter().map(|h| h.id.0.as_str()).collect();
    for task in document["tasks"].as_array().into_iter().flatten() {
        for link in task
            .get("code_reviews")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let id = link.get("id").and_then(Value::as_str).unwrap_or_default();
            if !known.contains(id) {
                let name = link.get("ref").and_then(Value::as_str).unwrap_or(id);
                warnings.push(format!(
                    "{}: its code review {name} isn't in the project (not merged here yet, or deleted)",
                    id_of(task).unwrap_or("?")
                ));
            }
        }
    }
    (errors, warnings)
}
