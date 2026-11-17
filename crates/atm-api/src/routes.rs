//! The REST API (docs/multi-project-api.md; ADR-003 §5), one module per area, as atm-cli's commands are:
//!
//! - [`workspaces`]: the workspaces (`/api/v2/workspaces`), a workspace's projects with their summaries
//!   (`…/<w>/projects`) and search across them (`…/<w>/search`);
//! - [`tasks`]: a project's tasks with what atm derives for them (`…/<w>/projects/<p>/tasks`);
//! - [`claims`]: its claims and worktrees; [`logo`]: its logo;
//! - [`documents`]: its files for the document viewer (`documents?path=`, `files?path=`);
//! - [`reviews`]: its code reviews, the only routes that change anything (POST, PATCH, PUT; ATM-572).
//!
//! The v1 routes (`/api/projects`, `/api/search`, `/api/v1/projects/<p>/…`, `/p/<p>/…`) are the same for the `default`
//! workspace. Each through atm-lib, on a snapshot of the project opened for the request; each read with an `ETag`, so
//! a poll of something unchanged gets a 304. One project's broken files never break the list of all projects
//! (docs/security-review.md F18). This module has what they share, and the dispatch by path.

mod claims;
mod documents;
mod logo;
mod query;
mod reviews;
mod tasks;
mod workspaces;

use std::path::Path;

use atm_common::py::to_json;
use atm_common::sha1::sha1_hex;
use atm_common::time::now_iso;
use atm_lib::workspaces::{Workspace, Workspaces};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use serde_json::{Value, json};

use super::{send, send_error};

pub use reviews::{is_reviews_path, serve_review_change};

/// A project's own image (a logo, an image the document viewer shows), which could be an SVG with a script: shown as
/// an image, never run.
const IMAGE_HEADERS: [(&str, &str); 3] = [
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

/// A read about one project of `workspace`: `tasks`, `documents`, `files`, `claims`, `worktrees` or `logo`.
fn serve_project_read(
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
        "tasks" => tasks::get_tasks(&member, query, request),
        "documents" => documents::get_document(&member, query, request),
        "files" => documents::get_file(&member, query, request),
        "logo" => logo::get_logo(&member, request),
        "worktrees" => claims::get_worktrees(&member, request),
        _ => claims::get_claims(&member, request),
    }
}

/// The v2 reads, past `/api/v2/workspaces/`: `<w>/projects`, `<w>/search`, `<w>/projects/<p>/<route>`.
fn serve_workspace_read(
    projects: &Projects,
    rest: &str,
    query: &str,
    request: &HeaderMap,
) -> Response {
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
            workspaces::get_projects(&workspace, request)
        }
        ["search"] => workspaces::get_search(&workspace, query),
        ["projects", id, route]
            if !id.is_empty()
                && matches!(
                    route,
                    "tasks" | "documents" | "files" | "claims" | "worktrees" | "logo"
                ) =>
        {
            serve_project_read(&workspace, id, route, query, request)
        }
        _ => send_error(StatusCode::NOT_FOUND, "not found"),
    }
}

/// The reads past the page, its assets and whoami; everything else is 404.
pub fn serve(projects: &Projects, path: &str, query: &str, request: &HeaderMap) -> Response {
    if path == "/api/v2/workspaces" {
        return workspaces::get_workspaces(projects);
    }
    if let Some(rest) = path.strip_prefix("/api/v2/workspaces/") {
        return serve_workspace_read(projects, rest, query, request);
    }
    // The v1 routes: the default workspace's
    let default = projects.workspaces.default_workspace();
    if path == "/api/projects" {
        // The dashboard reads `default`'s list here: opening it, as `…/default/projects` would
        let _ = projects.workspaces.opened(default.id());
        return workspaces::get_projects(&default, request);
    }
    if path == "/api/search" {
        return workspaces::get_search(&default, query);
    }
    if let Some(rest) = path.strip_prefix("/api/v1/projects/")
        && let Some((id, route @ ("documents" | "files" | "tasks"))) = rest.split_once('/')
        && !id.is_empty()
    {
        return serve_project_read(&default, id, route, query, request);
    }
    if let Some(rest) = path.strip_prefix("/p/")
        && let Some((id, name)) = rest.split_once('/')
        && !id.is_empty()
        && matches!(name, "claims.json" | "worktrees.json" | "logo")
    {
        return serve_project_read(&default, id, name.trim_end_matches(".json"), query, request);
    }
    send_error(StatusCode::NOT_FOUND, "not found")
}
