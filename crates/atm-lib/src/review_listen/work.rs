//! What is the agent's work: the tasks it held and their files, so a comment or a decision is told to the agent it is about.

use super::{
    Chain, Comment, FileStore, HashMap, Kind, Path, PathBuf, Project, Review, ReviewHeader, Status,
    Value, reviews, sha256_hex,
};

/// What the agent works on, as atm's records have it now.
pub(super) struct Work {
    /// The task it holds now, its key.
    pub(super) held: Option<String>,
    /// Each task it has held (started or finished), its key and the files it touched (declared, its commit's).
    pub(super) tasks: Vec<(String, Vec<String>)>,
    /// Every other task's declared files, by key: a batch review's file another task names is that task's.
    pub(super) others: Vec<(String, Vec<String>)>,
    /// The working copy's uncommitted files.
    pub(super) uncommitted: Vec<String>,
}

pub(super) fn declared_files(task: &atm_common::Task) -> Vec<String> {
    task.get("files")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(String::from)
        .collect()
}

/// The files each commit changed, as git said once (ATM-708): a commit never changes, and asking git again for every
/// task on every look made a look take seconds. In atm's state folder, a file per project.
pub(super) fn commit_files_file(root: Option<&Path>) -> PathBuf {
    let key = root.map(|r| r.display().to_string()).unwrap_or_default();
    atm_common::home::state_dir().join("watches").join(format!(
        "{}.commits.json",
        &sha256_hex(key.as_bytes())[..16]
    ))
}

pub(super) fn work_of(project: &Project, me: &str) -> Work {
    let mut work = Work {
        held: None,
        tasks: Vec::new(),
        others: Vec::new(),
        uncommitted: Vec::new(),
    };
    let Ok(vocab) = project.store().vocabulary() else {
        return work;
    };
    let vcs = reviews::vcs(project).ok();
    let known_file = commit_files_file(project.root());
    let mut known: HashMap<String, Vec<String>> = std::fs::read_to_string(&known_file)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    let mut learned = false;
    for task in project.store().list_tasks().unwrap_or_default() {
        let by = |field: &str| task.get(field).and_then(Value::as_str) == Some(me);
        // As a review names it: its key, never with the `~` people read (ATM-594)
        let key = project.task_key(task);
        if !(by("started_by") || by("completed_by")) {
            work.others.push((key, declared_files(task)));
            continue;
        }
        if vocab.kind_of(task.get("status")) == Some(Kind::Active) && by("started_by") {
            work.held = Some(key.clone());
        }
        let mut files = declared_files(task);
        let commit = task.get("commit").and_then(Value::as_str);
        if let Some(changed) = commit.and_then(|c| known.get(c)) {
            files.extend(changed.iter().cloned());
        } else if let (Some(vcs), Some(commit)) = (&vcs, commit)
            // The task's commit is task text: a revision only once checked (ATM-659)
            && let (Ok(parent), Ok(rev)) = (
                atm_vcs::Rev::new(&format!("{commit}^")),
                atm_vcs::Rev::new(commit),
            )
            && let Ok(diff) = vcs.diff_range(&parent, &rev, &[atm_common::TASK_DIR])
        {
            let changed: Vec<String> = diff.files.into_iter().map(|f| f.path).collect();
            files.extend(changed.iter().cloned());
            // Kept only for a commit named by its full ID: a name could move
            if atm_vcs::is_object_id(commit) {
                known.insert(commit.to_string(), changed);
                learned = true;
            }
        }
        work.tasks.push((key, files));
    }
    if learned && let (Some(dir), Ok(text)) = (known_file.parent(), serde_json::to_string(&known)) {
        let _ = std::fs::create_dir_all(dir);
        let _ = atm_common::home::write_text_atomic(&known_file, &text);
    }
    if let Some(vcs) = &vcs
        && let Ok(diff) = vcs.diff_working(&atm_vcs::Rev::head(), &[atm_common::TASK_DIR])
    {
        work.uncommitted = diff.files.into_iter().map(|f| f.path).collect();
    }
    work
}

/// Whether `path` is one of `files` (a file named, or under a folder named with a trailing `/`).
pub(super) fn touches(files: &[String], path: &str) -> bool {
    files
        .iter()
        .any(|f| f == path || (f.ends_with('/') && path.starts_with(f.as_str())))
}

/// Whether a comment is about the agent's work.
pub(super) enum Mine {
    /// Not the agent's.
    No,
    /// The agent's, about this task, or (none) about no task of its.
    About(Option<String>),
}

/// Whether a comment in `chain` of `review` is about the agent's `work`, and the task it is about (§3.9's table).
pub(super) fn about_my_work(
    work: &Work,
    me: &str,
    review: &Review,
    chain: &Chain,
    comment: &Comment,
) -> Mine {
    use Mine::{About, No};
    let path = chain.anchor.path.as_deref();
    let task_of_path = |path: Option<&str>| {
        path.and_then(|p| {
            work.tasks
                .iter()
                .find(|(_, files)| touches(files, p))
                .map(|(key, _)| key.clone())
        })
    };
    let mine: Vec<&String> = work
        .tasks
        .iter()
        .map(|(key, _)| key)
        .filter(|key| review.header.tasks.contains(key))
        .collect();
    // A live review: everything in it, the task whose work touched the file, else the one held now, else none
    if review.header.status == Status::Live {
        let task = task_of_path(path).or_else(|| {
            work.held
                .clone()
                .filter(|_| path.is_none_or(|p| work.uncommitted.iter().any(|u| u == p)))
        });
        return About(task);
    }
    if mine.is_empty() {
        return No;
    }
    // A review of one of its tasks alone: everything in it
    if review.header.tasks.len() == 1 {
        return About(Some(mine[0].clone()));
    }
    // A batch review: its files, its tasks named, its own chains. A file another task it names says it touches is that
    // task's, whoever's working copy it is changed in
    if let Some(task) = task_of_path(path).filter(|t| review.header.tasks.contains(t)) {
        return About(Some(task));
    }
    let anothers = path.is_some_and(|p| {
        work.others
            .iter()
            .any(|(key, files)| review.header.tasks.contains(key) && touches(files, p))
    });
    if !anothers && path.is_some_and(|p| work.uncommitted.iter().any(|u| u == p)) {
        return About(work.held.clone());
    }
    if let Some(task) = mine.iter().find(|key| comment.body.contains(key.as_str())) {
        return About(Some((*task).clone()));
    }
    let wrote_in = chain
        .comments
        .iter()
        .any(|c| c.all().any(|x| x.author.0 == me));
    if wrote_in { About(None) } else { No }
}

/// A review's tasks as they are keyed now (ATM-630): a review written before a task's number moved at a merge names
/// the key it had (an alias now), an older one `~KEY` or an earlier prefix's; each resolved as every command resolves
/// one, so the agent's work is found whatever key its review was written with; an old key another task holds now as
/// the review in `store` meant it ([`reviews::key_now`]). One the project doesn't have stays.
pub(super) fn keyed_now(project: &Project, store: &FileStore, header: &mut ReviewHeader) {
    header.tasks = header
        .tasks
        .iter()
        .map(|key| reviews::key_now(project, Some(store), header, key))
        .collect();
}

/// Whether a decided review is of the agent's work, and the task it decides: its own review, or one naming a task it
/// has held (ATM-600).
pub(super) fn decision_about_my_work(work: &Work, me: &str, review: &ReviewHeader) -> Mine {
    let task = work
        .tasks
        .iter()
        .map(|(key, _)| key)
        .find(|key| review.tasks.contains(key))
        .cloned();
    let own = review.author.as_ref().is_some_and(|a| a.0 == me);
    if task.is_some() || own {
        Mine::About(task)
    } else {
        Mine::No
    }
}
