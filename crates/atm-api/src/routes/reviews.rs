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

use atm_common::py::to_json;
use atm_lib::Project;
use atm_lib::review_requests::{self, request_from};
use atm_lib::reviews::{CommentId, ReviewHeader, ReviewRef};
use axum::http::{Method, StatusCode};
use axum::response::Response;
use serde_json::{Value, json};

use super::{Projects, error_response, stamped};
use crate::{send, send_error};

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
fn post_review(project: &mut Project, body: Value) -> Response {
    review_answer(
        true,
        request_from(body)
            .and_then(|opening| review_requests::open_review(project, opening))
            .and_then(|h| as_json(&h)),
    )
}

/// `PATCH …/reviews/<CR>`: the review closed.
fn patch_review(project: &mut Project, review: &ReviewRef, body: Value) -> Response {
    review_answer(
        false,
        request_from(body)
            .and_then(|closing| review_requests::close_review(project, review, &closing))
            .and_then(|h| as_json(&h)),
    )
}

/// `PUT …/reviews/<CR>/tasks`: the tasks it reviews, as a whole.
fn put_review_tasks(project: &mut Project, review: &ReviewRef, body: Value) -> Response {
    review_answer(
        false,
        request_from(body)
            .and_then(|tasks| review_requests::set_review_tasks(project, review, &tasks))
            .and_then(|h| as_json(&h)),
    )
}

/// `POST …/reviews/<CR>/decisions`: a reviewer's decision (or the user's, overriding who decides).
fn post_decision(project: &mut Project, review: &ReviewRef, body: Value) -> Response {
    review_answer(
        true,
        request_from(body)
            .and_then(|decision| review_requests::submit_decision(project, review, &decision))
            .and_then(|h| as_json(&h)),
    )
}

/// `POST …/reviews/<CR>/comments`: a new chain.
fn post_comment(project: &Project, review: &ReviewRef, body: Value) -> Response {
    review_answer(
        true,
        request_from(body)
            .and_then(|comment| review_requests::add_comment(project, review, comment))
            .map(|id| json!({ "review": review.0, "comment": id })),
    )
}

/// `PATCH …/reviews/<CR>/comments/<c>`: a comment's text, or its resolved box.
fn patch_comment(
    project: &Project,
    review: &ReviewRef,
    comment: &CommentId,
    body: Value,
) -> Response {
    review_answer(
        false,
        request_from(body)
            .and_then(|update| review_requests::update_comment(project, review, comment, update))
            .map(|()| json!({ "review": review.0, "comment": comment })),
    )
}

/// `POST …/reviews/<CR>/comments/<c>/replies`: a reply.
fn post_reply(project: &Project, review: &ReviewRef, comment: &CommentId, body: Value) -> Response {
    review_answer(
        true,
        request_from(body)
            .and_then(|reply| review_requests::reply_to_comment(project, review, comment, &reply))
            .map(|id| json!({ "review": review.0, "comment": id, "reply_to": comment })),
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

/// A review change, to its route by method and resource (the table above).
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
    let Some(workspace) = projects.workspaces.get(w) else {
        return send_error(StatusCode::NOT_FOUND, "unknown workspace");
    };
    let mut project = match workspace.member(p).and_then(|m| m.project_as_user()) {
        Ok(project) => project,
        Err(e) => return error_response(&e),
    };
    let review = |cr: &str| ReviewRef((*cr).to_string());
    let comment = |c: &str| CommentId((*c).to_string());
    match (method.as_str(), resource) {
        ("POST", []) => post_review(&mut project, body),
        ("PATCH", [cr]) => patch_review(&mut project, &review(cr), body),
        ("PUT", [cr, "tasks"]) => put_review_tasks(&mut project, &review(cr), body),
        ("POST", [cr, "decisions"]) => post_decision(&mut project, &review(cr), body),
        ("POST", [cr, "comments"]) => post_comment(&project, &review(cr), body),
        ("PATCH", [cr, "comments", c]) => patch_comment(&project, &review(cr), &comment(c), body),
        ("POST", [cr, "comments", c, "replies"]) => {
            post_reply(&project, &review(cr), &comment(c), body)
        }
        (
            _,
            []
            | [_]
            | [_, "tasks" | "decisions" | "comments"]
            | [_, "comments", _]
            | [_, "comments", _, "replies"],
        ) => send_error(StatusCode::METHOD_NOT_ALLOWED, "not with this method"),
        _ => send_error(StatusCode::NOT_FOUND, "not found"),
    }
}
