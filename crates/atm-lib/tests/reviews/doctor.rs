//! What `atm doctor` says about reviews.

use serde_json::json;

use crate::fixtures::{dir, keyed, open_review, project, reopen};
use atm_lib::Project;
use atm_lib::reviews::{ReviewStore, Status};

#[test]
fn a_project_without_a_folder_or_git_has_no_reviews() {
    let memory = Project::new(Box::new(
        atm_db::MemoryStore::new(Vec::new(), Vec::new(), &serde_json::Map::new()).unwrap(),
    ));
    assert_eq!(
        atm_lib::reviews::store(&memory).unwrap_err().to_string(),
        "this project has no folder to keep reviews in"
    );
    assert_eq!(
        atm_lib::reviews::vcs(&memory).unwrap_err().to_string(),
        "reviews are of changes in git: this project isn't in a git repository"
    );
    // nothing to check either
    assert_eq!(
        atm_lib::reviews::check(&memory, &json!({"tasks": []})),
        (Vec::new(), Vec::new())
    );
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
    let (errors, warnings) = atm_lib::reviews::check(&project, &document);
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
fn doctor_names_a_decided_review_whose_task_still_waits_and_the_command_that_records_it() {
    let dir = dir();
    let mut project = keyed(&dir.0, "claude:1");
    let header = open_review(&mut project, &["ATM-12", "ATM-11"]);
    // As an atm before ATM-594 left it: the review approved, its decision not on the tasks
    let mut store = atm_lib::reviews::store(&project).unwrap();
    store
        .set_status(&header.id, Status::Approved, &"Hakan".into())
        .unwrap();
    let document = project.store().document().unwrap().clone();
    let (errors, warnings) = atm_lib::reviews::check(&project, &document);
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
        atm_lib::changes::change_task_status(
            &mut hakan,
            atm_lib::state::Command::Approve,
            &id,
            &atm_lib::state::Options::default(),
        )
        .unwrap();
    }
    let document = hakan.store().document().unwrap().clone();
    assert_eq!(
        atm_lib::reviews::check(&hakan, &document),
        (Vec::new(), Vec::new())
    );
}
