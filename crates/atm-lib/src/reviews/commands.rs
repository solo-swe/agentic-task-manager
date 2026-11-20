//! The review commands: open, round, summary, submit, close, the tasks named.

use super::{
    Command, Error, Kind, NewReview, NewRound, Opening, Options, Project, Result, Review,
    ReviewHeader, ReviewRef, ReviewStore, Reviews, Status, Verdict, Vocabulary, actor_of,
    agent_at_work, agent_of_tasks, ask_review, change_task_status, checkpoint_now, is_agent,
    key_of_id, kind_of, link, named, openings, resolve, review_keys, store,
};

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
pub(super) fn check_decision(
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
