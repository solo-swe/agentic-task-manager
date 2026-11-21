//! The review commands: open, round, summary, submit, close, the tasks named.

use crate::lifecycle::Event;

use super::{
    Command, Error, Kind, NewReview, NewRound, Options, Project, Result, ReviewHeader, ReviewRef,
    ReviewStore, Reviews, Status, Verdict, Vocabulary, actor_of, agent_at_work, agent_of_tasks,
    ask_review, change_task_status, checkpoint_now, is_agent, key_of_id, link, resolve,
    review_keys, store,
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
    // An agent opening a review of its work: every task checked first, through the machine (ATM-631, ATM-712). A
    // person may open one of files for a task not started yet, for an agent to answer when it takes the task (the
    // sandbox's red-team review): it is linked, as it is
    let taken_back = crate::lifecycle::check_open(project, new.status, &ids)?;
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
    // Checked whole first (ATM-633, ATM-594): a round is never kept without its tasks going back in review
    let before = reviews.get_review(reference)?;
    let outcome = crate::lifecycle::check(project, &before, &Event::Round, &round.by)?;
    let number = Reviews::new(&mut reviews).add_round(reference, round)?;
    let header = reviews.get_review(reference)?.header;
    // A draft waits on no one yet: its tasks stay as they are (ATM-713)
    if header.status == Status::Draft {
        return Ok(number);
    }
    // The author's own task its lapse sent back to to do, started again first (the design's hole 1); its review note
    // says so, as opening a review does
    for id in &outcome.take_back {
        change_task_status(project, Command::Start, id, &Options::default())?;
    }
    let (back, rest): (Vec<String>, Vec<String>) = outcome
        .named
        .into_iter()
        .partition(|id| outcome.take_back.contains(id));
    ask_review(project, &rest, &header, &format!("round {number}"))?;
    let lapsed = format!("round {number} (its claim had lapsed: taken back for this round)");
    ask_review(project, &back, &header, &lapsed)?;
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
    // Checked whole first (ATM-633): the review's rules, and every task the decision changes, found (ATM-594) and
    // able to move (ATM-670): all or none
    let before = reviews.get_review(reference)?;
    let event = Event::Submit {
        verdict,
        overriding,
        also_open: &[],
    };
    let outcome = crate::lifecycle::check(project, &before, &event, &by)?;
    // A live review is approved as the working copy is now (ATM-576): that is the checkpoint its task's change is
    // compared with when it is done
    if verdict == Verdict::Approve && before.header.status == Status::Live {
        checkpoint_now(project, reference)?;
    }
    Reviews::new(&mut reviews).submit(reference, verdict, body, overriding, &by)?;
    let options = crate::lifecycle::decision_options(&reviews.get_review(reference)?);
    for (id, command) in outcome.tasks {
        change_task_status(project, command, &id, &options)?;
    }
    Ok(())
}

/// Whether `command` may change each of `ids` in review, checked through the state machine on a copy before anything
/// is written (ATM-670, audit R7): a decision was recorded on the review, then the second task's change failed, and
/// the two were left split. A task not in review is left as it is, as [`submit`] does.
///
/// # Errors
/// The first task the state machine refuses, naming it.
pub(crate) fn check_decision(
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
    // Checked whole first (ATM-633): with `reject`, every task it rejects found and able to move, before it closes
    let before = reviews.get_review(reference)?;
    let outcome = crate::lifecycle::check(project, &before, &Event::Close { reject }, &by)?;
    Reviews::new(&mut reviews).close(reference, reason, &by)?;
    let options = Options {
        reason: Some(format!("{} closed: {reason}", before.header.reference)),
        ..Options::default()
    };
    for (id, command) in outcome.tasks {
        change_task_status(project, command, &id, &options)?;
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
    let before = reviews.get_review(reference)?;
    let header = before.header.clone();
    let (added, removed) = (resolve(project, add)?, resolve(project, remove)?);
    // Checked whole first (ATM-712): the review's rules, then each added task as a new review checks its tasks
    // (ATM-631), so it is never named beside a task that can't follow it
    let taken_back = crate::lifecycle::check_set_tasks(project, &before, &added, &by)?;
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
    for id in &taken_back {
        change_task_status(project, Command::Start, id, &Options::default())?;
    }
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
