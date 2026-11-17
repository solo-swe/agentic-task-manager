//! What every [`ReviewStore`] promises (docs/code-review-design.md §3.10), whatever holds the reviews: each store's
//! tests run these on a fresh store of theirs, so the memory store tests stand in for the folder's faithfully.

use atm_common::Error;

use crate::model::{
    Actor, Anchor, Chain, CommentId, Patch, ReviewHeader, ReviewId, ReviewRef, RoundKind, Status,
};
use crate::store::{CommentFilter, NewReview, NewRound, ReviewFilter, ReviewStore};

fn agent() -> Actor {
    Actor::from("claude:0f56a916")
}

fn hakan() -> Actor {
    Actor::from("Hakan Serce")
}

fn round(patch: &str) -> NewRound {
    NewRound {
        kind: RoundKind::Working { base: "a1".into() },
        files: Vec::new(),
        patch: Patch(patch.into()),
        by: agent(),
    }
}

fn new_review(title: &str, status: Status, tasks: &[&str]) -> NewReview {
    NewReview {
        title: title.into(),
        summary: "Why and what.".into(),
        tasks: tasks.iter().map(|t| (*t).to_string()).collect(),
        author: Some(agent()),
        reviewers: vec![hakan()],
        status,
        first: round("diff --git a/x b/x\n+one\n"),
    }
}

fn anchor() -> Anchor {
    Anchor {
        round: 1,
        path: Some("src/a.rs".into()),
        lines: Some((3, 4)),
        quote: Some("let shell = 0;".into()),
        ..Anchor::default()
    }
}

/// An empty store lists nothing and finds nothing.
pub(crate) fn an_empty_store_lists_and_finds_nothing(store: &mut dyn ReviewStore) {
    assert_eq!(
        store.list_reviews(&ReviewFilter::default()).unwrap(),
        Vec::<ReviewHeader>::new()
    );
    assert!(matches!(
        store.get_review(&ReviewRef::numbered(1)),
        Err(Error::NotFound(_))
    ));
}

/// A new review is numbered after the last, keeps what it was given, and its first round with its diff.
pub(crate) fn a_new_review_is_kept_numbered_with_its_first_round(store: &mut dyn ReviewStore) {
    let first = store
        .create_review(new_review("One", Status::Draft, &["ATM-1"]))
        .unwrap();
    let second = store
        .create_review(new_review("Two", Status::Live, &[]))
        .unwrap();
    assert_eq!(
        (
            first.number,
            &first.reference,
            second.number,
            &second.reference
        ),
        (1, &ReviewRef::numbered(1), 2, &ReviewRef::numbered(2))
    );
    assert_ne!(first.id, second.id);
    let review = store.get_review(&ReviewRef::numbered(1)).unwrap();
    assert_eq!(review.header, first);
    assert_eq!(
        (
            review.header.title.as_str(),
            review.header.status,
            review.summary.as_str()
        ),
        ("One", Status::Draft, "Why and what.")
    );
    assert_eq!(review.header.tasks, ["ATM-1"]);
    assert_eq!(review.header.author, Some(agent()));
    assert_eq!(review.header.reviewers, [hakan()]);
    assert_eq!(review.chains, Vec::<Chain>::new());
    assert_eq!(review.rounds.len(), 1);
    let round = &review.rounds[0];
    assert_eq!(
        (round.number, &round.by, &round.kind),
        (1, &agent(), &RoundKind::Working { base: "a1".into() })
    );
    assert_eq!(
        store.round_diff(&first.id, 1).unwrap().0,
        "diff --git a/x b/x\n+one\n"
    );
    assert!(matches!(
        store.round_diff(&first.id, 2),
        Err(Error::NotFound(_))
    ));
    // Found by its ID as well as its reference
    assert_eq!(
        store
            .get_review(&ReviewRef(second.id.0.clone()))
            .unwrap()
            .header
            .title,
        "Two"
    );
}

/// Reviews are listed newest first, by status (one or several) and by task.
pub(crate) fn reviews_are_listed_newest_first_and_filtered(store: &mut dyn ReviewStore) {
    store
        .create_review(new_review("One", Status::Open, &["ATM-1"]))
        .unwrap();
    store
        .create_review(new_review("Two", Status::Draft, &["ATM-1", "ATM-2"]))
        .unwrap();
    store
        .create_review(new_review("Three", Status::Live, &[]))
        .unwrap();
    let titles = |filter: ReviewFilter| -> Vec<String> {
        store
            .list_reviews(&filter)
            .unwrap()
            .into_iter()
            .map(|h| h.title)
            .collect()
    };
    for (filter, listed) in [
        (ReviewFilter::default(), vec!["Three", "Two", "One"]),
        (
            ReviewFilter {
                statuses: vec![Status::Open],
                task: None,
            },
            vec!["One"],
        ),
        (
            ReviewFilter {
                statuses: vec![Status::Open, Status::Live],
                task: None,
            },
            vec!["Three", "One"],
        ),
        (
            ReviewFilter {
                statuses: Vec::new(),
                task: Some("ATM-1".into()),
            },
            vec!["Two", "One"],
        ),
        (
            ReviewFilter {
                statuses: vec![Status::Draft],
                task: Some("ATM-1".into()),
            },
            vec!["Two"],
        ),
        (
            ReviewFilter {
                statuses: vec![Status::Closed],
                task: None,
            },
            vec![],
        ),
        (
            ReviewFilter {
                statuses: Vec::new(),
                task: Some("ATM-9".into()),
            },
            vec![],
        ),
    ] {
        assert_eq!(titles(filter.clone()), listed, "{filter:?}");
    }
}

/// A review's header and summary are changed as asked, and its rounds added after the last.
pub(crate) fn a_reviews_header_and_rounds_are_changed_as_asked(store: &mut dyn ReviewStore) {
    let mut unknown = new_review("One", Status::Open, &["ATM-1"]);
    unknown.author = None;
    let id = store.create_review(unknown).unwrap().id;
    store
        .set_status(&id, Status::ChangesRequested, &hakan())
        .unwrap();
    store.edit_summary(&id, "Retitled", "New summary.").unwrap();
    store
        .set_tasks(&id, vec!["ATM-2".into(), "ATM-3".into()])
        .unwrap();
    store.set_author(&id, &agent()).unwrap();
    assert_eq!(store.add_round(&id, round("+two\n")).unwrap(), 2);
    assert_eq!(store.add_round(&id, round("+three\n")).unwrap(), 3);
    let review = store.get_review(&ReviewRef::numbered(1)).unwrap();
    assert_eq!(
        (
            review.header.status,
            review.header.title.as_str(),
            review.summary.as_str()
        ),
        (Status::ChangesRequested, "Retitled", "New summary.")
    );
    assert_eq!(review.header.tasks, ["ATM-2", "ATM-3"]);
    assert_eq!(review.header.author, Some(agent()));
    assert!(review.header.updated >= review.header.created);
    assert_eq!(
        review.rounds.iter().map(|r| r.number).collect::<Vec<_>>(),
        [1, 2, 3]
    );
    assert_eq!(store.round_diff(&id, 3).unwrap().0, "+three\n");
    // An empty task list is kept as empty
    store.set_tasks(&id, Vec::new()).unwrap();
    assert_eq!(
        store
            .get_review(&ReviewRef::numbered(1))
            .unwrap()
            .header
            .tasks,
        Vec::<String>::new()
    );
}

/// Chains are started, replied to, edited, resolved and listed, and a missing comment isn't found.
pub(crate) fn comments_are_kept_replied_to_edited_and_resolved(store: &mut dyn ReviewStore) {
    let id = store
        .create_review(new_review("One", Status::Open, &[]))
        .unwrap()
        .id;
    let question = store.add_comment(&id, anchor(), "Why?", &hakan()).unwrap();
    let answer = store.reply(&id, &question, "Because.", &agent()).unwrap();
    let note = store
        .add_comment(&id, Anchor::default(), "Overall fine.", &hakan())
        .unwrap();
    assert_eq!(
        (question.0.as_str(), answer.0.as_str(), note.0.as_str()),
        ("c1", "c2", "c3")
    );
    let reply = store.get_comment(&id, &answer).unwrap();
    assert_eq!(
        (reply.body.as_str(), reply.resolved, &reply.author),
        ("Because.", None, &agent())
    );
    store
        .edit_comment(&id, &answer, "Because, edited.")
        .unwrap();
    store.resolve(&id, &note, true).unwrap();
    assert_eq!(
        store.get_comment(&id, &answer).unwrap().body,
        "Because, edited."
    );
    let chains = store.list_comments(&id, &CommentFilter::default()).unwrap();
    assert_eq!(chains.len(), 2);
    assert_eq!(chains[0].anchor, anchor());
    assert_eq!(chains[0].comments[0].replies[0].id, answer);
    let open = store
        .list_comments(&id, &CommentFilter { open_only: true })
        .unwrap();
    assert_eq!(
        open.iter().map(|c| c.id.0.as_str()).collect::<Vec<_>>(),
        ["t1"]
    );
    // Reopened
    store.resolve(&id, &note, false).unwrap();
    assert_eq!(store.get_comment(&id, &note).unwrap().resolved, Some(false));
    // What isn't there, or isn't a top-level comment
    let missing = CommentId("c9".into());
    assert!(matches!(
        store.get_comment(&id, &missing),
        Err(Error::NotFound(_))
    ));
    assert!(matches!(
        store.reply(&id, &missing, "x", &agent()),
        Err(Error::NotFound(_))
    ));
    assert!(matches!(
        store.edit_comment(&id, &missing, "x"),
        Err(Error::NotFound(_))
    ));
    assert!(matches!(
        store.resolve(&id, &answer, true),
        Err(Error::NotFound(_))
    ));
}

/// Every method on a review the store hasn't is `NotFound`.
pub(crate) fn every_method_on_a_review_it_hasnt_is_not_found(store: &mut dyn ReviewStore) {
    store
        .create_review(new_review("One", Status::Open, &[]))
        .unwrap();
    let id = ReviewId("0193a8f2-0000-7000-8000-00000000dead".into());
    let c1 = CommentId("c1".into());
    let results = [
        ("set_status", store.set_status(&id, Status::Open, &hakan())),
        ("edit_summary", store.edit_summary(&id, "t", "s")),
        ("set_tasks", store.set_tasks(&id, Vec::new())),
        ("set_author", store.set_author(&id, &agent())),
        ("add_round", store.add_round(&id, round("")).map(|_| ())),
        ("round_diff", store.round_diff(&id, 1).map(|_| ())),
        (
            "list_comments",
            store
                .list_comments(&id, &CommentFilter::default())
                .map(|_| ()),
        ),
        ("get_comment", store.get_comment(&id, &c1).map(|_| ())),
        (
            "add_comment",
            store
                .add_comment(&id, Anchor::default(), "x", &hakan())
                .map(|_| ()),
        ),
        ("reply", store.reply(&id, &c1, "x", &hakan()).map(|_| ())),
        ("edit_comment", store.edit_comment(&id, &c1, "x")),
        ("resolve", store.resolve(&id, &c1, true)),
    ];
    for (method, result) in results {
        assert!(matches!(result, Err(Error::NotFound(_))), "{method}");
    }
    // The review it has is untouched
    let review = store.get_review(&ReviewRef::numbered(1)).unwrap();
    assert_eq!((review.rounds.len(), review.chains.len()), (1, 0));
}
