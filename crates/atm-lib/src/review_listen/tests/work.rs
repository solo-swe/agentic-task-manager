//! What is the agent's work: which comments and decisions are about it.

use super::*;

/// ATM-600: a decision is the agent's when the review is its own or names a task it has held: the task it decides
/// (the first of its own the review names), or none for its own review of no task of its
#[test]
fn a_decision_is_about_the_agents_work_when_the_review_is_its_own_or_names_its_task() {
    let mut theirs = review(Status::Approved, &["ATM-3"]).header;
    theirs.author = Some(Actor::from("claude:other"));
    for (header, expected) in [
        (
            review(Status::Approved, &["ATM-2"]).header,
            Some(Some("ATM-2")),
        ),
        (
            review(Status::Approved, &["ATM-3", "ATM-1"]).header,
            Some(Some("ATM-1")),
        ),
        (review(Status::Closed, &[]).header, Some(None)),
        (theirs.clone(), None),
        (
            ReviewHeader {
                tasks: vec!["ATM-1".into()],
                ..theirs
            },
            Some(Some("ATM-1")),
        ),
    ] {
        let got = match decision_about_my_work(&work(), "claude:me", &header) {
            Mine::No => None,
            Mine::About(task) => Some(task),
        };
        assert_eq!(got, expected.map(|t| t.map(String::from)), "{header:?}");
    }
}

#[test]
fn a_file_is_touched_when_named_or_under_a_folder_named() {
    let list = files(&["src/a.rs", "docs/"]);
    for (path, expected) in [
        ("src/a.rs", true),
        ("docs/x.md", true),
        ("docs/deep/y.md", true),
        ("src/a.rs.bak", false),
        ("src/", false),
        ("docs", false),
        ("other.rs", false),
    ] {
        assert_eq!(touches(&list, path), expected, "{path}");
    }
    assert!(!touches(&[], "a"));
}

// ATM-624: where a watch was, kept for the next

#[test]
fn a_live_review_is_the_agents_about_the_task_that_touched_the_file_else_the_one_held() {
    let live = review(Status::Live, &[]);
    for (path, task) in [
        (Some("src/a.rs"), "ATM-1"),
        (Some("docs/guide.md"), "ATM-2"),
        // a file changed in its copy: the task it holds now
        (Some("src/c.rs"), "ATM-1"),
        // the review as a whole: the task held
        (None, "ATM-1"),
        // a file neither touched nor changed: about no task, still the agent's
        (Some("README.md"), NO_TASK),
    ] {
        assert_eq!(about(&live, &by_hakan(path, "x")), task, "{path:?}");
    }
}

#[test]
fn a_review_of_none_of_the_agents_tasks_isnt_its() {
    let theirs = review(Status::Waiting, &["ATM-3"]);
    assert_eq!(
        about(&theirs, &by_hakan(Some("src/a.rs"), "ATM-1")),
        NOT_MINE
    );
}

#[test]
fn a_review_of_one_of_its_tasks_alone_is_all_about_that_task() {
    let one = review(Status::ChangesRequested, &["ATM-2"]);
    for path in [Some("anything.rs"), None] {
        assert_eq!(about(&one, &by_hakan(path, "x")), "ATM-2", "{path:?}");
    }
}

#[test]
fn in_a_batch_review_a_comment_is_the_agents_by_file_by_task_named_or_by_its_own_chain() {
    let batch = review(Status::Waiting, &["ATM-1", "ATM-2", "ATM-3"]);
    for (path, body, expected) in [
        // a file its task touched
        (Some("docs/a.md"), "x", "ATM-2"),
        // a file changed in its copy, which no other named task claims: the task held
        (Some("src/c.rs"), "x", "ATM-1"),
        // a file another named task declares is that task's, even changed here
        (Some("src/b.rs"), "x", NOT_MINE),
        // its task named in the comment
        (None, "see ATM-2 again", "ATM-2"),
        (Some("other.rs"), "nothing of its", NOT_MINE),
    ] {
        assert_eq!(
            about(&batch, &by_hakan(path, body)),
            expected,
            "{path:?} {body}"
        );
    }
    // a chain it wrote in is its, about no task
    let own = chain(
        Some("other.rs"),
        vec![comment(
            "c1",
            "Hakan",
            "x",
            vec![comment("c2", "claude:me", "y", Vec::new())],
        )],
    );
    assert_eq!(about(&batch, &own), NO_TASK);
}
