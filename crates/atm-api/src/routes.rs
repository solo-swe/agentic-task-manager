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
#[cfg(test)]
mod test_support;
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

#[cfg(test)]
mod tests {
    use super::test_support::{PROJECT, a_state_with_a_project, answer, having};
    use super::*;

    #[test]
    fn an_etag_is_quoted_and_changes_with_what_it_depends_on() {
        let tag = etag(&json!(["claims", 1]));
        assert_eq!(tag.len(), 18);
        assert!(tag.starts_with('"') && tag.ends_with('"'));
        assert_eq!(tag, etag(&json!(["claims", 1])));
        assert_ne!(tag, etag(&json!(["claims", 2])));
    }

    #[test]
    fn a_json_answer_starts_with_when_it_was_made() {
        let body = stamped(json!({"tasks": [], "meta": {}}));
        let keys: Vec<&str> = body
            .as_object()
            .expect("an object")
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(keys, ["generated_at", "tasks", "meta"]);
    }

    #[test]
    fn a_request_is_unchanged_only_with_the_same_tag() {
        assert!(unchanged(&having("\"a\""), "\"a\""));
        assert!(!unchanged(&having("\"b\""), "\"a\""));
        assert!(!unchanged(&HeaderMap::new(), "\"a\""));
        let answer = answer(not_modified("\"a\"", &IMAGE_HEADERS));
        assert_eq!(answer.status, StatusCode::NOT_MODIFIED);
        assert_eq!(answer.header("etag"), "\"a\"");
        assert_eq!(answer.body, b"");
        assert_eq!(answer.header("cache-control"), "no-cache");
    }

    #[test]
    fn atm_libs_refusals_are_http_statuses() {
        use atm_common::Error;
        for (error, status) in [
            (Error::Invalid("x".into()), StatusCode::BAD_REQUEST),
            (Error::NotFound("x".into()), StatusCode::NOT_FOUND),
            (Error::TooLarge("x".into()), StatusCode::PAYLOAD_TOO_LARGE),
            (Error::Refused("x".into()), StatusCode::FORBIDDEN),
            (Error::failed("x"), StatusCode::INTERNAL_SERVER_ERROR),
        ] {
            let answer = answer(error_response(&error));
            assert_eq!(answer.status, status, "{error}");
            assert_eq!(answer.json()["error"], json!(error.to_string()));
        }
    }

    /// Every read reaches its route, in v2 and in v1 (the `default` workspace's); anything else is 404
    #[test]
    fn each_path_reaches_its_read() {
        let home = a_state_with_a_project("dispatch");
        let projects = Projects::new(&home);
        let read =
            |path: &str, query: &str| answer(serve(&projects, path, query, &HeaderMap::new()));
        let v2 = format!("/api/v2/workspaces/work/projects/{PROJECT}");
        let v1 = format!("/api/v1/projects/{PROJECT}");
        let p = format!("/p/{PROJECT}");
        for (path, query, key) in [
            ("/api/v2/workspaces".to_string(), "", "workspaces"),
            ("/api/v2/workspaces/work/projects".into(), "", "projects"),
            ("/api/projects".into(), "", "projects"),
            ("/api/v2/workspaces/work/search".into(), "q=needle", PROJECT),
            ("/api/search".into(), "q=needle", PROJECT),
            (format!("{v2}/tasks"), "", "tasks"),
            (format!("{v1}/tasks"), "", "tasks"),
            (format!("{v2}/documents"), "path=README.md", "path"),
            (format!("{v1}/documents"), "path=README.md", "path"),
            (format!("{v2}/claims"), "", "claims"),
            (format!("{p}/claims.json"), "", "claims"),
            (format!("{v2}/worktrees"), "", "worktrees"),
            (format!("{p}/worktrees.json"), "", "worktrees"),
        ] {
            let answer = read(&path, query);
            assert_eq!(answer.status, StatusCode::OK, "{path}");
            assert!(
                answer.json().get(key).is_some(),
                "{path}: {}",
                answer.json()
            );
        }
        for path in [
            format!("{v2}/files"),
            format!("{v1}/files"),
            format!("{v2}/logo"),
            format!("{p}/logo"),
        ] {
            let answer = read(&path, "path=pixel.png");
            assert_eq!(answer.status, StatusCode::OK, "{path}");
            assert_eq!(answer.header("content-type"), "image/png", "{path}");
        }
        for (path, error) in [
            (
                "/api/v2/workspaces/nope/projects".to_string(),
                "unknown workspace",
            ),
            ("/api/v2/workspaces//projects".into(), "unknown workspace"),
            ("/api/v2/workspaces/work/other".into(), "not found"),
            (format!("{v2}/reviews"), "not found"),
            (
                "/api/v2/workspaces/work/projects//tasks".into(),
                "not found",
            ),
            (format!("{v1}/claims"), "not found"),
            (format!("{p}/tasks.json"), "not found"),
            ("/api/v1/projects//tasks".into(), "not found"),
            ("/elsewhere".into(), "not found"),
        ] {
            let answer = read(&path, "");
            assert_eq!(answer.status, StatusCode::NOT_FOUND, "{path}");
            assert_eq!(answer.json()["error"], json!(error), "{path}");
        }
        assert_eq!(
            read("/api/v2/workspaces/work/projects/nope/tasks", "").status,
            StatusCode::NOT_FOUND
        );
        std::fs::remove_dir_all(&home).expect("cleaned up");
    }

    /// Opening a workspace's list is what a read records: the workspace `/` goes to
    #[test]
    fn reading_a_workspaces_projects_opens_it() {
        let home = a_state_with_a_project("opened");
        let projects = Projects::new(&home);
        assert_eq!(projects.workspaces.recent(), "default");
        let list = "/api/v2/workspaces/work/projects";
        assert_eq!(
            answer(serve(&projects, list, "", &HeaderMap::new())).status,
            StatusCode::OK
        );
        assert_eq!(projects.workspaces.recent(), "work");
        let _ = serve(&projects, "/api/projects", "", &HeaderMap::new());
        assert_eq!(projects.workspaces.recent(), "default");
        std::fs::remove_dir_all(&home).expect("cleaned up");
    }
}
