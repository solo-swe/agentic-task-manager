//! ATM-770: an agent asks for a task's review through a code review. A move to review without a review the task waits
//! on is refused to an agent (the dashboard decides a task only through a review, so the task would wait on nobody);
//! a person may still ask, and `atm doctor` names a task stranded so.

use serde_json::json;

use crate::fixtures::{dir, open_review, project, reopen, task};
use atm_lib::Project;
use atm_lib::changes::change_task_status;
use atm_lib::reviews::{ReviewStore, Status};
use atm_lib::state::{Command, Options};

fn ask(project: &mut Project, id: &str) -> atm_common::Result<()> {
    change_task_status(project, Command::Review, id, &Options::default()).map(|_| ())
}

#[test]
fn an_agent_is_refused_a_review_with_no_code_review_and_told_how_to_open_one() {
    let dir = dir();
    let mut agent = project(&dir.0, "claude:1");
    let refused = ask(&mut agent, "T-1").unwrap_err();
    assert!(
        matches!(refused, atm_common::Error::Refused(_)),
        "{refused}"
    );
    assert!(
        refused.to_string().starts_with(
            "T-1 is reviewed through a code review: `atm review create --task T-1 --range BASE..HEAD"
        ),
        "{refused}"
    );
    assert_eq!(
        task(&agent, "T-1")["status"],
        "in_progress",
        "nothing changed"
    );
}

#[test]
fn a_review_of_another_task_or_one_decided_or_closed_is_none_the_task_waits_on() {
    let dir = dir();
    let mut agent = project(&dir.0, "claude:1");
    // T-2's review: open, but not T-1's
    let header = open_review(&mut agent, &["T-2"]);
    assert!(ask(&mut agent, "T-1").is_err());
    // A review of T-1 decided or closed: nothing waits on it
    let mut store = atm_lib::reviews::store(&agent).unwrap();
    for status in [Status::Approved, Status::Closed] {
        store
            .set_status(&header.id, status, &"Hakan".into())
            .unwrap();
        let mut agent = reopen(&dir.0, "claude:1");
        assert!(ask(&mut agent, "T-1").is_err(), "{status:?}");
    }
}

#[test]
fn opening_a_review_puts_the_task_in_review_as_before_and_a_round_after_changes_asked_for_too() {
    let dir = dir();
    let mut agent = project(&dir.0, "claude:1");
    let header = open_review(&mut agent, &["T-1"]);
    assert_eq!(task(&agent, "T-1")["status"], "in_review");
    // Changes asked for: the review is sent back, and the agent's next round asks for the review again
    let mut hakan = reopen(&dir.0, "Hakan");
    let reason = Options {
        reason: Some("CR-1 round 1: 1 open comment".into()),
        ..Options::default()
    };
    change_task_status(&mut hakan, Command::RequestChanges, "T-1", &reason).unwrap();
    atm_lib::reviews::store(&hakan)
        .unwrap()
        .set_status(&header.id, Status::ChangesRequested, &"Hakan".into())
        .unwrap();
    let mut agent = reopen(&dir.0, "claude:1");
    ask(&mut agent, "T-1").unwrap();
    assert_eq!(task(&agent, "T-1")["status"], "in_review");
}

#[test]
fn a_person_may_ask_for_a_review_without_one_and_a_project_without_a_folder_has_nothing_to_say() {
    let dir = dir();
    let mut hakan = project(&dir.0, "Hakan");
    ask(&mut hakan, "T-2").unwrap();
    assert_eq!(task(&hakan, "T-2")["status"], "in_review");
    // No folder to keep reviews in: atm can't tell, so it doesn't refuse
    let mut memory = Project::new(Box::new(
        atm_db::MemoryStore::new(
            vec![
                json!({"id": "T-1", "name": "One", "status": "in_progress"})
                    .as_object()
                    .unwrap()
                    .clone(),
            ],
            Vec::new(),
            &serde_json::Map::new(),
        )
        .unwrap(),
    ))
    .with_actor(Some("claude:1".into()));
    ask(&mut memory, "T-1").unwrap();
}

#[test]
fn doctor_names_a_task_stranded_in_review_and_not_one_a_review_waits_on() {
    let dir = dir();
    // As an atm before ATM-770 left it: an agent's task in review with no code review; a person's ask is the same
    let mut hakan = project(&dir.0, "Hakan");
    ask(&mut hakan, "T-2").unwrap();
    let document = hakan.store().document().unwrap().clone();
    let (errors, warnings) = atm_lib::reviews::check(&hakan, &document);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(
        warnings,
        [
            "T-2: in review with no code review waiting on it, so the dashboard can't decide it: \
          `atm review create --task T-2 …` opens one, or `atm task review approve|request-changes|reject T-2` decides it"
        ]
    );
    // A review waiting on it: nothing to say
    let mut agent = reopen(&dir.0, "claude:1");
    open_review(&mut agent, &["T-1", "T-2"]);
    let document = agent.store().document().unwrap().clone();
    assert_eq!(
        atm_lib::reviews::check(&agent, &document),
        (Vec::new(), Vec::new())
    );
}
