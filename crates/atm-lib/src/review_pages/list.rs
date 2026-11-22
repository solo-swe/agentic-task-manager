//! The reviews list: which reviews a page's query keeps, the latest activity first (ATM-574).

use super::{
    Deserialize, Error, FileStore, Found, Project, Result, Review, ReviewFilter, ReviewHeader,
    ReviewRef, ReviewStore, Status, TASK_DIR, Value, json, me, needs, reviews, waiting_for,
};

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
pub(super) fn status_named(name: &str) -> Result<Vec<Status>> {
    Status::filter_named(name).ok_or_else(|| {
        Error::Invalid(format!(
            "{name}: not a review status (open, draft, waiting, live, changes_requested, approved, closed)"
        ))
    })
}

/// Whether a review found on `branch` fits the query's author, reviewer and branch (the store has done its statuses
/// and task).
pub(super) fn fits(header: &ReviewHeader, branch: Option<&str>, query: &ReviewQuery) -> bool {
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
pub(super) fn words(q: &str) -> Vec<String> {
    q.split_whitespace().map(str::to_lowercase).collect()
}

/// Whether every word is in the review's title, summary or a comment.
pub(super) fn mentions(review: &Review, words: &[String]) -> bool {
    let mut text = format!("{} {}", review.header.title, review.summary);
    for comment in review.comments() {
        text.push(' ');
        text.push_str(&comment.body);
    }
    let text = text.to_lowercase();
    words.iter().all(|w| text.contains(w))
}

/// A review found in one of the working copies, read from that copy (else the project's own store).
pub(super) fn whole(found: &Found, store: &FileStore) -> Result<Review> {
    FileStore::open(&found.working_copy.join(TASK_DIR))
        .and_then(|s| s.get_review(&ReviewRef(found.header.id.0.clone())))
        .or_else(|_| store.get_review(&found.header.reference))
}

/// The reviews of every working copy the query keeps, the latest activity first: `{reviews: [{review, branch, chains,
/// open, needs_you, waiting, waiting_since}], unreadable: [{folder, reference, problem}], you}` (`waiting`:
/// [`waiting_for`]'s why, or `null`; `waiting_since`: its latest round's time, ATM-610; `unreadable`: the reviews'
/// folders atm can't read, ATM-617).
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
    for found in reviews::list_naming(project, root, &filter)? {
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
            // When it began waiting on its reviewers: its latest round (ATM-610, the list's "waited 3 days")
            "waiting_since": review.rounds.last().map(|r| r.at.clone()),
            // Decided, approved or closed (ATM-714): atm's rule, the list's groups read it instead of their own copy
            "ended": review.header.status.ended(),
        });
        listed.push((review.header.updated.clone(), entry));
    }
    listed.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(json!({
        "reviews": listed.into_iter().map(|(_, e)| e).collect::<Vec<_>>(),
        // Whatever the query (ATM-617): a review atm can't read has no status or task to filter by, and left out it
        // would only be missing
        "unreadable": store.unreadable()?,
        "you": me.0,
    }))
}
