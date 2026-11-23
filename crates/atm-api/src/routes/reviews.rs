//! A project's code reviews (docs/code-review-design.md §3.12, docs/multi-project-api.md §2.9, ATM-572): the only
//! routes that change anything. Each is one atm-lib call (`review_requests`), as the user. `serve_request` has
//! checked the request first: the Host, the Origin, the session token, a JSON body.
//!
//! | Method | Resource | Does |
//! |---|---|---|
//! | POST | `reviews` | open a review |
//! | PATCH | `reviews/<CR>` | close it |
//! | PUT | `reviews/<CR>/tasks` | the tasks it reviews |
//! | POST | `reviews/<CR>/decisions` | a reviewer's decision |
//! | POST | `reviews/<CR>/comments` | a new chain |
//! | PATCH | `reviews/<CR>/comments/<c>` | a comment's text or resolved box |
//! | POST | `reviews/<CR>/comments/<c>/replies` | a reply |
//! | POST | `reviews/<CR>/pokes` | a poke (ATM-611): a nudge its agent answers |
//! | POST | `reviews/<CR>/drafts` | a draft of the user's (ATM-612), sent with their next decision |
//! | PATCH | `reviews/<CR>/drafts/<d>` | a draft's text |
//! | DELETE | `reviews/<CR>/drafts/<d>` | a draft thrown away |
//!
//! And one read here, as the user: `GET reviews/<CR>/drafts`, their drafts (another's are never shown).

use atm_common::py::to_json;
use atm_lib::review_drafts::Draft;
use atm_lib::review_requests::request_from;
use atm_lib::reviews::{CommentId, ReviewHeader, ReviewRef};
use axum::http::{HeaderMap, Method, StatusCode};
use axum::response::Response;
use serde_json::{Value, json};

use atm_lib::review_pages::ReviewQuery;

use super::query::{parameter, parse_query};
use super::source::{ProjectSource, ReviewChanges};
use super::{Projects, error_response, etag, not_modified, ok_json, stamped, unchanged};
use crate::{send, send_error};

/// `GET …/reviews?q=&status=&author=&reviewer=&task=&branch=`: the reviews the query keeps (`status` repeats; the
/// others are each their first value), the latest activity first, which need the user. Never cached: a review changes
/// with each comment.
pub(super) fn get_reviews(project: &dyn ProjectSource, query: &str) -> Response {
    let given = parse_query(query);
    let first = |name: &str| {
        given
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.clone())
    };
    let wanted = ReviewQuery {
        q: first("q").unwrap_or_default(),
        status: given
            .iter()
            .filter(|(n, _)| n == "status")
            .map(|(_, v)| v.clone())
            .collect(),
        author: first("author"),
        reviewer: first("reviewer"),
        task: first("task"),
        branch: first("branch"),
    };
    match project.reviews(&wanted) {
        Ok(listed) => ok_json(&stamped(listed), None),
        Err(e) => error_response(&e),
    }
}

/// A round's number from the query: none when it isn't given; else a whole number from 1, or why it isn't one.
fn round_number(query: &str, name: &str) -> Result<Option<u32>, String> {
    let given = parameter(query, name);
    if given.is_empty() {
        return Ok(None);
    }
    given
        .parse::<u32>()
        .ok()
        .filter(|n| *n > 0)
        .map(Some)
        .ok_or_else(|| format!("{name} is a round's number, from 1"))
}

/// `GET …/reviews/<CR>?round=&against=`: the review's page for one view, the whole change of a round (the latest when
/// none) or a round against an earlier one. Never cached, as the list, except a live review's page of its working copy
/// now (ATM-607): tagged from what it is made of (the reviews, the working copy's state, who asks), so the page's poll
/// gets a 304 without a diff being made while nothing changed.
pub(super) fn get_review(
    project: &dyn ProjectSource,
    review: &str,
    query: &str,
    request: &HeaderMap,
) -> Response {
    let (round, against) = match (round_number(query, "round"), round_number(query, "against")) {
        (Ok(round), Ok(against)) => (round, against),
        (Err(why), _) | (_, Err(why)) => return send_error(StatusCode::BAD_REQUEST, &why),
    };
    let reference = ReviewRef(review.to_string());
    let tag = match project.live_review_stamp(&reference, round, against) {
        Ok(stamp) => stamp.map(|s| etag(&s)),
        Err(e) => return error_response(&e),
    };
    if let Some(tag) = &tag
        && unchanged(request, tag)
    {
        return not_modified(tag, &[]);
    }
    match project.review(&reference, round, against) {
        Ok(page) => ok_json(&stamped(page), tag.as_deref()),
        Err(e) => error_response(&e),
    }
}

/// The answer to a review request: 201 with what was made, 200 with what changed, or atm-lib's refusal.
fn review_answer(made: bool, result: atm_common::Result<Value>) -> Response {
    match result {
        Ok(body) => send(
            if made {
                StatusCode::CREATED
            } else {
                StatusCode::OK
            },
            to_json(&stamped(body)).into_bytes(),
            "application/json",
            None,
            &[],
        ),
        Err(e) => error_response(&e),
    }
}

fn as_json(header: &ReviewHeader) -> atm_common::Result<Value> {
    serde_json::to_value(header).map_err(|e| atm_common::Error::failed(e.to_string()))
}

/// `POST …/reviews`: a review opened (or a draft, or a live review).
fn post_review(reviews: &mut dyn ReviewChanges, body: Value) -> Response {
    review_answer(
        true,
        request_from(body)
            .and_then(|opening| reviews.open_review(opening))
            .and_then(|h| as_json(&h)),
    )
}

/// `PATCH …/reviews/<CR>`: the review closed.
fn patch_review(reviews: &mut dyn ReviewChanges, review: &ReviewRef, body: Value) -> Response {
    review_answer(
        false,
        request_from(body)
            .and_then(|closing| reviews.close_review(review, &closing))
            .and_then(|h| as_json(&h)),
    )
}

/// `PUT …/reviews/<CR>/tasks`: the tasks it reviews, as a whole.
fn put_review_tasks(reviews: &mut dyn ReviewChanges, review: &ReviewRef, body: Value) -> Response {
    review_answer(
        false,
        request_from(body)
            .and_then(|tasks| reviews.set_review_tasks(review, &tasks))
            .and_then(|h| as_json(&h)),
    )
}

/// `POST …/reviews/<CR>/decisions`: a reviewer's decision (or the user's, overriding who decides).
fn post_decision(reviews: &mut dyn ReviewChanges, review: &ReviewRef, body: Value) -> Response {
    review_answer(
        true,
        request_from(body)
            .and_then(|decision| reviews.submit_decision(review, &decision))
            .and_then(|h| as_json(&h)),
    )
}

/// `POST …/reviews/<CR>/comments`: a new chain.
fn post_comment(reviews: &mut dyn ReviewChanges, review: &ReviewRef, body: Value) -> Response {
    review_answer(
        true,
        request_from(body)
            .and_then(|comment| reviews.add_comment(review, comment))
            .map(|id| json!({ "review": review.0, "comment": id })),
    )
}

/// `PATCH …/reviews/<CR>/comments/<c>`: a comment's text, or its resolved box.
fn patch_comment(
    reviews: &mut dyn ReviewChanges,
    review: &ReviewRef,
    comment: &CommentId,
    body: Value,
) -> Response {
    review_answer(
        false,
        request_from(body)
            .and_then(|update| reviews.update_comment(review, comment, update))
            .map(|()| json!({ "review": review.0, "comment": comment })),
    )
}

/// `POST …/reviews/<CR>/comments/<c>/replies`: a reply.
fn post_reply(
    reviews: &mut dyn ReviewChanges,
    review: &ReviewRef,
    comment: &CommentId,
    body: Value,
) -> Response {
    review_answer(
        true,
        request_from(body)
            .and_then(|reply| reviews.reply_to_comment(review, comment, &reply))
            .map(|id| json!({ "review": review.0, "comment": id, "reply_to": comment })),
    )
}

/// `POST …/reviews/<CR>/pokes` (ATM-611): a poke, with an optional message.
fn post_poke(reviews: &mut dyn ReviewChanges, review: &ReviewRef, body: Value) -> Response {
    review_answer(
        true,
        request_from(body)
            .and_then(|poke| reviews.poke_review(review, &poke))
            .map(|id| json!({ "review": review.0, "poke": id })),
    )
}

fn draft_json(draft: &Draft) -> atm_common::Result<Value> {
    serde_json::to_value(draft).map_err(|e| atm_common::Error::failed(e.to_string()))
}

/// `GET …/reviews/<CR>/drafts` (ATM-612): the user's drafts on the review, read as the user; never cached.
pub(super) fn get_drafts(project: &dyn ProjectSource, review: &str) -> Response {
    let reference = ReviewRef(review.to_string());
    let listed = project
        .reviews_as_user()
        .and_then(|reviews| reviews.list_drafts(&reference));
    match listed.and_then(|drafts| {
        serde_json::to_value(&drafts).map_err(|e| atm_common::Error::failed(e.to_string()))
    }) {
        Ok(drafts) => ok_json(
            &stamped(json!({ "review": reference.0, "drafts": drafts })),
            None,
        ),
        Err(e) => error_response(&e),
    }
}

/// `POST …/reviews/<CR>/drafts`: a draft.
fn post_draft(reviews: &mut dyn ReviewChanges, review: &ReviewRef, body: Value) -> Response {
    review_answer(
        true,
        request_from(body)
            .and_then(|draft| reviews.add_draft(review, draft))
            .and_then(|d| draft_json(&d)),
    )
}

/// `PATCH …/reviews/<CR>/drafts/<d>`: a draft's text.
fn patch_draft(
    reviews: &mut dyn ReviewChanges,
    review: &ReviewRef,
    draft: &str,
    body: Value,
) -> Response {
    review_answer(
        false,
        request_from(body)
            .and_then(|update| reviews.update_draft(review, draft, &update))
            .and_then(|d| draft_json(&d)),
    )
}

/// `DELETE …/reviews/<CR>/drafts/<d>`: a draft thrown away; its body, if any, an empty object.
fn delete_draft(
    reviews: &mut dyn ReviewChanges,
    review: &ReviewRef,
    draft: &str,
    body: &Value,
) -> Response {
    if body.as_object().is_none_or(|o| !o.is_empty()) {
        return send_error(
            StatusCode::BAD_REQUEST,
            "a draft is removed with an empty body ({})",
        );
    }
    review_answer(
        false,
        reviews
            .remove_draft(review, draft)
            .map(|()| json!({ "review": review.0, "draft": draft, "removed": true })),
    )
}

/// Whether `path` is under a project's reviews (`/api/v2/workspaces/<w>/projects/<p>/reviews…`), where a POST, PATCH
/// or PUT may change something; on any other path those methods are refused as on a read-only server.
#[must_use]
pub fn is_reviews_path(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    matches!(
        parts.as_slice(),
        ["", "api", "v2", "workspaces", w, "projects", p, "reviews", ..] if !w.is_empty() && !p.is_empty()
    )
}

/// One of the routes of the table above.
enum Route {
    OpenReview,
    CloseReview(ReviewRef),
    SetTasks(ReviewRef),
    Decide(ReviewRef),
    Comment(ReviewRef),
    UpdateComment(ReviewRef, CommentId),
    Reply(ReviewRef, CommentId),
    Poke(ReviewRef),
    AddDraft(ReviewRef),
    UpdateDraft(ReviewRef, String),
    RemoveDraft(ReviewRef, String),
}

/// The route for `method` on `resource` (past `reviews`): a resource no route has is 404, one the method isn't for
/// 405.
fn route(method: &Method, resource: &[&str]) -> Result<Route, (StatusCode, &'static str)> {
    let review = |cr: &str| ReviewRef(cr.to_string());
    let comment = |c: &str| CommentId(c.to_string());
    Ok(match (method.as_str(), resource) {
        ("POST", []) => Route::OpenReview,
        ("PATCH", [cr]) => Route::CloseReview(review(cr)),
        ("PUT", [cr, "tasks"]) => Route::SetTasks(review(cr)),
        ("POST", [cr, "decisions"]) => Route::Decide(review(cr)),
        ("POST", [cr, "comments"]) => Route::Comment(review(cr)),
        ("PATCH", [cr, "comments", c]) => Route::UpdateComment(review(cr), comment(c)),
        ("POST", [cr, "comments", c, "replies"]) => Route::Reply(review(cr), comment(c)),
        ("POST", [cr, "pokes"]) => Route::Poke(review(cr)),
        ("POST", [cr, "drafts"]) => Route::AddDraft(review(cr)),
        ("PATCH", [cr, "drafts", d]) => Route::UpdateDraft(review(cr), (*d).to_string()),
        ("DELETE", [cr, "drafts", d]) => Route::RemoveDraft(review(cr), (*d).to_string()),
        (
            _,
            []
            | [_]
            | [_, "tasks" | "decisions" | "comments" | "pokes" | "drafts"]
            | [_, "comments" | "drafts", _]
            | [_, "comments", _, "replies"],
        ) => return Err((StatusCode::METHOD_NOT_ALLOWED, "not with this method")),
        _ => return Err((StatusCode::NOT_FOUND, "not found")),
    })
}

/// A review change, to its route by method and resource (the table above); the project is opened only for a route
/// there is.
#[must_use]
pub fn serve_review_change(
    projects: &Projects,
    method: &Method,
    path: &str,
    body: Value,
) -> Response {
    let Some(rest) = path.strip_prefix("/api/v2/workspaces/") else {
        return send_error(StatusCode::NOT_FOUND, "not found");
    };
    let parts: Vec<&str> = rest.split('/').collect();
    let [w, "projects", p, "reviews", resource @ ..] = parts.as_slice() else {
        return send_error(StatusCode::NOT_FOUND, "not found");
    };
    if resource.iter().any(|part| part.is_empty()) {
        return send_error(StatusCode::NOT_FOUND, "not found");
    }
    let route = match route(method, resource) {
        Ok(route) => route,
        Err((status, why)) => return send_error(status, why),
    };
    let Some(workspace) = projects.source.workspace(w) else {
        return send_error(StatusCode::NOT_FOUND, "unknown workspace");
    };
    let mut reviews = match workspace.project(p).and_then(|m| m.reviews_as_user()) {
        Ok(reviews) => reviews,
        Err(e) => return error_response(&e),
    };
    let reviews = reviews.as_mut();
    match route {
        Route::OpenReview => post_review(reviews, body),
        Route::CloseReview(review) => patch_review(reviews, &review, body),
        Route::SetTasks(review) => put_review_tasks(reviews, &review, body),
        Route::Decide(review) => post_decision(reviews, &review, body),
        Route::Comment(review) => post_comment(reviews, &review, body),
        Route::UpdateComment(review, comment) => patch_comment(reviews, &review, &comment, body),
        Route::Reply(review, comment) => post_reply(reviews, &review, &comment, body),
        Route::Poke(review) => post_poke(reviews, &review, body),
        Route::AddDraft(review) => post_draft(reviews, &review, body),
        Route::UpdateDraft(review, draft) => patch_draft(reviews, &review, &draft, body),
        Route::RemoveDraft(review, draft) => delete_draft(reviews, &review, &draft, &body),
    }
}

#[cfg(test)]
mod tests;
