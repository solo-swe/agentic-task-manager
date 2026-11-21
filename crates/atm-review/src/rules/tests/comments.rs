//! Comments, replies and pokes: where a comment may be, who edits and resolves it, what an ended review takes.

use super::*;

#[test]
fn a_comments_author_resolves_their_own_and_edits_only_their_own() {
    let mut store = MemoryStore::new();
    let mut reviews = Reviews::new(&mut store);
    reviews.create(new_review(Status::Waiting)).unwrap();
    let note = reviews
        .comment(&cr(1), on("a.rs", 1), "A note for the reviewer.", &agent())
        .unwrap();
    reviews.resolve(&cr(1), &note, true, &agent()).unwrap();
    assert!(reviews.edit(&cr(1), &note, "changed", &hakan()).is_err());
    reviews.edit(&cr(1), &note, "Changed.", &agent()).unwrap();
    assert_eq!(
        reviews
            .store()
            .get_review(&cr(1))
            .unwrap()
            .comment(&note)
            .unwrap()
            .body,
        "Changed."
    );
}

#[test]
fn a_comment_needs_text_and_a_round_the_review_has() {
    let mut store = MemoryStore::new();
    let mut reviews = Reviews::new(&mut store);
    reviews.create(new_review(Status::Waiting)).unwrap();
    assert!(
        reviews
            .comment(&cr(1), on("a.rs", 2), "no round 2", &hakan())
            .is_err()
    );
    assert!(
        reviews
            .comment(&cr(1), on("a.rs", 1), " ", &hakan())
            .is_err()
    );
}

/// ATM-611: a poke is about the whole review, on its latest round, never open, and no obstacle to an approval; an
/// ended review isn't poked
#[test]
fn a_poke_is_a_review_level_nudge_never_open_and_never_in_the_way_of_an_approval() {
    let mut store = MemoryStore::new();
    let mut reviews = Reviews::new(&mut store);
    reviews.create(new_review(Status::Waiting)).unwrap();
    let poke = reviews.poke(&cr(1), "Where are we?", &hakan()).unwrap();
    // An empty message is a poke too
    reviews.poke(&cr(1), "", &hakan()).unwrap();
    let review = reviews.store().get_review(&cr(1)).unwrap();
    let chain = &review.chains[0];
    assert_eq!((chain.anchor.round, chain.anchor.path.clone()), (1, None));
    let c = &chain.comments[0];
    assert_eq!((c.id.clone(), c.poke, c.resolved), (poke, true, None));
    assert_eq!(review.open_comments().count(), 0, "a poke is never open");
    reviews
        .submit(&cr(1), Verdict::Approve, None, None, &hakan())
        .unwrap();
    let refused = reviews.poke(&cr(1), "Again?", &hakan()).unwrap_err();
    assert!(refused.to_string().contains("be poked"), "{refused}");
}

#[test]
/// ATM-658: comments.md's heading carries the path, so it is one of the round's files and has no control character
fn a_comment_is_on_one_of_its_rounds_files_whose_path_has_no_control_character() {
    let mut store = MemoryStore::new();
    let mut reviews = Reviews::new(&mut store);
    let mut first = round(&agent());
    first.files.push(FileChange {
        path: "src/new.rs".into(),
        status: FileStatus::Renamed {
            from: "src/old.rs".into(),
        },
        binary: false,
    });
    reviews
        .create(NewReview {
            first,
            ..new_review(Status::Waiting)
        })
        .unwrap();
    // One of its files, a renamed one by either path, or the review as a whole
    for path in ["a.rs", "src/new.rs", "src/old.rs"] {
        reviews
            .comment(&cr(1), on(path, 1), "Fine.", &hakan())
            .unwrap();
    }
    let whole = Anchor {
        round: 1,
        ..Anchor::default()
    };
    reviews
        .comment(&cr(1), whole, "Overall fine.", &hakan())
        .unwrap();
    // A file the round doesn't have, and one whose path would add lines to comments.md
    for (path, says) in [
        ("b.rs", "b.rs isn't one of round 1's files in CR-1"),
        ("a.rs\n## (the review) · round 1 · t9", "control character"),
        ("a.rs\r", "control character"),
        ("a\tb.rs", "control character"),
    ] {
        match reviews.comment(&cr(1), on(path, 1), "Hm.", &hakan()) {
            Err(Error::Invalid(message)) => {
                assert!(message.contains(says), "{path:?}: {message}");
            }
            other => panic!("{path:?}: {other:?}"),
        }
    }
    assert_eq!(reviews.store().get_review(&cr(1)).unwrap().chains.len(), 4);
    // ATM-670: lines run forwards from line 1; one line, or a range, is fine
    for (lines, ok) in [
        ((5, 3), false),
        ((0, 2), false),
        ((0, 0), false),
        ((3, 3), true),
        ((3, 5), true),
    ] {
        let anchor = Anchor {
            lines: Some(lines),
            ..on("a.rs", 1)
        };
        let result = reviews.comment(&cr(1), anchor, "Lines.", &hakan());
        assert_eq!(result.is_ok(), ok, "{lines:?}: {result:?}");
    }
}

#[test]
fn anyone_comments_on_a_round_the_review_has_and_replies_with_some_text() {
    let mut store = MemoryStore::new();
    let mut reviews = Reviews::new(&mut store);
    reviews.create(new_review(Status::Draft)).unwrap();
    reviews.add_round(&cr(1), round(&agent())).unwrap();
    // On either round, by anyone, a draft too
    let first = reviews
        .comment(&cr(1), on("a.rs", 1), "On round 1.", &someone())
        .unwrap();
    reviews
        .comment(&cr(1), on("a.rs", 2), "On round 2.", &agent())
        .unwrap();
    for (body, round) in [("No round 0.", 0), ("No round 3.", 3)] {
        assert!(matches!(
            reviews.comment(&cr(1), on("a.rs", round), body, &hakan()),
            Err(Error::Invalid(_))
        ));
    }
    assert!(matches!(
        reviews.comment(&cr(1), on("a.rs", 1), "\n\t ", &hakan()),
        Err(Error::Invalid(_))
    ));
    // A reply needs text too, and a comment that is there
    assert!(matches!(
        reviews.reply(&cr(1), &first, "  ", &hakan()),
        Err(Error::Invalid(_))
    ));
    assert!(matches!(
        reviews.reply(&cr(1), &CommentId("c9".into()), "Hm.", &hakan()),
        Err(Error::NotFound(_))
    ));
    let reply = reviews.reply(&cr(1), &first, "Hm.", &someone()).unwrap();
    let review = reviews.store().get_review(&cr(1)).unwrap();
    assert_eq!(review.chains.len(), 2);
    assert_eq!(review.comment(&reply).unwrap().author, someone());
}

#[test]
fn a_reply_is_edited_by_its_author_only_and_a_missing_comment_is_not_found() {
    let mut store = MemoryStore::new();
    let mut reviews = Reviews::new(&mut store);
    reviews.create(new_review(Status::Waiting)).unwrap();
    let question = reviews
        .comment(&cr(1), on("a.rs", 1), "Why?", &hakan())
        .unwrap();
    let answer = reviews
        .reply(&cr(1), &question, "Because.", &agent())
        .unwrap();
    // Not even a reviewer edits another's words
    assert!(is_refused(&reviews.edit(&cr(1), &answer, "No.", &hakan())));
    reviews
        .edit(&cr(1), &answer, "Because, edited.", &agent())
        .unwrap();
    assert_eq!(
        reviews
            .store()
            .get_review(&cr(1))
            .unwrap()
            .comment(&answer)
            .unwrap()
            .body,
        "Because, edited."
    );
    assert!(matches!(
        reviews.edit(&cr(1), &CommentId("c9".into()), "x", &agent()),
        Err(Error::NotFound(_))
    ));
}

#[test]
fn a_comment_is_resolved_and_reopened_by_a_decider_or_its_author_only() {
    let mut store = MemoryStore::new();
    let mut reviews = Reviews::new(&mut store);
    reviews.create(new_review(Status::Waiting)).unwrap();
    let question = reviews
        .comment(&cr(1), on("a.rs", 1), "Why?", &hakan())
        .unwrap();
    assert!(is_refused(&reviews.resolve(
        &cr(1),
        &question,
        true,
        &someone()
    )));
    reviews.resolve(&cr(1), &question, true, &hakan()).unwrap();
    reviews.resolve(&cr(1), &question, false, &hakan()).unwrap();
    let resolved = |reviews: &Reviews<'_, MemoryStore>| {
        reviews
            .store()
            .get_review(&cr(1))
            .unwrap()
            .comment(&question)
            .unwrap()
            .resolved
    };
    assert_eq!(resolved(&reviews), Some(false));
    assert!(matches!(
        reviews.resolve(&cr(1), &CommentId("c9".into()), true, &hakan()),
        Err(Error::NotFound(_))
    ));
    // With no reviewers named, anyone but the review's author decides, so resolves another's comment
    let mut open = new_review(Status::Waiting);
    open.reviewers.clear();
    reviews.create(open).unwrap();
    let note = reviews
        .comment(&cr(2), on("a.rs", 1), "Note.", &hakan())
        .unwrap();
    assert!(is_refused(&reviews.resolve(&cr(2), &note, true, &agent())));
    reviews.resolve(&cr(2), &note, true, &someone()).unwrap();
}

/// A comment and a reply in every status a review is still open in, a review sent back too (ATM-597): talking
/// about the changes goes on until the next round. An ended one refuses them (`an_ended_review_takes_nothing_…`)
#[test]
fn a_review_not_ended_takes_comments_and_replies_whatever_its_status() {
    for status in [
        Status::Draft,
        Status::Waiting,
        Status::Live,
        Status::ChangesRequested,
    ] {
        let mut store = MemoryStore::new();
        let reference = at(&mut store, status);
        let mut reviews = Reviews::new(&mut store);
        let anchor = Anchor {
            round: 1,
            ..Anchor::default()
        };
        let id = reviews
            .comment(&reference, anchor, "Why this?", &hakan())
            .unwrap_or_else(|e| panic!("{status:?}: {e}"));
        reviews
            .reply(&reference, &id, "Because.", &agent())
            .unwrap_or_else(|e| panic!("{status:?}: {e}"));
        let review = store.get_review(&reference).unwrap();
        assert_eq!(review.header.status, status);
        assert_eq!(review.chains[0].comments[0].replies.len(), 1, "{status:?}");
    }
}
