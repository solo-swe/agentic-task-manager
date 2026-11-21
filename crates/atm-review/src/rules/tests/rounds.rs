//! Rounds and checkpoints: whose they are, the author a first round makes, what a round does to the status.

use super::*;

#[test]
fn a_review_without_an_author_yet_takes_the_first_rounds_maker_never_a_reviewer() {
    let mut store = MemoryStore::new();
    let mut reviews = Reviews::new(&mut store);
    // A live review Hakan opened on an agent's working copy, nobody's change yet
    let mut live = new_review(Status::Live);
    live.author = None;
    reviews.create(live).unwrap();
    // Hakan reviews it: his round would make him the change's author, so it is refused
    assert!(reviews.add_round(&cr(1), round(&hakan())).is_err());
    // The agent ends it with a round: it becomes the author, and Hakan, a reviewer, decides
    reviews.add_round(&cr(1), round(&agent())).unwrap();
    let header = reviews.store().get_review(&cr(1)).unwrap().header;
    assert_eq!(header.author, Some(agent()));
    assert_eq!(header.status, Status::Waiting);
    assert!(decides(&header, &hakan()) && !decides(&header, &agent()));
}

#[test]
fn a_live_review_takes_checkpoints_and_is_approved_or_ended_with_a_round() {
    let mut store = MemoryStore::new();
    let mut reviews = Reviews::new(&mut store);
    reviews.create(new_review(Status::Live)).unwrap();
    let checkpoint = NewRound {
        kind: RoundKind::Working { base: "a1".into() },
        ..round(&hakan())
    };
    assert_eq!(reviews.checkpoint(&cr(1), checkpoint.clone()).unwrap(), 2);
    assert_eq!(status(reviews.store(), 1), Status::Live);
    // Sending back is for an open review; a live one is commented on as it grows
    assert!(
        reviews
            .submit(&cr(1), Verdict::RequestChanges, None, None, &hakan())
            .is_err()
    );

    reviews.create(new_review(Status::Live)).unwrap();
    reviews
        .submit(&cr(2), Verdict::Approve, None, None, &hakan())
        .unwrap();
    assert_eq!(status(reviews.store(), 2), Status::Approved);

    reviews.add_round(&cr(1), round(&agent())).unwrap();
    assert_eq!(status(reviews.store(), 1), Status::Waiting);
    assert!(reviews.checkpoint(&cr(1), checkpoint).is_err());
}

#[test]
fn a_round_reopens_a_review_sent_back_or_live_and_leaves_a_draft_or_open_one_as_it_is() {
    for (before, after) in [
        (Status::Draft, Status::Draft),
        (Status::Waiting, Status::Waiting),
        (Status::Live, Status::Waiting),
        (Status::ChangesRequested, Status::Waiting),
    ] {
        let mut store = MemoryStore::new();
        let reference = at(&mut store, before);
        assert_eq!(
            Reviews::new(&mut store)
                .add_round(&reference, round(&agent()))
                .unwrap(),
            2,
            "{before:?}"
        );
        assert_eq!(
            store.get_review(&reference).unwrap().header.status,
            after,
            "{before:?}"
        );
    }
}

#[test]
fn a_round_is_its_authors_and_someone_elses_is_refused_and_not_kept() {
    let mut store = MemoryStore::new();
    let mut reviews = Reviews::new(&mut store);
    reviews.create(new_review(Status::Waiting)).unwrap();
    for by in [hakan(), someone()] {
        assert!(is_refused(&reviews.add_round(&cr(1), round(&by))), "{by:?}");
    }
    assert_eq!(reviews.store().get_review(&cr(1)).unwrap().rounds.len(), 1);
}

#[test]
fn a_review_without_an_author_yet_and_no_reviewers_takes_anyones_round_as_theirs() {
    let mut store = MemoryStore::new();
    let mut reviews = Reviews::new(&mut store);
    let mut live = new_review(Status::Live);
    live.author = None;
    live.reviewers.clear();
    reviews.create(live).unwrap();
    reviews.add_round(&cr(1), round(&someone())).unwrap();
    let header = reviews.store().get_review(&cr(1)).unwrap().header;
    assert_eq!(header.author, Some(someone()));
    // And from then on its rounds are theirs only
    assert!(is_refused(&reviews.add_round(&cr(1), round(&agent()))));
}

#[test]
fn a_checkpoint_is_only_a_live_reviews_and_anyone_taking_part_makes_one() {
    for status in [Status::Draft, Status::Waiting, Status::ChangesRequested] {
        let mut store = MemoryStore::new();
        let reference = at(&mut store, status);
        assert!(
            is_not_now(&Reviews::new(&mut store).checkpoint(&reference, round(&hakan()))),
            "{status:?}"
        );
        assert_eq!(store.get_review(&reference).unwrap().rounds.len(), 1);
    }
    let mut store = MemoryStore::new();
    let mut reviews = Reviews::new(&mut store);
    reviews.create(new_review(Status::Live)).unwrap();
    // Not the author, not a reviewer, and the author is unchanged
    assert_eq!(reviews.checkpoint(&cr(1), round(&someone())).unwrap(), 2);
    let review = reviews.store().get_review(&cr(1)).unwrap();
    assert_eq!(review.header.author, Some(agent()));
    assert_eq!(review.rounds[1].by, someone());
}
