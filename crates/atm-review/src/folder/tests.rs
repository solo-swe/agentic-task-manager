use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use super::{FileStore, folder_name, list_everywhere};
use crate::files::review_md;
use crate::model::{
    Actor, Anchor, FileChange, FileStatus, Patch, ReviewHeader, ReviewId, ReviewRef, RoundKind,
    Status,
};
use crate::rules::{Reviews, Verdict};
use crate::store::{NewReview, NewRound, ReviewFilter, ReviewStore};
use crate::test_dir::TestDir;

fn agent() -> Actor {
    Actor::from("claude:0f56a916")
}
fn hakan() -> Actor {
    Actor::from("Hakan Serce")
}
fn new_review(title: &str) -> NewReview {
    NewReview {
        title: title.into(),
        summary: "Why and what.".into(),
        tasks: vec!["ATM-482".into()],
        author: Some(agent()),
        reviewers: vec![hakan()],
        status: Status::Waiting,
        first: NewRound {
            kind: RoundKind::Range {
                // Commits as atm records them, full IDs (ATM-659)
                base: "a1".repeat(20),
                head: "b2".repeat(20),
            },
            submodule: None,
            files: vec![FileChange {
                path: "src/a.rs".into(),
                status: FileStatus::Modified,
                binary: false,
            }],
            patch: Patch("diff --git a/x b/x\n+one\n".into()),
            by: agent(),
        },
    }
}
fn git(args: &[&str], cwd: &Path) {
    let mut all = vec!["-c", "user.name=t", "-c", "user.email=t@example.com"];
    all.extend_from_slice(args);
    assert!(atm_fs::git::git(&all, cwd).is_some(), "git {args:?}");
}

#[test]
fn a_review_is_kept_in_its_folder_and_read_back_by_another_store() {
    let dir = TestDir::new();
    let task_dir = dir.path().join(".agentic-task-manager");
    let mut store = FileStore::open(&task_dir).unwrap();
    let mut reviews = Reviews::new(&mut store);
    let header = reviews.create(new_review("Claim hold on Windows")).unwrap();
    assert_eq!(header.reference, ReviewRef::numbered(1));
    let cr = header.reference.clone();
    let anchor = Anchor {
        round: 1,
        path: Some("src/a.rs".into()),
        lines: Some((3, 4)),
        quote: Some("let shell = 0;".into()),
        ..Anchor::default()
    };
    let question = reviews.comment(&cr, anchor, "Why?", &hakan()).unwrap();
    reviews.reply(&cr, &question, "Because.", &agent()).unwrap();
    reviews
        .submit(&cr, Verdict::RequestChanges, None, None, &hakan())
        .unwrap();
    let mut second = new_review("").first;
    second.patch = Patch("diff --git a/x b/x\n+two\n".into());
    assert_eq!(reviews.add_round(&cr, second).unwrap(), 2);

    let folder = task_dir.join("reviews").join(folder_name(1, &header.id));
    for file in [
        "review.md",
        "comments.md",
        "rounds/1.json",
        "rounds/1.diff",
        "rounds/2.diff",
    ] {
        assert!(folder.join(file).is_file(), "{file}");
    }
    let comments = std::fs::read_to_string(folder.join("comments.md")).unwrap();
    assert!(
        comments.starts_with("## src/a.rs:3-4 · round 1 · t1\n> let shell = 0;\n"),
        "{comments}"
    );

    // Another store (another process) reads the same
    let again = FileStore::open(&task_dir).unwrap();
    let review = again.get_review(&cr).unwrap();
    assert_eq!(review.header.status, Status::Waiting);
    assert_eq!(review.rounds.len(), 2);
    assert_eq!(review.chains[0].comments[0].replies[0].body, "Because.");
    assert_eq!(
        again.round_diff(&header.id, 2).unwrap().0,
        "diff --git a/x b/x\n+two\n"
    );
    assert_eq!(
        again
            .get_review(&ReviewRef(header.id.0.clone()))
            .unwrap()
            .header
            .id,
        header.id
    );
    assert_eq!(
        again.list_reviews(&ReviewFilter::default()).unwrap().len(),
        1
    );
    assert_eq!(again.problems().unwrap(), Vec::<String>::new());
}

/// ATM-674, audit R10: a round's diff is written before its JSON, so a round stopped between the two leaves no round
/// without its diff: here its diff can't be written (a folder is in its place), and no round 2 is there
#[test]
fn a_round_whose_diff_cant_be_written_is_no_round() {
    let dir = TestDir::new();
    let task_dir = dir.path().join(".agentic-task-manager");
    let mut store = FileStore::open(&task_dir).unwrap();
    let mut reviews = Reviews::new(&mut store);
    let header = reviews.create(new_review("Rounds")).unwrap();
    let rounds = task_dir
        .join("reviews")
        .join(folder_name(1, &header.id))
        .join("rounds");
    std::fs::create_dir_all(rounds.join("2.diff")).unwrap();
    assert!(
        reviews
            .add_round(&header.reference, new_review("").first)
            .is_err()
    );
    assert!(!rounds.join("2.json").exists(), "no round without its diff");
    let again = FileStore::open(&task_dir).unwrap();
    assert_eq!(again.get_review(&header.reference).unwrap().rounds.len(), 1);
}

#[test]
fn worktrees_of_one_clone_never_take_the_same_number_and_are_listed_together() {
    let dir = TestDir::new();
    let main = dir.path().join("main");
    std::fs::create_dir(&main).unwrap();
    git(&["init", "-q", "-b", "main"], &main);
    git(&["commit", "-q", "--allow-empty", "-m", "start"], &main);
    let wt = dir.path().join("wt");
    // Relative to `main`: git takes no path in Windows' extended form (`\\?\C:\…`, the test folder's)
    git(&["worktree", "add", "-q", "-b", "agent-2", "../wt"], &main);

    let mut here = FileStore::open(&main.join(".agentic-task-manager")).unwrap();
    let mut there = FileStore::open(&wt.join(".agentic-task-manager")).unwrap();
    assert_eq!(here.create_review(new_review("one")).unwrap().number, 1);
    assert_eq!(there.create_review(new_review("two")).unwrap().number, 2);
    assert_eq!(here.create_review(new_review("three")).unwrap().number, 3);

    let found = list_everywhere(&main, &ReviewFilter::default()).unwrap();
    let listed: Vec<(u32, Option<&str>)> = found
        .iter()
        .map(|f| (f.header.number, f.branch.as_deref()))
        .collect();
    assert!(
        listed.contains(&(3, Some("main"))) && listed.contains(&(1, Some("main"))),
        "{listed:?}"
    );
    assert!(listed.contains(&(2, Some("agent-2"))), "{listed:?}");
}

#[test]
fn outside_git_the_counter_is_in_the_reviews_folder() {
    let dir = TestDir::new();
    let task_dir = dir.path().join(".agentic-task-manager");
    let mut store = FileStore::open(&task_dir).unwrap();
    store.create_review(new_review("one")).unwrap();
    assert!(task_dir.join("reviews/counter.json").is_file());
    assert_eq!(store.create_review(new_review("two")).unwrap().number, 2);
}

/// A review written as another clone would have, with its own number.
fn plant(task_dir: &Path, number: u32, title: &str, created: &str) -> ReviewHeader {
    let id = ReviewId::new();
    let header = ReviewHeader {
        id: id.clone(),
        reference: ReviewRef::numbered(number),
        number,
        title: title.into(),
        status: Status::Waiting,
        tasks: Vec::new(),
        author: Some(agent()),
        reviewers: Vec::new(),
        created: created.into(),
        updated: created.into(),
    };
    let folder = task_dir.join("reviews").join(folder_name(number, &id));
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(
        folder.join("review.md"),
        review_md::write(&header, "").unwrap(),
    )
    .unwrap();
    header
}

#[test]
fn two_clones_cr_8_are_cr_8_1_and_cr_8_2_and_cr_8_names_both() {
    let dir = TestDir::new();
    let task_dir = dir.path().join(".agentic-task-manager");
    let later = plant(&task_dir, 8, "Mine", "2026-11-17T05:00:00Z");
    let earlier = plant(&task_dir, 8, "Theirs", "2026-11-17T04:00:00Z");
    plant(&task_dir, 7, "Alone", "2026-11-17T03:00:00Z");
    let store = FileStore::open(&task_dir).unwrap();
    assert_eq!(
        store
            .get_review(&ReviewRef("CR-8.1".into()))
            .unwrap()
            .header
            .id,
        earlier.id
    );
    assert_eq!(
        store
            .get_review(&ReviewRef("CR-8.2".into()))
            .unwrap()
            .header
            .id,
        later.id
    );
    assert_eq!(
        store
            .get_review(&ReviewRef::numbered(7))
            .unwrap()
            .header
            .title,
        "Alone"
    );
    let both = store
        .get_review(&ReviewRef::numbered(8))
        .unwrap_err()
        .to_string();
    assert!(
        both.contains("CR-8.1 (Theirs)") && both.contains("CR-8.2 (Mine)"),
        "{both}"
    );
    // The next number is past them all
    let mut store = store;
    assert_eq!(store.create_review(new_review("next")).unwrap().number, 9);
}

#[test]
fn a_review_folder_that_cant_be_read_is_reported_and_the_others_are_listed() {
    let dir = TestDir::new();
    let task_dir = dir.path().join(".agentic-task-manager");
    plant(&task_dir, 1, "Fine", "2026-11-17T03:00:00Z");
    std::fs::create_dir_all(task_dir.join("reviews/CR-2-abcd")).unwrap();
    std::fs::write(task_dir.join("reviews/CR-2-abcd/review.md"), "not a header").unwrap();
    let store = FileStore::open(&task_dir).unwrap();
    assert_eq!(
        store.list_reviews(&ReviewFilter::default()).unwrap().len(),
        1
    );
    let problems = store.problems().unwrap();
    assert!(problems[0].starts_with("CR-2-abcd: "), "{problems:?}");
}

#[cfg(unix)]
#[test]
fn a_symlinked_reviews_folder_or_review_is_refused() {
    let dir = TestDir::new();
    let task_dir = dir.path().join(".agentic-task-manager");
    let elsewhere = dir.path().join("elsewhere");
    let planted = plant(&elsewhere, 3, "Planted", "2026-11-17T03:00:00Z");
    std::fs::create_dir_all(task_dir.join("reviews")).unwrap();
    let target = elsewhere.join("reviews").join(folder_name(3, &planted.id));
    std::os::unix::fs::symlink(&target, task_dir.join("reviews/CR-3-ffff")).unwrap();
    let store = FileStore::open(&task_dir).unwrap();
    assert_eq!(
        store.list_reviews(&ReviewFilter::default()).unwrap().len(),
        0
    );
    assert!(store.problems().unwrap()[0].contains("symlink"));

    let linked = dir.path().join("linked");
    std::fs::create_dir_all(&linked).unwrap();
    std::os::unix::fs::symlink(elsewhere.join("reviews"), linked.join("reviews")).unwrap();
    assert!(
        FileStore::open(&linked)
            .unwrap_err()
            .to_string()
            .contains("symlink")
    );
}

/// A store on a fresh task folder outside git, and the folder (kept while the store is used).
fn fresh() -> (TestDir, FileStore) {
    let dir = TestDir::new();
    let store = FileStore::open(&dir.path().join(".agentic-task-manager")).unwrap();
    (dir, store)
}

/// Every store's promises (`store_contract`), kept by the folder's
mod contract {
    use super::fresh;
    use crate::store_contract as contract;

    #[test]
    fn an_empty_store_lists_and_finds_nothing() {
        contract::an_empty_store_lists_and_finds_nothing(&mut fresh().1);
    }

    #[test]
    fn a_new_review_is_kept_numbered_with_its_first_round() {
        let (_dir, mut store) = fresh();
        contract::a_new_review_is_kept_numbered_with_its_first_round(&mut store);
    }

    #[test]
    fn reviews_are_listed_newest_first_and_filtered() {
        let (_dir, mut store) = fresh();
        contract::reviews_are_listed_newest_first_and_filtered(&mut store);
    }

    #[test]
    fn a_reviews_header_and_rounds_are_changed_as_asked() {
        let (_dir, mut store) = fresh();
        contract::a_reviews_header_and_rounds_are_changed_as_asked(&mut store);
    }

    #[test]
    fn comments_are_kept_replied_to_edited_and_resolved() {
        let (_dir, mut store) = fresh();
        contract::comments_are_kept_replied_to_edited_and_resolved(&mut store);
    }

    #[test]
    fn every_method_on_a_review_it_hasnt_is_not_found() {
        let (_dir, mut store) = fresh();
        contract::every_method_on_a_review_it_hasnt_is_not_found(&mut store);
    }
}

#[test]
fn a_folder_is_a_reviews_only_when_named_cr_its_number_and_four_lowercase_hex_digits() {
    for (name, number) in [
        ("CR-7-3fa1", Some(7)),
        ("CR-12-0000", Some(12)),
        ("CR-7-3FA1", None),
        ("CR-7-3fa", None),
        ("CR-7-3fa12", None),
        ("CR-7-3fg1", None),
        ("CR-x-3fa1", None),
        ("CR--3fa1", None),
        ("CR-7", None),
        ("cr-7-3fa1", None),
        ("counter.json", None),
        ("notes", None),
    ] {
        assert_eq!(super::folder_number(name), number, "{name}");
    }
    // A new folder's name is one of them: the number and the UUID's last four hex digits
    let id = ReviewId("0193a8f2-0000-7000-8000-000000003fa1".into());
    assert_eq!(folder_name(7, &id), "CR-7-3fa1");
    assert_eq!(super::folder_number(&folder_name(7, &id)), Some(7));
}

#[test]
fn reviews_sharing_a_number_and_a_time_are_ordered_by_their_ids() {
    let dir = TestDir::new();
    let task_dir = dir.path().join(".agentic-task-manager");
    let a = plant(&task_dir, 4, "A", "2026-11-17T03:00:00Z");
    let b = plant(&task_dir, 4, "B", "2026-11-17T03:00:00Z");
    let (low, high) = if a.id < b.id { (a, b) } else { (b, a) };
    let store = FileStore::open(&task_dir).unwrap();
    let reference = |n: &str| store.get_review(&ReviewRef(n.into())).unwrap().header.id;
    assert_eq!(
        (reference("CR-4.1"), reference("CR-4.2")),
        (low.id, high.id)
    );
    assert!(matches!(
        store.get_review(&ReviewRef("CR-4.3".into())),
        Err(atm_common::Error::NotFound(_))
    ));
    assert!(matches!(
        store.get_review(&ReviewRef("CR-4".into())),
        Err(atm_common::Error::Invalid(_))
    ));
}

#[test]
fn other_entries_in_the_reviews_folder_are_left_alone() {
    let dir = TestDir::new();
    let task_dir = dir.path().join(".agentic-task-manager");
    plant(&task_dir, 1, "One", "2026-11-17T03:00:00Z");
    std::fs::create_dir_all(task_dir.join("reviews/notes")).unwrap();
    std::fs::write(task_dir.join("reviews/README.md"), "hi").unwrap();
    std::fs::write(task_dir.join("reviews/CR-2-abcd"), "a file, not a folder").unwrap();
    let store = FileStore::open(&task_dir).unwrap();
    assert_eq!(
        store.list_reviews(&ReviewFilter::default()).unwrap().len(),
        1
    );
    // A file named like a review's folder can't be read as one, and is reported
    let problems = store.problems().unwrap();
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].starts_with("CR-2-abcd: "), "{problems:?}");
}

#[test]
fn a_review_atm_cant_read_is_kept_with_its_folder_its_reference_and_why() {
    let dir = TestDir::new();
    let task_dir = dir.path().join(".agentic-task-manager");
    plant(&task_dir, 1, "Fine", "2026-11-17T03:00:00Z");
    // A status a newer atm wrote, and a review.md that isn't one
    let newer = plant(&task_dir, 13, "Newer", "2026-11-17T03:00:00Z");
    let newer_md = task_dir
        .join("reviews")
        .join(folder_name(13, &newer.id))
        .join("review.md");
    let text = std::fs::read_to_string(&newer_md).unwrap();
    assert_eq!(text.matches("open").count(), 1, "{text}");
    std::fs::write(&newer_md, text.replace("open", "frozen")).unwrap();
    let broken = task_dir.join("reviews/CR-14-abcd");
    std::fs::create_dir_all(&broken).unwrap();
    std::fs::write(broken.join("review.md"), "no header").unwrap();
    let store = FileStore::open(&task_dir).unwrap();
    let unreadable = store.unreadable().unwrap();
    let named: Vec<(&str, &str)> = unreadable
        .iter()
        .map(|u| (u.folder.as_str(), u.reference.as_str()))
        .collect();
    assert_eq!(
        named,
        [
            (folder_name(13, &newer.id).as_str(), "CR-13"),
            ("CR-14-abcd", "CR-14")
        ]
    );
    assert!(unreadable[0].problem.contains("frozen"), "{unreadable:?}");
    assert_ne!(unreadable[1].problem, "");
    // The readable one is still listed; a store with none unreadable has none
    assert_eq!(
        store.list_reviews(&ReviewFilter::default()).unwrap().len(),
        1
    );
    let empty = TestDir::new();
    let none = FileStore::open(&empty.path().join(".agentic-task-manager")).unwrap();
    assert_eq!(none.unreadable().unwrap(), Vec::new());
}

#[test]
fn a_review_folder_without_review_md_or_with_a_bad_round_is_reported() {
    let dir = TestDir::new();
    let task_dir = dir.path().join(".agentic-task-manager");
    let reviews = task_dir.join("reviews");
    std::fs::create_dir_all(reviews.join("CR-1-aaaa")).unwrap();
    let wrong = plant(&task_dir, 2, "Wrong round", "2026-11-17T03:00:00Z");
    let wrong_rounds = reviews.join(folder_name(2, &wrong.id)).join("rounds");
    std::fs::create_dir_all(&wrong_rounds).unwrap();
    std::fs::write(
        wrong_rounds.join("1.json"),
        r#"{"number":2,"kind":{"kind":"files"},"files":[],"by":"a","at":"t"}"#,
    )
    .unwrap();
    let broken = plant(&task_dir, 3, "Broken round", "2026-11-17T03:00:00Z");
    let broken_rounds = reviews.join(folder_name(3, &broken.id)).join("rounds");
    std::fs::create_dir_all(&broken_rounds).unwrap();
    std::fs::write(broken_rounds.join("1.json"), "{").unwrap();
    // Files there that aren't a round's are no round
    let fine = plant(&task_dir, 4, "Fine", "2026-11-17T03:00:00Z");
    let fine_rounds = reviews.join(folder_name(4, &fine.id)).join("rounds");
    std::fs::create_dir_all(&fine_rounds).unwrap();
    std::fs::write(fine_rounds.join("1.diff"), "").unwrap();
    std::fs::write(fine_rounds.join("notes.json"), "{").unwrap();
    let store = FileStore::open(&task_dir).unwrap();
    let listed: Vec<String> = store
        .list_reviews(&ReviewFilter::default())
        .unwrap()
        .into_iter()
        .map(|h| h.title)
        .collect();
    assert_eq!(listed, ["Fine"]);
    let problems = store.problems().unwrap();
    assert_eq!(problems.len(), 3, "{problems:?}");
    assert!(
        problems.iter().any(|p| p.contains("has no review.md")),
        "{problems:?}"
    );
    assert!(
        problems.iter().any(|p| p.contains("says it is round 2")),
        "{problems:?}"
    );
    assert!(
        problems
            .iter()
            .any(|p| p.starts_with(&format!("{}: ", folder_name(3, &broken.id)))),
        "{problems:?}"
    );
}

#[test]
fn rounds_are_read_in_their_numbers_order_not_the_folders() {
    let (_dir, mut store) = fresh();
    let id = store.create_review(new_review("Rounds")).unwrap().id;
    for _ in 0..10 {
        store.add_round(&id, new_review("").first).unwrap();
    }
    let numbers: Vec<u32> = store
        .get_review(&ReviewRef::numbered(1))
        .unwrap()
        .rounds
        .iter()
        .map(|r| r.number)
        .collect();
    assert_eq!(numbers, (1..=11).collect::<Vec<_>>());
}

#[test]
fn comments_md_lines_atm_cant_read_are_reported_and_kept_through_a_change() {
    let (dir, mut store) = fresh();
    let header = store.create_review(new_review("Kept")).unwrap();
    let comments = dir
        .path()
        .join(".agentic-task-manager/reviews")
        .join(folder_name(1, &header.id))
        .join("comments.md");
    std::fs::write(&comments, "A note by hand.\n").unwrap();
    let problems = store.problems().unwrap();
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0].starts_with(&format!(
            "{}: comments.md line 1",
            folder_name(1, &header.id)
        )),
        "{problems:?}"
    );
    store
        .add_comment(&header.id, Anchor::default(), "Fine.", &hakan())
        .unwrap();
    let text = std::fs::read_to_string(&comments).unwrap();
    assert!(
        text.starts_with("A note by hand.\n") && text.contains("Fine."),
        "{text}"
    );
}

#[test]
fn the_counter_is_never_behind_the_folder_nor_the_folder_behind_the_counter() {
    let (dir, mut store) = fresh();
    let reviews = dir.path().join(".agentic-task-manager/reviews");
    std::fs::create_dir_all(&reviews).unwrap();
    // A counter ahead of the folders (a review made in another worktree): its number is taken
    std::fs::write(reviews.join("counter.json"), "{\"next_number\": 5}\n").unwrap();
    assert_eq!(store.create_review(new_review("five")).unwrap().number, 5);
    // A counter that can't be read counts from the folders
    std::fs::write(reviews.join("counter.json"), "not json").unwrap();
    assert_eq!(store.create_review(new_review("six")).unwrap().number, 6);
    let counter = std::fs::read_to_string(reviews.join("counter.json")).unwrap();
    assert!(counter.contains("\"next_number\":7"), "{counter}");
}

#[test]
fn a_review_made_in_a_worktree_merged_into_main_is_listed_once_from_main() {
    let dir = TestDir::new();
    let main = dir.path().join("main");
    std::fs::create_dir(&main).unwrap();
    git(&["init", "-q", "-b", "main"], &main);
    git(&["commit", "-q", "--allow-empty", "-m", "start"], &main);
    let mut store = FileStore::open(&main.join(".agentic-task-manager")).unwrap();
    store.create_review(new_review("shared")).unwrap();
    git(&["add", "-A"], &main);
    git(&["commit", "-q", "-m", "review"], &main);
    git(&["worktree", "add", "-q", "-b", "agent-2", "../wt"], &main);
    // The worktree has the same review, committed
    let found = list_everywhere(&main, &ReviewFilter::default()).unwrap();
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].branch.as_deref(), Some("main"));
    // A filter is the stores'
    let none = list_everywhere(
        &main,
        &ReviewFilter {
            statuses: vec![Status::Closed],
            task: None,
        },
    )
    .unwrap();
    assert_eq!(none.len(), 0);
}

#[test]
fn outside_git_only_the_folders_own_reviews_are_listed_with_no_branch() {
    let dir = TestDir::new();
    let mut store = FileStore::open(&dir.path().join(".agentic-task-manager")).unwrap();
    store.create_review(new_review("alone")).unwrap();
    let found = list_everywhere(dir.path(), &ReviewFilter::default()).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].branch, None);
    assert_eq!(found[0].working_copy, dir.path());
    // No reviews folder yet: nothing, not an error
    let empty = TestDir::new();
    assert_eq!(
        list_everywhere(empty.path(), &ReviewFilter::default())
            .unwrap()
            .len(),
        0
    );
}

#[cfg(unix)]
#[test]
fn a_symlinked_task_folder_or_file_of_a_review_is_refused() {
    let dir = TestDir::new();
    let real = dir.path().join("real");
    std::fs::create_dir_all(&real).unwrap();
    let linked = dir.path().join(".agentic-task-manager");
    std::os::unix::fs::symlink(&real, &linked).unwrap();
    assert!(matches!(
        FileStore::open(&linked),
        Err(atm_common::Error::Refused(_))
    ));

    // A review whose review.md, or whose rounds folder, is a symlink: reported, not read
    let task_dir = dir.path().join("project/.agentic-task-manager");
    let outside = dir.path().join("outside.md");
    let header = plant(&task_dir, 1, "Linked file", "2026-11-17T03:00:00Z");
    let folder = task_dir.join("reviews").join(folder_name(1, &header.id));
    std::fs::rename(folder.join("review.md"), &outside).unwrap();
    std::os::unix::fs::symlink(&outside, folder.join("review.md")).unwrap();
    let rounds = plant(&task_dir, 2, "Linked rounds", "2026-11-17T03:00:00Z");
    let elsewhere = dir.path().join("rounds");
    std::fs::create_dir_all(&elsewhere).unwrap();
    std::os::unix::fs::symlink(
        &elsewhere,
        task_dir
            .join("reviews")
            .join(folder_name(2, &rounds.id))
            .join("rounds"),
    )
    .unwrap();
    let store = FileStore::open(&task_dir).unwrap();
    assert_eq!(
        store.list_reviews(&ReviewFilter::default()).unwrap().len(),
        0
    );
    let problems = store.problems().unwrap();
    assert_eq!(problems.len(), 2, "{problems:?}");
    assert!(
        problems.iter().all(|p| p.contains("symlink")),
        "{problems:?}"
    );
}

#[test]
fn a_subscription_is_told_when_a_review_changes() {
    let dir = TestDir::new();
    let task_dir = dir.path().join(".agentic-task-manager");
    let mut store = FileStore::open(&task_dir).unwrap();
    let (sent, received) = std::sync::mpsc::channel();
    let subscription = store
        .subscribe(Arc::new(move || {
            let _ = sent.send(());
        }))
        .unwrap();
    store.create_review(new_review("watched")).unwrap();
    received
        .recv_timeout(Duration::from_secs(10))
        .expect("told of the new review");
    drop(subscription);
}
