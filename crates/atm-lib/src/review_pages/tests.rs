use super::*;
use crate::reviews::ReviewId;

fn header(status: Status, author: Option<&str>, reviewers: &[&str]) -> ReviewHeader {
    ReviewHeader {
        id: ReviewId("r".into()),
        reference: ReviewRef("CR-1".into()),
        number: 1,
        title: "T".into(),
        status,
        tasks: Vec::new(),
        author: author.map(Actor::from),
        reviewers: reviewers.iter().map(|r| Actor::from(*r)).collect(),
        created: "2026-11-17T10:00:00Z".into(),
        updated: "2026-11-17T10:00:00Z".into(),
    }
}

/// A review of `header` with one chain: a top-level comment by `top` at minute 0, and replies by `replies` at the
/// minutes after
fn talked(header: ReviewHeader, top: &str, replies: &[&str]) -> Review {
    let comment = |id: usize, by: &str| atm_review::model::Comment {
        id: atm_review::model::CommentId(format!("c{id}")),
        author: Actor::from(by),
        at: format!("2026-11-18T10:{id:02}:00Z"),
        body: "x".into(),
        resolved: None,
        poke: false,
        replies: Vec::new(),
    };
    let mut first = comment(0, top);
    first.resolved = Some(false);
    first.replies = replies
        .iter()
        .enumerate()
        .map(|(i, by)| comment(i + 1, by))
        .collect();
    Review {
        header,
        summary: String::new(),
        chains: vec![Chain {
            id: atm_review::model::ChainId("t1".into()),
            anchor: atm_review::model::Anchor::default(),
            comments: vec![first],
        }],
        rounds: Vec::new(),
    }
}

/// ATM-606: what a review waits on the user for: its decision or round, or an answer they haven't answered yet
#[test]
fn a_review_waits_on_you_for_its_decision_or_for_an_answer_since_you_last_wrote() {
    let ana = Actor::from("Ana");
    let agent = Some("claude:a");
    for (case, review, expected) in [
        // Its decision is yours: whatever was written
        (
            "open, you its reviewer",
            talked(header(Status::Waiting, agent, &["Ana"]), "Ana", &[]),
            Some("review"),
        ),
        // Sent back, the agent answered you after your comment
        (
            "sent back, answered",
            talked(
                header(Status::ChangesRequested, agent, &["Ana"]),
                "Ana",
                &["claude:a"],
            ),
            Some("answered"),
        ),
        // ...and you answered back: nothing new
        (
            "sent back, you wrote last",
            talked(
                header(Status::ChangesRequested, agent, &["Ana"]),
                "Ana",
                &["claude:a", "Ana"],
            ),
            None,
        ),
        // Someone else's review: you never wrote in it and aren't named
        (
            "not yours",
            talked(
                header(Status::ChangesRequested, agent, &["Bo"]),
                "Bo",
                &["claude:a"],
            ),
            None,
        ),
        // A draft waits on no one, even one naming you
        (
            "a draft naming you",
            talked(header(Status::Draft, agent, &["Ana"]), "claude:a", &[]),
            None,
        ),
        // A live review you wrote in, answered by its agent: its decision is yours anyway
        (
            "live, answered",
            talked(header(Status::Live, agent, &["Ana"]), "Ana", &["claude:a"]),
            Some("review"),
        ),
        // Your own review: the comments of others on it
        (
            "your own, commented on",
            talked(header(Status::Waiting, Some("Ana"), &["Bo"]), "Bo", &[]),
            Some("answered"),
        ),
        // Decided: nothing waits, answered or not
        (
            "approved",
            talked(
                header(Status::Approved, agent, &["Ana"]),
                "Ana",
                &["claude:a"],
            ),
            None,
        ),
        (
            "closed",
            talked(
                header(Status::Closed, agent, &["Ana"]),
                "Ana",
                &["claude:a"],
            ),
            None,
        ),
    ] {
        assert_eq!(waiting_for(&review, &ana), expected, "{case}");
    }
}

#[test]
fn a_review_needs_its_reviewers_while_open_and_its_author_once_sent_back() {
    let ana = Actor::from("Ana");
    for (status, author, reviewers, expected) in [
        (Status::Waiting, Some("claude:a"), &["Ana"][..], true),
        (Status::Waiting, Some("claude:a"), &["Bo"][..], false),
        // No reviewers named: anyone but the author
        (Status::Waiting, Some("claude:a"), &[][..], true),
        (Status::Waiting, Some("Ana"), &[][..], false),
        (Status::Waiting, None, &[][..], true),
        (Status::Live, Some("claude:a"), &["Ana"][..], true),
        (Status::Live, Some("Ana"), &["Bo"][..], false),
        (Status::ChangesRequested, Some("Ana"), &["Bo"][..], true),
        (
            Status::ChangesRequested,
            Some("claude:a"),
            &["Ana"][..],
            false,
        ),
        (Status::Draft, Some("claude:a"), &["Ana"][..], false),
        (Status::Approved, Some("Ana"), &["Ana"][..], false),
        (Status::Closed, None, &[][..], false),
    ] {
        assert_eq!(
            needs(&header(status, author, reviewers), &ana),
            expected,
            "{status:?} {author:?} {reviewers:?}"
        );
    }
}

#[test]
fn a_review_fits_the_author_reviewer_and_branch_asked_for() {
    let h = header(Status::Waiting, Some("claude:a"), &["Ana", "Bo"]);
    let query = |author: Option<&str>, reviewer: Option<&str>, branch: Option<&str>| ReviewQuery {
        author: author.map(String::from),
        reviewer: reviewer.map(String::from),
        branch: branch.map(String::from),
        ..ReviewQuery::default()
    };
    for (q, on, expected) in [
        (query(None, None, None), Some("main"), true),
        (query(Some(""), Some(""), Some("")), None, true),
        (query(Some("claude:a"), None, None), None, true),
        (query(Some("claude:b"), None, None), None, false),
        (query(None, Some("Bo"), None), None, true),
        (query(None, Some("Cy"), None), None, false),
        (query(None, None, Some("dev/1")), Some("dev/1"), true),
        (query(None, None, Some("dev/1")), Some("main"), false),
        (query(None, None, Some("dev/1")), None, false),
        (
            query(Some("claude:a"), Some("Ana"), Some("main")),
            Some("main"),
            true,
        ),
    ] {
        assert_eq!(fits(&h, on, &q), expected, "{q:?} on {on:?}");
    }
    // No author yet: only a query naming none fits
    let unknown = header(Status::Waiting, None, &[]);
    assert!(!fits(&unknown, None, &query(Some("claude:a"), None, None)));
    assert!(fits(&unknown, None, &query(None, None, None)));
}

#[test]
fn a_status_is_one_of_a_reviews_or_open_every_one_not_decided() {
    assert_eq!(
        status_named("changes_requested").unwrap(),
        [Status::ChangesRequested]
    );
    assert_eq!(status_named("waiting").unwrap(), [Status::Waiting]);
    // ATM-613: the list's Open
    assert_eq!(status_named("open").unwrap(), Status::OPEN);
    for bad in ["", "Open", "merged"] {
        assert!(matches!(status_named(bad), Err(Error::Invalid(_))), "{bad}");
    }
}

fn review(title: &str, summary: &str, rounds: Vec<Round>) -> Review {
    let mut h = header(Status::Waiting, None, &[]);
    h.title = title.into();
    Review {
        header: h,
        summary: summary.into(),
        chains: Vec::new(),
        rounds,
    }
}

#[test]
fn a_review_mentions_the_query_when_every_word_is_in_its_title_or_summary() {
    let r = review("Fix the Parser", "Handles *empty* input", Vec::new());
    for (q, expected) in [
        ("", true),
        ("parser", true),
        ("PARSER empty", true),
        ("pars inp", true),
        ("parser lexer", false),
        ("missing", false),
    ] {
        assert_eq!(mentions(&r, &words(q)), expected, "{q:?}");
    }
}

#[test]
fn a_round_the_review_doesnt_have_is_not_found() {
    let r = review("T", "", Vec::new());
    let error = round_of(&r, 2).unwrap_err();
    assert!(matches!(error, Error::NotFound(_)), "{error:?}");
    assert_eq!(error.to_string(), "CR-1 has no round 2");
}

#[test]
fn a_status_list_with_one_unknown_status_is_refused_before_anything_is_read() {
    let memory = Project::new(Box::new(
        atm_db::MemoryStore::new(Vec::new(), Vec::new(), &serde_json::Map::new()).unwrap(),
    ));
    let query = ReviewQuery {
        status: vec!["open".into(), "merged".into()],
        ..ReviewQuery::default()
    };
    assert!(matches!(
        list_reviews(&memory, &query),
        Err(Error::Invalid(_))
    ));
    // valid statuses, no folder: not found
    let query = ReviewQuery {
        status: vec!["open".into()],
        ..ReviewQuery::default()
    };
    assert!(matches!(
        list_reviews(&memory, &query),
        Err(Error::NotFound(_))
    ));
}

#[test]
fn the_querys_words_are_every_word_in_any_case() {
    assert_eq!(words("  Two  words "), ["two", "words"]);
    assert_eq!(words(""), Vec::<String>::new());
}

const MODIFIED: &str = "diff --git a/src/a.rs b/src/a.rs\nindex 1..2 100644\n--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1 +1 @@\n-one\n+two\n";
const ADDED: &str = "diff --git a/new.md b/new.md\nnew file mode 100644\n--- /dev/null\n+++ b/new.md\n@@ -0,0 +1 @@\n+hi\n";
const DELETED: &str = "diff --git a/old.md b/old.md\ndeleted file mode 100644\n--- a/old.md\n+++ /dev/null\n@@ -1 +0,0 @@\n-bye\n";
const RENAMED: &str = "diff --git a/x.txt b/y.txt\nsimilarity index 90%\nrename from x.txt\nrename to y.txt\n@@ -1 +1 @@\n-a\n+b\n";
const MOVED: &str = "diff --git a/p q.txt b/r s.txt\nsimilarity index 100%\nrename from p q.txt\nrename to r s.txt\n";
const BINARY: &str = "diff --git a/img.png b/img.png\nindex 1..2 100644\nBinary files a/img.png and b/img.png differ\n";
const MODE: &str = "diff --git a/run me.sh b/run me.sh\nold mode 100644\nnew mode 100755\n";

#[test]
fn a_patch_is_cut_per_file_each_with_its_paths_and_how_it_changed() {
    let all = [MODIFIED, ADDED, DELETED, RENAMED, MOVED, BINARY, MODE].concat();
    let files = file_patches(&all);
    let summary: Vec<(&str, &str, &str, bool)> = files
        .iter()
        .map(|f| (f.old_path.as_str(), f.path.as_str(), f.status, f.binary))
        .collect();
    assert_eq!(
        summary,
        [
            ("src/a.rs", "src/a.rs", "modified", false),
            ("new.md", "new.md", "added", false),
            ("old.md", "old.md", "deleted", false),
            ("x.txt", "y.txt", "renamed", false),
            ("p q.txt", "r s.txt", "renamed", false),
            ("img.png", "img.png", "modified", true),
            ("run me.sh", "run me.sh", "modified", false),
        ]
    );
    // Each file's part is its own, whole: together they are the patch
    assert_eq!(files[0].patch, MODIFIED);
    assert_eq!(
        files.iter().map(|f| f.patch.as_str()).collect::<String>(),
        all
    );
}

/// A submodule entry's two commits, as the test reads them.
type Commits<'a> = Option<(Option<&'a str>, Option<&'a str>)>;

/// ATM-583: a moved, added or removed submodule is one entry with its commits; a file saying the words isn't one
#[test]
fn a_submodules_move_is_an_entry_with_its_two_commits() {
    let moved = "diff --git a/lib b/lib\nindex 1111111..2222222 160000\n--- a/lib\n+++ b/lib\n@@ -1 +1 @@\n-Subproject commit 1111111111\n+Subproject commit 2222222222-dirty\n";
    let added = "diff --git a/ext b/ext\nnew file mode 160000\nindex 0000000..3333333\n--- /dev/null\n+++ b/ext\n@@ -0,0 +1 @@\n+Subproject commit 3333333333\n";
    let removed = "diff --git a/old b/old\ndeleted file mode 160000\nindex 4444444..0000000\n--- a/old\n+++ /dev/null\n@@ -1 +0,0 @@\n-Subproject commit 4444444444\n";
    let text = "diff --git a/notes.md b/notes.md\nindex 5..6 100644\n--- a/notes.md\n+++ b/notes.md\n@@ -1 +1 @@\n-Subproject commit 1\n+Subproject commit 2\n";
    let files = file_patches(&[moved, added, removed, text, MODIFIED].concat());
    let entries: Vec<(&str, &str, Commits)> = files
        .iter()
        .map(|f| {
            let sub = f
                .submodule
                .as_ref()
                .map(|(a, b)| (a.as_deref(), b.as_deref()));
            (f.path.as_str(), f.status, sub)
        })
        .collect();
    assert_eq!(
        entries,
        [
            (
                "lib",
                "modified",
                Some((Some("1111111111"), Some("2222222222")))
            ),
            ("ext", "added", Some((None, Some("3333333333")))),
            ("old", "deleted", Some((Some("4444444444"), None))),
            ("notes.md", "modified", None),
            ("src/a.rs", "modified", None),
        ]
    );
}

#[test]
fn an_empty_or_headless_patch_has_no_files() {
    assert_eq!(file_patches(""), []);
    assert_eq!(file_patches("@@ -1 +1 @@\n-a\n+b\n"), []);
    // A line in a file's change that looks like a header is its content, not another file's
    let content = "diff --git a/a.md b/a.md\n--- a/a.md\n+++ b/a.md\n@@ -1 +1 @@\n-+++ b/other\n++++ b/else\n";
    let files = file_patches(content);
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, "a.md");
}

#[test]
fn a_header_names_one_path_twice_or_none() {
    assert_eq!(paths_of_header("a/x b/x").as_deref(), Some("x"));
    assert_eq!(paths_of_header("a/a b b/a b").as_deref(), Some("a b"));
    assert_eq!(paths_of_header("a/x b/y"), None);
    assert_eq!(paths_of_header("a/"), None);
    assert_eq!(paths_of_header(""), None);
}
