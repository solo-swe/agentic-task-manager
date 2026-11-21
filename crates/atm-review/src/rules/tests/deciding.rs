//! Who decides a review and how: its reviewers, an override with its reason, approving, sending back, a comment on the whole.

use super::*;

#[test]
fn a_review_is_decided_by_its_reviewers_or_with_none_by_anyone_but_its_author() {
    let mut store = MemoryStore::new();
    let header = Reviews::new(&mut store)
        .create(new_review(Status::Waiting))
        .unwrap();
    assert!(decides(&header, &hakan()));
    assert!(!decides(&header, &agent()));
    assert!(!decides(&header, &Actor::from("someone else")));
    let mut unnamed = header;
    unnamed.reviewers.clear();
    assert!(decides(&unnamed, &Actor::from("someone else")));
    assert!(!decides(&unnamed, &agent()));
}

#[test]
fn a_person_may_override_who_decides_saying_why_and_it_is_recorded() {
    let mut store = MemoryStore::new();
    let mut reviews = Reviews::new(&mut store);
    // Hakan's own review, no reviewers named: by the rules, not his to decide
    let mut own = new_review(Status::Waiting);
    own.author = Some(hakan());
    own.reviewers.clear();
    reviews.create(own).unwrap();
    let refused = reviews
        .submit(&cr(1), Verdict::Approve, None, None, &hakan())
        .unwrap_err()
        .to_string();
    assert!(refused.contains("may override"), "{refused}");
    // An override needs its reason
    assert!(
        reviews
            .submit(&cr(1), Verdict::Approve, None, Some("  "), &hakan())
            .is_err()
    );
    reviews
        .submit(
            &cr(1),
            Verdict::Approve,
            None,
            Some("Ana reviewed it on a call; her notes are in the chains."),
            &hakan(),
        )
        .unwrap();
    let review = reviews.store().get_review(&cr(1)).unwrap();
    assert_eq!(review.header.status, Status::Approved);
    let record = &review.chains.last().unwrap().comments[0];
    assert_eq!(record.author, hakan());
    assert_eq!(record.resolved, Some(true));
    assert!(
        record.body.contains("overridden")
            && record.body.contains("its author")
            && record.body.contains("Ana reviewed it"),
        "{}",
        record.body
    );

    // It lifts who may decide, nothing else: an approval still needs every comment resolved
    let mut other = new_review(Status::Waiting);
    other.author = Some(hakan());
    other.reviewers.clear();
    reviews.create(other).unwrap();
    reviews
        .comment(&cr(2), on("a.rs", 1), "Still open.", &agent())
        .unwrap();
    assert!(
        reviews
            .submit(
                &cr(2),
                Verdict::Approve,
                None,
                Some("Decided elsewhere."),
                &hakan()
            )
            .unwrap_err()
            .to_string()
            .contains("comments are open")
    );
    // A reviewer who may decide records no override
    reviews.create(new_review(Status::Waiting)).unwrap();
    reviews
        .submit(
            &cr(3),
            Verdict::Approve,
            None,
            Some("Not needed."),
            &hakan(),
        )
        .unwrap();
    assert_eq!(
        reviews.store().get_review(&cr(3)).unwrap().chains.len(),
        0,
        "no override recorded"
    );
}

#[test]
fn sent_back_fixed_in_a_round_then_approved_once_every_comment_is_resolved() {
    let mut store = MemoryStore::new();
    let mut reviews = Reviews::new(&mut store);
    reviews.create(new_review(Status::Waiting)).unwrap();
    let question = reviews
        .comment(
            &cr(1),
            on("crates/atm-cli/src/process.rs", 1),
            "Find the parent instead?",
            &hakan(),
        )
        .unwrap();
    let answer = reviews
        .reply(&cr(1), &question, "Done in round 2.", &agent())
        .unwrap();

    // The agent can't resolve the reviewer's question, nor decide its own review
    assert!(reviews.resolve(&cr(1), &question, true, &agent()).is_err());
    assert!(
        reviews
            .submit(&cr(1), Verdict::Approve, None, None, &agent())
            .is_err()
    );

    reviews
        .submit(
            &cr(1),
            Verdict::RequestChanges,
            Some("One more thing."),
            None,
            &hakan(),
        )
        .unwrap();
    assert_eq!(status(reviews.store(), 1), Status::ChangesRequested);
    assert_eq!(reviews.add_round(&cr(1), round(&agent())).unwrap(), 2);
    assert_eq!(status(reviews.store(), 1), Status::Waiting);

    // Approving names what is still open: the question and the request's own words
    let refused = reviews
        .submit(&cr(1), Verdict::Approve, None, None, &hakan())
        .unwrap_err()
        .to_string();
    assert!(
        refused.contains("c1") && refused.contains("c3"),
        "{refused}"
    );
    // A reply has no box of its own: its top-level comment is resolved
    let reply = reviews
        .resolve(&cr(1), &answer, true, &hakan())
        .unwrap_err()
        .to_string();
    assert!(
        reply.contains("is a reply") && reply.contains("c1"),
        "{reply}"
    );
    reviews.resolve(&cr(1), &question, true, &hakan()).unwrap();
    let request = reviews
        .store()
        .get_review(&cr(1))
        .unwrap()
        .open_comments()
        .next()
        .unwrap()
        .id
        .clone();
    reviews.resolve(&cr(1), &request, true, &hakan()).unwrap();
    reviews
        .submit(&cr(1), Verdict::Approve, Some("Good."), None, &hakan())
        .unwrap();
    assert_eq!(status(reviews.store(), 1), Status::Approved);

    // An approved review takes no more comments or rounds
    assert!(
        reviews
            .comment(&cr(1), on("x", 1), "late", &hakan())
            .is_err()
    );
    assert!(reviews.add_round(&cr(1), round(&agent())).is_err());
}

#[test]
fn whose_change_it_is_and_who_decides_when_no_author_is_known() {
    let mut store = MemoryStore::new();
    let mut header = Reviews::new(&mut store)
        .create(new_review(Status::Waiting))
        .unwrap();
    assert!(is_author(&header, &agent()) && !is_author(&header, &hakan()));
    header.author = None;
    header.reviewers.clear();
    // Nobody is the author while it isn't known, so anyone decides
    assert!(!is_author(&header, &agent()));
    assert!(decides(&header, &agent()) && decides(&header, &hakan()));
}

/// ATM-612: a decision checked before a reviewer's drafts are sent with it, refusing what `submit` would, with the
/// drafts counted as open, and writing nothing.
#[test]
fn a_decision_is_checked_with_drafts_counted_and_nothing_written() {
    let drafts = ["d1".to_string(), "d2".to_string()];
    let mut store = MemoryStore::new();
    let reference = at(&mut store, Status::Waiting);
    let reviews = Reviews::new(&mut store);
    // Approving with drafts that would be open comments: refused, naming them
    let message = reviews
        .check_submit(&reference, Verdict::Approve, None, &hakan(), &drafts)
        .unwrap_err()
        .to_string();
    assert_eq!(
        message,
        "CR-1 can't be approved while comments are open: d1, d2"
    );
    // Without them, or a decision drafts don't stand in the way of: taken
    for verdict in [Verdict::RequestChanges, Verdict::Comment] {
        reviews
            .check_submit(&reference, verdict, None, &hakan(), &drafts)
            .unwrap();
    }
    reviews
        .check_submit(&reference, Verdict::Approve, None, &hakan(), &[])
        .unwrap();
    // What submit refuses: its author deciding without an override, a sent-back review sent back again
    assert!(is_refused(&reviews.check_submit(
        &reference,
        Verdict::Comment,
        None,
        &agent(),
        &[]
    )));
    let review = store.get_review(&reference).unwrap();
    assert_eq!(
        (review.header.status, review.chains.len()),
        (Status::Waiting, 0)
    );
    let mut store = MemoryStore::new();
    let sent_back = at(&mut store, Status::ChangesRequested);
    assert!(is_not_now(&Reviews::new(&mut store).check_submit(
        &sent_back,
        Verdict::RequestChanges,
        None,
        &hakan(),
        &[]
    )));
}

#[test]
fn a_comment_verdict_keeps_the_status_and_leaves_its_words_open() {
    for status in [
        Status::Draft,
        Status::Waiting,
        Status::Live,
        Status::ChangesRequested,
    ] {
        let mut store = MemoryStore::new();
        let reference = at(&mut store, status);
        Reviews::new(&mut store)
            .submit(
                &reference,
                Verdict::Comment,
                Some("Looking."),
                None,
                &hakan(),
            )
            .unwrap();
        let review = store.get_review(&reference).unwrap();
        assert_eq!(review.header.status, status);
        let chain = &review.chains[0];
        assert_eq!(chain.anchor.path, None);
        assert_eq!(chain.comments[0].resolved, Some(false), "{status:?}");
    }
}

#[test]
fn a_submits_words_are_about_the_last_round_and_blank_words_are_none() {
    let mut store = MemoryStore::new();
    let mut reviews = Reviews::new(&mut store);
    reviews.create(new_review(Status::Waiting)).unwrap();
    reviews.add_round(&cr(1), round(&agent())).unwrap();
    reviews
        .submit(&cr(1), Verdict::Comment, Some(" \n"), None, &hakan())
        .unwrap();
    reviews
        .submit(&cr(1), Verdict::Comment, None, None, &hakan())
        .unwrap();
    assert_eq!(
        reviews.store().get_review(&cr(1)).unwrap().chains,
        Vec::<Chain>::new()
    );
    reviews
        .submit(
            &cr(1),
            Verdict::RequestChanges,
            Some("Fix it."),
            None,
            &hakan(),
        )
        .unwrap();
    let review = reviews.store().get_review(&cr(1)).unwrap();
    assert_eq!(review.chains[0].anchor.round, 2);
    assert_eq!(review.chains[0].comments[0].resolved, Some(false));
}

#[test]
fn who_isnt_a_decider_is_refused_every_verdict_without_an_override() {
    for verdict in [Verdict::Approve, Verdict::RequestChanges, Verdict::Comment] {
        let mut store = MemoryStore::new();
        let mut reviews = Reviews::new(&mut store);
        reviews.create(new_review(Status::Waiting)).unwrap();
        for by in [agent(), someone()] {
            assert!(
                is_refused(&reviews.submit(&cr(1), verdict, Some("x"), None, &by)),
                "{verdict:?} {by:?}"
            );
        }
        let review = reviews.store().get_review(&cr(1)).unwrap();
        assert_eq!(
            (review.header.status, review.chains.len()),
            (Status::Waiting, 0)
        );
    }
}

/// Each status and decision: allowed or refused, the status it leaves, and whether the reviewer's words are kept
#[test]
fn an_approval_is_for_an_open_live_or_sent_back_review_and_a_send_back_for_an_open_one() {
    for (status, verdict, after) in [
        (Status::Draft, Verdict::Approve, None),
        (Status::Waiting, Verdict::Approve, Some(Status::Approved)),
        (Status::Live, Verdict::Approve, Some(Status::Approved)),
        // Sent back, then approved after all (ATM-609)
        (
            Status::ChangesRequested,
            Verdict::Approve,
            Some(Status::Approved),
        ),
        (Status::Draft, Verdict::RequestChanges, None),
        (
            Status::Waiting,
            Verdict::RequestChanges,
            Some(Status::ChangesRequested),
        ),
        (Status::Live, Verdict::RequestChanges, None),
        // Sent back already: not again (CR-2); further words are a comment, below
        (Status::ChangesRequested, Verdict::RequestChanges, None),
        (Status::Draft, Verdict::Comment, Some(Status::Draft)),
        (Status::Waiting, Verdict::Comment, Some(Status::Waiting)),
        (Status::Live, Verdict::Comment, Some(Status::Live)),
        (
            Status::ChangesRequested,
            Verdict::Comment,
            Some(Status::ChangesRequested),
        ),
    ] {
        let mut store = MemoryStore::new();
        let reference = at(&mut store, status);
        let result =
            Reviews::new(&mut store).submit(&reference, verdict, Some("x"), None, &hakan());
        assert_eq!(result.is_ok(), after.is_some(), "{status:?} {verdict:?}");
        if let Some(after) = after {
            let review = store.get_review(&reference).unwrap();
            assert_eq!(review.header.status, after, "{status:?} {verdict:?}");
            assert_eq!(review.chains.len(), 1, "{status:?} {verdict:?}");
            assert_eq!(review.chains[0].comments[0].body, "x");
        } else {
            assert!(is_not_now(&result), "{status:?} {verdict:?}");
            // Refused before anything is written
            assert_eq!(
                store.get_review(&reference).unwrap().chains,
                Vec::<Chain>::new()
            );
        }
    }
}

#[test]
fn an_override_by_someone_not_a_reviewer_is_recorded_as_such() {
    let mut store = MemoryStore::new();
    let mut reviews = Reviews::new(&mut store);
    reviews.create(new_review(Status::Waiting)).unwrap();
    reviews
        .submit(
            &cr(1),
            Verdict::RequestChanges,
            Some("Please fix."),
            Some("  Hakan is away.  "),
            &someone(),
        )
        .unwrap();
    let review = reviews.store().get_review(&cr(1)).unwrap();
    assert_eq!(review.header.status, Status::ChangesRequested);
    let record = &review.chains[0].comments[0];
    assert!(
        record
            .body
            .contains("someone else is not one of its reviewers")
            && record.body.ends_with("Why: Hakan is away."),
        "{}",
        record.body
    );
    assert_eq!(record.resolved, Some(true));
    // The request's own words follow, open
    assert_eq!(review.chains[1].comments[0].body, "Please fix.");
    assert_eq!(review.chains[1].comments[0].resolved, Some(false));
}
