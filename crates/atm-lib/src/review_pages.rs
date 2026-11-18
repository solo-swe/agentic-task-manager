//! What the reviews' pages read (docs/code-review-design.md §3.11, ATM-574): the list (search and filters, the latest
//! activity first, which need the user), and a review's page for one view of it: the whole change of a round, or a
//! round against an earlier one (§3.6), each file with its patch and the two sides to read it from (a commit, or a
//! tree a round is rebuilt to from its diff), and every chain placed in the round shown, or drifted. Both front ends
//! answer these (the REST API's read routes, the app's commands), with the project opened as the user.

use atm_common::{Error, Result, TASK_DIR};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::project::Project;
use crate::reviews::{
    self, Actor, Chain, FileStore, Found, Patch, Review, ReviewFilter, ReviewHeader, ReviewId,
    ReviewRef, ReviewStore, Round, RoundKind, Status, list_everywhere, rounds,
};
use atm_review::rounds::RoundDiffs;

/// Which reviews a page lists (`…/reviews?q=&status=&author=&reviewer=&task=&branch=`): each one given must fit.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewQuery {
    /// Every word in its title, summary or comments, in any case.
    #[serde(default)]
    pub q: String,
    /// One of these statuses (none: any).
    #[serde(default)]
    pub status: Vec<String>,
    /// Its author.
    #[serde(default)]
    pub author: Option<String>,
    /// One of its reviewers.
    #[serde(default)]
    pub reviewer: Option<String>,
    /// It reviews this task (its key).
    #[serde(default)]
    pub task: Option<String>,
    /// It is on this branch.
    #[serde(default)]
    pub branch: Option<String>,
}

/// The statuses a query's `status=` stands for: `open`, every review not decided yet (ATM-613: the list's *Open*);
/// any other, its own.
fn status_named(name: &str) -> Result<Vec<Status>> {
    Status::filter_named(name).ok_or_else(|| {
        Error::Invalid(format!(
            "{name}: not a review status (open, draft, waiting, live, changes_requested, approved, closed)"
        ))
    })
}

/// Whether `header` waits on `me`: open or live, with `me` one of its reviewers (anyone but its author when it names
/// none); or sent back, with `me` its author.
#[must_use]
pub fn needs(header: &ReviewHeader, me: &Actor) -> bool {
    let authored = header.author.as_ref() == Some(me);
    match header.status {
        Status::Waiting | Status::Live => {
            if header.reviewers.is_empty() {
                !authored
            } else {
                header.reviewers.contains(me)
            }
        }
        Status::ChangesRequested => authored,
        Status::Draft | Status::Approved | Status::Closed => false,
    }
}

/// What a review waits on `me` for (ATM-606), if anything: `"review"`, its decision or round is theirs ([`needs`]);
/// `"answered"`, someone else wrote in it after `me` last did, in a review `me` takes part in (its author, a reviewer
/// it names, or one who wrote in it) that isn't decided yet. Nothing is stored of what `me` has seen: a GET writes
/// nothing (security I11), and an answer is cleared by answering, or by the review being decided.
#[must_use]
pub fn waiting_for(review: &Review, me: &Actor) -> Option<&'static str> {
    let header = &review.header;
    if needs(header, me) {
        return Some("review");
    }
    // Decided, or a draft its author is still putting together (§3.1: waiting on no one)
    if matches!(
        header.status,
        Status::Approved | Status::Closed | Status::Draft
    ) {
        return None;
    }
    // Which came last, by the comments' numbers (c1, c2, … in the order they were written; two in one second have the
    // same time)
    let number = |id: &str| id.trim_start_matches('c').parse::<u64>().unwrap_or(0);
    let (mut mine, mut others): (Option<u64>, Option<u64>) = (None, None);
    for comment in review.comments() {
        let latest = if &comment.author == me {
            &mut mine
        } else {
            &mut others
        };
        *latest = (*latest).max(Some(number(&comment.id.0)));
    }
    let takes_part =
        mine.is_some() || header.author.as_ref() == Some(me) || header.reviewers.contains(me);
    match (others, mine) {
        (Some(_), None) if takes_part => Some("answered"),
        (Some(theirs), Some(own)) if theirs > own => Some("answered"),
        _ => None,
    }
}

/// The reviews of the project's own store that wait on `me` (ATM-606), as [`waiting_for`] says: `(header, why)`.
/// Its working copies' reviews aren't read: this is for every project's summary, on each refresh. A store that can't
/// be read has none.
#[must_use]
pub fn waiting_reviews(project: &Project) -> Vec<(ReviewHeader, &'static str)> {
    let (Some(root), Ok(store)) = (project.root(), reviews::store(project)) else {
        return Vec::new();
    };
    // The user at the machine, whatever environment the summary is made in (an agent's shell started the server, say)
    let me = Actor(crate::actor::user_identity(root));
    let mut waiting: Vec<(ReviewHeader, &'static str)> = store
        .reviews()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|r| waiting_for(&r, &me).map(|why| (r.header, why)))
        .collect();
    // The longest waiting first
    waiting.sort_by(|a, b| a.0.updated.cmp(&b.0.updated));
    waiting
}

/// What changes when any of the project's reviews does ([`FileStore::stamp`]), for a summary's cache: `null` without
/// a review folder.
#[must_use]
pub fn reviews_stamp(project: &Project) -> Value {
    reviews::store(project).map_or(Value::Null, |s| {
        let (files, latest) = s.stamp();
        json!([files, latest.to_string()])
    })
}

/// Whether a review found on `branch` fits the query's author, reviewer and branch (the store has done its statuses
/// and task).
fn fits(header: &ReviewHeader, branch: Option<&str>, query: &ReviewQuery) -> bool {
    let named = |wanted: &Option<String>, actual: Option<&str>| {
        wanted
            .as_deref()
            .is_none_or(|w| w.is_empty() || actual == Some(w))
    };
    named(&query.author, header.author.as_ref().map(|a| a.0.as_str()))
        && query
            .reviewer
            .as_deref()
            .is_none_or(|r| r.is_empty() || header.reviewers.iter().any(|x| x.0 == r))
        && named(&query.branch, branch)
}

/// The query's words, in lower case.
fn words(q: &str) -> Vec<String> {
    q.split_whitespace().map(str::to_lowercase).collect()
}

/// Whether every word is in the review's title, summary or a comment.
fn mentions(review: &Review, words: &[String]) -> bool {
    let mut text = format!("{} {}", review.header.title, review.summary);
    for comment in review.comments() {
        text.push(' ');
        text.push_str(&comment.body);
    }
    let text = text.to_lowercase();
    words.iter().all(|w| text.contains(w))
}

/// A review found in one of the working copies, read from that copy (else the project's own store).
fn whole(found: &Found, store: &FileStore) -> Result<Review> {
    FileStore::open(&found.working_copy.join(TASK_DIR))
        .and_then(|s| s.get_review(&ReviewRef(found.header.id.0.clone())))
        .or_else(|_| store.get_review(&found.header.reference))
}

fn me(project: &Project) -> Actor {
    Actor(project.actor().unwrap_or_default().to_string())
}

/// The reviews of every working copy the query keeps, the latest activity first: `{reviews: [{review, branch, chains,
/// open, needs_you, waiting}], you}` (`waiting`: [`waiting_for`]'s why, or `null`).
///
/// # Errors
/// A status that isn't one; the project has no folder; a review that can't be read.
pub fn list_reviews(project: &Project, query: &ReviewQuery) -> Result<Value> {
    let statuses = query
        .status
        .iter()
        .map(|s| status_named(s))
        .collect::<Result<Vec<_>>>()?
        .concat();
    let root = project
        .root()
        .ok_or_else(|| Error::NotFound("this project has no folder with reviews".into()))?;
    let store = reviews::store(project)?;
    let filter = ReviewFilter {
        statuses,
        task: query
            .task
            .as_deref()
            .filter(|t| !t.is_empty())
            .map(|t| reviews::task_reference(project, t)),
    };
    let (me, words) = (me(project), words(&query.q));
    let mut listed: Vec<(String, Value)> = Vec::new();
    for found in list_everywhere(root, &filter)? {
        if !fits(&found.header, found.branch.as_deref(), query) {
            continue;
        }
        let review = whole(&found, &store)?;
        if !mentions(&review, &words) {
            continue;
        }
        let entry = json!({
            "review": review.header,
            "branch": found.branch,
            "chains": review.chains.len(),
            "open": review.open_comments().count(),
            "needs_you": needs(&review.header, &me),
            "waiting": waiting_for(&review, &me),
        });
        listed.push((review.header.updated.clone(), entry));
    }
    listed.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(json!({
        "reviews": listed.into_iter().map(|(_, e)| e).collect::<Vec<_>>(),
        "you": me.0,
    }))
}

/// One file of a patch: its paths, how it changed, and its part of the patch (from its `diff --git` line).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePatch {
    /// Its path after.
    pub path: String,
    /// Its path before (the same unless it moved).
    pub old_path: String,
    /// `added`, `modified`, `deleted` or `renamed`.
    pub status: &'static str,
    /// Binary: listed, no lines.
    pub binary: bool,
    /// A submodule this change moved (ATM-583): the commits it was at and is at (none where it was added or
    /// removed), a `-dirty` mark dropped; none for a file.
    pub submodule: Option<(Option<String>, Option<String>)>,
    /// Its part of the patch.
    pub patch: String,
}

/// A submodule's move, from its part of the patch: git writes it as `-Subproject commit <old>` and `+Subproject
/// commit <new>`, no file lines.
fn moved_submodule(lines: &[&str]) -> Option<(Option<String>, Option<String>)> {
    let commit = |sign: char| {
        lines.iter().find_map(|l| {
            let rest = l.trim_end_matches('\n').strip_prefix(sign)?;
            let sha = rest.strip_prefix("Subproject commit ")?;
            Some(sha.trim_end_matches("-dirty").to_string())
        })
    };
    let gitlink = lines
        .iter()
        .take_while(|l| !l.starts_with("@@"))
        .any(|l| l.contains(" 160000"));
    let (old, new) = (commit('-'), commit('+'));
    (gitlink && (old.is_some() || new.is_some())).then_some((old, new))
}

/// The two paths of a `diff --git a/<p> b/<p>` line, when it names one path twice (a path with a space included).
fn paths_of_header(rest: &str) -> Option<String> {
    let len = rest.len().checked_sub(5)?;
    if len % 2 != 0 {
        return None;
    }
    let path = rest.get(2..2 + len / 2)?;
    (rest == format!("a/{path} b/{path}")).then(|| path.to_string())
}

/// A patch, cut per file.
#[must_use]
pub fn file_patches(patch: &str) -> Vec<FilePatch> {
    let mut out: Vec<FilePatch> = Vec::new();
    let mut lines: Vec<&str> = Vec::new();
    let mut finish = |lines: &mut Vec<&str>| {
        if let Some(file) = one_file(lines) {
            out.push(file);
        }
        lines.clear();
    };
    for line in patch.split_inclusive('\n') {
        if line.starts_with("diff --git ") && !lines.is_empty() {
            finish(&mut lines);
        }
        lines.push(line);
    }
    finish(&mut lines);
    out
}

fn one_file(lines: &[&str]) -> Option<FilePatch> {
    let header = lines
        .first()?
        .trim_end_matches('\n')
        .strip_prefix("diff --git ")?;
    let (mut old, mut new) = (None::<String>, None::<String>);
    let (mut status, mut binary) = ("modified", false);
    for line in lines.iter().map(|l| l.trim_end_matches('\n')) {
        if line.starts_with("@@") {
            break;
        }
        if line.starts_with("new file mode") {
            status = "added";
        } else if line.starts_with("deleted file mode") {
            status = "deleted";
        } else if let Some(from) = line.strip_prefix("rename from ") {
            (old, status) = (Some(from.to_string()), "renamed");
        } else if let Some(to) = line.strip_prefix("rename to ") {
            new = Some(to.to_string());
        } else if let Some(a) = line.strip_prefix("--- a/") {
            old.get_or_insert_with(|| a.to_string());
        } else if let Some(b) = line.strip_prefix("+++ b/") {
            new.get_or_insert_with(|| b.to_string());
        } else if line.starts_with("Binary files ") || line == "GIT binary patch" {
            binary = true;
        }
    }
    let same = paths_of_header(header);
    let path = new
        .clone()
        .or_else(|| old.clone())
        .or_else(|| same.clone())?;
    Some(FilePatch {
        old_path: old.or(same).unwrap_or_else(|| path.clone()),
        path,
        status,
        binary,
        submodule: moved_submodule(lines),
        patch: lines.concat(),
    })
}

fn round_of(review: &Review, number: u32) -> Result<&Round> {
    review
        .rounds
        .iter()
        .find(|r| r.number == number)
        .ok_or_else(|| {
            Error::NotFound(format!("{} has no round {number}", review.header.reference))
        })
}

/// A patch's files, a submodule only moved among them known by its sides' trees: renamed with its commit the same, it
/// has no `Subproject commit` lines (ATM-599: `markdown-rs`, moved into `crates/`, showed as a file not in its
/// commit).
fn with_links(
    repo: &atm_vcs::GitVcs,
    patches: Vec<FilePatch>,
    before: Option<&String>,
    after: &String,
) -> Vec<FilePatch> {
    let linked =
        |object: Option<&String>, path: &str| object.and_then(|o| repo.gitlink_in(o, path));
    patches
        .into_iter()
        .map(|mut file| {
            // Only an entry with no lines can be a submodule git didn't mark: one whose commit changed has its
            // `Subproject commit` lines, and `submodule` already. No lines is a pure rename (the moved submodule), a
            // mode change, an empty or a binary file: only those are looked up in the trees, a git process each,
            // never the files with lines, so a large review costs nothing more
            let no_lines = !file.patch.contains("\n@@");
            if file.submodule.is_none() && no_lines {
                let (old, new) = (
                    linked(before, &file.old_path),
                    linked(Some(after), &file.path),
                );
                if old.is_some() || new.is_some() {
                    file.submodule = Some((old, new));
                }
            }
            file
        })
        .collect()
}

/// The files of one view: the whole change of round `shown` (its base to its result), or what changed in it since
/// round `against`; each with its patch and the objects (a commit, or a tree) its two sides are read from.
fn view<S: RoundDiffs + ?Sized>(
    vcs: &atm_vcs::GitVcs,
    store: &S,
    review: &Review,
    shown: u32,
    against: Option<u32>,
) -> Result<Value> {
    let round = round_of(review, shown)?;
    let id = &review.header.id;
    let diff = store.diff_of(id, shown)?;
    let (before, after, patch, listed) = match against {
        None => {
            let before = match &round.kind {
                RoundKind::Range { base, .. } | RoundKind::Working { base } => Some(base.clone()),
                RoundKind::Files => None,
            };
            let after = match &round.kind {
                RoundKind::Range { head, .. } => head.clone(),
                _ => rounds::tree(vcs, round, &diff)?,
            };
            (before, after, diff, Some(&round.files))
        }
        Some(from) if from == shown => {
            return Err(Error::Invalid(format!(
                "round {shown} against itself: name an earlier round"
            )));
        }
        Some(from) => {
            let earlier = round_of(review, from)?;
            let before = rounds::tree(vcs, earlier, &store.diff_of(id, from)?)?;
            let after = rounds::tree(vcs, round, &diff)?;
            let between = rounds::compare(vcs, store, review, from, shown)?;
            (Some(before), after, between, None)
        }
    };
    let repo = rounds::repository(vcs, round).unwrap_or_else(|_| vcs.clone());
    let patches = with_links(&repo, file_patches(&patch.0), before.as_ref(), &after);
    // A moved submodule's entry: its two commits and its repository's web address, when it has one, for links
    let submodule = |file: &FilePatch| {
        file.submodule.as_ref().map(|(from, to)| {
            let repository = vcs
                .submodule_at(&file.path)
                .and_then(|sub| sub.remote_url())
                .and_then(|url| atm_common::repository::repository_url(&url));
            json!({ "from": from, "to": to, "repository": repository })
        })
    };
    let entry = |file: &FilePatch, before: Option<&String>, after: &String| {
        json!({
            "path": file.path,
            "old_path": file.old_path,
            "status": file.status,
            "binary": file.binary,
            "submodule": submodule(file),
            "patch": file.patch,
            "before": if file.status == "added" { None } else { before.cloned() },
            "after": if file.status == "deleted" { None } else { Some(after.clone()) },
        })
    };
    // A submodule moved from one commit to another: its entry, then the files its own repository changed between
    // them, named as the project has them, their sides the submodule's commits (ATM-599, Hakan on CR-4); none when it
    // isn't checked out here, or a commit is gone from it
    let side = |file: &FilePatch| {
        let mut out = vec![entry(file, before.as_ref(), &after)];
        if let Some((Some(from), Some(to))) = &file.submodule
            && from != to
            && let Some(sub) = vcs.submodule_at(&file.path)
            && let Ok(diff) = sub.diff_range(from, to, &[])
        {
            out.extend(
                file_patches(&diff.patch)
                    .iter()
                    .map(|inner| entry(inner, Some(from), to)),
            );
        }
        out
    };
    // The whole change lists the round's files (a binary one has no patch); between rounds, what the diff has
    let files: Vec<Value> = match listed {
        Some(files) => files
            .iter()
            .flat_map(|f| {
                let found = patches.iter().find(|p| p.path == f.path).cloned();
                side(&found.unwrap_or_else(|| FilePatch {
                    path: f.path.clone(),
                    old_path: match &f.status {
                        reviews::FileStatus::Renamed { from } => from.clone(),
                        _ => f.path.clone(),
                    },
                    status: match &f.status {
                        reviews::FileStatus::Added => "added",
                        reviews::FileStatus::Deleted => "deleted",
                        reviews::FileStatus::Renamed { .. } => "renamed",
                        reviews::FileStatus::Modified => "modified",
                    },
                    binary: f.binary,
                    submodule: None,
                    patch: String::new(),
                }))
            })
            .collect(),
        None => patches.iter().flat_map(side).collect(),
    };
    Ok(json!({ "round": shown, "against": against, "files": files }))
}

/// A chain, with where it shows in round `shown`: `placed` its lines there (none for the review's or a whole file's),
/// `drifted` the round and lines it was made on when its text is gone from there.
fn placed<S: RoundDiffs + ?Sized>(
    vcs: &atm_vcs::GitVcs,
    store: &S,
    review: &Review,
    chain: &Chain,
    shown: u32,
) -> Value {
    let mut out = serde_json::to_value(chain).unwrap_or(Value::Null);
    let anchor = &chain.anchor;
    let made = || json!({ "round": anchor.round, "lines": anchor.lines });
    let (at, drifted) = match rounds::place(vcs, store, review, anchor, shown) {
        Ok(rounds::Placement::Whole) => (Value::Null, Value::Null),
        Ok(rounds::Placement::At(lines)) => (json!(lines), Value::Null),
        Ok(rounds::Placement::Drifted {
            last,
            made_on,
            lines,
        }) => (json!(last), json!({ "round": made_on, "lines": lines })),
        // The round it was made on can't be had here (its base is gone from this clone): where it was made
        Err(_) => (Value::Null, made()),
    };
    if let Some(map) = out.as_object_mut() {
        map.insert("placed".into(), at);
        map.insert("drifted".into(), drifted);
    }
    out
}

/// A store's rounds, and the working copy now as the round after them, not stored (ATM-576)
struct WithNow<'a> {
    store: &'a FileStore,
    number: u32,
    patch: Patch,
}

impl RoundDiffs for WithNow<'_> {
    fn diff_of(&self, id: &ReviewId, round: u32) -> Result<Patch> {
        if round == self.number {
            Ok(self.patch.clone())
        } else {
            self.store.round_diff(id, round)
        }
    }
}

/// A view and its chains placed in it.
fn shown_with<S: RoundDiffs + ?Sized>(
    vcs: &atm_vcs::GitVcs,
    diffs: &S,
    review: &Review,
    shown: u32,
    against: Option<u32>,
) -> Result<(Value, Vec<Value>)> {
    let view = view(vcs, diffs, review, shown, against)?;
    let chains = review
        .chains
        .iter()
        .map(|c| placed(vcs, diffs, review, c, shown))
        .collect();
    Ok((view, chains))
}

/// A review's page for one view (`round`, the latest when none; `against`, an earlier round): `{review, summary,
/// branch, rounds, chains, view: {round, against, now, files}, needs_you, you}`. A live review shown with no round
/// is its working copy now (`now`, §3.5, ATM-576): when it changed since the last checkpoint, as the round after the
/// stored ones (`rounds` lists only those), which a comment on it makes a checkpoint.
///
/// # Errors
/// No such review, round, or the round against itself; the project isn't in git; a round's tree can't be had here.
pub fn review_page(
    project: &Project,
    reference: &ReviewRef,
    round: Option<u32>,
    against: Option<u32>,
) -> Result<Value> {
    let store = reviews::store(project)?;
    let mut review = store.get_review(reference)?;
    let stored = review.rounds.clone();
    let latest = review
        .rounds
        .last()
        .map(|r| r.number)
        .ok_or_else(|| Error::NotFound(format!("{} has no rounds", review.header.reference)))?;
    let vcs = reviews::vcs(project)?;
    let now = round.is_none() && review.header.status == Status::Live;
    let working = if now {
        reviews::working_now(project, &review)?
    } else {
        None
    };
    let (mut view, chains) = match working {
        Some(new) => {
            let number = latest + 1;
            let (unstored, patch) = rounds::unstored(number, new, atm_common::time::now_iso());
            review.rounds.push(unstored);
            let diffs = WithNow {
                store: &store,
                number,
                patch,
            };
            shown_with(&vcs, &diffs, &review, number, against)?
        }
        None => shown_with(&vcs, &store, &review, round.unwrap_or(latest), against)?,
    };
    if let Some(map) = view.as_object_mut() {
        map.insert("now".into(), json!(now));
    }
    let branch = project
        .root()
        .and_then(|root| list_everywhere(root, &ReviewFilter::default()).ok())
        .and_then(|all| all.into_iter().find(|f| f.header.id == review.header.id))
        .and_then(|f| f.branch);
    let me = me(project);
    Ok(json!({
        "review": review.header,
        "summary": review.summary,
        "branch": branch,
        "rounds": stored,
        "chains": chains,
        "view": view,
        "needs_you": needs(&review.header, &me),
        "you": me.0,
    }))
}

#[cfg(test)]
mod tests {
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
        let query =
            |author: Option<&str>, reviewer: Option<&str>, branch: Option<&str>| ReviewQuery {
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
}
