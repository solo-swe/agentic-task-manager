//! The read API the dashboard polls (docs/multi-project-api.md; the Python atm's `api/server.py`): the registered
//! projects with their summaries (`/api/projects`), a project's tasks with what atm derives for them
//! (`/api/v1/projects/<id>/tasks`), its claims and worktrees (`/p/<id>/claims.json`, `worktrees.json`) and its logo
//! (`/p/<id>/logo`). Each through atm-lib, on a snapshot of the project opened for the request; each with an `ETag`,
//! so a poll of something unchanged gets a 304. One project's broken files never break the list of all projects
//! (docs/security-review.md F18).

use std::path::Path;

use atm_common::py::to_json;
use atm_common::sha1::sha1_hex;
use atm_common::time::now_iso;
use atm_lib::archive::task_snapshot;
use atm_lib::query::{Filters, list_claims};
use atm_lib::workspaces::{MAX_SERVED_BYTES, Workspace, Workspaces, file_stamp, too_large};
use atm_lib::worktrees::worktree_copies;
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

/// The projects the server shows: the `default` workspace's, through atm-lib (its summaries cached there).
pub struct Projects {
    workspace: Workspace,
}

impl Projects {
    /// The projects of the state folder `home`.
    #[must_use]
    pub fn new(home: &Path) -> Self {
        Self {
            workspace: Workspaces::at(home).default_workspace(),
        }
    }

    fn find(&self, id: &str) -> Option<Value> {
        self.workspace.find(id)
    }
}

/// A strong `ETag` from what a response depends on.
fn etag(parts: &Value) -> String {
    format!("\"{}\"", &sha1_hex(to_json(parts).as_bytes())[..16])
}

/// `/api/projects`: every registered project, with its summary. The `ETag` changes with the registry, and with each
/// project's task data, claims, worktrees, logo and the day.
fn api_projects(projects: &Projects, request: &HeaderMap) -> Response {
    let listing = projects.workspace.listing();
    let tag = etag(&listing.stamp);
    let body = || {
        let list = listing.projects();
        to_json(&json!({"generated_at": now_iso(), "projects": list})).into_bytes()
    };
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

/// atm-lib's filters from a v1 route's query string (docs/layered-architecture.md §6.2): the list filters
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

/// `GET /api/v1/projects/<p>/tasks`: the project's tasks the filters keep, each with its derived values, and in the
/// same snapshot its meta and how big its task data is. Read tolerantly: the dashboard shows a malformed task file,
/// with what is wrong with it (T-377). The `ETag` changes with the task data, the archive and the query.
fn api_v1_tasks(projects: &Projects, id: &str, query: &str, request: &HeaderMap) -> Response {
    let Some(entry) = projects.find(id) else {
        return send_error(StatusCode::NOT_FOUND, "unknown project");
    };
    let Some(stamp) = Workspace::data_stamp(&entry) else {
        return send_error(StatusCode::NOT_FOUND, "not found");
    };
    if too_large(&stamp) {
        // Each request reads the whole task data into memory: a runaway file is refused, not served
        return send_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            &format!("larger than {} MB", MAX_SERVED_BYTES / 1_000_000),
        );
    }
    let tag = etag(&json!([stamp["tasks"], stamp["archive"], query]));
    if request
        .get(axum::http::header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        == Some(tag.as_str())
    {
        return send(
            StatusCode::NOT_MODIFIED,
            Vec::new(),
            "application/json",
            Some(&tag),
            &[],
        );
    }
    let snapshot =
        Workspace::project_tolerant(&entry).and_then(|p| task_snapshot(&p, &task_filters(query)));
    match snapshot {
        Ok(snapshot) => {
            let mut body = json!({"generated_at": now_iso()});
            if let (Some(body), Some(snapshot)) = (body.as_object_mut(), snapshot.as_object()) {
                body.extend(snapshot.clone());
            }
            send(
                StatusCode::OK,
                to_json(&body).into_bytes(),
                "application/json",
                Some(&tag),
                &[],
            )
        }
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

/// `/p/<id>/claims.json`, `worktrees.json` and `logo`.
fn project_file(projects: &Projects, id: &str, name: &str, request: &HeaderMap) -> Response {
    let Some(entry) = projects.find(id) else {
        return send_error(StatusCode::NOT_FOUND, "unknown project");
    };
    let Some(stamp) = Workspace::data_stamp(&entry) else {
        return send_error(StatusCode::NOT_FOUND, "not found");
    };
    let opened = || Workspace::project(&entry);
    match name {
        "logo" => {
            // The project's logo (the store checks it is inside the project and small enough): kept by the browser,
            // asked for again each time (no-cache), a 304 while it is unchanged
            let Some((file, kind)) = Workspace::logo(&entry) else {
                return send_error(StatusCode::NOT_FOUND, "no logo");
            };
            let stamp = file_stamp(&file);
            if stamp.is_null() {
                return send_error(StatusCode::NOT_FOUND, "no logo");
            }
            let read = || std::fs::read(&file).unwrap_or_default();
            send_conditional(request, &etag(&stamp), read, kind, &LOGO_HEADERS)
        }
        "worktrees.json" => {
            // The working copies, main first, and the tasks that differ in the others; the claims decide which copy
            // of a task is stale
            let tag = etag(&json!(["worktrees", stamp["copies"], stamp["claims"]]));
            let body = || {
                let copies = opened().map(|p| worktree_copies(&p)).unwrap_or_default();
                to_json(&json!({"generated_at": now_iso(), "worktrees": copies})).into_bytes()
            };
            send_conditional(request, &tag, body, "application/json", &[])
        }
        _ => {
            // The claims of all its worktrees, from the shared claims store
            let tag = etag(&json!(["claims", stamp["claims"]]));
            let body = || {
                let claims = opened().map(|p| list_claims(&p)).unwrap_or_default();
                to_json(&json!({"generated_at": now_iso(), "claims": claims})).into_bytes()
            };
            send_conditional(request, &tag, body, "application/json", &[])
        }
    }
}

/// `/api/search?q=`: per project, the IDs of the working tasks (not the archive) in which every word of the query
/// appears, case-insensitively, as the dashboard's own search finds them; the project's name counts as part of every
/// task's text. Read-only, like every route; `{}` for an empty query.
fn api_search(projects: &Projects, query: &str) -> Response {
    let q = parse_query(query)
        .into_iter()
        .find(|(n, _)| n == "q")
        .map(|(_, v)| v)
        .unwrap_or_default();
    let results = projects.workspace.search(&q);
    send(
        StatusCode::OK,
        to_json(&json!(results)).into_bytes(),
        "application/json",
        None,
        &[],
    )
}

/// atm-lib's refusals as HTTP statuses: a malformed path, no such file, one too large, one atm doesn't show.
fn file_error(error: &atm_common::Error) -> Response {
    let status = match error {
        atm_common::Error::Invalid(_) => StatusCode::BAD_REQUEST,
        atm_common::Error::NotFound(_) => StatusCode::NOT_FOUND,
        atm_common::Error::TooLarge(_) => StatusCode::PAYLOAD_TOO_LARGE,
        atm_common::Error::Refused(_) => StatusCode::FORBIDDEN,
        atm_common::Error::Failed { .. } => StatusCode::INTERNAL_SERVER_ERROR,
    };
    send_error(status, &error.to_string())
}

/// `GET /api/v1/projects/<p>/documents?path=` (the viewer's JSON) or `…/files?path=` (an image), through atm-lib,
/// which checks the path and reads the file from the working copy. The `ETag` is the file's size and mtime (and, for
/// a document, the current commit, which it names), so the viewer's poll gets a 304 without the file being read again.
fn api_v1_file(
    projects: &Projects,
    id: &str,
    route: &str,
    query: &str,
    request: &HeaderMap,
) -> Response {
    let Some(entry) = projects.find(id) else {
        return send_error(StatusCode::NOT_FOUND, "unknown project");
    };
    let Some(vcs) = Workspace::project(&entry)
        .ok()
        .filter(|_| Workspace::data_stamp(&entry).is_some())
        .and_then(|p| p.vcs())
    else {
        return send_error(StatusCode::NOT_FOUND, "not found");
    };
    let path = parse_query(query)
        .into_iter()
        .find(|(n, _)| n == "path")
        .map(|(_, v)| v)
        .unwrap_or_default();
    let stamp = match atm_lib::documents::document_stamp(Some(&vcs), &path) {
        Ok(stamp) => stamp,
        Err(e) => return file_error(&e),
    };
    let unchanged = |tag: &str| {
        request
            .get(axum::http::header::IF_NONE_MATCH)
            .and_then(|v| v.to_str().ok())
            == Some(tag)
    };
    if route == "documents" {
        let tag = etag(&json!([
            "document",
            [stamp.0, stamp.1.to_string()],
            vcs.head()
        ]));
        if unchanged(&tag) {
            return send(
                StatusCode::NOT_MODIFIED,
                Vec::new(),
                "application/json",
                Some(&tag),
                &[],
            );
        }
        return match atm_lib::documents::read_document(Some(&vcs), &path) {
            Ok(document) => send(
                StatusCode::OK,
                to_json(&document).into_bytes(),
                "application/json",
                Some(&tag),
                &[],
            ),
            Err(e) => file_error(&e),
        };
    }
    let tag = format!("\"{:x}-{:x}\"", stamp.0, stamp.1);
    if unchanged(&tag) {
        return send(
            StatusCode::NOT_MODIFIED,
            Vec::new(),
            "application/json",
            Some(&tag),
            &LOGO_HEADERS,
        );
    }
    match atm_lib::documents::read_image(Some(&vcs), &path) {
        Ok((data, content_type)) => send(
            StatusCode::OK,
            data,
            content_type,
            Some(&tag),
            &LOGO_HEADERS,
        ),
        Err(e) => file_error(&e),
    }
}

/// The routes past the page, its assets and whoami: the read API; everything else is 404.
pub fn serve(projects: &Projects, path: &str, query: &str, request: &HeaderMap) -> Response {
    if path == "/api/projects" {
        return api_projects(projects, request);
    }
    if path == "/api/search" {
        return api_search(projects, query);
    }
    if let Some(rest) = path.strip_prefix("/api/v1/projects/")
        && let Some((id, route @ ("documents" | "files"))) = rest.split_once('/')
        && !id.is_empty()
    {
        return api_v1_file(projects, id, route, query, request);
    }
    if let Some(rest) = path.strip_prefix("/api/v1/projects/")
        && let Some((id, "tasks")) = rest.split_once('/')
        && !id.is_empty()
    {
        return api_v1_tasks(projects, id, query, request);
    }
    if let Some(rest) = path.strip_prefix("/p/")
        && let Some((id, name)) = rest.split_once('/')
        && !id.is_empty()
        && matches!(name, "claims.json" | "worktrees.json" | "logo")
    {
        return project_file(projects, id, name, request);
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
