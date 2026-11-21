//! The review table and a review's life: how it starts, every status × event, what an ended review takes, its tasks and summary, closing.

use super::*;

#[test]
fn a_review_starts_as_a_draft_open_or_live_with_a_title_and_its_first_round() {
    let mut store = MemoryStore::new();
    let mut reviews = Reviews::new(&mut store);
    let header = reviews.create(new_review(Status::Waiting)).unwrap();
    assert_eq!(header.reference, cr(1));
    assert_eq!(header.number, 1);
    assert!(reviews.create(new_review(Status::Approved)).is_err());
    let mut untitled = new_review(Status::Waiting);
    untitled.title = "  ".into();
    assert!(reviews.create(untitled).is_err());
    assert_eq!(
        reviews.create(new_review(Status::Draft)).unwrap().reference,
        cr(2)
    );
    let review = store.get_review(&cr(1)).unwrap();
    assert_eq!(review.rounds.len(), 1);
    assert_eq!(
        store.round_diff(&review.header.id, 1).unwrap().0,
        "diff --git a/x b/x\n"
    );
}

#[test]
fn a_draft_goes_to_its_reviewers_by_its_author() {
    let mut store = MemoryStore::new();
    let mut reviews = Reviews::new(&mut store);
    reviews.create(new_review(Status::Draft)).unwrap();
    assert!(
        reviews
            .submit(&cr(1), Verdict::Approve, None, None, &hakan())
            .is_err()
    );
    assert!(reviews.ask_for_review(&cr(1), &hakan()).is_err());
    reviews.ask_for_review(&cr(1), &agent()).unwrap();
    assert_eq!(status(reviews.store(), 1), Status::Waiting);
    assert!(reviews.ask_for_review(&cr(1), &agent()).is_err());
}

#[test]
fn closed_by_its_author_or_a_reviewer_with_the_reason_kept() {
    let mut store = MemoryStore::new();
    let mut reviews = Reviews::new(&mut store);
    reviews.create(new_review(Status::Waiting)).unwrap();
    assert!(
        reviews
            .close(&cr(1), "no", &Actor::from("someone else"))
            .is_err()
    );
    reviews
        .close(&cr(1), "Replaced by CR-2.", &agent())
        .unwrap();
    let review = reviews.store().get_review(&cr(1)).unwrap();
    assert_eq!(review.header.status, Status::Closed);
    assert_eq!(review.chains[0].comments[0].body, "Replaced by CR-2.");
    assert!(review.chains[0].anchor.path.is_none());
    assert!(reviews.close(&cr(1), "again", &agent()).is_err());
}

#[test]
fn its_tasks_are_changed_by_its_author_or_a_reviewer() {
    let mut store = MemoryStore::new();
    let mut reviews = Reviews::new(&mut store);
    reviews.create(new_review(Status::Waiting)).unwrap();
    assert!(
        reviews
            .set_tasks(&cr(1), vec![], &Actor::from("someone else"))
            .is_err()
    );
    reviews
        .set_tasks(&cr(1), vec!["ATM-482".into(), "ATM-483".into()], &hakan())
        .unwrap();
    assert_eq!(
        reviews.store().get_review(&cr(1)).unwrap().header.tasks,
        ["ATM-482", "ATM-483"]
    );
}

#[test]
fn the_review_table_has_a_row_for_every_status_and_event() {
    use crate::rules::review_next;
    use std::collections::HashSet;
    let rows = review_rows();
    // Every pair has its row, once
    let mut seen = HashSet::new();
    for (status, name, _) in &rows {
        assert!(
            seen.insert((status.name(), *name)),
            "two rows for {} {name}",
            status.name()
        );
    }
    for status in TABLE_STATUSES {
        for name in TABLE_EVENTS {
            assert!(
                seen.contains(&(status.name(), name)),
                "no row for {} {name}",
                status.name()
            );
        }
    }
    for (status, name, expected) in rows {
        let mut store = MemoryStore::new();
        let reference = at(&mut store, status);
        let review = store.get_review(&reference).unwrap();
        let got = review_next(&review, &table_event(name), &table_by(name));
        match (&got, expected) {
            (Ok(next), Ok(want)) => assert_eq!(next.status, want, "{} {name}", status.name()),
            (Err(error), Err(words)) => {
                assert!(
                    error.to_string().contains(words),
                    "{} {name}: {error}",
                    status.name()
                );
            }
            _ => panic!("{} {name}: {got:?}, expected {expected:?}", status.name()),
        }
    }
}

/// ATM-633: the table reads the review it is given and writes nothing (§5.2's `can` asks it)
#[test]
fn the_review_table_writes_nothing() {
    use crate::rules::{ReviewEvent, review_next};
    let mut store = MemoryStore::new();
    let reference = at(&mut store, Status::Waiting);
    let review = store.get_review(&reference).unwrap();
    let next = review_next(
        &review,
        &ReviewEvent::Submit {
            verdict: Verdict::Approve,
            overriding: None,
            also_open: &[],
        },
        &hakan(),
    )
    .unwrap();
    assert_eq!(next.status, Some(Status::Approved));
    assert_eq!(status(&store, 1), Status::Waiting);
}

#[test]
fn only_draft_open_and_live_are_statuses_a_review_starts_in() {
    for (status, starts) in [
        (Status::Draft, true),
        (Status::Waiting, true),
        (Status::Live, true),
        (Status::ChangesRequested, false),
        (Status::Approved, false),
        (Status::Closed, false),
    ] {
        let mut store = MemoryStore::new();
        let created = Reviews::new(&mut store).create(new_review(status));
        assert_eq!(created.is_ok(), starts, "{status:?}");
        if !starts {
            assert!(matches!(created, Err(Error::Invalid(_))), "{status:?}");
            assert_eq!(
                store.list_reviews(&ReviewFilter::default()).unwrap(),
                Vec::<ReviewHeader>::new()
            );
        }
    }
}

#[test]
fn a_review_that_isnt_there_is_not_found_by_every_rule() {
    let mut store = MemoryStore::new();
    let mut reviews = Reviews::new(&mut store);
    let missing = cr(9);
    let c1 = CommentId("c1".into());
    let results = [
        reviews.ask_for_review(&missing, &agent()),
        reviews.add_round(&missing, round(&agent())).map(|_| ()),
        reviews.checkpoint(&missing, round(&agent())).map(|_| ()),
        reviews
            .comment(&missing, on("a.rs", 1), "x", &hakan())
            .map(|_| ()),
        reviews.reply(&missing, &c1, "x", &hakan()).map(|_| ()),
        reviews.edit(&missing, &c1, "x", &hakan()),
        reviews.resolve(&missing, &c1, true, &hakan()),
        reviews.submit(&missing, Verdict::Approve, None, None, &hakan()),
        reviews.close(&missing, "x", &hakan()),
        reviews.set_tasks(&missing, Vec::new(), &hakan()),
    ];
    for (i, result) in results.into_iter().enumerate() {
        assert!(matches!(result, Err(Error::NotFound(_))), "rule {i}");
    }
}

#[test]
fn an_ended_review_takes_nothing_more_and_says_where_it_stands() {
    for ended in [Status::Approved, Status::Closed] {
        let mut store = MemoryStore::new();
        let reference = at(&mut store, ended);
        let mut reviews = Reviews::new(&mut store);
        let c1 = CommentId("c1".into());
        let results = [
            reviews.add_round(&reference, round(&agent())).map(|_| ()),
            reviews
                .comment(&reference, on("a.rs", 1), "x", &hakan())
                .map(|_| ()),
            reviews.reply(&reference, &c1, "x", &hakan()).map(|_| ()),
            reviews.edit(&reference, &c1, "x", &hakan()),
            reviews.resolve(&reference, &c1, true, &hakan()),
            reviews.submit(&reference, Verdict::Comment, Some("x"), None, &hakan()),
            reviews.close(&reference, "x", &hakan()),
            reviews.set_tasks(&reference, Vec::new(), &hakan()),
        ];
        for (i, result) in results.into_iter().enumerate() {
            let message = result.unwrap_err().to_string();
            assert!(
                message.starts_with(&format!("CR-1 is {}: it can't", ended.name())),
                "{ended:?} rule {i}: {message}"
            );
        }
        let review = store.get_review(&reference).unwrap();
        assert_eq!((review.header.status, review.chains.len()), (ended, 0));
    }
}

#[test]
fn a_status_is_named_in_words_when_a_rule_says_it_cant() {
    let mut store = MemoryStore::new();
    let reference = at(&mut store, Status::ChangesRequested);
    let message = Reviews::new(&mut store)
        .checkpoint(&reference, round(&hakan()))
        .unwrap_err()
        .to_string();
    assert!(
        message.starts_with("CR-1 is changes requested: it can't take a checkpoint"),
        "{message}"
    );
}

#[test]
fn only_a_draft_is_asked_for_review() {
    for status in [Status::Waiting, Status::Live, Status::ChangesRequested] {
        let mut store = MemoryStore::new();
        let reference = at(&mut store, status);
        assert!(
            is_not_now(&Reviews::new(&mut store).ask_for_review(&reference, &agent())),
            "{status:?}"
        );
        assert_eq!(store.get_review(&reference).unwrap().header.status, status);
    }
    // Not its author's: refused, the draft stays one
    let mut store = MemoryStore::new();
    let reference = at(&mut store, Status::Draft);
    assert!(is_refused(
        &Reviews::new(&mut store).ask_for_review(&reference, &hakan())
    ));
    assert_eq!(
        store.get_review(&reference).unwrap().header.status,
        Status::Draft
    );
}

#[test]
fn closing_without_a_reason_records_no_comment_and_a_draft_or_live_review_closes_too() {
    for status in [Status::Draft, Status::Live, Status::ChangesRequested] {
        let mut store = MemoryStore::new();
        let reference = at(&mut store, status);
        Reviews::new(&mut store)
            .close(&reference, "  ", &hakan())
            .unwrap();
        let review = store.get_review(&reference).unwrap();
        assert_eq!(
            (review.header.status, review.chains.len()),
            (Status::Closed, 0)
        );
    }
    // A reviewer closes it as well as its author, the reason resolved
    let mut store = MemoryStore::new();
    let reference = at(&mut store, Status::Waiting);
    Reviews::new(&mut store)
        .close(&reference, "Not needed.", &hakan())
        .unwrap();
    let review = store.get_review(&reference).unwrap();
    assert_eq!(review.chains[0].comments[0].resolved, Some(true));
    assert_eq!(review.chains[0].comments[0].author, hakan());
}

#[test]
fn its_tasks_are_changed_by_its_author_too_even_to_none() {
    let mut store = MemoryStore::new();
    let mut reviews = Reviews::new(&mut store);
    reviews.create(new_review(Status::Draft)).unwrap();
    reviews.set_tasks(&cr(1), Vec::new(), &agent()).unwrap();
    assert_eq!(
        reviews.store().get_review(&cr(1)).unwrap().header.tasks,
        Vec::<String>::new()
    );
    assert!(is_refused(&reviews.set_tasks(
        &cr(1),
        vec!["ATM-1".into()],
        &someone()
    )));
}

/// ATM-587's review: the summary (its Description) is its author's to change, until it has ended; the title stays
#[test]
fn its_summary_is_changed_by_its_author_while_it_is_open() {
    let mut store = MemoryStore::new();
    let mut reviews = Reviews::new(&mut store);
    reviews.create(new_review(Status::Waiting)).unwrap();
    let title = reviews.store().get_review(&cr(1)).unwrap().header.title;
    reviews
        .edit_summary(&cr(1), "1. Do this.\n2. Then that.", &agent())
        .unwrap();
    let review = reviews.store().get_review(&cr(1)).unwrap();
    assert_eq!(review.summary, "1. Do this.\n2. Then that.");
    assert_eq!(review.header.title, title);
    // Not a reviewer's, nor anyone else's
    for who in [hakan(), someone()] {
        let refused = reviews.edit_summary(&cr(1), "mine", &who);
        assert!(is_refused(&refused), "{who:?}");
    }
    assert_eq!(
        reviews.store().get_review(&cr(1)).unwrap().summary,
        "1. Do this.\n2. Then that."
    );
    // Nor once it has ended
    reviews.close(&cr(1), "not needed", &agent()).unwrap();
    assert!(is_not_now(&reviews.edit_summary(&cr(1), "late", &agent())));
}

#[test]
fn a_subscription_is_told_of_every_write_until_it_is_dropped() {
    let mut store = MemoryStore::new();
    let told = Arc::new(AtomicUsize::new(0));
    let counter = told.clone();
    let subscription = store
        .subscribe(Arc::new(move || {
            counter.fetch_add(1, Ordering::Relaxed);
        }))
        .unwrap();
    let mut reviews = Reviews::new(&mut store);
    reviews.create(new_review(Status::Waiting)).unwrap();
    reviews
        .comment(&cr(1), on("a.rs", 1), "One.", &hakan())
        .unwrap();
    assert_eq!(told.load(Ordering::Relaxed), 2);
    drop(subscription);
    reviews
        .comment(&cr(1), on("a.rs", 1), "Two.", &hakan())
        .unwrap();
    assert_eq!(told.load(Ordering::Relaxed), 2);
}
