//! The read API the dashboard polls (docs/multi-project-api.md; ADR-003 §5): the workspaces
//! (`/api/v2/workspaces`), a workspace's projects with their summaries (`…/<w>/projects`) and search across them
//! (`…/<w>/search`), and a project's tasks with what atm derives for them, its documents and images, claims,
//! worktrees and logo (`…/<w>/projects/<p>/…`). The v1 routes (`/api/projects`, `/api/search`,
//! `/api/v1/projects/<p>/…`, `/p/<p>/…`) are the same for the `default` workspace. Each through atm-lib, on a
//! snapshot of the project opened for the request; each with an `ETag`, so a poll of something unchanged gets a 304.
//! One project's broken files never break the list of all projects (docs/security-review.md F18).

use std::path::Path;

use atm_common::py::to_json;
use atm_common::sha1::sha1_hex;
use atm_common::time::now_iso;
use atm_lib::query::Filters;
use atm_lib::workspaces::{Member, Workspace, Workspaces, file_stamp, too_large};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use serde_json::{Value, json};

use super::{send, send_conditional, send_error};

/// A logo is the project's own file, which could be an SVG with a script: it is shown as an image, never run.
const LOGO_HEADERS: [(&str, &str); 3] = [
    (
        "Content-Security-Policy",
        "default-src 'none'; style-src 'unsafe-inline'; sandbox",
    ),
    ("X-Content-Type-Options", "nosniff"),
    ("Cache-Control", "no-cache"),
];

/// What the server shows: the state folder's workspaces, through atm-lib (their summaries cached there).
pub struct Projects {
    workspaces: Workspaces,
}

impl Projects {
    /// The workspaces of the state folder `home`.
    #[must_use]
    pub fn new(home: &Path) -> Self {
        Self {
            workspaces: Workspaces::at(home),
        }
    }
}

/// A strong `ETag` from what a response depends on.
fn etag(parts: &Value) -> String {
    format!("\"{}\"", &sha1_hex(to_json(parts).as_bytes())[..16])
}

/// `body` with `generated_at` first, as every JSON answer has it.
fn stamped(body: Value) -> Value {
    let mut out = json!({"generated_at": now_iso()});
    if let (Some(out), Value::Object(body)) = (out.as_object_mut(), body) {
        out.extend(body);
    }
    out
}

fn ok_json(body: &Value, tag: Option<&str>) -> Response {
    send(
        StatusCode::OK,
        to_json(body).into_bytes(),
        "application/json",
        tag,
        &[],
    )
}

fn unchanged(request: &HeaderMap, tag: &str) -> bool {
    request
        .get(axum::http::header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        == Some(tag)
}

fn not_modified(tag: &str, headers: &[(&str, &str)]) -> Response {
    send(
        StatusCode::NOT_MODIFIED,
        Vec::new(),
        "application/json",
        Some(tag),
        headers,
    )
}

/// atm-lib's refusals as HTTP statuses: a malformed request, no such thing, one too large, one atm doesn't show.
fn error_response(error: &atm_common::Error) -> Response {
    let status = match error {
        atm_common::Error::Invalid(_) => StatusCode::BAD_REQUEST,
        atm_common::Error::NotFound(_) => StatusCode::NOT_FOUND,
        atm_common::Error::TooLarge(_) => StatusCode::PAYLOAD_TOO_LARGE,
        atm_common::Error::Refused(_) => StatusCode::FORBIDDEN,
        atm_common::Error::Failed { .. } => StatusCode::INTERNAL_SERVER_ERROR,
    };
    send_error(status, &error.to_string())
}

/// `/api/v2/workspaces`: the workspaces, with how many projects each lists, and the one opened last.
fn api_workspaces(projects: &Projects) -> Response {
    ok_json(&stamped(projects.workspaces.summary()), None)
}

/// A workspace's projects, with their summaries. The `ETag` changes with its file, and with each project's task data,
/// claims, worktrees, logo and the day.
fn api_projects(workspace: &Workspace, request: &HeaderMap) -> Response {
    let listing = workspace.listing();
    let tag = etag(&listing.stamp);
    let body = || to_json(&stamped(json!({"projects": listing.projects()}))).into_bytes();
    send_conditional(request, &tag, body, "application/json", &[])
}

/// Percent-decoding of a query's part, `+` as a space, as Python's `parse_qs` reads it.
fn decode(text: &str) -> String {
    let hex = |b: u8| {
        char::from(b)
            .to_digit(16)
            .and_then(|d| u8::try_from(d).ok())
    };
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let pair = (
            bytes.get(i + 1).copied().and_then(hex),
            bytes.get(i + 2).copied().and_then(hex),
        );
        match (bytes[i], pair) {
            (b'+', _) => out.push(b' '),
            (b'%', (Some(high), Some(low))) => {
                out.push(high * 16 + low);
                i += 2;
            }
            (b, _) => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A query string's parameters, as Python's `parse_qs` gives them: each name's values in order, blank ones left out.
fn parse_query(query: &str) -> Vec<(String, String)> {
    query
        .split('&')
        .filter_map(|part| {
            let (name, value) = part.split_once('=')?;
            let value = decode(value);
            (!value.is_empty()).then(|| (decode(name), value))
        })
        .collect()
}

/// The first value of the query parameter `name`, or an empty text.
fn parameter(query: &str, name: &str) -> String {
    parse_query(query)
        .into_iter()
        .find(|(n, _)| n == name)
        .map(|(_, v)| v)
        .unwrap_or_default()
}

/// atm-lib's filters from a tasks route's query string (docs/layered-architecture.md §6.2): the list filters
/// repeatable, the flags `true` or `1`, other parameters ignored.
fn task_filters(query: &str) -> Filters {
    let given = parse_query(query);
    let all = |name: &str| -> Vec<String> {
        given
            .iter()
            .filter(|(n, _)| n == name)
            .map(|(_, v)| v.clone())
            .collect()
    };
    let first = |name: &str| all(name).into_iter().next();
    let flag =
        |name: &str| first(name).is_some_and(|v| matches!(v.to_lowercase().as_str(), "true" | "1"));
    Filters {
        sprint: all("sprint"),
        backlog: flag("backlog"),
        release: first("release"),
        component: first("component"),
        status: all("status"),
        type_: first("type"),
        priority: first("priority"),
        label: all("label"),
        agent: first("agent"),
        ready: flag("ready"),
        blocked: flag("blocked"),
        in_pr: flag("in_pr"),
        with_user: flag("with_user"),
        archived: flag("archived"),
    }
}

/// A project's tasks the filters keep, each with its derived values, and in the same snapshot its meta and how big
/// its task data is. Read tolerantly: the dashboard shows a malformed task file, with what is wrong with it (T-377).
/// The `ETag` changes with the task data, the archive and the query.
fn api_tasks(member: &Member, query: &str, request: &HeaderMap) -> Response {
    if too_large(&member.stamp) {
        // Each request reads the whole task data into memory: a runaway file is refused, not served
        return error_response(&member.tasks(&Filters::default()).unwrap_err());
    }
    let tag = etag(&json!([
        member.stamp["tasks"],
        member.stamp["archive"],
        query
    ]));
    if unchanged(request, &tag) {
        return not_modified(&tag, &[]);
    }
    match member.tasks(&task_filters(query)) {
        Ok(snapshot) => ok_json(&stamped(snapshot), Some(&tag)),
        // A filter naming no such sprint or release (400), an unreadable task file (500)
        Err(e) => {
            let status = if e.code() == 2 {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            send_error(status, &e.to_string())
        }
    }
}

/// A project's claims, worktrees or logo.
fn project_file(member: &Member, name: &str, request: &HeaderMap) -> Response {
    let stamp = &member.stamp;
    match name {
        "logo" => {
            // The project's logo (the store checks it is inside the project and small enough): kept by the browser,
            // asked for again each time (no-cache), a 304 while it is unchanged
            let Some((file, kind)) = member.logo() else {
                return send_error(StatusCode::NOT_FOUND, "no logo");
            };
            let stamp = file_stamp(&file);
            if stamp.is_null() {
                return send_error(StatusCode::NOT_FOUND, "no logo");
            }
            let read = || std::fs::read(&file).unwrap_or_default();
            send_conditional(request, &etag(&stamp), read, kind, &LOGO_HEADERS)
        }
        "worktrees" => {
            // The working copies, main first, and the tasks that differ in the others; the claims decide which copy
            // of a task is stale
            let tag = etag(&json!(["worktrees", stamp["copies"], stamp["claims"]]));
            let body = || to_json(&stamped(json!({"worktrees": member.worktrees()}))).into_bytes();
            send_conditional(request, &tag, body, "application/json", &[])
        }
        _ => {
            // The claims of all its worktrees, from the shared claims store
            let tag = etag(&json!(["claims", stamp["claims"]]));
            let body = || to_json(&stamped(json!({"claims": member.claims()}))).into_bytes();
            send_conditional(request, &tag, body, "application/json", &[])
        }
    }
}

/// Search: per project, the IDs of the working tasks (not the archive) in which every word of the query appears,
/// case-insensitively, as the dashboard's own search finds them; the project's name counts as part of every task's
/// text. Read-only, like every route; `{}` for an empty query.
fn api_search(workspace: &Workspace, query: &str) -> Response {
    ok_json(&json!(workspace.search(&parameter(query, "q"))), None)
}

/// A project's document (`documents?path=`, the viewer's JSON) or image (`files?path=`), through atm-lib, which checks
/// the path and reads the file from the working copy. The `ETag` is the file's size and mtime (and, for a document,
/// the current commit, which it names), so the viewer's poll gets a 304 without the file being read again.
fn api_file(member: &Member, route: &str, query: &str, request: &HeaderMap) -> Response {
    let Some(vcs) = member.project().ok().and_then(|p| p.vcs()) else {
        return send_error(StatusCode::NOT_FOUND, "not found");
    };
    let path = parameter(query, "path");
    let stamp = match atm_lib::documents::document_stamp(Some(&vcs), &path) {
        Ok(stamp) => stamp,
        Err(e) => return error_response(&e),
    };
    if route == "documents" {
        let tag = etag(&json!([
            "document",
            [stamp.0, stamp.1.to_string()],
            vcs.head()
        ]));
        if unchanged(request, &tag) {
            return not_modified(&tag, &[]);
        }
        return match member.document(&path) {
            Ok(document) => ok_json(&document, Some(&tag)),
            Err(e) => error_response(&e),
        };
    }
    let tag = format!("\"{:x}-{:x}\"", stamp.0, stamp.1);
    if unchanged(request, &tag) {
        return not_modified(&tag, &LOGO_HEADERS);
    }
    match member.image(&path) {
        Ok((data, content_type)) => send(
            StatusCode::OK,
            data,
            content_type,
            Some(&tag),
            &LOGO_HEADERS,
        ),
        Err(e) => error_response(&e),
    }
}

/// A route about one project of `workspace`: `tasks`, `documents`, `files`, `claims`, `worktrees` or `logo`.
fn project_route(
    workspace: &Workspace,
    id: &str,
    route: &str,
    query: &str,
    request: &HeaderMap,
) -> Response {
    let member = match workspace.member(id) {
        Ok(member) => member,
        Err(e) => return error_response(&e),
    };
    match route {
        "tasks" => api_tasks(&member, query, request),
        "documents" | "files" => api_file(&member, route, query, request),
        _ => project_file(&member, route, request),
    }
}

/// The v2 routes, past `/api/v2/workspaces/`: `<w>/projects`, `<w>/search`, `<w>/projects/<p>/<route>`.
fn serve_v2(projects: &Projects, rest: &str, query: &str, request: &HeaderMap) -> Response {
    let parts: Vec<&str> = rest.split('/').collect();
    let Some(workspace) = parts
        .first()
        .filter(|w| !w.is_empty())
        .and_then(|w| projects.workspaces.get(w))
    else {
        return send_error(StatusCode::NOT_FOUND, "unknown workspace");
    };
    match parts[1..] {
        ["projects"] => {
            // Opening a workspace's list is opening the workspace: the last opened is the one `/` goes to. A choice
            // of view, the one thing a read records (ADR-003 §5); written only when it changes
            let _ = projects.workspaces.opened(workspace.id());
            api_projects(&workspace, request)
        }
        ["search"] => api_search(&workspace, query),
        ["projects", id, route]
            if !id.is_empty()
                && matches!(
                    route,
                    "tasks" | "documents" | "files" | "claims" | "worktrees" | "logo"
                ) =>
        {
            project_route(&workspace, id, route, query, request)
        }
        _ => send_error(StatusCode::NOT_FOUND, "not found"),
    }
}

/// The routes past the page, its assets and whoami: the read API; everything else is 404.
pub fn serve(projects: &Projects, path: &str, query: &str, request: &HeaderMap) -> Response {
    if path == "/api/v2/workspaces" {
        return api_workspaces(projects);
    }
    if let Some(rest) = path.strip_prefix("/api/v2/workspaces/") {
        return serve_v2(projects, rest, query, request);
    }
    // The v1 routes: the default workspace's
    let default = projects.workspaces.default_workspace();
    if path == "/api/projects" {
        // The dashboard reads `default`'s list here: opening it, as `…/default/projects` would
        let _ = projects.workspaces.opened(default.id());
        return api_projects(&default, request);
    }
    if path == "/api/search" {
        return api_search(&default, query);
    }
    if let Some(rest) = path.strip_prefix("/api/v1/projects/")
        && let Some((id, route @ ("documents" | "files" | "tasks"))) = rest.split_once('/')
        && !id.is_empty()
    {
        return project_route(&default, id, route, query, request);
    }
    if let Some(rest) = path.strip_prefix("/p/")
        && let Some((id, name)) = rest.split_once('/')
        && !id.is_empty()
        && matches!(name, "claims.json" | "worktrees.json" | "logo")
    {
        return project_route(&default, id, name.trim_end_matches(".json"), query, request);
    }
    send_error(StatusCode::NOT_FOUND, "not found")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_query_is_read_as_parse_qs_reads_it() {
        assert_eq!(
            parse_query("status=todo&status=blocked&q=a+b%21&empty=&x"),
            [
                ("status".to_string(), "todo".to_string()),
                ("status".into(), "blocked".into()),
                ("q".into(), "a b!".into())
            ]
        );
        let filters = task_filters("sprint=S1&ready=TRUE&archived=0&type=bugfix");
        assert_eq!(filters.sprint, ["S1"]);
        assert!(filters.ready && !filters.archived);
        assert_eq!(filters.type_.as_deref(), Some("bugfix"));
        assert_eq!(decode("%zz%4"), "%zz%4");
    }
}
