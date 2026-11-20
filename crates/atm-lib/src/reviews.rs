//! Code reviews and tasks (docs/code-review-design.md §3.1's link, §3.8, ATM-565): the one place the two meet. A
//! review names its tasks (by key), each task links its reviews (`code_reviews`, by UUID and reference), and a
//! reviewer's decision on a review is every named task's review decision, recorded as the reviewer (I23: the user's,
//! never an agent's). A live review's approval is a task's when the task's change hasn't moved since (§3.5).
//!
//! The front ends call these, never `atm-review`'s store directly, so the tasks always follow.

use atm_common::edits::is_agent;
use atm_common::{Error, Kind, Result, TASK_DIR, Task, Vocabulary};
use serde_json::{Value, json};

pub use atm_review::files::comments_md;
pub use atm_review::folder::{FileStore, Found, list_everywhere};
pub use atm_review::rounds;
pub use atm_review::{
    Side,
    model::{FileStatus, Round},
};

/// The project's git, for a review's rounds.
///
/// # Errors
/// The project isn't in a git repository: reviews are of changes in git.
pub fn vcs(project: &Project) -> Result<atm_vcs::GitVcs> {
    project
        .vcs()
        .filter(atm_vcs::GitVcs::in_repository)
        .ok_or_else(|| {
            Error::failed("reviews are of changes in git: this project isn't in a git repository")
        })
}
/// The repository a round made in `cwd` is of (ATM-583): in a submodule, the submodule's (its revisions and
/// uncommitted changes are its own), its files named as the project has them; else the project's (`vcs`).
///
/// # Errors
/// `cwd` is in a submodule nested in another one: rounds are of the project's repository or a submodule of it.
pub fn repository_here(vcs: &atm_vcs::GitVcs, cwd: &std::path::Path) -> Result<atm_vcs::GitVcs> {
    match vcs.submodule_containing(cwd) {
        None => Ok(vcs.clone()),
        Some(folder) => vcs.submodule_at(&folder).ok_or_else(|| {
            Error::Invalid(format!(
                "{folder} is in a submodule inside another one: review from the outer submodule's folder, or the project's"
            ))
        }),
    }
}

/// The repository files reviewed whole are read from (ATM-583): a submodule's when every file is in it, else the
/// project's (`vcs`).
///
/// # Errors
/// `Invalid` when the files are in different repositories: each repository's are a review of their own.
pub fn repository_of_files(vcs: &atm_vcs::GitVcs, paths: &[String]) -> Result<atm_vcs::GitVcs> {
    let mut found: Option<Option<String>> = None;
    for path in paths {
        // The outermost folder on the way that is a submodule, if any
        let parts: Vec<&str> = path.split('/').collect();
        let folder = (1..parts.len())
            .map(|n| parts[..n].join("/"))
            .find(|f| vcs.submodule_at(f).is_some());
        match &found {
            None => found = Some(folder),
            Some(first) if *first == folder => {}
            Some(_) => {
                return Err(Error::Invalid(
                    "these files are in different repositories (the project's and a submodule's): review each repository's files in a review of their own".into(),
                ));
            }
        }
    }
    match found.flatten() {
        None => Ok(vcs.clone()),
        Some(folder) => vcs
            .submodule_at(&folder)
            .ok_or_else(|| Error::failed(format!("{folder}: not a submodule here"))),
    }
}

pub use atm_review::rules::Verdict;
pub use atm_review::{
    Actor, Anchor, Chain, Comment, CommentFilter, CommentId, NewReview, NewRound, Patch, Review,
    ReviewFilter, ReviewHeader, ReviewId, ReviewRef, ReviewStore, Reviews, RoundKind, Status,
};

use crate::changes::change_task_status;
use crate::project::Project;
use crate::review_keys;
use crate::state::{Command, Options};

/// The project's reviews, in its task folder.
///
/// # Errors
/// The project wasn't opened from a folder, or its reviews' folder is a symlink.
pub fn store(project: &Project) -> Result<FileStore> {
    let root = project
        .root()
        .ok_or_else(|| Error::failed("this project has no folder to keep reviews in"))?;
    FileStore::open(&root.join(TASK_DIR))
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
            }
        }
    }
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

fn actor_of(project: &Project) -> Actor {
    Actor(project.actor().unwrap_or("someone").to_string())
}

fn id_of(task: &Value) -> Option<&str> {
    task.get("id").and_then(Value::as_str)
}

/// A task's kind and review state now, none when it is gone.
fn kind_of(project: &Project, id: &str) -> Result<Option<(Kind, Option<String>)>> {
    let vocab: &Vocabulary = project.store().vocabulary()?;
    Ok(project.store().get_task(id)?.map(|task: &Task| {
        (
            vocab.kind_of(task.get("status")).unwrap_or(Kind::Todo),
            task.get("review_state")
                .and_then(Value::as_str)
                .map(String::from),
        )
    }))
}

/// Add (or, with `add` false, remove) the link to `header`'s review on the tasks `ids`.
/// The agent holding a claim in this working copy (a live review's author, §3.5), if one does.
fn agent_at_work(project: &Project) -> Option<Actor> {
    let root = project.root()?;
    let here = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    crate::query::list_claims(project)
        .into_iter()
        .find_map(|claim| {
            let worktree = claim.get("worktree").and_then(Value::as_str)?;
            let there = std::fs::canonicalize(worktree).unwrap_or_else(|_| worktree.into());
            let agent = claim.get("agent").and_then(Value::as_str)?;
            (there == here && is_agent(Some(agent))).then(|| Actor(agent.to_string()))
        })
}

/// The agent that worked the first of these tasks (who started it), if an agent did.
fn agent_of_tasks(project: &Project, ids: &[String]) -> Option<Actor> {
    ids.iter().find_map(|id| {
        let task = project.store().get_task(id).ok().flatten()?;
        let agent = task.get("started_by").and_then(Value::as_str)?;
        is_agent(Some(agent)).then(|| Actor(agent.to_string()))
    })
}

fn link(project: &mut Project, header: &ReviewHeader, ids: &[String], add: bool) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    let entry = json!({ "id": header.id.0, "ref": header.reference.0 });
    project.store_mut().change(&mut |work| {
        for task in work.tasks().iter_mut() {
            if !id_of(task).is_some_and(|id| ids.iter().any(|i| i == id)) {
                continue;
            }
            let Some(task) = task.as_object_mut() else {
                continue;
            };
            let mut links: Vec<Value> = task
                .get("code_reviews")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            links.retain(|l| l.get("id") != entry.get("id"));
            if add {
                links.push(entry.clone());
            }
            if links.is_empty() {
                task.shift_remove("code_reviews");
            } else {
                task.insert("code_reviews".into(), Value::Array(links));
            }
        }
        Ok(())
    })
}

/// The task IDs of `keys` (keys, old IDs or UUIDs).
fn resolve(project: &Project, keys: &[String]) -> Result<Vec<String>> {
    keys.iter().map(|k| project.resolve_task(k)).collect()
}

/// The task IDs a review in `reviews` names: each key as the review meant it (ATM-630, [`review_keys::task_of`]), so
/// a decision reaches the task it was about even when a merge gave its old number to another.
fn named(project: &Project, reviews: &FileStore, header: &ReviewHeader) -> Result<Vec<String>> {
    header
        .tasks
        .iter()
        .map(|key| review_keys::task_of(project, Some(reviews), header, key))
        .collect()
}

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
fn naming(
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
fn key_of_id(project: &Project, id: &str) -> String {
    project
        .store()
        .get_task(id)
        .ok()
        .flatten()
        .map_or_else(|| id.to_string(), |t| project.task_key(t))
}

/// Put the in-progress tasks among `ids` in review, with the review's link in their note (§3.8).
fn ask_review(
    project: &mut Project,
    ids: &[String],
    header: &ReviewHeader,
    what: &str,
) -> Result<()> {
    for id in ids {
        if kind_of(project, id)?.is_some_and(|(kind, _)| kind == Kind::Active) {
            let options = Options {
                note: Some(format!("{}: {what}", header.reference)),
                ..Options::default()
            };
            change_task_status(project, Command::Review, id, &options)?;
        }
    }
    Ok(())
}

/// What opening a review that waits on its reviewers does to one of its tasks (ATM-631).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Opening {
    /// In progress, held by the author or by no one: into review.
    Review,
    /// The author's own claim lapsed (the task back in to do, or about to be): started again, then into review.
    TakeBack,
    /// Already in review, with no other review waiting on it: linked, as it is.
    Link,
}

use crate::leases::holder;

/// The open review, other than a draft or a live one, that already carries `task`'s review, if one does.
fn waiting_review(project: &Project, task: &Task) -> Result<Option<String>> {
    let links = task
        .get("code_reviews")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let reviews = store(project)?;
    for reference in links
        .iter()
        .filter_map(|l| l.get("ref").and_then(Value::as_str))
    {
        let Ok(found) = reviews.get_review(&ReviewRef(reference.to_string())) else {
            continue;
        };
        if matches!(
            found.header.status,
            Status::Waiting | Status::ChangesRequested
        ) {
            return Ok(Some(reference.to_string()));
        }
    }
    Ok(None)
}

/// What opening a review waiting on its reviewers does to each of `ids`, decided before anything is written, so the
/// review is never kept beside a task that didn't follow it (ATM-631). Exit code 4 when another agent holds a task.
///
/// # Errors
/// A task another agent holds, one blocked, done or skipped, one in to do that the author didn't start, or one
/// another review already waits on: each says what to do instead.
fn openings(project: &Project, ids: &[String]) -> Result<Vec<Opening>> {
    let me = project.actor().map(String::from);
    let now = atm_common::time::now();
    let claims = crate::query::list_claims(project);
    let vocab: &Vocabulary = project.store().vocabulary()?;
    let mut planned = Vec::with_capacity(ids.len());
    for id in ids {
        let Some(task) = project.store().get_task(id)? else {
            return Err(Error::NotFound(format!("no task {id}")));
        };
        let key = project.task_key(task);
        let claim = claims
            .iter()
            .find(|c| c.get("task").and_then(Value::as_str) == Some(id.as_str()));
        let held_by = claim.and_then(|c| holder(c, now));
        if let Some(other) = held_by.filter(|agent| Some(*agent) != me.as_deref()) {
            return Err(Error::Conflict(format!(
                "can't open a review of {key}: it is claimed by {other}, who opens its review"
            )));
        }
        // The author's own claim, run out but not yet dropped: the task is about to go back to to do
        let own_lapsed = claim.is_some_and(|c| {
            held_by.is_none() && c.get("agent").and_then(Value::as_str) == me.as_deref()
        });
        let started_by_me = task.get("started_by").and_then(Value::as_str) == me.as_deref();
        let kind = vocab.kind_of(task.get("status")).unwrap_or(Kind::Todo);
        planned.push(match kind {
            Kind::Active if own_lapsed => Opening::TakeBack,
            Kind::Active => Opening::Review,
            Kind::Todo if started_by_me && me.is_some() => Opening::TakeBack,
            Kind::Todo => {
                return Err(Error::failed(format!(
                    "can't open a review of {key}: it is to do, and you didn't start it. Start it first \
                     (atm task state {key} in_progress), then open its review"
                )));
            }
            Kind::Review => match waiting_review(project, task)? {
                Some(other) => {
                    return Err(Error::failed(format!(
                        "can't open a review of {key}: {other} already waits on it. Add a round to it instead \
                         (atm review round {other})"
                    )));
                }
                None => Opening::Link,
            },
            Kind::Blocked => {
                return Err(Error::failed(format!(
                    "can't open a review of {key}: it is blocked. Its question is answered first \
                     (atm task state {key} todo --answer '…'), then it is started again"
                )));
            }
            Kind::Done | Kind::Skipped => {
                return Err(Error::failed(format!(
                    "can't open a review of {key}: it is {}. Reopen it (atm task state {key} todo) to review new \
                     work on it",
                    if kind == Kind::Done { "done" } else { "skipped" }
                )));
            }
        });
    }
    Ok(planned)
}

/// Open a review, link its tasks, and put them in review (one waiting on its reviewers; a draft or a live one leaves
/// them as they are). An agent's review waiting on its reviewers has every task checked first ([`openings`]): the
/// command either opens the review with all its tasks in review, or refuses and writes nothing (ATM-631). A task whose
/// own claim lapsed while the agent worked (the checks ran long) is started again first, and its note says so. A
/// person's review puts the tasks in progress in review and links the others as they are (a review of files for a
/// task an agent hasn't started yet).
///
/// # Errors
/// What the review's rules refuse (`atm_review::rules`), an unknown task, a task [`openings`] refuses, or a store
/// error.
pub fn create(project: &mut Project, mut new: NewReview) -> Result<ReviewHeader> {
    let ids = resolve(project, &new.tasks)?;
    // An agent opening a review of its work: checked first (ATM-631). A person may open one of files for a task not
    // started yet, for an agent to answer when it takes the task (the sandbox's red-team review): it is linked, as it is
    let by_agent = is_agent(project.actor());
    let taken_back: Vec<String> = if new.status == Status::Waiting && by_agent {
        let planned = openings(project, &ids)?;
        ids.iter()
            .zip(planned)
            .filter(|(_, opening)| *opening == Opening::TakeBack)
            .map(|(id, _)| id.clone())
            .collect()
    } else {
        Vec::new()
    };
    for id in &taken_back {
        change_task_status(project, Command::Start, id, &Options::default())?;
    }
    // A live review a person opens is of an agent's work (§3.5): the person reviews it; the change is the agent's at
    // work in this copy, else its tasks', else nobody's yet (an agent's first round, or a task linked later, names it)
    if new.status == Status::Live
        && let Some(opener) = new.author.clone()
        && !is_agent(Some(&opener.0))
    {
        if !new.reviewers.contains(&opener) {
            new.reviewers.push(opener);
        }
        new.author = agent_at_work(project).or_else(|| agent_of_tasks(project, &ids));
    }
    // The review names its tasks by key, without the `~` of a provisional one (ATM-594): a reference atm resolves
    new.tasks = ids.iter().map(|id| key_of_id(project, id)).collect();
    let mut reviews = store(project)?;
    let header = Reviews::new(&mut reviews).create(new)?;
    // The first review: its files merge by atm on branches too (staged with the review's folder, by whoever commits)
    if let Some(root) = project.root()
        && let Ok(paths) = atm_db::jsongit::paths::find_paths(root)
    {
        let _ = atm_db::jsongit::setup::ensure_review_merge(&paths);
    }
    link(project, &header, &ids, true)?;
    if header.status == Status::Waiting {
        let (back, rest): (Vec<String>, Vec<String>) =
            ids.into_iter().partition(|id| taken_back.contains(id));
        ask_review(project, &rest, &header, &header.title)?;
        let lapsed = format!(
            "{} (its claim had lapsed: taken back to open this review)",
            header.title
        );
        ask_review(project, &back, &header, &lapsed)?;
    }
    Ok(header)
}

/// The author's next round: the review is open again, and its tasks in progress go back in review.
///
/// # Errors
/// What the review's rules refuse, or a store error.
pub fn add_round(project: &mut Project, reference: &ReviewRef, round: NewRound) -> Result<u32> {
    let mut reviews = store(project)?;
    // Its tasks found first (ATM-594): a round is never kept without its tasks going back in review
    let ids = named(project, &reviews, &reviews.get_review(reference)?.header).map_err(|e| {
        Error::failed(format!(
            "{}'s round can't reach its tasks, so it isn't added: {e}",
            reference.0
        ))
    })?;
    let number = Reviews::new(&mut reviews).add_round(reference, round)?;
    let header = reviews.get_review(reference)?.header;
    ask_review(project, &ids, &header, &format!("round {number}"))?;
    Ok(number)
}

/// The author changes the review's summary, its Description (with a round: `atm review round --summary-file`).
///
/// # Errors
/// What the review's rules refuse (an ended review, not its author), or a store error.
pub fn edit_summary(project: &mut Project, reference: &ReviewRef, summary: &str) -> Result<()> {
    let by = actor_of(project);
    let mut reviews = store(project)?;
    Reviews::new(&mut reviews).edit_summary(reference, summary, &by)
}

/// A reviewer submits (§3.11's *Submit*); approving or sending back is every named task's decision, recorded as the
/// reviewer: approved, or changes requested with a note linking the review (`atm loop next` then hands the task back).
/// A task not in review (a live review's, still in progress) waits for [`apply_live_approval`]. `overriding`: a
/// person's explicit override of who may decide, with why, recorded in the review (the rules' `submit`).
///
/// # Errors
/// What the review's rules refuse, or a store error.
pub fn submit(
    project: &mut Project,
    reference: &ReviewRef,
    verdict: Verdict,
    body: Option<&str>,
    overriding: Option<&str>,
) -> Result<()> {
    let by = actor_of(project);
    let mut reviews = store(project)?;
    // Every task it names found first (ATM-594): a decision is never kept without reaching its tasks
    let tasks = named(project, &reviews, &reviews.get_review(reference)?.header).map_err(|e| {
        Error::failed(format!(
            "{reference}'s decision can't reach its tasks, so it isn't recorded: {e}",
            reference = reference.0
        ))
    })?;
    // A live review is approved as the working copy is now (ATM-576): that is the checkpoint its task's change is
    // compared with when it is done (unless the approval would be refused anyway, with comments open)
    let before = reviews.get_review(reference)?;
    let command = match verdict {
        Verdict::Approve => Some(Command::Approve),
        Verdict::RequestChanges => Some(Command::RequestChanges),
        Verdict::Comment => None,
    };
    let options_for = |review: &Review| {
        let round = review.rounds.last().map_or(1, |r| r.number);
        let open = review.open_comments().count();
        Options {
            note: Some(format!("in {} (round {round})", review.header.reference)),
            reason: Some(format!(
                "{} round {round}: {open} open comment{}",
                review.header.reference,
                if open == 1 { "" } else { "s" }
            )),
            ..Options::default()
        }
    };
    // Every task the decision changes checked before the review records it (ATM-670): all or none
    if let Some(command) = command {
        check_decision(project, &tasks, command, &options_for(&before))?;
    }
    if verdict == Verdict::Approve
        && before.header.status == Status::Live
        && before.open_comments().next().is_none()
    {
        checkpoint_now(project, reference)?;
    }
    Reviews::new(&mut reviews).submit(reference, verdict, body, overriding, &by)?;
    let Some(command) = command else {
        return Ok(());
    };
    let options = options_for(&reviews.get_review(reference)?);
    for id in tasks {
        if kind_of(project, &id)?.is_some_and(|(kind, _)| kind == Kind::Review) {
            change_task_status(project, command, &id, &options)?;
        }
    }
    Ok(())
}

/// Whether `command` may change each of `ids` in review, checked through the state machine on a copy before anything
/// is written (ATM-670, audit R7): a decision was recorded on the review, then the second task's change failed, and
/// the two were left split. A task not in review is left as it is, as [`submit`] does.
///
/// # Errors
/// The first task the state machine refuses, naming it.
fn check_decision(
    project: &Project,
    ids: &[String],
    command: Command,
    options: &Options,
) -> Result<()> {
    let vocab: &Vocabulary = project.store().vocabulary()?;
    let tasks = project.store().list_tasks()?;
    let stamp = atm_common::time::now_iso();
    for id in ids {
        let Some(task) = project.store().get_task(id)? else {
            return Err(Error::NotFound(format!("no task {id}")));
        };
        if vocab.kind_of(task.get("status")) != Some(Kind::Review) {
            continue;
        }
        let mut copy = task.clone();
        crate::state::change_status(
            &mut copy,
            command,
            options,
            vocab,
            &tasks,
            project.actor(),
            &stamp,
            &|id| crate::query::archived_task(project.store(), id),
        )
        .map_err(|e| {
            Error::failed(format!(
                "the decision can't reach {}, so it isn't recorded: {e}",
                project.task_key(task)
            ))
        })?;
    }
    Ok(())
}

/// End a review without approval; with `reject`, its tasks in review are rejected too (only when the user says so,
/// §3.8).
///
/// # Errors
/// What the review's rules refuse, or a store error.
pub fn close(
    project: &mut Project,
    reference: &ReviewRef,
    reason: &str,
    reject: bool,
) -> Result<()> {
    let by = actor_of(project);
    let mut reviews = store(project)?;
    Reviews::new(&mut reviews).close(reference, reason, &by)?;
    if reject {
        let header = reviews.get_review(reference)?.header;
        let options = Options {
            reason: Some(format!("{} closed: {reason}", header.reference)),
            ..Options::default()
        };
        for id in named(project, &reviews, &header)? {
            if kind_of(project, &id)?.is_some_and(|(kind, _)| kind == Kind::Review) {
                change_task_status(project, Command::Reject, &id, &options)?;
            }
        }
    }
    Ok(())
}

/// Name more tasks, or fewer (§3.2 ③): added ones are linked (and, for an open review, those in progress go in
/// review); removed ones are unlinked, with a note saying so.
///
/// # Errors
/// What the review's rules refuse, an unknown task, or a store error.
pub fn set_tasks(
    project: &mut Project,
    reference: &ReviewRef,
    add: &[String],
    remove: &[String],
) -> Result<ReviewHeader> {
    let by = actor_of(project);
    let mut reviews = store(project)?;
    let header = reviews.get_review(reference)?.header;
    let (added, removed) = (resolve(project, add)?, resolve(project, remove)?);
    // A key stays unless the task the review meant by it is removed (ATM-630: an old key may mean the task that moved)
    let mut tasks: Vec<String> = header
        .tasks
        .iter()
        .filter(|t| {
            !remove.contains(t)
                && review_keys::task_of(project, Some(&reviews), &header, t)
                    .ok()
                    .is_none_or(|id| !removed.contains(&id))
        })
        .cloned()
        .collect();
    for id in &added {
        let key = key_of_id(project, id);
        if !tasks.contains(&key) {
            tasks.push(key);
        }
    }
    Reviews::new(&mut reviews).set_tasks(reference, tasks, &by)?;
    // A review nobody's change yet becomes the agent's whose task it now names
    if header.author.is_none()
        && let Some(agent) = agent_of_tasks(project, &added)
        && !header.reviewers.contains(&agent)
    {
        reviews.set_author(&header.id, &agent)?;
    }
    let header = reviews.get_review(reference)?.header;
    link(project, &header, &added, true)?;
    link(project, &header, &removed, false)?;
    for id in &removed {
        crate::changes::add_task_note(
            project,
            id,
            &format!("No longer reviewed in {}", header.reference),
        )?;
    }
    if header.status == Status::Waiting {
        ask_review(project, &added, &header, &header.title)?;
    }
    Ok(header)
}

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

#[cfg(test)]
mod tests {
    use std::path::Path;

    use serde_json::{Value, json};

    use super::{NewReview, NewRound, Patch, ReviewRef, ReviewStore, RoundKind, Status, Verdict};
    use crate::project::Project;

    struct Dir(std::path::PathBuf);
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn dir() -> Dir {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let count = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("atm-lib-reviews-{}-{count}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        // Without Windows' `\\?\` prefix, as atm and git name the folder
        Dir(atm_common::pathtext::canonical(&path).unwrap())
    }
    fn git(args: &[&str], cwd: &Path) {
        let mut all = vec!["-c", "user.name=t", "-c", "user.email=t@example.com"];
        all.extend_from_slice(args);
        assert!(atm_db::git::git(&all, cwd).is_some(), "git {args:?}");
    }
    /// A project in git with tasks T-1 and T-2 (in progress, `blocker`), as `actor`.
    fn project(root: &Path, actor: &str) -> Project {
        let tasks = json!({
            "meta": { "project": "Reviews", "review": "blocker" },
            "tasks": [
                { "id": "T-1", "name": "One", "type": "feature", "status": "in_progress", "started_by": "claude:1" },
                { "id": "T-2", "name": "Two", "type": "feature", "status": "in_progress" },
            ]
        });
        std::fs::create_dir_all(root.join(".agentic-task-manager")).unwrap();
        std::fs::write(
            root.join(".agentic-task-manager/tasks.json"),
            tasks.to_string(),
        )
        .unwrap();
        Project::open(root).unwrap().with_actor(Some(actor.into()))
    }
    fn reopen(root: &Path, actor: &str) -> Project {
        Project::open(root).unwrap().with_actor(Some(actor.into()))
    }

    /// ATM-670 (audit R7): a decision is checked against every task it changes before the review records it
    #[test]
    fn a_decision_is_checked_on_every_task_before_anything_is_written() {
        use crate::state::{Command, Options};
        let dir = dir();
        project(&dir.0, "Hakan");
        let file = dir.0.join(".agentic-task-manager/tasks.json");
        let mut tasks: Value =
            serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        tasks["tasks"][1]["status"] = json!("in_review");
        tasks["tasks"][1]["review_state"] = json!("waiting");
        std::fs::write(&file, tasks.to_string()).unwrap();
        let before = std::fs::read_to_string(&file).unwrap();
        let p = reopen(&dir.0, "Hakan");
        let ids = ["T-1".to_string(), "T-2".to_string()];
        // A request for changes without its reason: refused, naming the task in review it can't reach
        let refused = super::check_decision(&p, &ids, Command::RequestChanges, &Options::default())
            .unwrap_err();
        assert!(
            refused
                .to_string()
                .starts_with("the decision can't reach T-2, so it isn't recorded"),
            "{refused}"
        );
        // With it, fine; T-1, in progress, is left as it is
        let options = Options {
            reason: Some("CR-1 round 1: 1 open comment".into()),
            ..Options::default()
        };
        super::check_decision(&p, &ids, Command::RequestChanges, &options).unwrap();
        super::check_decision(&p, &ids, Command::Approve, &options).unwrap();
        // Checking writes nothing
        assert_eq!(std::fs::read_to_string(&file).unwrap(), before);
        // An unknown task is named
        let missing = super::check_decision(&p, &["T-9".to_string()], Command::Approve, &options)
            .unwrap_err();
        assert_eq!(missing.to_string(), "no task T-9");
    }
    fn round() -> NewRound {
        NewRound {
            // Commits as atm records them, full IDs (ATM-659)
            kind: RoundKind::Range {
                base: "a".repeat(40),
                head: "b".repeat(40),
            },
            submodule: None,
            files: Vec::new(),
            patch: Patch(String::new()),
            by: "claude:1".into(),
        }
    }
    fn task(project: &Project, id: &str) -> Value {
        Value::Object(project.store().get_task(id).unwrap().unwrap().clone())
    }

    /// ATM-606: a project's summary counts the reviews waiting on the user at the machine (their git name), and a task
    /// in review isn't counted again through them; the summary's stamp moves with a comment
    #[test]
    fn a_projects_summary_counts_the_reviews_waiting_on_the_user_once() {
        let dir = dir();
        git(&["init", "-q"], &dir.0);
        git(&["config", "user.name", "Hakan"], &dir.0);
        let mut project = project(&dir.0, "claude:1");
        let summary = |p: &Project| crate::summary::project_summary(p).unwrap();
        assert_eq!(summary(&project)["needs_you"]["code_reviews"], 0);
        let stamp = crate::review_pages::reviews_stamp(&project);
        let cr = super::create(
            &mut project,
            NewReview {
                title: "One".into(),
                summary: String::new(),
                tasks: vec!["T-1".into()],
                author: Some("claude:1".into()),
                reviewers: vec!["Hakan".into()],
                status: Status::Waiting,
                first: round(),
            },
        )
        .unwrap()
        .reference;
        assert_ne!(crate::review_pages::reviews_stamp(&project), stamp);
        // Made by the agent, read as anyone: the user at the machine is the one it waits on
        let s = summary(&reopen(&dir.0, "claude:1"));
        assert_eq!(s["needs_you"]["code_reviews"], 1);
        // T-1 is in review, through CR-1: not counted again
        assert_eq!(task(&project, "T-1")["status"], "in_review");
        assert_eq!(s["needs_you"]["review"], 0);
        // With its ID, as T-1's code_reviews link it: the dashboard drops T-1 from its own list by it (ATM-640)
        let link = task(&project, "T-1")["code_reviews"][0]["id"].clone();
        assert_eq!(
            s["needs_you_reviews"],
            json!([{ "id": link, "reference": "CR-1", "title": "One", "why": "review" }])
        );
        // Sent back by the user: off their list until the agent answers
        let mut hakan = reopen(&dir.0, "Hakan");
        let mut reviews = super::store(&hakan).unwrap();
        let anchor = super::Anchor {
            round: 1,
            ..super::Anchor::default()
        };
        super::Reviews::new(&mut reviews)
            .comment(&cr, anchor, "Fix it.", &"Hakan".into())
            .unwrap();
        super::submit(&mut hakan, &cr, Verdict::RequestChanges, None, None).unwrap();
        assert_eq!(summary(&hakan)["needs_you"]["code_reviews"], 0);
        let open = reviews
            .get_review(&cr)
            .unwrap()
            .open_comments()
            .next()
            .unwrap()
            .id
            .clone();
        super::Reviews::new(&mut reviews)
            .reply(&cr, &open, "Fixing.", &"claude:1".into())
            .unwrap();
        assert_eq!(summary(&hakan)["needs_you_reviews"][0]["why"], "answered");
    }

    /// A project of `tasks`, its claims `claims` (by task ID), as `actor`.
    fn project_of(root: &Path, actor: &str, tasks: &Value, claims: &Value) -> Project {
        let folder = root.join(".agentic-task-manager");
        std::fs::create_dir_all(&folder).unwrap();
        let document =
            json!({ "meta": { "project": "Reviews", "review": "blocker" }, "tasks": tasks });
        std::fs::write(folder.join("tasks.json"), document.to_string()).unwrap();
        std::fs::write(
            folder.join("claims-store.json"),
            json!({ "claims": claims }).to_string(),
        )
        .unwrap();
        reopen(root, actor)
    }
    fn claim(task: &str, agent: &str, until: &str) -> Value {
        json!({ "task": task, "agent": agent, "lease_expires_at": until, "claimed_at": "2026-11-19T10:00:00Z" })
    }
    const LATER: &str = "2999-01-01T00:00:00Z";
    const LAPSED: &str = "2000-01-01T00:00:00Z";
    fn waiting(project: &mut Project, tasks: &[&str]) -> atm_common::Result<super::ReviewHeader> {
        super::create(
            project,
            NewReview {
                title: "The change".into(),
                summary: String::new(),
                tasks: tasks.iter().map(|t| (*t).to_string()).collect(),
                author: Some("claude:1".into()),
                reviewers: vec!["Hakan".into()],
                status: Status::Waiting,
                first: round(),
            },
        )
    }
    fn no_review(project: &Project) -> bool {
        super::store(project)
            .unwrap()
            .get_review(&ReviewRef("CR-1".into()))
            .is_err()
    }

    /// One case of opening a review: its name, the task, its claims, opened or the refusal's words, its status after
    type Case = (
        &'static str,
        Value,
        Value,
        Result<(), &'static str>,
        &'static str,
    );

    /// ATM-631: opening a review waiting on its reviewers puts each task in review, or refuses and writes nothing
    #[test]
    fn opening_a_review_moves_each_task_into_review_or_writes_nothing() {
        let task = |status: &str, started_by: Option<&str>| {
            let mut t = json!({ "id": "T-1", "name": "One", "type": "feature", "status": status });
            if let Some(agent) = started_by {
                t["started_by"] = json!(agent);
            }
            t
        };
        let cases: [Case; 9] = [
            (
                "in progress, claimed by the author",
                task("in_progress", Some("claude:1")),
                json!({ "T-1": claim("T-1", "claude:1", LATER) }),
                Ok(()),
                "in_review",
            ),
            (
                "in progress, no claim",
                task("in_progress", None),
                json!({}),
                Ok(()),
                "in_review",
            ),
            (
                "in progress, the author's claim run out",
                task("in_progress", Some("claude:1")),
                json!({ "T-1": claim("T-1", "claude:1", LAPSED) }),
                Ok(()),
                "in_review",
            ),
            (
                "to do after the author's claim lapsed",
                task("todo", Some("claude:1")),
                json!({}),
                Ok(()),
                "in_review",
            ),
            (
                "claimed by another agent",
                task("in_progress", Some("claude:2")),
                json!({ "T-1": claim("T-1", "claude:2", LATER) }),
                Err("claimed by claude:2"),
                "in_progress",
            ),
            (
                "to do, never started by the author",
                task("todo", Some("claude:2")),
                json!({}),
                Err("you didn't start it"),
                "todo",
            ),
            (
                "blocked",
                task("blocked", Some("claude:1")),
                json!({}),
                Err("it is blocked"),
                "blocked",
            ),
            (
                "done",
                task("done", Some("claude:1")),
                json!({}),
                Err("it is done"),
                "done",
            ),
            (
                "skipped",
                task("skipped", Some("claude:1")),
                json!({}),
                Err("it is skipped"),
                "skipped",
            ),
        ];
        for (case, t, claims, expected, after) in cases {
            let dir = dir();
            let mut project = project_of(&dir.0, "claude:1", &json!([t]), &claims);
            let opened = waiting(&mut project, &["T-1"]);
            match expected {
                Ok(()) => {
                    opened.unwrap_or_else(|e| panic!("{case}: {e}"));
                    let t = task_of(&project, "T-1");
                    assert_eq!(t["code_reviews"][0]["ref"], "CR-1", "{case}");
                }
                Err(words) => {
                    let message = opened.expect_err(case).to_string();
                    assert!(message.contains(words), "{case}: {message}");
                    assert!(no_review(&project), "{case}: a review was kept");
                }
            }
            assert_eq!(task_of(&project, "T-1")["status"], after, "{case}");
        }
    }

    #[test]
    fn a_task_taken_back_after_its_claim_lapsed_says_so_and_is_claimed_again() {
        let dir = dir();
        let tasks = json!([{ "id": "T-1", "name": "One", "type": "feature", "status": "todo", "started_by": "claude:1" }]);
        let mut project = project_of(&dir.0, "claude:1", &tasks, &json!({}));
        waiting(&mut project, &["T-1"]).unwrap();
        let t = task_of(&project, "T-1");
        assert_eq!(t["status"], "in_review");
        assert!(
            t["notes"].to_string().contains("its claim had lapsed"),
            "{}",
            t["notes"]
        );
    }

    #[test]
    fn a_task_in_review_is_linked_unless_another_review_waits_on_it() {
        let dir = dir();
        let mut project = project(&dir.0, "claude:1");
        waiting(&mut project, &["T-1"]).unwrap();
        // CR-1 waits on T-1: a second review is refused, and points to a round of CR-1
        let refused = waiting(&mut project, &["T-1"]).unwrap_err().to_string();
        assert!(
            refused.contains("CR-1 already waits on it")
                && refused.contains("atm review round CR-1"),
            "{refused}"
        );
        // In review with no review waiting on it (CR-1 closed): linked, still in review
        let mut reviews = super::store(&project).unwrap();
        super::Reviews::new(&mut reviews)
            .close(&ReviewRef("CR-1".into()), "Replaced.", &"claude:1".into())
            .unwrap();
        let second = waiting(&mut project, &["T-1"]).unwrap();
        let t = task_of(&project, "T-1");
        assert_eq!(t["status"], "in_review");
        assert!(t["code_reviews"].to_string().contains(&second.reference.0));
    }

    #[test]
    fn a_review_refused_for_one_task_moves_none_of_the_others() {
        let dir = dir();
        let tasks = json!([
            { "id": "T-1", "name": "One", "type": "feature", "status": "todo", "started_by": "claude:1" },
            { "id": "T-2", "name": "Two", "type": "feature", "status": "done" },
        ]);
        let mut project = project_of(&dir.0, "claude:1", &tasks, &json!({}));
        assert!(waiting(&mut project, &["T-1", "T-2"]).is_err());
        assert!(no_review(&project));
        assert_eq!(task_of(&project, "T-1")["status"], "todo");
    }

    /// ATM-607: a live review's page is told of the working copy's changes it shows, not of `.git/`, atm's task folder
    /// or the files git ignores
    #[test]
    fn the_working_copy_is_watched_for_changes_a_live_review_shows() {
        use std::time::Duration;
        let dir = dir();
        git(&["init", "-q"], &dir.0);
        std::fs::write(dir.0.join(".gitignore"), "target/\n").unwrap();
        std::fs::create_dir_all(dir.0.join("target")).unwrap();
        let project = project(&dir.0, "claude:1");
        let (sent, received) = std::sync::mpsc::channel();
        let sent = std::sync::Mutex::new(sent);
        let subscription = super::subscribe_working_copy(
            &project,
            std::sync::Arc::new(move || {
                if let Ok(sent) = sent.lock() {
                    let _ = sent.send(());
                }
            }),
        )
        .unwrap();
        // What was written just before the watch began (the .gitignore, git's own files) may still be told on Linux,
        // where the first events come late: let them pass, a few bursts at most (a watch waking itself, as reading
        // the ignore rules once did there, would never go quiet)
        std::thread::sleep(Duration::from_millis(300));
        let settled = (0..10).any(|_| received.recv_timeout(Duration::from_millis(600)).is_err());
        assert!(settled, "the watch never went quiet");
        let quiet = |what: &str| {
            assert!(
                received.recv_timeout(Duration::from_millis(1200)).is_err(),
                "{what}: nothing told"
            );
        };
        std::fs::write(dir.0.join("target/build.o"), "o").unwrap();
        quiet("an ignored file");
        std::fs::write(dir.0.join(".git/scratch"), "g").unwrap();
        quiet("a file in .git/");
        std::fs::write(dir.0.join(".agentic-task-manager/notes.txt"), "n").unwrap();
        quiet("atm's task folder");
        std::fs::write(dir.0.join("a.txt"), "a").unwrap();
        received
            .recv_timeout(Duration::from_secs(10))
            .expect("a file of the work: told");
        drop(subscription);
    }

    /// ATM-610: the list says since when each review waits on its reviewers, its latest round's time
    #[test]
    fn the_list_says_since_when_each_review_waits() {
        let dir = dir();
        let mut project = project(&dir.0, "claude:1");
        waiting(&mut project, &["T-1"]).unwrap();
        let cr = ReviewRef("CR-1".into());
        let listed = crate::review_pages::list_reviews(
            &project,
            &crate::review_pages::ReviewQuery::default(),
        )
        .unwrap();
        let first = super::store(&project).unwrap().get_review(&cr).unwrap();
        assert_eq!(
            listed["reviews"][0]["waiting_since"],
            json!(first.rounds[0].at)
        );
        let mut reviews = super::store(&project).unwrap();
        let mut later = round();
        later.kind = RoundKind::Range {
            base: "a".repeat(40),
            head: "c".repeat(40),
        };
        super::Reviews::new(&mut reviews)
            .add_round(&cr, later)
            .unwrap();
        let again = crate::review_pages::list_reviews(
            &project,
            &crate::review_pages::ReviewQuery::default(),
        )
        .unwrap();
        let second = super::store(&project).unwrap().get_review(&cr).unwrap();
        assert_eq!(
            again["reviews"][0]["waiting_since"],
            json!(second.rounds[1].at),
            "its latest round"
        );
    }

    /// A person's review of files for a task an agent hasn't started (the sandbox's red-team review): linked, as it is
    #[test]
    fn a_persons_review_of_a_task_not_started_links_it_as_it_is() {
        let dir = dir();
        let tasks = json!([{ "id": "T-1", "name": "One", "type": "feature", "status": "todo" }]);
        let mut project = project_of(&dir.0, "Hakan", &tasks, &json!({}));
        waiting(&mut project, &["T-1"]).unwrap();
        let t = task_of(&project, "T-1");
        assert_eq!(t["status"], "todo");
        assert_eq!(t["code_reviews"][0]["ref"], "CR-1");
    }

    #[test]
    fn a_draft_review_leaves_its_tasks_as_they_are_whatever_their_state() {
        let dir = dir();
        let tasks = json!([{ "id": "T-1", "name": "One", "type": "feature", "status": "done" }]);
        let mut project = project_of(&dir.0, "claude:1", &tasks, &json!({}));
        let draft = NewReview {
            title: "Draft".into(),
            summary: String::new(),
            tasks: vec!["T-1".into()],
            author: Some("claude:1".into()),
            reviewers: Vec::new(),
            status: Status::Draft,
            first: round(),
        };
        super::create(&mut project, draft).unwrap();
        assert_eq!(task_of(&project, "T-1")["status"], "done");
    }

    fn task_of(project: &Project, id: &str) -> Value {
        task(&reopen(project.root().unwrap(), "claude:1"), id)
    }

    #[test]
    fn a_review_of_two_tasks_puts_both_in_review_and_its_decisions_are_theirs() {
        let dir = dir();
        let mut project = project(&dir.0, "claude:1");
        let header = super::create(
            &mut project,
            NewReview {
                title: "Both".into(),
                summary: String::new(),
                tasks: vec!["T-1".into(), "T-2".into()],
                author: Some("claude:1".into()),
                reviewers: vec!["Hakan".into()],
                status: Status::Waiting,
                first: round(),
            },
        )
        .unwrap();
        let cr = header.reference.clone();
        for id in ["T-1", "T-2"] {
            let t = task(&project, id);
            assert_eq!(t["status"], "in_review", "{id}");
            assert_eq!(
                t["code_reviews"],
                json!([{ "id": header.id.0, "ref": "CR-1" }])
            );
            assert!(t["notes"].to_string().contains("CR-1: Both"));
        }

        // The reviewer comments and sends it back: both tasks have changes asked for, naming the review
        let mut project = reopen(&dir.0, "Hakan");
        let mut reviews = super::store(&project).unwrap();
        super::Reviews::new(&mut reviews)
            .comment(
                &cr,
                super::Anchor {
                    round: 1,
                    ..super::Anchor::default()
                },
                "Fix it.",
                &"Hakan".into(),
            )
            .unwrap();
        super::submit(&mut project, &cr, Verdict::RequestChanges, None, None).unwrap();
        let t1 = task(&project, "T-1");
        assert_eq!(t1["review_state"], "changes_requested");
        assert!(
            t1["notes"]
                .to_string()
                .contains("CR-1 round 1: 1 open comment"),
            "{}",
            t1["notes"]
        );

        // The agent takes it back, fixes it, adds a round: in review again
        let mut agent = reopen(&dir.0, "claude:1");
        crate::changes::change_task_status(
            &mut agent,
            crate::state::Command::Start,
            "T-1",
            &crate::state::Options::default(),
        )
        .unwrap();
        super::add_round(&mut agent, &cr, round()).unwrap();
        assert_eq!(task(&agent, "T-1")["review_state"], "waiting");

        // Approved once the comment is resolved: approved for the task in review
        let mut project = reopen(&dir.0, "Hakan");
        let mut reviews = super::store(&project).unwrap();
        let open = reviews
            .get_review(&cr)
            .unwrap()
            .open_comments()
            .next()
            .unwrap()
            .id
            .clone();
        super::Reviews::new(&mut reviews)
            .resolve(&cr, &open, true, &"Hakan".into())
            .unwrap();
        super::submit(&mut project, &cr, Verdict::Approve, None, None).unwrap();
        assert_eq!(task(&project, "T-1")["review_state"], "approved");
        // T-2 stayed in review with changes asked for: approved now too
        assert_eq!(task(&project, "T-2")["review_state"], "approved");
    }

    /// ATM-609: the reviewer approves a review they sent back, with no round since: the review and its task approved
    #[test]
    fn a_review_sent_back_is_approved_without_a_round_since_and_so_is_its_task() {
        let dir = dir();
        let mut project = project(&dir.0, "claude:1");
        let cr = super::create(
            &mut project,
            NewReview {
                title: "One".into(),
                summary: String::new(),
                tasks: vec!["T-1".into()],
                author: Some("claude:1".into()),
                reviewers: vec!["Hakan".into()],
                status: Status::Waiting,
                first: round(),
            },
        )
        .unwrap()
        .reference;
        let mut project = reopen(&dir.0, "Hakan");
        let mut reviews = super::store(&project).unwrap();
        let anchor = super::Anchor {
            round: 1,
            ..super::Anchor::default()
        };
        super::Reviews::new(&mut reviews)
            .comment(&cr, anchor, "Fix it.", &"Hakan".into())
            .unwrap();
        super::submit(&mut project, &cr, Verdict::RequestChanges, None, None).unwrap();
        assert_eq!(task(&project, "T-1")["review_state"], "changes_requested");
        // With its comment open, still refused
        assert!(super::submit(&mut project, &cr, Verdict::Approve, None, None).is_err());
        let open = reviews
            .get_review(&cr)
            .unwrap()
            .open_comments()
            .next()
            .unwrap()
            .id
            .clone();
        super::Reviews::new(&mut reviews)
            .resolve(&cr, &open, true, &"Hakan".into())
            .unwrap();
        super::submit(&mut project, &cr, Verdict::Approve, None, None).unwrap();
        let reviews = super::store(&project).unwrap();
        assert_eq!(
            reviews.get_review(&cr).unwrap().header.status,
            Status::Approved
        );
        assert_eq!(task(&project, "T-1")["review_state"], "approved");
    }

    fn open_review(project: &mut Project, tasks: &[&str]) -> super::ReviewHeader {
        super::create(
            project,
            NewReview {
                title: "Both".into(),
                summary: String::new(),
                tasks: tasks.iter().map(|t| (*t).to_string()).collect(),
                author: Some("claude:1".into()),
                reviewers: vec!["Hakan".into()],
                status: Status::Waiting,
                first: round(),
            },
        )
        .unwrap()
    }

    #[test]
    fn a_project_without_a_folder_or_git_has_no_reviews() {
        let memory = Project::new(Box::new(
            atm_db::MemoryStore::new(Vec::new(), Vec::new(), &serde_json::Map::new()).unwrap(),
        ));
        assert_eq!(
            super::store(&memory).unwrap_err().to_string(),
            "this project has no folder to keep reviews in"
        );
        assert_eq!(
            super::vcs(&memory).unwrap_err().to_string(),
            "reviews are of changes in git: this project isn't in a git repository"
        );
        // nothing to check either
        assert_eq!(
            super::check(&memory, &json!({"tasks": []})),
            (Vec::new(), Vec::new())
        );
    }

    #[test]
    fn a_comment_only_submission_leaves_the_tasks_waiting() {
        let dir = dir();
        let mut project = project(&dir.0, "claude:1");
        let header = open_review(&mut project, &["T-1"]);
        let mut hakan = reopen(&dir.0, "Hakan");
        super::submit(
            &mut hakan,
            &header.reference,
            Verdict::Comment,
            Some("hm"),
            None,
        )
        .unwrap();
        assert_eq!(task(&hakan, "T-1")["review_state"], "waiting");
    }

    #[test]
    fn closing_a_review_rejects_its_tasks_only_when_asked() {
        for (reject, state) in [(false, "waiting"), (true, "rejected")] {
            let dir = dir();
            let mut project = project(&dir.0, "claude:1");
            let header = open_review(&mut project, &["T-1"]);
            let mut hakan = reopen(&dir.0, "Hakan");
            super::close(&mut hakan, &header.reference, "not needed", reject).unwrap();
            let t1 = task(&hakan, "T-1");
            assert_eq!(t1["review_state"], state, "reject {reject}");
            if reject {
                assert!(t1["notes"].to_string().contains("CR-1 closed: not needed"));
            }
        }
    }

    #[test]
    fn doctor_warns_of_a_task_linked_to_a_review_the_project_doesnt_have() {
        let dir = dir();
        let mut project = project(&dir.0, "claude:1");
        let header = open_review(&mut project, &["T-1"]);
        let document = json!({"tasks": [
            {"id": "T-1", "code_reviews": [{"id": header.id.0, "ref": "CR-1"}]},
            {"id": "T-2", "code_reviews": [{"id": "gone", "ref": "CR-7"}]},
            {"id": "T-3", "code_reviews": [{"id": "gone-too"}]}
        ]});
        let (errors, warnings) = super::check(&project, &document);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(
            warnings,
            [
                "T-2: its code review CR-7 isn't in the project (not merged here yet, or deleted)",
                "T-3: its code review gone-too isn't in the project (not merged here yet, or deleted)"
            ]
        );
    }

    #[test]
    fn tasks_named_later_are_linked_and_removed_ones_unlinked_with_a_note() {
        let dir = dir();
        let mut project = project(&dir.0, "claude:1");
        let header = super::create(
            &mut project,
            NewReview {
                title: "Draft".into(),
                summary: String::new(),
                tasks: vec!["T-1".into()],
                author: Some("claude:1".into()),
                reviewers: Vec::new(),
                status: Status::Draft,
                first: round(),
            },
        )
        .unwrap();
        // A draft puts nothing in review
        assert_eq!(task(&project, "T-1")["status"], "in_progress");
        let header2 = super::set_tasks(
            &mut project,
            &header.reference,
            &["T-2".into()],
            &["T-1".into()],
        )
        .unwrap();
        assert_eq!(header2.tasks, ["T-2"]);
        assert!(task(&project, "T-1").get("code_reviews").is_none());
        assert!(
            task(&project, "T-1")["notes"]
                .to_string()
                .contains("No longer reviewed in CR-1")
        );
        assert_eq!(task(&project, "T-2")["code_reviews"][0]["ref"], "CR-1");
    }

    /// A live review opened by Hakan on the agent's change to `a.txt` (one line added), committed `one`
    fn live(dir: &Path) -> (Project, ReviewRef) {
        git(&["init", "-q"], dir);
        std::fs::write(dir.join("a.txt"), "one\n").unwrap();
        let project = project(dir, "claude:1");
        git(&["add", "-A"], dir);
        git(&["commit", "-q", "-m", "base"], dir);
        std::fs::write(dir.join("a.txt"), "one\ntwo\n").unwrap();
        let first =
            super::rounds::from_working(&project.vcs().unwrap(), "HEAD", &"Hakan".into()).unwrap();
        let mut hakan = reopen(dir, "Hakan");
        let header = super::create(
            &mut hakan,
            NewReview {
                title: "Live".into(),
                summary: String::new(),
                tasks: vec!["T-1".into()],
                author: Some("Hakan".into()),
                reviewers: Vec::new(),
                status: Status::Live,
                first,
            },
        )
        .unwrap();
        (hakan, ReviewRef(header.reference.0))
    }

    /// ATM-576: the page shows a live review's working copy as it is now, after its checkpoints, not stored; a
    /// comment on it makes it a checkpoint first, and is anchored there; a named round is that round
    #[test]
    fn a_live_reviews_page_follows_the_working_copy_and_a_comment_makes_it_a_checkpoint() {
        let dir = dir();
        let (hakan, cr) = live(&dir.0);
        let page = || crate::review_pages::review_page(&hakan, &cr, None, None).unwrap();
        // ATM-607: what its page now is made from: the same while nothing changes, another once a file is written; a
        // round named isn't the page now, and isn't tagged
        let stamp = || crate::review_pages::live_page_stamp(&hakan, &cr, None, None).unwrap();
        let before = stamp().expect("a live review's page now is tagged");
        assert_eq!(stamp(), Some(before.clone()));
        assert_eq!(
            crate::review_pages::live_page_stamp(&hakan, &cr, Some(1), None).unwrap(),
            None
        );
        // Nothing changed since its first round: that round, now
        let unchanged = page();
        assert_eq!(
            (
                unchanged["view"]["round"].clone(),
                unchanged["view"]["now"].clone()
            ),
            (json!(1), json!(true))
        );
        // Changed: the working copy as the round after the stored ones, which `rounds` doesn't list
        std::fs::write(dir.0.join("a.txt"), "one\ntwo\nthree\n").unwrap();
        assert_ne!(stamp(), Some(before), "a file written: another tag");
        let changed = page();
        assert_eq!(changed["view"]["round"], 2);
        assert_eq!(changed["rounds"].as_array().unwrap().len(), 1);
        let patch = changed["view"]["files"][0]["patch"].as_str().unwrap();
        assert!(patch.contains("+three"), "{patch}");
        // Read again, nothing is stored
        assert_eq!(
            super::store(&hakan)
                .unwrap()
                .get_review(&cr)
                .unwrap()
                .rounds
                .len(),
            1
        );
        // A comment on what is shown: the checkpoint, round 2, then the comment on it
        let comment = crate::review_requests::NewComment {
            path: Some("a.txt".into()),
            lines: Some((3, 3)),
            side: super::Side::default(),
            quote: Some("three".into()),
            round: Some(2),
            body: "Why three?".into(),
        };
        crate::review_requests::add_comment(&hakan, &cr, comment.clone()).unwrap();
        let review = super::store(&hakan).unwrap().get_review(&cr).unwrap();
        assert_eq!(review.rounds.len(), 2);
        assert_eq!(review.chains[0].anchor.round, 2);
        assert_eq!(page()["view"]["round"], 2);
        // Another comment with nothing changed since: no other checkpoint
        crate::review_requests::add_comment(&hakan, &cr, comment).unwrap();
        assert_eq!(
            super::store(&hakan)
                .unwrap()
                .get_review(&cr)
                .unwrap()
                .rounds
                .len(),
            2
        );
        // A round named: that round, not now
        let first = crate::review_pages::review_page(&hakan, &cr, Some(1), None).unwrap();
        assert_eq!(
            (first["view"]["round"].clone(), first["view"]["now"].clone()),
            (json!(1), json!(false))
        );
    }

    /// ATM-599, Hakan on CR-4: a commit of the project that moves a submodule to a later commit shows the files that
    /// changed inside it, each with its diff and its two sides, as the submodule's commits have them
    #[test]
    fn a_project_commit_moving_a_submodule_shows_the_files_changed_inside_it() {
        let dir = dir();
        let sub = dir.0.join("sub-origin");
        std::fs::create_dir_all(&sub).unwrap();
        git(&["init", "-q"], &sub);
        std::fs::write(sub.join("a.txt"), "a\n").unwrap();
        git(&["add", "-A"], &sub);
        git(&["commit", "-q", "-m", "sub"], &sub);
        let root = dir.0.join("repo");
        std::fs::create_dir_all(&root).unwrap();
        git(&["init", "-q"], &root);
        let project = project(&root, "claude:1");
        let add = [
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "--quiet",
            "add",
            "../sub-origin",
            "lib",
        ];
        git(&add, &root);
        git(&["add", "-A"], &root);
        git(&["commit", "-q", "-m", "base"], &root);
        // In the submodule: a file changed, one added, committed there; the project records its new commit
        std::fs::write(root.join("lib/a.txt"), "a, changed\n").unwrap();
        std::fs::write(root.join("lib/b.txt"), "b\n").unwrap();
        git(&["add", "-A"], &root.join("lib"));
        git(&["commit", "-q", "-m", "inside"], &root.join("lib"));
        git(&["add", "lib"], &root);
        git(&["commit", "-q", "-m", "the submodule moved on"], &root);
        let vcs = project.vcs().unwrap();
        let first = super::rounds::from_range(&vcs, "HEAD~1", "HEAD", &"claude:1".into()).unwrap();
        let mut agent = reopen(&root, "claude:1");
        let header = super::create(
            &mut agent,
            NewReview {
                title: "Moved on".into(),
                summary: String::new(),
                tasks: vec!["T-1".into()],
                author: None,
                reviewers: Vec::new(),
                status: Status::Waiting,
                first,
            },
        )
        .unwrap();
        let page =
            crate::review_pages::review_page(&agent, &ReviewRef(header.reference.0), None, None)
                .unwrap();
        let files = page["view"]["files"].as_array().unwrap();
        let paths: Vec<&str> = files.iter().filter_map(|f| f["path"].as_str()).collect();
        // The submodule's entry, and the files inside it
        assert!(paths.contains(&"lib"), "{paths:?}");
        let inside = |path: &str| {
            files
                .iter()
                .find(|f| f["path"] == path)
                .unwrap_or_else(|| panic!("{path} not shown: {paths:?}"))
        };
        let a = inside("lib/a.txt");
        assert_eq!(a["status"], "modified");
        let patch = a["patch"].as_str().unwrap();
        assert!(
            patch.contains("-a\n") && patch.contains("+a, changed\n"),
            "{patch}"
        );
        assert_eq!(inside("lib/b.txt")["status"], "added");
        // Each side reads as the submodule's commits have it
        let read = |revision: &Value| {
            crate::documents::read_document_at(Some(&vcs), revision.as_str().unwrap(), "lib/a.txt")
                .unwrap()["text"]
                .clone()
        };
        assert_eq!(read(&a["before"]), "a\n");
        assert_eq!(read(&a["after"]), "a, changed\n");
    }

    /// ATM-599 (`markdown-rs` moved into `crates/`, in CompMD): a commit that only moves a submodule's folder shows it as
    /// the submodule, at its one commit, not as a file whose sides are read
    #[test]
    fn a_submodule_only_moved_shows_as_the_submodule_at_its_commit() {
        let dir = dir();
        let sub = dir.0.join("sub-origin");
        std::fs::create_dir_all(&sub).unwrap();
        git(&["init", "-q"], &sub);
        std::fs::write(sub.join("a.txt"), "a\n").unwrap();
        git(&["add", "-A"], &sub);
        git(&["commit", "-q", "-m", "sub"], &sub);
        let root = dir.0.join("repo");
        std::fs::create_dir_all(&root).unwrap();
        git(&["init", "-q"], &root);
        let project = project(&root, "claude:1");
        let add = [
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "--quiet",
            "add",
            "../sub-origin",
            "lib",
        ];
        git(&add, &root);
        git(&["add", "-A"], &root);
        git(&["commit", "-q", "-m", "base"], &root);
        std::fs::create_dir_all(root.join("crates")).unwrap();
        git(&["mv", "lib", "crates/lib"], &root);
        git(&["commit", "-q", "-m", "moved"], &root);
        let vcs = project.vcs().unwrap();
        let first = super::rounds::from_range(&vcs, "HEAD~1", "HEAD", &"claude:1".into()).unwrap();
        let mut agent = reopen(&root, "claude:1");
        let header = super::create(
            &mut agent,
            NewReview {
                title: "Moved".into(),
                summary: String::new(),
                tasks: vec!["T-1".into()],
                author: None,
                reviewers: Vec::new(),
                status: Status::Waiting,
                first,
            },
        )
        .unwrap();
        let page =
            crate::review_pages::review_page(&agent, &ReviewRef(header.reference.0), None, None)
                .unwrap();
        let files = page["view"]["files"].as_array().unwrap();
        let moved = files.iter().find(|f| f["path"] == "crates/lib").unwrap();
        assert_eq!(moved["old_path"], "lib");
        let (from, to) = (&moved["submodule"]["from"], &moved["submodule"]["to"]);
        assert!(from.is_string() && from == to, "{moved}");
        // The other entries are files, as before
        assert!(
            files
                .iter()
                .filter(|f| f["path"] != "crates/lib")
                .all(|f| f["submodule"].is_null())
        );
    }

    /// ATM-576: approving a live review approves the working copy as it is then, a checkpoint, which its task's change
    /// is compared with when it is done
    #[test]
    fn approving_a_live_review_approves_the_working_copy_as_it_is() {
        let dir = dir();
        let (mut hakan, cr) = live(&dir.0);
        std::fs::write(dir.0.join("a.txt"), "one\ntwo\nthree\n").unwrap();
        super::submit(&mut hakan, &cr, Verdict::Approve, None, None).unwrap();
        let review = super::store(&hakan).unwrap().get_review(&cr).unwrap();
        assert_eq!(
            (review.header.status, review.rounds.len()),
            (Status::Approved, 2)
        );
        let mut agent = reopen(&dir.0, "claude:1");
        assert!(
            super::apply_live_approval(&mut agent, "T-1")
                .unwrap()
                .is_some()
        );
        assert_eq!(task(&agent, "T-1")["review_state"], "approved");
    }

    #[test]
    fn an_approved_live_review_covers_a_task_until_its_change_moves() {
        let dir = dir();
        git(&["init", "-q"], &dir.0);
        std::fs::write(dir.0.join("a.txt"), "one\n").unwrap();
        let project = project(&dir.0, "claude:1");
        git(&["add", "-A"], &dir.0);
        git(&["commit", "-q", "-m", "base"], &dir.0);
        let vcs = project.vcs().unwrap();
        std::fs::write(dir.0.join("a.txt"), "one\ntwo\n").unwrap();
        let checkpoint = super::rounds::from_working(&vcs, "HEAD", &"Hakan".into()).unwrap();
        // Hakan opens a live review on the agent's work (§3.5): he reviews it, the change is the agent's who started
        // T-1, so he decides it
        let mut hakan = reopen(&dir.0, "Hakan");
        let header = super::create(
            &mut hakan,
            NewReview {
                title: "Live".into(),
                summary: String::new(),
                tasks: vec!["T-1".into()],
                author: Some("Hakan".into()),
                reviewers: Vec::new(),
                status: Status::Live,
                first: checkpoint,
            },
        )
        .unwrap();
        assert_eq!(header.author, Some("claude:1".into()));
        assert_eq!(header.reviewers, vec![super::Actor::from("Hakan")]);
        // Live: the task stays in progress
        assert_eq!(task(&project, "T-1")["status"], "in_progress");
        super::submit(
            &mut hakan,
            &ReviewRef(header.reference.0.clone()),
            Verdict::Approve,
            None,
            None,
        )
        .unwrap();

        // Changed since: not covered
        std::fs::write(dir.0.join("a.txt"), "one\ntwo\nthree\n").unwrap();
        let mut agent = reopen(&dir.0, "claude:1");
        assert_eq!(super::apply_live_approval(&mut agent, "T-1").unwrap(), None);
        // As approved: covered, the task approved naming the review
        std::fs::write(dir.0.join("a.txt"), "one\ntwo\n").unwrap();
        assert_eq!(
            super::apply_live_approval(&mut agent, "T-1")
                .unwrap()
                .unwrap()
                .reference
                .0,
            "CR-1"
        );
        let t1 = task(&agent, "T-1");
        assert_eq!(t1["review_state"], "approved");
        assert!(
            t1["notes"]
                .to_string()
                .contains("approved in the live review CR-1")
        );
        crate::changes::change_task_status(
            &mut agent,
            crate::state::Command::Done,
            "T-1",
            &crate::state::Options::default(),
        )
        .unwrap();
    }

    // ATM-594: a provisional key (`~ATM-12` as people read it) in reviews

    const PROVISIONAL: &str = "0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8012";

    /// A project in task keys' format: ATM-11 final, ATM-12 provisional, both in progress (`blocker`), as `actor`.
    fn keyed(root: &Path, actor: &str) -> Project {
        let tasks = json!({
            "meta": { "project": "Keyed", "format": 2, "key": "ATM", "next_number": 13, "review": "blocker" },
            "tasks": [
                { "id": "0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8011", "number": 11, "key_final": true, "name": "Final",
                  "type": "feature", "status": "in_progress", "started_by": "claude:1" },
                { "id": PROVISIONAL, "number": 12, "key_final": false, "name": "Provisional",
                  "type": "feature", "status": "in_progress", "started_by": "claude:1" },
            ]
        });
        std::fs::create_dir_all(root.join(".agentic-task-manager")).unwrap();
        std::fs::write(
            root.join(".agentic-task-manager/tasks.json"),
            tasks.to_string(),
        )
        .unwrap();
        Project::open(root).unwrap().with_actor(Some(actor.into()))
    }

    #[test]
    fn a_review_names_a_provisional_task_by_its_key_and_its_decision_reaches_it() {
        let dir = dir();
        let mut project = keyed(&dir.0, "claude:1");
        // Named as atm prints it, with the mark: the key itself
        let header = open_review(&mut project, &["~ATM-12", "ATM-11"]);
        assert_eq!(header.tasks, ["ATM-12", "ATM-11"]);
        let text = std::fs::read_to_string(
            std::fs::read_dir(dir.0.join(".agentic-task-manager/reviews"))
                .unwrap()
                .filter_map(Result::ok)
                .find(|e| e.path().is_dir())
                .unwrap()
                .path()
                .join("review.md"),
        )
        .unwrap();
        assert!(text.contains("tasks = [\"ATM-12\", \"ATM-11\"]"), "{text}");
        assert_eq!(task(&project, PROVISIONAL)["status"], "in_review");
        // The approval reaches both
        let mut hakan = reopen(&dir.0, "Hakan");
        super::submit(&mut hakan, &header.reference, Verdict::Approve, None, None).unwrap();
        assert_eq!(task(&hakan, PROVISIONAL)["review_state"], "approved");
        // And every command takes the key as people read it
        assert_eq!(hakan.resolve_task("~ATM-12").unwrap(), PROVISIONAL);
        assert_eq!(hakan.resolve_task("ATM-12").unwrap(), PROVISIONAL);
        assert!(hakan.resolve_task("~").is_err(), "a mark alone is no key");
    }

    #[test]
    fn a_decision_that_cant_reach_a_named_task_isnt_recorded_and_says_why() {
        let dir = dir();
        let mut project = keyed(&dir.0, "claude:1");
        let header = open_review(&mut project, &["ATM-12"]);
        // The task is gone from the project (a hand edit, a branch that dropped it)
        let mut store = super::store(&project).unwrap();
        store.set_tasks(&header.id, vec!["ATM-99".into()]).unwrap();
        let mut hakan = reopen(&dir.0, "Hakan");
        let error =
            super::submit(&mut hakan, &header.reference, Verdict::Approve, None, None).unwrap_err();
        assert!(
            error
                .to_string()
                .starts_with("CR-1's decision can't reach its tasks, so it isn't recorded: "),
            "{error}"
        );
        let reviews = super::store(&hakan).unwrap();
        assert_eq!(
            reviews.get_review(&header.reference).unwrap().header.status,
            Status::Waiting
        );
        // A round neither: refused before anything is written
        let mut agent = reopen(&dir.0, "claude:1");
        let error = super::add_round(&mut agent, &header.reference, round()).unwrap_err();
        assert!(
            error
                .to_string()
                .starts_with("CR-1's round can't reach its tasks, so it isn't added: "),
            "{error}"
        );
        let reviews = super::store(&agent).unwrap();
        assert_eq!(
            reviews.get_review(&header.reference).unwrap().rounds.len(),
            1
        );
    }

    #[test]
    fn doctor_names_a_decided_review_whose_task_still_waits_and_the_command_that_records_it() {
        let dir = dir();
        let mut project = keyed(&dir.0, "claude:1");
        let header = open_review(&mut project, &["ATM-12", "ATM-11"]);
        // As an atm before ATM-594 left it: the review approved, its decision not on the tasks
        let mut store = super::store(&project).unwrap();
        store
            .set_status(&header.id, Status::Approved, &"Hakan".into())
            .unwrap();
        let document = project.store().document().unwrap().clone();
        let (errors, warnings) = super::check(&project, &document);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(
            warnings,
            [
                "ATM-12: CR-1 is approved but the task still waits for its review: `atm task review approve ATM-12` records the decision",
                "ATM-11: CR-1 is approved but the task still waits for its review: `atm task review approve ATM-11` records the decision",
            ]
        );
        // Recorded: nothing more to say
        let mut hakan = reopen(&dir.0, "Hakan");
        for key in ["ATM-12", "ATM-11"] {
            let id = hakan.resolve_task(key).unwrap();
            crate::changes::change_task_status(
                &mut hakan,
                crate::state::Command::Approve,
                &id,
                &crate::state::Options::default(),
            )
            .unwrap();
        }
        let document = hakan.store().document().unwrap().clone();
        assert_eq!(super::check(&hakan, &document), (Vec::new(), Vec::new()));
    }

    /// ATM-630: a review names its task by the key it was written with; atm finds the task through it as every command
    /// does: a task whose number moved (its old key an alias, ADR-009), an old review's `~KEY`; a current key over an
    /// alias, as ADR-007 §5 has it
    #[test]
    fn a_review_names_its_task_by_the_key_it_had_an_alias_now_or_with_an_old_mark() {
        let dir = dir();
        let id = |n: u32| format!("0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f80{n:02}");
        let tasks = json!({
            "meta": { "project": "Moved", "format": 2, "key": "ATM", "next_number": 15 },
            "tasks": [
                // Moved off 9 at a merge, 9 free since
                { "id": id(14), "number": 14, "aliases": ["ATM-9"], "name": "Moved", "type": "feature", "status": "todo" },
                // 12 is this task's; another one moved off 12 to 13
                { "id": id(12), "number": 12, "name": "Holds 12", "type": "feature", "status": "todo" },
                { "id": id(13), "number": 13, "aliases": ["ATM-12"], "name": "Moved off 12", "type": "feature", "status": "todo" },
            ]
        });
        std::fs::create_dir_all(dir.0.join(".agentic-task-manager")).unwrap();
        std::fs::write(
            dir.0.join(".agentic-task-manager/tasks.json"),
            tasks.to_string(),
        )
        .unwrap();
        let project = reopen(&dir.0, "Hakan");
        let header = |keys: &[&str]| super::ReviewHeader {
            id: super::ReviewId("r".into()),
            reference: super::ReviewRef("CR-1".into()),
            number: 1,
            title: "T".into(),
            status: Status::Waiting,
            tasks: keys.iter().map(|k| (*k).to_string()).collect(),
            author: None,
            reviewers: Vec::new(),
            created: String::new(),
            updated: String::new(),
        };
        // Without its review's folder to read (review_keys' tests read one), a key resolves as every command's does
        let names =
            |keys: &[&str], wanted: &str| super::names_task(&project, None, &header(keys), wanted);
        // Its key then, an alias now: the task, by its key now; with an old review's mark too
        assert!(names(&["ATM-9"], "ATM-14"));
        assert!(names(&["~ATM-9"], "ATM-14"));
        assert!(names(&["atm-9", "ATM-12"], "ATM-14"));
        // A key a task holds now names that task, not the one that moved off it
        assert!(names(&["ATM-12"], "ATM-12"));
        assert!(!names(&["ATM-12"], "ATM-13"));
        // One the project doesn't have names no task of it
        assert!(!names(&["ATM-99"], "ATM-14"));
        assert_eq!(super::task_reference(&project, "ATM-99"), "ATM-99");
        assert_eq!(super::task_reference(&project, "~ATM-9"), "ATM-14");
    }
}
