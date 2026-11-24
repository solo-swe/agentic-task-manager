//! A project's inbox (docs/inbox-design.md §7, ATM-761): its items, read with an `ETag`, and the page's changes. Each
//! change is one atm-lib call (`inbox`), as the user at the machine; `serve_request` has checked the request first, as
//! for a review change: the Host, the server's own Origin, the session token, a JSON body (I11).
//!
//! | Method | Resource | Does |
//! |---|---|---|
//! | GET | `inbox?state=&archived=` | the items (the open ones by default) |
//! | POST | `inbox` | an item added |
//! | PATCH | `inbox/<IN>` | an open item's text |
//! | POST | `inbox/<IN>/decisions` | discarded (with a reason) or restored |
//!
//! No route injects an item: an agent records the tasks an item became, with `atm inbox inject`.

use atm_common::py::to_json;
use atm_lib::inbox::{ItemDecision, ItemEdit, NewItem, listing_of};
use atm_lib::review_requests::request_from;
use axum::http::{HeaderMap, Method, StatusCode};
use axum::response::Response;
use serde_json::Value;

use super::query::parameter;
use super::source::{InboxChanges, ProjectSource};
use super::{Projects, error_response, etag, not_modified, ok_json, stamped, unchanged};
use crate::{send, send_error};

/// `GET …/inbox?state=&archived=`: the items `state` names (`created`, the open ones, by default; `discarded`,
/// `injected`, `all`), of the archive with `archived=1`, newest first, with the counts. Tagged by what it holds, so a
/// poll of an unchanged inbox is a 304.
pub(super) fn get_inbox(project: &dyn ProjectSource, query: &str, request: &HeaderMap) -> Response {
    let state = parameter(query, "state");
    let which = match listing_of((!state.is_empty()).then_some(state.as_str())) {
        Ok(which) => which,
        Err(e) => return error_response(&e),
    };
    let archived = match parameter(query, "archived").as_str() {
        "" | "0" | "false" => false,
        "1" | "true" => true,
        _ => return send_error(StatusCode::BAD_REQUEST, "archived is 1 or 0"),
    };
    let listed = match project.inbox(which, archived) {
        Ok(listed) => listed,
        Err(e) => return error_response(&e),
    };
    let tag = etag(&serde_json::json!(["inbox", archived, &listed]));
    if unchanged(request, &tag) {
        return not_modified(&tag, &[]);
    }
    ok_json(&stamped(listed), Some(&tag))
}

/// The answer to an inbox change: 201 with the item made, 200 with the item changed, or atm-lib's refusal.
fn answer(made: bool, result: atm_common::Result<Value>) -> Response {
    match result {
        Ok(item) => send(
            if made {
                StatusCode::CREATED
            } else {
                StatusCode::OK
            },
            to_json(&stamped(serde_json::json!({ "item": item }))).into_bytes(),
            "application/json",
            None,
            &[],
        ),
        Err(e) => error_response(&e),
    }
}

/// Whether `path` is under a project's inbox (`/api/v2/workspaces/<w>/projects/<p>/inbox…`), where a POST or PATCH may
/// change something.
#[must_use]
pub fn is_inbox_path(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    matches!(
        parts.as_slice(),
        ["", "api", "v2", "workspaces", w, "projects", p, "inbox", ..] if !w.is_empty() && !p.is_empty()
    )
}

/// One of the change routes of the table above.
enum Route<'a> {
    Add,
    Edit(&'a str),
    Decide(&'a str),
}

/// The route for `method` on `resource` (past `inbox`): a resource no route has is 404, one the method isn't for 405.
fn route<'a>(
    method: &Method,
    resource: &[&'a str],
) -> Result<Route<'a>, (StatusCode, &'static str)> {
    Ok(match (method.as_str(), resource) {
        ("POST", []) => Route::Add,
        ("PATCH", [item]) => Route::Edit(item),
        ("POST", [item, "decisions"]) => Route::Decide(item),
        (_, [] | [_] | [_, "decisions"]) => {
            return Err((StatusCode::METHOD_NOT_ALLOWED, "not with this method"));
        }
        _ => return Err((StatusCode::NOT_FOUND, "not found")),
    })
}

/// The body, when it is an object: a list would fill a request's fields in order
fn object(body: Value) -> atm_common::Result<Value> {
    if body.is_object() {
        Ok(body)
    } else {
        Err(atm_common::Error::Invalid(
            "not this request: the body is an object".into(),
        ))
    }
}

fn add(inbox: &mut dyn InboxChanges, body: Value) -> Response {
    answer(
        true,
        object(body)
            .and_then(request_from::<NewItem>)
            .and_then(|item| inbox.add_item(&item)),
    )
}

fn edit(inbox: &mut dyn InboxChanges, item: &str, body: Value) -> Response {
    answer(
        false,
        object(body)
            .and_then(request_from::<ItemEdit>)
            .and_then(|e| inbox.edit_item(item, &e)),
    )
}

fn decide(inbox: &mut dyn InboxChanges, item: &str, body: Value) -> Response {
    answer(
        true,
        object(body)
            .and_then(request_from::<ItemDecision>)
            .and_then(|d| inbox.decide_item(item, &d)),
    )
}

/// An inbox change, to its route by method and resource; the project is opened only for a route there is.
#[must_use]
pub fn serve_inbox_change(
    projects: &Projects,
    method: &Method,
    path: &str,
    body: Value,
) -> Response {
    let Some(rest) = path.strip_prefix("/api/v2/workspaces/") else {
        return send_error(StatusCode::NOT_FOUND, "not found");
    };
    let parts: Vec<&str> = rest.split('/').collect();
    let [w, "projects", p, "inbox", resource @ ..] = parts.as_slice() else {
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
    let mut inbox = match workspace.project(p).and_then(|m| m.inbox_as_user()) {
        Ok(inbox) => inbox,
        Err(e) => return error_response(&e),
    };
    let inbox = inbox.as_mut();
    match route {
        Route::Add => add(inbox, body),
        Route::Edit(item) => edit(inbox, item, body),
        Route::Decide(item) => decide(inbox, item, body),
    }
}

#[cfg(test)]
mod tests;
