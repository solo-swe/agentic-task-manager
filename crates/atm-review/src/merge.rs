//! Merging a review's two text files when it changed on two branches (docs/code-review-design.md §4): atm's merge
//! driver hands them here. Rounds never conflict (each is a new file), and two clones' reviews are two folders; what
//! can conflict is the same review commented on, or decided, on both.
//!
//! - `comments.md`: chains and comments by ID. What one side changed since the base is taken; what both added is kept,
//!   the other branch's new chains and comments renumbered when their IDs clash (IDs are counters, and replies are
//!   nested, so nothing else names them). A comment's text edited differently on both is a conflict: this branch's is
//!   kept, and it is reported.
//! - `review.md`: field by field, what one side changed since the base; changed on both, the side updated last.

use std::collections::{HashMap, HashSet};

use atm_common::Result;

use crate::files::comments_md::{self, CommentsFile};
use crate::files::review_md;
use crate::model::{Chain, ChainId, Comment, CommentId, ReviewHeader};

/// An ID's number: `c12` is 12.
fn number(id: &str) -> u32 {
    id.get(1..).and_then(|n| n.parse().ok()).unwrap_or(0)
}

fn same_comment(a: &Comment, b: &Comment) -> bool {
    a.author == b.author && a.at == b.at && a.body == b.body
}

fn same_chain(a: &Chain, b: &Chain) -> bool {
    a.anchor == b.anchor
        && match (a.comments.first(), b.comments.first()) {
            (Some(x), Some(y)) => x.author == y.author && x.at == y.at,
            (None, None) => true,
            _ => false,
        }
}

/// One side's value of a field against the base: the one that changed, or `None` when both did and differ.
fn three_way<'v, T: PartialEq>(base: Option<&T>, ours: &'v T, theirs: &'v T) -> Option<&'v T> {
    if ours == theirs || base == Some(theirs) {
        Some(ours)
    } else if base == Some(ours) {
        Some(theirs)
    } else {
        None
    }
}

struct Merger {
    base_chains: HashSet<String>,
    base_comments: HashMap<String, Comment>,
    used_comments: HashSet<String>,
    used_chains: HashSet<String>,
    next_comment: u32,
    next_chain: u32,
    conflicts: Vec<String>,
}

impl Merger {
    fn fresh_comment(&mut self) -> CommentId {
        self.next_comment += 1;
        CommentId(format!("c{}", self.next_comment))
    }

    /// A comment new on the other branch, with its replies, under IDs of its own.
    fn adopt(&mut self, comment: &Comment) -> Comment {
        let mut comment = comment.clone();
        if self.used_comments.contains(&comment.id.0) {
            comment.id = self.fresh_comment();
        }
        self.used_comments.insert(comment.id.0.clone());
        comment.replies = comment.replies.iter().map(|r| self.adopt(r)).collect();
        comment
    }

    /// The other branch's comments into this branch's list (top-level comments, or one comment's replies).
    fn merge_list(&mut self, ours: &mut Vec<Comment>, theirs: &[Comment]) {
        for their in theirs {
            let base = self.base_comments.get(&their.id.0).cloned();
            let at = ours.iter().position(|c| c.id == their.id);
            match at {
                Some(i) if base.is_some() || same_comment(&ours[i], their) => {
                    let mine = &mut ours[i];
                    match three_way(base.as_ref().map(|b| &b.body), &mine.body, &their.body) {
                        Some(body) => mine.body = body.clone(),
                        None => self.conflicts.push(format!(
                            "comments.md {}: its text changed on both branches (this branch's kept)",
                            mine.id
                        )),
                    }
                    // A resolved box both changed can only differ one way: resolved on one, reopened on the other.
                    // Open wins, so a reviewer's open question isn't closed by a merge
                    mine.resolved = three_way(
                        base.as_ref().map(|b| &b.resolved),
                        &mine.resolved,
                        &their.resolved,
                    )
                    .copied()
                    .unwrap_or(Some(false));
                    let replies = their.replies.clone();
                    let mut mine_replies = std::mem::take(&mut ours[i].replies);
                    self.merge_list(&mut mine_replies, &replies);
                    ours[i].replies = mine_replies;
                }
                // This branch deleted it (by hand): it stays deleted
                _ if base.is_some() => {}
                _ => {
                    let adopted = self.adopt(their);
                    ours.push(adopted);
                }
            }
        }
        // Oldest first, as the file keeps them (the times are ISO 8601, so they sort as text)
        ours.sort_by(|a, b| a.at.cmp(&b.at));
    }
}

fn all_ids(file: &CommentsFile) -> (Vec<String>, Vec<String>) {
    let chains = file.chains.iter().map(|c| c.id.0.clone()).collect();
    let comments = file
        .chains
        .iter()
        .flat_map(|chain| chain.comments.iter().flat_map(Comment::all))
        .map(|c| c.id.0.clone())
        .collect();
    (chains, comments)
}

/// `comments.md` merged: the text to write, and the conflicts (each kept as this branch has it).
#[must_use]
pub fn merge_comments(base: &str, ours: &str, theirs: &str) -> (String, Vec<String>) {
    let [base, ours, theirs] = [base, ours, theirs].map(comments_md::read);
    let (base_chain_ids, _) = all_ids(&base);
    let (our_chains, our_comments) = all_ids(&ours);
    let max = |ids: &[&CommentsFile], chains: bool| {
        ids.iter()
            .flat_map(|f| {
                let (c, m) = all_ids(f);
                if chains { c } else { m }
            })
            .map(|id| number(&id))
            .max()
            .unwrap_or(0)
    };
    let mut merger = Merger {
        base_chains: base_chain_ids.into_iter().collect(),
        base_comments: base
            .chains
            .iter()
            .flat_map(|chain| chain.comments.iter().flat_map(Comment::all))
            .map(|c| (c.id.0.clone(), c.clone()))
            .collect(),
        used_comments: our_comments.into_iter().collect(),
        used_chains: our_chains.into_iter().collect(),
        next_comment: max(&[&base, &ours, &theirs], false),
        next_chain: max(&[&base, &ours, &theirs], true),
        conflicts: Vec::new(),
    };
    let mut result = ours.clone();
    for their in &theirs.chains {
        let in_base = merger.base_chains.contains(&their.id.0);
        match result.chains.iter().position(|c| c.id == their.id) {
            Some(i) if in_base || same_chain(&result.chains[i], their) => {
                let mut comments = std::mem::take(&mut result.chains[i].comments);
                merger.merge_list(&mut comments, &their.comments);
                result.chains[i].comments = comments;
            }
            _ if in_base => {} // deleted on this branch
            _ => {
                let mut chain = their.clone();
                if merger.used_chains.contains(&chain.id.0) {
                    merger.next_chain += 1;
                    chain.id = ChainId(format!("t{}", merger.next_chain));
                }
                merger.used_chains.insert(chain.id.0.clone());
                chain.comments = their.comments.iter().map(|c| merger.adopt(c)).collect();
                result.chains.push(chain);
            }
        }
    }
    let kept: HashSet<String> = result.unread.iter().map(|u| u.text.clone()).collect();
    result.unread.extend(
        theirs
            .unread
            .iter()
            .filter(|u| !kept.contains(&u.text))
            .cloned(),
    );
    (comments_md::write(&result), merger.conflicts)
}

/// A field of `review.md`: what one side changed since the base, or, changed on both, the later side's.
fn pick<T: PartialEq + Clone>(base: Option<&T>, ours: &T, theirs: &T, theirs_later: bool) -> T {
    three_way(base, ours, theirs)
        .unwrap_or(if theirs_later { theirs } else { ours })
        .clone()
}

/// `review.md` merged: field by field, what one side changed since the base; what both changed, the side updated
/// last. Never a conflict.
///
/// # Errors
/// A side whose header can't be read (the merge is then git's, by hand).
pub fn merge_review(base: &str, ours: &str, theirs: &str) -> Result<String> {
    let base = review_md::read(base).ok();
    let (our_header, our_summary) = review_md::read(ours)?;
    let (their_header, their_summary) = review_md::read(theirs)?;
    let theirs_later = their_header.updated > our_header.updated;
    let (b_header, b_summary) = match &base {
        Some((h, s)) => (Some(h), Some(s)),
        None => (None, None),
    };
    let header = ReviewHeader {
        title: pick(
            b_header.map(|h| &h.title),
            &our_header.title,
            &their_header.title,
            theirs_later,
        ),
        status: pick(
            b_header.map(|h| &h.status),
            &our_header.status,
            &their_header.status,
            theirs_later,
        ),
        tasks: pick(
            b_header.map(|h| &h.tasks),
            &our_header.tasks,
            &their_header.tasks,
            theirs_later,
        ),
        reviewers: pick(
            b_header.map(|h| &h.reviewers),
            &our_header.reviewers,
            &their_header.reviewers,
            theirs_later,
        ),
        updated: if theirs_later {
            their_header.updated.clone()
        } else {
            our_header.updated.clone()
        },
        ..our_header.clone()
    };
    let summary = pick(b_summary, &our_summary, &their_summary, theirs_later);
    review_md::write(&header, &summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Actor, Anchor, ReviewId, ReviewRef, Status};

    fn comment(id: &str, author: &str, at: &str, body: &str, resolved: Option<bool>) -> Comment {
        Comment {
            id: CommentId(id.into()),
            author: Actor(author.into()),
            at: at.into(),
            body: body.into(),
            resolved,
            replies: Vec::new(),
        }
    }

    fn chain(id: &str, path: Option<&str>, comments: Vec<Comment>) -> Chain {
        Chain {
            id: ChainId(id.into()),
            anchor: Anchor {
                round: 1,
                path: path.map(String::from),
                lines: path.map(|_| (1, 2)),
                ..Anchor::default()
            },
            comments,
        }
    }

    fn file(chains: Vec<Chain>) -> String {
        comments_md::write(&CommentsFile {
            chains,
            unread: Vec::new(),
        })
    }

    const T1: &str = "2026-11-17T10:00:00Z";
    const T2: &str = "2026-11-17T11:00:00Z";
    const T3: &str = "2026-11-17T12:00:00Z";

    #[test]
    fn comments_added_on_both_branches_are_all_kept_the_others_renumbered() {
        let first = comment("c1", "Hakan", T1, "Why?", Some(false));
        let base = file(vec![chain("t1", Some("a.rs"), vec![first.clone()])]);
        // Ours: a reply (c2) and a new chain (t2, c3); theirs: another reply (c2) and another chain (t2, c3)
        let mut ours_first = first.clone();
        ours_first
            .replies
            .push(comment("c2", "claude:A", T2, "Because.", None));
        let ours = file(vec![
            chain("t1", Some("a.rs"), vec![ours_first]),
            chain(
                "t2",
                Some("b.rs"),
                vec![comment("c3", "Hakan", T2, "On b.", Some(false))],
            ),
        ]);
        let mut their_first = first.clone();
        their_first
            .replies
            .push(comment("c2", "Ana", T3, "Agreed.", None));
        their_first.resolved = Some(true);
        let theirs = file(vec![
            chain("t1", Some("a.rs"), vec![their_first]),
            chain(
                "t2",
                None,
                vec![comment("c3", "Ana", T3, "Overall fine.", Some(false))],
            ),
        ]);
        let (text, conflicts) = merge_comments(&base, &ours, &theirs);
        assert!(conflicts.is_empty(), "{conflicts:?}");
        let merged = comments_md::read(&text);
        assert!(merged.unread.is_empty(), "{text}");
        let ids: Vec<(String, Vec<(String, String)>)> = merged
            .chains
            .iter()
            .map(|c| {
                (
                    c.id.0.clone(),
                    c.comments
                        .iter()
                        .flat_map(Comment::all)
                        .map(|x| (x.id.0.clone(), x.body.clone()))
                        .collect(),
                )
            })
            .collect();
        assert_eq!(
            ids,
            [
                (
                    "t1".into(),
                    vec![
                        ("c1".into(), "Why?".into()),
                        ("c2".into(), "Because.".into()),
                        ("c4".into(), "Agreed.".into())
                    ]
                ),
                ("t2".into(), vec![("c3".into(), "On b.".into())]),
                ("t3".into(), vec![("c5".into(), "Overall fine.".into())]),
            ]
        );
        assert_eq!(
            merged.chains[0].comments[0].resolved,
            Some(true),
            "resolved on theirs, untouched on ours"
        );
    }

    #[test]
    fn the_same_text_edited_on_both_is_a_conflict_and_ours_stays_and_open_wins() {
        let base = file(vec![chain(
            "t1",
            None,
            vec![comment("c1", "Hakan", T1, "One.", Some(true))],
        )]);
        let ours = file(vec![chain(
            "t1",
            None,
            vec![comment("c1", "Hakan", T1, "One, ours.", Some(false))],
        )]);
        let theirs = file(vec![chain(
            "t1",
            None,
            vec![comment("c1", "Hakan", T1, "One, theirs.", Some(true))],
        )]);
        let (text, conflicts) = merge_comments(&base, &ours, &theirs);
        assert_eq!(conflicts.len(), 1);
        let merged = comments_md::read(&text);
        assert_eq!(merged.chains[0].comments[0].body, "One, ours.");
        // Theirs didn't change the box; ours reopened it
        assert_eq!(merged.chains[0].comments[0].resolved, Some(false));
        // An identical new comment on both (a merge of a merge) is one comment
        let same = file(vec![chain(
            "t1",
            None,
            vec![
                comment("c1", "Hakan", T1, "One.", Some(true)),
                comment("c2", "Ana", T2, "Two.", Some(false)),
            ],
        )]);
        let (text, _) = merge_comments(&base, &same, &same);
        assert_eq!(comments_md::read(&text).chains[0].comments.len(), 2);
    }

    fn header(status: Status, title: &str, updated: &str) -> ReviewHeader {
        ReviewHeader {
            id: ReviewId("0193a8f2-0000-7000-8000-000000003fa1".into()),
            reference: ReviewRef::numbered(7),
            number: 7,
            title: title.into(),
            status,
            tasks: vec!["ATM-1".into()],
            author: Some(Actor("claude:A".into())),
            reviewers: Vec::new(),
            created: T1.into(),
            updated: updated.into(),
        }
    }

    #[test]
    fn the_header_takes_each_sides_change_and_on_both_the_later() {
        let base = review_md::write(&header(Status::Open, "Title", T1), "Summary.").unwrap();
        // Ours retitled it (earlier); theirs decided it (later) and changed the summary
        let ours = review_md::write(&header(Status::Open, "Better title", T2), "Summary.").unwrap();
        let theirs =
            review_md::write(&header(Status::Approved, "Title", T3), "Summary, theirs.").unwrap();
        let (merged, summary) =
            review_md::read(&merge_review(&base, &ours, &theirs).unwrap()).unwrap();
        assert_eq!(
            (
                merged.title.as_str(),
                merged.status,
                merged.updated.as_str(),
                summary.as_str()
            ),
            ("Better title", Status::Approved, T3, "Summary, theirs.")
        );
        // Both changed the status: the later wins
        let ours =
            review_md::write(&header(Status::ChangesRequested, "Title", T2), "Summary.").unwrap();
        let (merged, _) = review_md::read(&merge_review(&base, &ours, &theirs).unwrap()).unwrap();
        assert_eq!(merged.status, Status::Approved);
        assert!(merge_review(&base, "no header", &theirs).is_err());
    }

    /// Each chain's ID with its comments' IDs and texts, depth first.
    fn shape(text: &str) -> Vec<(String, Vec<(String, String)>)> {
        comments_md::read(text)
            .chains
            .iter()
            .map(|c| {
                (
                    c.id.0.clone(),
                    c.comments
                        .iter()
                        .flat_map(Comment::all)
                        .map(|x| (x.id.0.clone(), x.body.clone()))
                        .collect(),
                )
            })
            .collect()
    }

    fn with_replies(mut comment: Comment, replies: Vec<Comment>) -> Comment {
        comment.replies = replies;
        comment
    }

    #[test]
    fn what_ours_deleted_since_the_base_stays_deleted() {
        let first = comment("c1", "Hakan", T1, "Why?", Some(false));
        let reply = comment("c3", "Ana", T2, "Because.", None);
        let base = file(vec![
            chain(
                "t1",
                Some("a.rs"),
                vec![with_replies(first.clone(), vec![reply.clone()])],
            ),
            chain(
                "t2",
                None,
                vec![comment("c2", "Hakan", T1, "Overall.", Some(false))],
            ),
        ]);
        // Ours deleted the chain t2 and the reply c3 by hand; theirs kept both, and edited them
        let ours = file(vec![chain("t1", Some("a.rs"), vec![first.clone()])]);
        let mut edited_reply = reply;
        edited_reply.body = "Because, edited.".into();
        let theirs = file(vec![
            chain(
                "t1",
                Some("a.rs"),
                vec![with_replies(first, vec![edited_reply])],
            ),
            chain(
                "t2",
                None,
                vec![comment("c2", "Hakan", T1, "Overall, edited.", Some(false))],
            ),
        ]);
        let (text, conflicts) = merge_comments(&base, &ours, &theirs);
        assert!(conflicts.is_empty(), "{conflicts:?}");
        assert_eq!(
            shape(&text),
            [("t1".into(), vec![("c1".into(), "Why?".into())])]
        );
    }

    #[test]
    fn a_text_edited_on_one_side_only_is_taken_from_that_side() {
        let base = file(vec![chain(
            "t1",
            None,
            vec![
                comment("c1", "Hakan", T1, "One.", Some(false)),
                comment("c2", "Ana", T2, "Two.", Some(false)),
            ],
        )]);
        // Ours edited c1, theirs edited c2
        let ours = file(vec![chain(
            "t1",
            None,
            vec![
                comment("c1", "Hakan", T1, "One, ours.", Some(false)),
                comment("c2", "Ana", T2, "Two.", Some(false)),
            ],
        )]);
        let theirs = file(vec![chain(
            "t1",
            None,
            vec![
                comment("c1", "Hakan", T1, "One.", Some(false)),
                comment("c2", "Ana", T2, "Two, theirs.", Some(false)),
            ],
        )]);
        let (text, conflicts) = merge_comments(&base, &ours, &theirs);
        assert!(conflicts.is_empty(), "{conflicts:?}");
        assert_eq!(
            shape(&text),
            [(
                "t1".into(),
                vec![
                    ("c1".into(), "One, ours.".into()),
                    ("c2".into(), "Two, theirs.".into())
                ]
            )]
        );
    }

    #[test]
    fn lines_atm_could_not_read_on_theirs_are_added_once() {
        let unread = |texts: &[&str]| {
            texts
                .iter()
                .enumerate()
                .map(|(i, t)| comments_md::Unread {
                    after: None,
                    line: i + 1,
                    text: (*t).into(),
                })
                .collect()
        };
        let chains = vec![chain(
            "t1",
            None,
            vec![comment("c1", "Hakan", T1, "One.", Some(false))],
        )];
        let side = |texts: &[&str]| {
            comments_md::write(&CommentsFile {
                chains: chains.clone(),
                unread: unread(texts),
            })
        };
        let base = side(&["stray A"]);
        let ours = side(&["stray A"]);
        let theirs = side(&["stray A", "stray B"]);
        let (text, _) = merge_comments(&base, &ours, &theirs);
        let texts: Vec<String> = comments_md::read(&text)
            .unread
            .into_iter()
            .map(|u| u.text)
            .collect();
        assert_eq!(texts, ["stray A", "stray B"], "{text}");
        assert_eq!(shape(&text).len(), 1);
    }

    #[test]
    fn a_chain_new_on_both_with_the_same_id_anchor_and_first_comment_is_one_chain() {
        let first = comment("c1", "Hakan", T1, "Hi.", Some(false));
        let ours = file(vec![chain("t1", Some("a.rs"), vec![first.clone()])]);
        let theirs = file(vec![chain(
            "t1",
            Some("a.rs"),
            vec![with_replies(
                first,
                vec![comment("c2", "Ana", T2, "Hello.", None)],
            )],
        )]);
        let (text, conflicts) = merge_comments("", &ours, &theirs);
        assert!(conflicts.is_empty(), "{conflicts:?}");
        assert_eq!(
            shape(&text),
            [(
                "t1".into(),
                vec![("c1".into(), "Hi.".into()), ("c2".into(), "Hello.".into())]
            )]
        );
    }

    #[test]
    fn comments_are_kept_oldest_first() {
        let first = comment("c1", "Hakan", T1, "One.", Some(false));
        let base = file(vec![chain("t1", None, vec![first.clone()])]);
        // Ours added c2 later than theirs added its c2, which is renumbered c3
        let ours = file(vec![chain(
            "t1",
            None,
            vec![
                first.clone(),
                comment("c2", "Hakan", T3, "Late.", Some(false)),
            ],
        )]);
        let theirs = file(vec![chain(
            "t1",
            None,
            vec![first, comment("c2", "Ana", T2, "Early.", Some(false))],
        )]);
        let (text, _) = merge_comments(&base, &ours, &theirs);
        assert_eq!(
            shape(&text),
            [(
                "t1".into(),
                vec![
                    ("c1".into(), "One.".into()),
                    ("c3".into(), "Early.".into()),
                    ("c2".into(), "Late.".into())
                ]
            )]
        );
    }

    #[test]
    fn with_no_base_both_sides_chains_are_kept() {
        assert_eq!(merge_comments("", "", ""), (String::new(), Vec::new()));
        let ours = file(vec![chain(
            "t1",
            Some("a.rs"),
            vec![comment("c1", "Hakan", T1, "Ours.", Some(false))],
        )]);
        let theirs = file(vec![chain(
            "t1",
            Some("a.rs"),
            vec![comment("c1", "Ana", T2, "Theirs.", Some(false))],
        )]);
        let (text, conflicts) = merge_comments("", &ours, &theirs);
        assert!(conflicts.is_empty(), "{conflicts:?}");
        assert_eq!(
            shape(&text),
            [
                ("t1".into(), vec![("c1".into(), "Ours.".into())]),
                ("t2".into(), vec![("c2".into(), "Theirs.".into())]),
            ]
        );
        // Only one side has a file
        assert_eq!(shape(&merge_comments("", "", &theirs).0), shape(&theirs));
        assert_eq!(shape(&merge_comments("", &ours, "").0), shape(&ours));
    }

    fn merged(base: &str, ours: &str, theirs: &str) -> (ReviewHeader, String) {
        review_md::read(&merge_review(base, ours, theirs).unwrap()).unwrap()
    }

    #[test]
    fn with_an_unreadable_base_both_changed_and_the_later_side_wins() {
        let ours = review_md::write(&header(Status::Open, "Ours", T2), "Ours.").unwrap();
        let theirs = review_md::write(&header(Status::Approved, "Theirs", T3), "Theirs.").unwrap();
        let (h, s) = merged("not a review", &ours, &theirs);
        assert_eq!(
            (h.title.as_str(), h.status, h.updated.as_str(), s.as_str()),
            ("Theirs", Status::Approved, T3, "Theirs.")
        );
        // Ours later: ours wins
        let ours = review_md::write(
            &header(Status::Open, "Ours", "2026-11-17T13:00:00Z"),
            "Ours.",
        )
        .unwrap();
        let (h, s) = merged("", &ours, &theirs);
        assert_eq!(
            (h.title.as_str(), h.status, s.as_str()),
            ("Ours", Status::Open, "Ours.")
        );
    }

    #[test]
    fn both_changed_and_updated_at_the_same_time_keeps_ours() {
        let base = review_md::write(&header(Status::Open, "Title", T1), "Summary.").unwrap();
        let ours =
            review_md::write(&header(Status::ChangesRequested, "Title", T2), "Summary.").unwrap();
        let theirs = review_md::write(&header(Status::Approved, "Title", T2), "Summary.").unwrap();
        let (h, _) = merged(&base, &ours, &theirs);
        assert_eq!(
            (h.status, h.updated.as_str()),
            (Status::ChangesRequested, T2)
        );
    }

    #[test]
    fn tasks_reviewers_and_summary_are_merged_field_by_field() {
        let base = review_md::write(&header(Status::Open, "Title", T1), "Summary.").unwrap();
        // Ours changed the tasks and the summary (earlier); theirs changed the reviewers and the title (later)
        let mut our_header = header(Status::Open, "Title", T2);
        our_header.tasks = vec!["ATM-1".into(), "ATM-2".into()];
        let ours = review_md::write(&our_header, "Summary, ours.").unwrap();
        let mut their_header = header(Status::Open, "Retitled", T3);
        their_header.reviewers = vec![Actor("Ana".into())];
        let theirs = review_md::write(&their_header, "Summary.").unwrap();
        let (h, s) = merged(&base, &ours, &theirs);
        assert_eq!(h.tasks, ["ATM-1", "ATM-2"]);
        assert_eq!(h.reviewers, [Actor("Ana".into())]);
        assert_eq!(h.title, "Retitled");
        assert_eq!(s, "Summary, ours.");
        assert_eq!(h.updated, T3);
    }

    #[test]
    fn an_unreadable_theirs_is_an_error() {
        let base = review_md::write(&header(Status::Open, "Title", T1), "Summary.").unwrap();
        assert!(merge_review(&base, &base, "no header").is_err());
    }
}
