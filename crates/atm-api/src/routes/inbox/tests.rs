//! The inbox's rules (who may add, from which state) are atm-lib's and its tests'; the checks before a change reaches
//! a route (Host, Origin, session token, JSON) are `serve_request`'s, as for reviews. Here: which route each method and
//! resource reach, the request each body becomes, the status each answer becomes, and the read's `ETag`.

use atm_common::inbox::InboxState;
use atm_lib::inbox::{Decided, Listing};
use serde_json::json;

use super::super::test_support::{
    Answer, Call, Fake, FakeProject, answer, bound, having, plain, refusals,
};
use super::*;

const INBOX: &str = "/api/v2/workspaces/work/projects/p1/inbox";

fn send(fake: Fake, method: &Method, resource: &str, body: Value) -> (Answer, Vec<Call>) {
    let (projects, calls) = fake.projects();
    let answer = answer(serve_inbox_change(
        &projects,
        method,
        &format!("{INBOX}{resource}"),
        body,
    ));
    (answer, calls.taken())
}

fn ok() -> Fake {
    Fake::with(FakeProject::default())
}

const OPENED: [Call; 0] = [];

fn opened() -> Vec<Call> {
    vec![Call::Project("work".into(), "p1".into()), Call::InboxAsUser]
}

#[test]
fn only_a_projects_inbox_takes_changes() {
    for (path, ok) in [
        ("/api/v2/workspaces/work/projects/p1/inbox", true),
        (
            "/api/v2/workspaces/work/projects/p1/inbox/IN-3/decisions",
            true,
        ),
        ("/api/v2/workspaces/work/projects/p1/inboxes", false),
        ("/api/v2/workspaces//projects/p1/inbox", false),
        ("/api/v2/workspaces/work/projects//inbox", false),
        ("/api/v1/projects/p1/inbox", false),
        ("/api/v2/workspaces/work/projects/p1/tasks", false),
    ] {
        assert_eq!(is_inbox_path(path), ok, "{path}");
    }
}

#[test]
fn the_list_asks_for_the_state_and_the_archive_the_query_names() {
    for (query, which, archived) in [
        ("", Listing::Open, false),
        ("state=created", Listing::Open, false),
        ("state=discarded", Listing::In(InboxState::Discarded), false),
        (
            "state=injected&archived=1",
            Listing::In(InboxState::Injected),
            true,
        ),
        ("state=all&archived=true", Listing::All, true),
        ("archived=0", Listing::Open, false),
    ] {
        let (project, calls) = bound(FakeProject::default());
        let got = answer(get_inbox(project.as_ref(), query, &plain()));
        assert_eq!(got.status, StatusCode::OK, "{query}");
        assert_eq!(calls.taken(), [Call::Inbox(which, archived)], "{query}");
    }
}

#[test]
fn a_state_or_an_archive_that_isnt_one_is_a_bad_request_before_atm_lib_is_asked() {
    for query in ["state=open", "archived=yes"] {
        let (project, calls) = bound(FakeProject::default());
        let got = answer(get_inbox(project.as_ref(), query, &plain()));
        assert_eq!(got.status, StatusCode::BAD_REQUEST, "{query}");
        assert_eq!(calls.taken(), OPENED, "{query}");
    }
}

#[test]
fn the_list_is_atm_libs_answer_stamped_with_an_etag_a_poll_of_it_unchanged_is_a_304() {
    let listed = json!({"items": [{"id": "IN-2"}], "created": 1, "discarded": 0, "injected": 0});
    let (project, _) = bound(FakeProject {
        inbox: Ok(listed.clone()),
        ..FakeProject::default()
    });
    let got = answer(get_inbox(project.as_ref(), "", &plain()));
    assert_eq!(got.unstamped(), listed);
    let tag = got.tag();
    assert!(tag.starts_with('"'), "{tag}");
    let again = answer(get_inbox(project.as_ref(), "", &having(&tag)));
    assert_eq!(again.status, StatusCode::NOT_MODIFIED);
    assert_eq!(again.body, Vec::<u8>::new());
    // The archive's listing is another version, even with the same items
    let archived = answer(get_inbox(project.as_ref(), "archived=1", &having(&tag)));
    assert_eq!(archived.status, StatusCode::OK);
    assert_ne!(archived.tag(), tag);
    // Another answer, another tag
    let (changed, _) = bound(FakeProject {
        inbox: Ok(json!({"items": [], "created": 0, "discarded": 1, "injected": 0})),
        ..FakeProject::default()
    });
    assert_eq!(
        answer(get_inbox(changed.as_ref(), "", &having(&tag))).status,
        StatusCode::OK
    );
}

#[test]
fn each_refusal_of_the_list_is_its_status() {
    for (error, status) in refusals() {
        let (project, _) = bound(FakeProject {
            inbox: Err(error.clone()),
            ..FakeProject::default()
        });
        let got = answer(get_inbox(project.as_ref(), "", &plain()));
        assert_eq!(got.status, status, "{error}");
        assert_eq!(got.json(), json!({"error": error.to_string()}));
    }
}

#[test]
fn each_change_reaches_its_call_as_the_user_with_its_status() {
    let item = || json!({"id": "IN-1", "state": "created"});
    for (method, resource, body, call, status) in [
        (
            Method::POST,
            "",
            json!({"text": "Note"}),
            Call::AddItem(NewItem {
                text: "Note".into(),
            }),
            StatusCode::CREATED,
        ),
        (
            Method::PATCH,
            "/IN-3",
            json!({"text": "Better"}),
            Call::EditItem(
                "IN-3".into(),
                ItemEdit {
                    text: "Better".into(),
                },
            ),
            StatusCode::OK,
        ),
        (
            Method::POST,
            "/IN-3/decisions",
            json!({"decision": "discarded", "reason": "Done already"}),
            Call::DecideItem(
                "IN-3".into(),
                ItemDecision {
                    decision: Decided::Discarded,
                    reason: Some("Done already".into()),
                },
            ),
            StatusCode::CREATED,
        ),
        (
            Method::POST,
            "/IN-3/decisions",
            json!({"decision": "restored"}),
            Call::DecideItem(
                "IN-3".into(),
                ItemDecision {
                    decision: Decided::Restored,
                    reason: None,
                },
            ),
            StatusCode::CREATED,
        ),
    ] {
        let (got, calls) = send(ok(), &method, resource, body);
        assert_eq!(got.status, status, "{method} {resource}");
        assert_eq!(got.unstamped(), json!({ "item": item() }));
        assert_eq!(got.header("cache-control"), "no-store");
        let mut expected = opened();
        expected.push(call);
        assert_eq!(calls, expected, "{method} {resource}");
    }
}

#[test]
fn a_body_that_isnt_the_request_is_a_bad_request_and_atm_lib_changes_nothing() {
    for (method, resource, body) in [
        // An unknown field: deny_unknown_fields
        (Method::POST, "", json!({"text": "x", "by": "claude:a"})),
        (Method::POST, "", json!({})),
        (Method::POST, "", json!(["x"])),
        (Method::PATCH, "/IN-3", json!({"text": 3})),
        // An injection isn't the page's: no such decision
        (
            Method::POST,
            "/IN-3/decisions",
            json!({"decision": "injected", "tasks": ["ATM-1"]}),
        ),
        (
            Method::POST,
            "/IN-3/decisions",
            json!({"decision": "discarded", "why": "x"}),
        ),
    ] {
        let (got, calls) = send(ok(), &method, resource, body.clone());
        assert_eq!(got.status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(calls, opened(), "{body}");
    }
}

#[test]
fn a_resource_no_route_has_is_404_and_a_method_it_isnt_for_405_before_the_project_is_opened() {
    for (method, resource, status) in [
        (Method::GET, "", StatusCode::METHOD_NOT_ALLOWED),
        (Method::PUT, "", StatusCode::METHOD_NOT_ALLOWED),
        (Method::DELETE, "/IN-3", StatusCode::METHOD_NOT_ALLOWED),
        (
            Method::PATCH,
            "/IN-3/decisions",
            StatusCode::METHOD_NOT_ALLOWED,
        ),
        (Method::POST, "/IN-3/injections", StatusCode::NOT_FOUND),
        (Method::POST, "/IN-3/decisions/x", StatusCode::NOT_FOUND),
        (Method::PATCH, "//", StatusCode::NOT_FOUND),
    ] {
        let (got, calls) = send(ok(), &method, resource, json!({}));
        assert_eq!(got.status, status, "{method} {resource}");
        assert_eq!(calls, OPENED, "{method} {resource}");
    }
}

#[test]
fn each_refusal_of_a_change_is_its_status_and_a_project_not_opened_too() {
    for (error, status) in refusals() {
        let (got, _) = send(
            Fake::with(FakeProject {
                inbox_changes: Ok(Err(error.clone())),
                ..FakeProject::default()
            }),
            &Method::POST,
            "",
            json!({"text": "x"}),
        );
        assert_eq!(got.status, status, "{error}");
        assert_eq!(got.json(), json!({"error": error.to_string()}));
        let (got, _) = send(
            Fake::with(FakeProject {
                inbox_changes: Err(error.clone()),
                ..FakeProject::default()
            }),
            &Method::POST,
            "",
            json!({"text": "x"}),
        );
        assert_eq!(got.status, status, "{error}");
    }
}

#[test]
fn an_unknown_workspace_or_project_is_404() {
    let (projects, _) = ok().projects();
    for path in [
        "/api/v2/workspaces/nope/projects/p1/inbox",
        "/api/v2/workspaces/work/projects/nope/inbox",
    ] {
        let got = answer(serve_inbox_change(
            &projects,
            &Method::POST,
            path,
            json!({"text": "x"}),
        ));
        assert_eq!(got.status, StatusCode::NOT_FOUND, "{path}");
    }
}
