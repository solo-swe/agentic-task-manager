//! ATM-725: a review's views are its rounds' increments added up, however each round was opened.

use super::super::{Since, between, from_range, from_working};
use crate::memory::MemoryStore;
use crate::model::{Actor, Review, ReviewRef, Status};
use crate::rules::Reviews;
use crate::store::{NewReview, NewRound, ReviewStore};
use atm_common::TASK_DIR;
use atm_vcs::{GitVcs, Rev};

/// A repository with a base commit and three more, each changing what the one before left; the task folder changes
/// too, which no view shows
struct Repo {
    dir: crate::test_dir::TestDir,
    vcs: GitVcs,
    /// The base, then the three commits
    commits: Vec<String>,
}

impl Repo {
    fn new() -> Self {
        let dir = crate::test_dir::TestDir::new();
        let root = dir.path().to_path_buf();
        let git = |args: &[&str]| {
            let mut all = vec!["-c", "user.name=t", "-c", "user.email=t@example.com"];
            all.extend_from_slice(args);
            atm_fs::git::git(&all, &root).unwrap_or_else(|| panic!("git {args:?}"))
        };
        git(&["init", "-q"]);
        let steps: [&[(&str, &str)]; 4] = [
            &[
                ("a.rs", "1\n"),
                (".agentic-task-manager/tasks.json", "{}\n"),
            ],
            &[("a.rs", "1\n2\n"), ("b.rs", "b\n")],
            &[
                ("a.rs", "1\n2\n3\n"),
                (".agentic-task-manager/tasks.json", "{\"x\":1}\n"),
            ],
            &[("c.rs", "c\n"), ("b.rs", "b\nb2\n")],
        ];
        let mut commits = Vec::new();
        for (n, files) in steps.iter().enumerate() {
            for (path, text) in *files {
                let path = root.join(path);
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(path, text).unwrap();
            }
            git(&["add", "-A"]);
            git(&["commit", "-q", "-m", &format!("step {n}")]);
            commits.push(git(&["rev-parse", "HEAD"]).trim().to_string());
        }
        let vcs = GitVcs::new(&root);
        Self { dir, vcs, commits }
    }

    fn range(&self, from: usize, to: usize) -> NewRound {
        let by = Actor::from("claude:1");
        from_range(&self.vcs, &self.commits[from], &self.commits[to], &by).unwrap()
    }

    /// A review whose rounds are these ranges (indices into `commits`)
    fn review(&self, ranges: &[(usize, usize)]) -> (MemoryStore, Review) {
        let mut store = MemoryStore::new();
        let mut reviews = Reviews::new(&mut store);
        let (first, rest) = ranges.split_first().unwrap();
        reviews
            .create(NewReview {
                title: "Rounds".into(),
                summary: String::new(),
                tasks: Vec::new(),
                author: Some(Actor::from("claude:1")),
                reviewers: Vec::new(),
                status: Status::Waiting,
                first: self.range(first.0, first.1),
            })
            .unwrap();
        for (from, to) in rest {
            reviews
                .add_round(&ReviewRef::numbered(1), self.range(*from, *to))
                .unwrap();
        }
        let review = store.get_review(&ReviewRef::numbered(1)).unwrap();
        (store, review)
    }

    /// What git says changed from commit `from` to commit `to`, the task folder left out
    fn expected(&self, from: usize, to: usize) -> String {
        let (a, b) = (
            Rev::object_id(&self.commits[from]).unwrap(),
            Rev::object_id(&self.commits[to]).unwrap(),
        );
        self.vcs
            .diff_between(&a, &b, &[], &[TASK_DIR])
            .unwrap()
            .patch
    }
}

fn view(repo: &Repo, store: &MemoryStore, review: &Review, since: Since, to: u32) -> String {
    between(&repo.vcs, store, review, since, to)
        .unwrap()
        .diff
        .patch
}

#[test]
fn every_view_adds_up_the_increments_whatever_ranges_the_rounds_were_opened_with() {
    let repo = Repo::new();
    // Round N ends at commit N, each opened differently
    let openings: [(&str, [(usize, usize); 3]); 3] = [
        ("cumulative, from the base", [(0, 1), (0, 2), (0, 3)]),
        (
            "incremental, from the round before",
            [(0, 1), (1, 2), (2, 3)],
        ),
        (
            "neither: round 3 from the base, round 2 from round 1",
            [(0, 1), (1, 2), (0, 3)],
        ),
    ];
    for (how, ranges) in openings {
        let (store, review) = repo.review(&ranges);
        for n in 1..=3u32 {
            let i = usize::try_from(n).unwrap();
            // The whole change: from the review's base, whatever round N's range started from
            assert_eq!(
                view(&repo, &store, &review, Since::Base, n),
                repo.expected(0, i),
                "{how}: whole({n})"
            );
            for m in 1..n {
                let j = usize::try_from(m).unwrap();
                assert_eq!(
                    view(&repo, &store, &review, Since::Round(m), n),
                    repo.expected(j, i),
                    "{how}: since({m}, {n})"
                );
                // whole(N) = whole(M) + since(M, N): they meet at round M's tree, and end at round N's
                let whole_m = between(&repo.vcs, &store, &review, Since::Base, m).unwrap();
                let since = between(&repo.vcs, &store, &review, Since::Round(m), n).unwrap();
                let whole_n = between(&repo.vcs, &store, &review, Since::Base, n).unwrap();
                assert_eq!(
                    whole_m.after, since.before,
                    "{how}: whole({m}) ends where since({m}, {n}) starts"
                );
                assert_eq!(
                    since.after, whole_n.after,
                    "{how}: since({m}, {n}) ends where whole({n}) does"
                );
            }
        }
        // The task folder is never in a view, though commits 1 and 3 change it
        assert!(
            !view(&repo, &store, &review, Since::Base, 3).contains(TASK_DIR),
            "{how}"
        );
        // Round 1 is its own increment: the whole change of round 1
        assert!(
            view(&repo, &store, &review, Since::Base, 1).contains("+b"),
            "{how}"
        );
    }
}

#[test]
fn a_working_round_after_commits_adds_only_its_own_change_since_the_round_before() {
    let repo = Repo::new();
    let (mut store, _) = repo.review(&[(0, 1), (1, 2)]);
    std::fs::write(repo.dir.path().join("a.rs"), "1\n2\nworking\n").unwrap();
    // Its base an older commit than round 2's head: still measured from round 2's result
    let working = from_working(&repo.vcs, &repo.commits[1], &Actor::from("claude:1")).unwrap();
    Reviews::new(&mut store)
        .add_round(&ReviewRef::numbered(1), working)
        .unwrap();
    let review = store.get_review(&ReviewRef::numbered(1)).unwrap();
    let since = view(&repo, &store, &review, Since::Round(2), 3);
    assert!(
        since.contains("+working") && !since.contains("+2"),
        "{since}"
    );
    let whole = view(&repo, &store, &review, Since::Base, 3);
    assert!(
        whole.contains("+2") && whole.contains("+working") && whole.contains("+b"),
        "{whole}"
    );
}

/// A range that doesn't start where the round before ended is recorded from there, saying so; one that does, or a
/// round of another kind, is kept as it is
#[test]
fn a_new_range_round_is_recorded_as_what_it_changed_since_the_round_before() {
    use super::super::as_increment;
    let repo = Repo::new();
    let (_, review) = repo.review(&[(0, 1), (1, 2)]);
    let last = &review.rounds[1];
    // From the base: recorded from round 2's head, its diff only commit 3's change
    let (cumulative, said) = as_increment(&repo.vcs, last, repo.range(0, 3));
    assert_eq!(
        cumulative.kind,
        crate::model::RoundKind::Range {
            base: repo.commits[2].clone(),
            head: repo.commits[3].clone()
        }
    );
    assert_eq!(cumulative.patch.0, repo.expected(2, 3));
    let said = said.expect("it says so");
    assert!(
        said.contains("not where round 2 ended") && said.contains(&repo.commits[2][..12]),
        "{said}"
    );
    // From round 2's head already: as it is, nothing said
    let (incremental, said) = as_increment(&repo.vcs, last, repo.range(2, 3));
    assert_eq!(incremental, repo.range(2, 3));
    assert_eq!(said, None);
    // A working round: as it is
    let working = from_working(&repo.vcs, "HEAD", &Actor::from("claude:1")).unwrap();
    let (kept, said) = as_increment(&repo.vcs, last, working.clone());
    assert_eq!((kept, said), (working, None));
    // The round before's head isn't in this clone (made in another): as it is, nothing said
    let mut elsewhere = last.clone();
    elsewhere.kind = crate::model::RoundKind::Range {
        base: "a".repeat(40),
        head: "b".repeat(40),
    };
    let (kept, said) = as_increment(&repo.vcs, &elsewhere, repo.range(0, 3));
    assert_eq!((kept, said), (repo.range(0, 3), None));
}

/// A comment is in a view that holds its round's increment: on round 2, in whole(3) and since(1, 3), not in
/// since(2, 3) nor in whole(1)
#[test]
fn a_comment_is_in_the_views_that_hold_its_rounds_increment() {
    use super::super::in_view;
    let cases = [
        (Since::Base, 3, true),
        (Since::Base, 2, true),
        (Since::Base, 1, false),
        (Since::Round(1), 3, true),
        (Since::Round(1), 2, true),
        (Since::Round(2), 3, false),
    ];
    for (since, to, expected) in cases {
        assert_eq!(in_view(since, to, 2), expected, "{since:?} to {to}");
    }
}

#[test]
fn a_view_starts_before_its_round() {
    let repo = Repo::new();
    let (store, review) = repo.review(&[(0, 1), (1, 2)]);
    for m in [2, 3] {
        assert!(matches!(
            between(&repo.vcs, &store, &review, Since::Round(m), 2),
            Err(atm_common::Error::Invalid(_))
        ));
    }
    assert!(matches!(
        between(&repo.vcs, &store, &review, Since::Base, 3),
        Err(atm_common::Error::NotFound(_))
    ));
}
