//! The review rules (who may decide, what a comment may say) are atm-lib's and its tests'; the checks before a change
//! reaches a route (Host, Origin, session token, JSON) are `serve_request`'s. Here: which route each method and
//! resource reach, the request each body becomes, and the status each answer becomes.

use atm_common::Error;
use atm_lib::review_drafts::{DraftUpdate, NewDraft};
use atm_lib::review_requests::{
    ChangeToReview, CommentUpdate, Decision, DecisionKind, NewComment, NewPoke, NewReply,
    ReviewClosing, ReviewOpening, ReviewStart, ReviewTasks, RulesOverride,
};
use atm_lib::reviews::{Side, Status};

use super::super::test_support::{
    Answer, Call, Fake, FakeProject, FakeReviews, answer, bound, having, header, plain, refusals,
};
use super::*;

const REVIEWS: &str = "/api/v2/workspaces/work/projects/p1/reviews";

/// The fake's draft, as a route answers it
fn drafted() -> Value {
    json!({"id": "d1", "at": "2026-11-19T10:00:00Z", "body": "Why?", "kind": "comment",
               "path": null, "lines": null, "side": "new", "quote": null, "round": 1})
}

/// ATM-612: the user's drafts, read as the user, never cached; each refusal its status
#[test]
fn the_drafts_are_the_users_read_as_the_user_and_never_cached() {
    let (project, calls) = bound(FakeProject::default());
    let got = answer(get_drafts(project.as_ref(), "CR-2"));
    assert_eq!(got.status, StatusCode::OK);
    assert_eq!(
        got.unstamped(),
        json!({"review": "CR-2", "drafts": [drafted()]})
    );
    assert_eq!(got.header("cache-control"), "no-store");
    assert_eq!(calls.taken(), [Call::AsUser, Call::Drafts(r("CR-2"))]);
    for (error, status) in refusals() {
        let (project, _) = bound(FakeProject {
            reviews: Ok(FakeReviews {
                draft: Err(error.clone()),
                ..FakeReviews::default()
            }),
            ..FakeProject::default()
        });
        let got = answer(get_drafts(project.as_ref(), "CR-2"));
        assert_eq!(got.status, status, "{error}");
        assert_eq!(got.json(), json!({"error": error.to_string()}));
        // The project not opened as the user: the same
        let (project, _) = bound(FakeProject {
            reviews: Err(error.clone()),
            ..FakeProject::default()
        });
        assert_eq!(get_drafts(project.as_ref(), "CR-2").status(), status);
    }
}

/// A draft is removed with an empty body: anything else is refused before atm-lib is asked
#[test]
fn a_draft_is_removed_only_with_an_empty_body() {
    for body in [json!({"id": "d2"}), json!([]), Value::Null, json!("d1")] {
        let (answer, calls) = send(ok(), &Method::DELETE, "/CR-1/drafts/d1", body.clone());
        assert_eq!(answer.status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(
            calls,
            [Call::Project("work".into(), "p1".into()), Call::AsUser],
            "{body}"
        );
    }
}

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
    let listed =
        json!({"reviews": [{"review": {"reference": "CR-2"}, "needs_you": true}], "you": "Ana"});
    let page =
        json!({"review": {"reference": "CR-2"}, "view": {"round": 2, "files": [{"path": "a"}]}});
    let (project, _) = bound(FakeProject {
        listed: Ok(listed.clone()),
        page: Ok(page.clone()),
        ..FakeProject::default()
    });
    for (got, expected) in [
        (answer(get_reviews(project.as_ref(), "")), listed),
        (
            answer(get_review(project.as_ref(), "CR-2", "", &plain())),
            page,
        ),
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
            get_review(project.as_ref(), "CR-7", query, &plain()).status(),
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
        let refused = answer(get_review(project.as_ref(), "CR-7", query, &plain()));
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
            answer(get_review(project.as_ref(), "CR-1", "", &plain())),
        ] {
            assert_eq!(got.status, status, "{error}");
            assert_eq!(got.json(), json!({"error": error.to_string()}));
        }
    }
}

/// ATM-607: a live review's page of its working copy now is tagged from what it is made of; asked again with the
/// tag while that is the same, a 304 with no page made; once it changes, the page and a new tag
#[test]
fn a_live_pages_poll_gets_a_304_while_its_working_copy_is_the_same() {
    let live = |state: &str| FakeProject {
        live_stamp: Ok(Some(json!(["live", "r1", null, [1, 1], state, "Ana"]))),
        ..FakeProject::default()
    };
    let (project, calls) = bound(live("a.txt 3:1"));
    let first = answer(get_review(project.as_ref(), "CR-1", "", &plain()));
    assert_eq!(first.status, StatusCode::OK);
    let tag = first.tag();
    assert!(!tag.is_empty(), "tagged");
    assert_eq!(calls.taken(), [Call::Review(r("CR-1"), None, None)]);
    let again = answer(get_review(project.as_ref(), "CR-1", "", &having(&tag)));
    assert_eq!(again.status, StatusCode::NOT_MODIFIED);
    assert_eq!(again.tag(), tag);
    assert_eq!(calls.taken(), [], "no page made for a 304");
    // A file written: another tag, and the page made again
    let (project, calls) = bound(live("a.txt 4:2"));
    let changed = answer(get_review(project.as_ref(), "CR-1", "", &having(&tag)));
    assert_eq!(changed.status, StatusCode::OK);
    assert_ne!(changed.tag(), tag);
    assert_eq!(calls.taken(), [Call::Review(r("CR-1"), None, None)]);
}

#[test]
fn a_live_pages_stamp_refused_is_its_status() {
    for (error, status) in refusals() {
        let (project, calls) = bound(FakeProject {
            live_stamp: Err(error.clone()),
            ..FakeProject::default()
        });
        let got = answer(get_review(project.as_ref(), "CR-1", "", &plain()));
        assert_eq!(got.status, status, "{error}");
        assert_eq!(calls.taken(), [], "{error}: no page made");
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
        // ATM-611: a poke, with a message or without one
        (
            Method::POST,
            "/CR-1/pokes",
            json!({"body": "Where are we?"}),
            Call::Poke(
                r("CR-1"),
                NewPoke {
                    body: Some("Where are we?".into()),
                },
            ),
            StatusCode::CREATED,
            json!({"review": "CR-1", "poke": "c1"}),
        ),
        (
            Method::POST,
            "/CR-1/pokes",
            json!({}),
            Call::Poke(r("CR-1"), NewPoke::default()),
            StatusCode::CREATED,
            json!({"review": "CR-1", "poke": "c1"}),
        ),
        // ATM-612: a draft made, its text changed, thrown away
        (
            Method::POST,
            "/CR-1/drafts",
            json!({"path": "a.rs", "lines": [2, 3], "body": "Why?"}),
            Call::AddDraft(
                r("CR-1"),
                NewDraft {
                    path: Some("a.rs".into()),
                    lines: Some((2, 3)),
                    body: "Why?".into(),
                    ..NewDraft::default()
                },
            ),
            StatusCode::CREATED,
            drafted(),
        ),
        (
            Method::POST,
            "/CR-1/drafts",
            json!({"reply_to": "c4", "body": "Because."}),
            Call::AddDraft(
                r("CR-1"),
                NewDraft {
                    reply_to: Some("c4".into()),
                    body: "Because.".into(),
                    ..NewDraft::default()
                },
            ),
            StatusCode::CREATED,
            drafted(),
        ),
        (
            Method::PATCH,
            "/CR-1/drafts/d1",
            json!({"body": "Why not?"}),
            Call::UpdateDraft(
                r("CR-1"),
                "d1".into(),
                DraftUpdate {
                    body: "Why not?".into(),
                },
            ),
            StatusCode::OK,
            drafted(),
        ),
        (
            Method::DELETE,
            "/CR-1/drafts/d1",
            json!({}),
            Call::RemoveDraft(r("CR-1"), "d1".into()),
            StatusCode::OK,
            json!({"review": "CR-1", "draft": "d1", "removed": true}),
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
        (Method::DELETE, "/CR-1/comments/c1"),
        (Method::PUT, "/CR-1/drafts"),
        (Method::DELETE, "/CR-1/drafts"),
        (Method::POST, "/CR-1/drafts/d1"),
        (Method::PUT, "/CR-1/drafts/d1"),
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
        (Method::POST, "/CR-1/drafts", json!({"path": "a"})),
        (
            Method::POST,
            "/CR-1/drafts",
            json!({"body": "x", "author": "claude:x"}),
        ),
        (Method::PATCH, "/CR-1/drafts/d1", json!({})),
        (
            Method::PATCH,
            "/CR-1/drafts/d1",
            json!({"body": "x", "path": "b"}),
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
    let routes: [(Method, &str, Value); 10] = [
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
        (Method::POST, "/CR-1/drafts", json!({"body": "x"})),
        (Method::PATCH, "/CR-1/drafts/d1", json!({"body": "y"})),
        (Method::DELETE, "/CR-1/drafts/d1", json!({})),
    ];
    for (error, status) in refusals() {
        for (method, resource, body) in &routes {
            let fake = Fake::with(FakeProject {
                reviews: Ok(FakeReviews {
                    header: Err(error.clone()),
                    comment: Err(error.clone()),
                    updated: Err(error.clone()),
                    draft: Err(error.clone()),
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
