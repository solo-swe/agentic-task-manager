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
use atm_lib::review_requests::request_from;
use atm_lib::reviews::{CommentId, ReviewHeader, ReviewRef};
use axum::http::{Method, StatusCode};
use axum::response::Response;
use serde_json::{Value, json};

use atm_lib::review_pages::ReviewQuery;

use super::query::{parameter, parse_query};
use super::source::{ProjectSource, ReviewChanges};
use super::{Projects, error_response, ok_json, stamped};
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
/// none) or a round against an earlier one. Never cached, as the list.
pub(super) fn get_review(project: &dyn ProjectSource, review: &str, query: &str) -> Response {
    let (round, against) = match (round_number(query, "round"), round_number(query, "against")) {
        (Ok(round), Ok(against)) => (round, against),
        (Err(why), _) | (_, Err(why)) => return send_error(StatusCode::BAD_REQUEST, &why),
    };
    match project.review(&ReviewRef(review.to_string()), round, against) {
        Ok(page) => ok_json(&stamped(page), None),
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
        (
            _,
            []
            | [_]
            | [_, "tasks" | "decisions" | "comments"]
            | [_, "comments", _]
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
    }
}

/// The review rules (who may decide, what a comment may say) are atm-lib's and its tests'; the checks before a change
/// reaches a route (Host, Origin, session token, JSON) are `serve_request`'s. Here: which route each method and
/// resource reach, the request each body becomes, and the status each answer becomes.
#[cfg(test)]
mod tests {
    use atm_common::Error;
    use atm_lib::review_requests::{
        ChangeToReview, CommentUpdate, Decision, DecisionKind, NewComment, NewReply, ReviewClosing,
        ReviewOpening, ReviewStart, ReviewTasks, RulesOverride,
    };
    use atm_lib::reviews::{Side, Status};

    use super::super::test_support::{
        Answer, Call, Fake, FakeProject, FakeReviews, answer, bound, header, refusals,
    };
    use super::*;

    const REVIEWS: &str = "/api/v2/workspaces/work/projects/p1/reviews";

    #[test]
    fn the_list_asks_for_the_querys_filters_status_repeated_the_others_their_first() {
        for (query, wanted) in [
            ("", ReviewQuery::default()),
            ("unknown=x&&", ReviewQuery::default()),
            (
                "q=two+words&status=open&status=live&author=claude%3Aa&reviewer=Ana&task=ATM-1&branch=dev%2F1",
                ReviewQuery {
                    q: "two words".into(),
                    status: vec!["open".into(), "live".into()],
                    author: Some("claude:a".into()),
                    reviewer: Some("Ana".into()),
                    task: Some("ATM-1".into()),
                    branch: Some("dev/1".into()),
                },
            ),
            (
                "author=a&author=b&q=first&q=second",
                ReviewQuery {
                    q: "first".into(),
                    author: Some("a".into()),
                    ..ReviewQuery::default()
                },
            ),
            // A blank value is no value
            ("author=&status=", ReviewQuery::default()),
        ] {
            let (project, calls) = bound(FakeProject::default());
            assert_eq!(
                get_reviews(project.as_ref(), query).status(),
                StatusCode::OK
            );
            assert_eq!(calls.taken(), [Call::Reviews(wanted)], "{query}");
        }
    }

    #[test]
    fn the_list_and_a_page_are_atm_libs_answers_stamped_and_never_cached() {
        let listed = json!({"reviews": [{"review": {"reference": "CR-2"}, "needs_you": true}], "you": "Ana"});
        let page = json!({"review": {"reference": "CR-2"}, "view": {"round": 2, "files": [{"path": "a"}]}});
        let (project, _) = bound(FakeProject {
            listed: Ok(listed.clone()),
            page: Ok(page.clone()),
            ..FakeProject::default()
        });
        for (got, expected) in [
            (answer(get_reviews(project.as_ref(), "")), listed),
            (answer(get_review(project.as_ref(), "CR-2", "")), page),
        ] {
            assert_eq!(got.status, StatusCode::OK);
            assert_eq!(got.unstamped(), expected);
            assert!(got.json()["generated_at"].is_string());
            assert_eq!(got.tag(), "");
            assert_eq!(got.header("cache-control"), "no-store");
        }
    }

    #[test]
    fn a_page_asks_for_the_round_and_the_round_against() {
        for (query, round, against) in [
            ("", None, None),
            ("round=3", Some(3), None),
            ("round=3&against=1", Some(3), Some(1)),
            ("against=1", None, Some(1)),
            ("round=", None, None),
            ("round=2&round=9", Some(2), None),
        ] {
            let (project, calls) = bound(FakeProject::default());
            assert_eq!(
                get_review(project.as_ref(), "CR-7", query).status(),
                StatusCode::OK
            );
            assert_eq!(
                calls.taken(),
                [Call::Review(r("CR-7"), round, against)],
                "{query}"
            );
        }
    }

    #[test]
    fn a_round_that_isnt_a_rounds_number_is_400_and_asks_nothing() {
        for query in [
            "round=0",
            "round=-1",
            "round=x",
            "round=1.5",
            "round=99999999999",
            "against=0",
            "against=y",
        ] {
            let (project, calls) = bound(FakeProject::default());
            let refused = answer(get_review(project.as_ref(), "CR-7", query));
            assert_eq!(refused.status, StatusCode::BAD_REQUEST, "{query}");
            assert!(
                refused.json()["error"]
                    .as_str()
                    .is_some_and(|e| e.contains("a round's number"))
            );
            assert_eq!(calls.taken(), [], "{query}");
        }
    }

    #[test]
    fn a_refused_list_or_page_is_its_status() {
        for (error, status) in refusals() {
            let (project, _) = bound(FakeProject {
                listed: Err(error.clone()),
                page: Err(error.clone()),
                ..FakeProject::default()
            });
            for got in [
                answer(get_reviews(project.as_ref(), "")),
                answer(get_review(project.as_ref(), "CR-1", "")),
            ] {
                assert_eq!(got.status, status, "{error}");
                assert_eq!(got.json(), json!({"error": error.to_string()}));
            }
        }
    }

    fn send(fake: Fake, method: &Method, resource: &str, body: Value) -> (Answer, Vec<Call>) {
        let (projects, calls) = fake.projects();
        let answer = answer(serve_review_change(
            &projects,
            method,
            &format!("{REVIEWS}{resource}"),
            body,
        ));
        (answer, calls.taken())
    }

    fn ok() -> Fake {
        Fake::with(FakeProject::default())
    }

    fn r(cr: &str) -> ReviewRef {
        ReviewRef(cr.into())
    }

    fn c(id: &str) -> CommentId {
        CommentId(id.into())
    }

    #[test]
    fn only_a_projects_reviews_take_changes() {
        for path in [
            "/api/v2/workspaces/w/projects/p/reviews",
            "/api/v2/workspaces/w/projects/p/reviews/",
            "/api/v2/workspaces/w/projects/p/reviews/CR-1/comments/c1/replies",
        ] {
            assert!(is_reviews_path(path), "{path}");
        }
        for path in [
            "/api/v2/workspaces/w/projects/p/tasks",
            "/api/v2/workspaces/w/projects/p/reviewsx",
            "/api/v2/workspaces//projects/p/reviews",
            "/api/v2/workspaces/w/projects//reviews",
            "/api/v2/workspaces/w/project/p/reviews",
            "/api/v1/projects/p/reviews",
            "/api/v2/workspaces/w/projects/p",
            "api/v2/workspaces/w/projects/p/reviews",
            "/x/api/v2/workspaces/w/projects/p/reviews",
            "",
        ] {
            assert!(!is_reviews_path(path), "{path}");
        }
    }

    /// Each route: the body as its request, the one atm-lib call, as the user, and what it answers
    #[test]
    #[allow(clippy::too_many_lines)] // reason: one table, a row per route
    fn each_route_makes_its_call_as_the_user_and_answers_what_it_made_or_changed() {
        let made = json!(serde_json::to_value(header()).expect("a header"));
        let cases: Vec<(Method, &str, Value, Call, StatusCode, Value)> = vec![
            (
                Method::POST,
                "",
                json!({"title": "Look", "change": {"files": ["a.txt"]}}),
                Call::OpenReview(ReviewOpening {
                    title: "Look".into(),
                    summary: String::new(),
                    tasks: Vec::new(),
                    reviewers: Vec::new(),
                    start: ReviewStart::Open,
                    change: ChangeToReview {
                        files: vec!["a.txt".into()],
                        ..ChangeToReview::default()
                    },
                }),
                StatusCode::CREATED,
                made.clone(),
            ),
            (
                Method::PATCH,
                "/CR-1",
                json!({"status": "closed", "reason": "Not needed."}),
                Call::CloseReview(
                    r("CR-1"),
                    ReviewClosing {
                        status: Status::Closed,
                        reason: "Not needed.".into(),
                        reject_tasks: false,
                    },
                ),
                StatusCode::OK,
                made.clone(),
            ),
            (
                Method::PUT,
                "/CR-1/tasks",
                json!({"tasks": ["T-1", "T-2"]}),
                Call::SetTasks(
                    r("CR-1"),
                    ReviewTasks {
                        tasks: vec!["T-1".into(), "T-2".into()],
                    },
                ),
                StatusCode::OK,
                made.clone(),
            ),
            (
                Method::POST,
                "/CR-1/decisions",
                json!({"decision": "approve", "override_rules": {"reason": "On a call."}}),
                Call::Decide(
                    r("CR-1"),
                    Decision {
                        decision: DecisionKind::Approve,
                        body: None,
                        override_rules: Some(RulesOverride {
                            reason: "On a call.".into(),
                        }),
                    },
                ),
                StatusCode::CREATED,
                made,
            ),
            (
                Method::POST,
                "/CR-1/comments",
                json!({"path": "a.txt", "lines": [1, 2], "body": "Why?"}),
                Call::Comment(
                    r("CR-1"),
                    NewComment {
                        path: Some("a.txt".into()),
                        lines: Some((1, 2)),
                        side: Side::default(),
                        quote: None,
                        round: None,
                        body: "Why?".into(),
                    },
                ),
                StatusCode::CREATED,
                json!({"review": "CR-1", "comment": "c1"}),
            ),
            (
                Method::PATCH,
                "/CR-1/comments/c4",
                json!({"resolved": true}),
                Call::UpdateComment(
                    r("CR-1"),
                    c("c4"),
                    CommentUpdate {
                        body: None,
                        resolved: Some(true),
                    },
                ),
                StatusCode::OK,
                json!({"review": "CR-1", "comment": "c4"}),
            ),
            (
                Method::POST,
                "/CR-1/comments/c4/replies",
                json!({"body": "Because."}),
                Call::Reply(
                    r("CR-1"),
                    c("c4"),
                    NewReply {
                        body: "Because.".into(),
                    },
                ),
                StatusCode::CREATED,
                json!({"review": "CR-1", "comment": "c1", "reply_to": "c4"}),
            ),
        ];
        for (method, resource, body, call, status, answered) in cases {
            let (answer, calls) = send(ok(), &method, resource, body);
            assert_eq!(
                answer.status,
                status,
                "{method} {resource}: {}",
                answer.json()
            );
            assert_eq!(answer.unstamped(), answered, "{method} {resource}");
            assert!(answer.json()["generated_at"].is_string());
            assert_eq!(answer.header("cache-control"), "no-store");
            assert_eq!(
                calls,
                [
                    Call::Project("work".into(), "p1".into()),
                    Call::AsUser,
                    call
                ],
                "{method} {resource}"
            );
        }
    }

    /// A resource no route has is 404, one the method isn't for 405: before the project is opened
    #[test]
    fn a_method_or_resource_no_route_has_never_opens_the_project() {
        for (method, resource) in [
            (Method::GET, ""),
            (Method::PUT, ""),
            (Method::PATCH, ""),
            (Method::DELETE, "/CR-1"),
            (Method::POST, "/CR-1"),
            (Method::PUT, "/CR-1"),
            (Method::POST, "/CR-1/tasks"),
            (Method::PATCH, "/CR-1/tasks"),
            (Method::PUT, "/CR-1/decisions"),
            (Method::PATCH, "/CR-1/decisions"),
            (Method::PUT, "/CR-1/comments"),
            (Method::PATCH, "/CR-1/comments"),
            (Method::POST, "/CR-1/comments/c1"),
            (Method::PUT, "/CR-1/comments/c1"),
            (Method::PATCH, "/CR-1/comments/c1/replies"),
        ] {
            let (answer, calls) = send(ok(), &method, resource, json!({}));
            assert_eq!(
                answer.status,
                StatusCode::METHOD_NOT_ALLOWED,
                "{method} {resource}"
            );
            assert_eq!(answer.json(), json!({"error": "not with this method"}));
            assert_eq!(calls, [], "{method} {resource}");
        }
        for (method, resource) in [
            (Method::POST, "/CR-1/votes"),
            (Method::POST, "/CR-1/comments/c1/replies/r1"),
            (Method::POST, "/CR-1/tasks/T-1"),
            (Method::POST, "//comments"),
            (Method::POST, "/"),
            (Method::POST, "/CR-1/"),
        ] {
            let (answer, calls) = send(ok(), &method, resource, json!({}));
            assert_eq!(answer.status, StatusCode::NOT_FOUND, "{method} {resource}");
            assert_eq!(answer.json(), json!({"error": "not found"}));
            assert_eq!(calls, [], "{method} {resource}");
        }
    }

    #[test]
    fn a_path_outside_a_projects_reviews_is_not_found() {
        let (projects, calls) = ok().projects();
        for path in [
            "/elsewhere",
            "/api/v2/workspaces",
            "/api/v2/workspaces/work/projects/p1",
            "/api/v2/workspaces/work/projects/p1/tasks",
            "/api/v2/workspaces/work/p1/reviews",
        ] {
            let answer = answer(serve_review_change(
                &projects,
                &Method::POST,
                path,
                json!({}),
            ));
            assert_eq!(answer.status, StatusCode::NOT_FOUND, "{path}");
        }
        assert_eq!(calls.taken(), []);
    }

    #[test]
    fn an_unknown_workspace_or_project_or_one_that_cant_be_opened_is_refused_before_any_change() {
        let (projects, calls) = ok().projects();
        let at = |path: &str| {
            answer(serve_review_change(
                &projects,
                &Method::POST,
                path,
                json!({"title": "x", "change": {"files": ["a"]}}),
            ))
        };
        let unknown = at("/api/v2/workspaces/nope/projects/p1/reviews");
        assert_eq!(unknown.status, StatusCode::NOT_FOUND);
        assert_eq!(unknown.json(), json!({"error": "unknown workspace"}));
        assert_eq!(calls.taken(), []);
        let unknown = at("/api/v2/workspaces/work/projects/nope/reviews");
        assert_eq!(unknown.status, StatusCode::NOT_FOUND);
        assert_eq!(unknown.json(), json!({"error": "unknown project"}));
        assert_eq!(calls.taken(), [Call::Project("work".into(), "nope".into())]);

        for (error, status) in refusals() {
            let fake = Fake::with(FakeProject {
                reviews: Err(error.clone()),
                ..FakeProject::default()
            });
            let (answer, calls) = send(fake, &Method::POST, "/CR-1/comments", json!({"body": "x"}));
            assert_eq!(answer.status, status, "{error}");
            assert_eq!(
                calls,
                [Call::Project("work".into(), "p1".into()), Call::AsUser],
                "{error}"
            );
        }
    }

    /// A body that isn't its route's request (a field it doesn't take, one missing, a wrong type, not an object) is
    /// 400, and reaches no atm-lib call
    #[test]
    fn a_body_that_isnt_the_routes_request_is_400_and_changes_nothing() {
        for (method, resource, body) in [
            (Method::POST, "", json!({"change": {"files": ["a"]}})),
            (
                Method::POST,
                "",
                json!({"title": "x", "author": "claude:x"}),
            ),
            (Method::POST, "", json!({"title": "x", "start": "later"})),
            (Method::POST, "", json!("a text")),
            (Method::POST, "", Value::Null),
            // (A status other than closed is atm-lib's to refuse: `approved` is a status)
            (
                Method::PATCH,
                "/CR-1",
                json!({"status": "shut", "reason": "x"}),
            ),
            (Method::PATCH, "/CR-1", json!({"status": "closed"})),
            (Method::PUT, "/CR-1/tasks", json!({"tasks": "T-1"})),
            (Method::PUT, "/CR-1/tasks", json!([])),
            (
                Method::POST,
                "/CR-1/decisions",
                json!({"decision": "merge"}),
            ),
            (
                Method::POST,
                "/CR-1/decisions",
                json!({"decision": "approve", "override_rules": {}}),
            ),
            (
                Method::POST,
                "/CR-1/comments",
                json!({"path": "a", "lines": [1]}),
            ),
            (
                Method::POST,
                "/CR-1/comments",
                json!({"body": "x", "lines": "1-2"}),
            ),
            (
                Method::PATCH,
                "/CR-1/comments/c1",
                json!({"resolved": "yes"}),
            ),
            (
                Method::PATCH,
                "/CR-1/comments/c1",
                json!({"body": "x", "author": "y"}),
            ),
            (Method::POST, "/CR-1/comments/c1/replies", json!({})),
            (
                Method::POST,
                "/CR-1/comments/c1/replies",
                json!({"body": "x", "author": "claude:x"}),
            ),
        ] {
            let (answer, calls) = send(ok(), &method, resource, body.clone());
            assert_eq!(
                answer.status,
                StatusCode::BAD_REQUEST,
                "{method} {resource} {body}"
            );
            assert!(
                answer.json()["error"]
                    .as_str()
                    .is_some_and(|e| e.starts_with("not this request")),
                "{}",
                answer.json()
            );
            assert_eq!(
                calls,
                [Call::Project("work".into(), "p1".into()), Call::AsUser],
                "{method} {resource} {body}"
            );
        }
    }

    /// Every refusal atm-lib can give (the rules', a malformed text, no such review), on every route: its status
    #[test]
    fn each_refusal_of_atm_lib_on_each_route_is_its_status() {
        let routes: [(Method, &str, Value); 7] = [
            (
                Method::POST,
                "",
                json!({"title": "x", "change": {"files": ["a"]}}),
            ),
            (
                Method::PATCH,
                "/CR-1",
                json!({"status": "closed", "reason": "x"}),
            ),
            (Method::PUT, "/CR-1/tasks", json!({"tasks": []})),
            (
                Method::POST,
                "/CR-1/decisions",
                json!({"decision": "approve"}),
            ),
            (Method::POST, "/CR-1/comments", json!({"body": "x"})),
            (Method::PATCH, "/CR-1/comments/c1", json!({"body": "y"})),
            (
                Method::POST,
                "/CR-1/comments/c1/replies",
                json!({"body": "z"}),
            ),
        ];
        for (error, status) in refusals() {
            for (method, resource, body) in &routes {
                let fake = Fake::with(FakeProject {
                    reviews: Ok(FakeReviews {
                        header: Err(error.clone()),
                        comment: Err(error.clone()),
                        updated: Err(error.clone()),
                    }),
                    ..FakeProject::default()
                });
                let (answer, calls) = send(fake, method, resource, body.clone());
                assert_eq!(answer.status, status, "{error}: {method} {resource}");
                assert_eq!(answer.json(), json!({"error": error.to_string()}));
                assert_eq!(calls.len(), 3, "the call was made: {method} {resource}");
            }
        }
        // The review rules' refusal in particular (the author deciding their own review): 403
        let rules = Error::Refused("the review's author can't decide it".into());
        let fake = Fake::with(FakeProject {
            reviews: Ok(FakeReviews {
                header: Err(rules),
                ..FakeReviews::default()
            }),
            ..FakeProject::default()
        });
        let (answer, _) = send(
            fake,
            &Method::POST,
            "/CR-1/decisions",
            json!({"decision": "approve"}),
        );
        assert_eq!(answer.status, StatusCode::FORBIDDEN);
    }

    /// A review's reference and a comment's ID are passed as the path has them: atm-lib resolves them
    #[test]
    fn the_review_and_comment_are_the_paths_own() {
        for cr in ["CR-1", "CR-42", "0193c0de"] {
            let (_, calls) = send(
                ok(),
                &Method::POST,
                &format!("/{cr}/comments/c9/replies"),
                json!({"body": "x"}),
            );
            assert_eq!(
                calls.last(),
                Some(&Call::Reply(r(cr), c("c9"), NewReply { body: "x".into() }))
            );
        }
    }
}
