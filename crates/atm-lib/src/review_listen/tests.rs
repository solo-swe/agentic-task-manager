use super::*;
use crate::reviews::{Actor, Anchor, CommentId, ReviewId};
use atm_review::model::ChainId;
fn comment(id: &str, author: &str, body: &str, replies: Vec<Comment>) -> Comment {
    Comment {
        id: CommentId(id.into()),
        author: Actor::from(author),
        at: "2026-11-17T10:00:00Z".into(),
        body: body.into(),
        resolved: None,
        poke: false,
        replies,
    }
}
fn chain(path: Option<&str>, comments: Vec<Comment>) -> Chain {
    Chain {
        id: ChainId("t1".into()),
        anchor: Anchor {
            round: 1,
            path: path.map(String::from),
            lines: path.map(|_| (3, 4)),
            ..Anchor::default()
        },
        comments,
    }
}
fn review(status: Status, tasks: &[&str]) -> Review {
    Review {
        header: ReviewHeader {
            id: ReviewId("r".into()),
            reference: ReviewRef("CR-1".into()),
            number: 1,
            title: "T".into(),
            status,
            tasks: tasks.iter().map(|t| (*t).to_string()).collect(),
            author: Some(Actor::from("claude:me")),
            reviewers: Vec::new(),
            created: String::new(),
            updated: String::new(),
        },
        summary: String::new(),
        chains: Vec::new(),
        rounds: Vec::new(),
    }
}
fn files(list: &[&str]) -> Vec<String> {
    list.iter().map(|f| (*f).to_string()).collect()
}
/// The agent held ATM-1 (src/a.rs, now) and finished ATM-2 (docs/); ATM-3, another's, names src/b.rs; src/c.rs
/// and src/b.rs are changed in its working copy.
fn work() -> Work {
    Work {
        held: Some("ATM-1".into()),
        tasks: vec![
            ("ATM-1".into(), files(&["src/a.rs"])),
            ("ATM-2".into(), files(&["docs/"])),
        ],
        others: vec![("ATM-3".into(), files(&["src/b.rs"]))],
        uncommitted: files(&["src/c.rs", "src/b.rs"]),
    }
}
/// Whether the chain's first comment is the agent's, in words: `not mine`, `no task` (the agent's, about none of
/// its tasks), or the task's key.
fn about(review: &Review, chain: &Chain) -> String {
    let c = &chain.comments[0];
    match about_my_work(&work(), "claude:me", review, chain, c) {
        Mine::No => NOT_MINE.into(),
        Mine::About(task) => task.unwrap_or_else(|| NO_TASK.into()),
    }
}
const NOT_MINE: &str = "not mine";
const NO_TASK: &str = "no task";
fn by_hakan(path: Option<&str>, body: &str) -> Chain {
    chain(path, vec![comment("c1", "Hakan", body, Vec::new())])
}
fn whole_review() -> Chain {
    chain(None, Vec::new())
}
fn header(n: u32, status: Status, author: &str, tasks: &[&str], updated: &str) -> ReviewHeader {
    let mut h = review(status, tasks).header;
    h.number = n;
    h.reference = ReviewRef(format!("CR-{n}"));
    h.author = Some(Actor::from(author));
    h.updated = updated.into();
    h
}

mod cursor;
mod heard;
mod inbox;
mod lease;
mod work;
