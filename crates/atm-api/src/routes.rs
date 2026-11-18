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
mod source;
mod tasks;
#[cfg(test)]
mod test_support;
mod workspaces;

use std::path::Path;

use atm_common::py::to_json;
use atm_common::sha1::sha1_hex;
use atm_common::time::now_iso;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use serde_json::{Value, json};

use super::{send, send_error};

pub use reviews::{is_reviews_path, serve_review_change};
use source::{AtmLib, Source, WorkspaceSource};

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
    source: Box<dyn Source>,
}

impl Projects {
    /// The workspaces of the state folder `home`.
    #[must_use]
    pub fn new(home: &Path) -> Self {
        Self {
            source: Box::new(AtmLib::at(home)),
        }
    }

    /// A unit test's: the routes ask `source` instead of atm-lib.
    #[cfg(test)]
    fn of(source: impl Source + 'static) -> Self {
        Self {
            source: Box::new(source),
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

/// A read about one project.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProjectRead<'a> {
    Tasks,
    Documents,
    Files,
    Claims,
    Worktrees,
    Logo,
    Reviews,
    Review(&'a str),
}

impl<'a> ProjectRead<'a> {
    /// The read a project's route names past `projects/<p>/` (v2's, which v1's `api/v1/projects/<p>/` and `p/<p>/`
    /// have a part of each).
    fn named(rest: &[&'a str]) -> Option<Self> {
        Some(match rest {
            ["tasks"] => Self::Tasks,
            ["documents"] => Self::Documents,
            ["files"] => Self::Files,
            ["claims"] => Self::Claims,
            ["worktrees"] => Self::Worktrees,
            ["logo"] => Self::Logo,
            ["reviews"] => Self::Reviews,
            ["reviews", review] if !review.is_empty() => Self::Review(review),
            _ => return None,
        })
    }
}

/// A read about project `id` of `workspace`.
fn serve_project_read(
    workspace: &dyn WorkspaceSource,
    id: &str,
    read: ProjectRead<'_>,
    query: &str,
    request: &HeaderMap,
) -> Response {
    let project = match workspace.project(id) {
        Ok(project) => project,
        Err(e) => return error_response(&e),
    };
    let project = project.as_ref();
    match read {
        ProjectRead::Tasks => tasks::get_tasks(project, query, request),
        ProjectRead::Documents => documents::get_document(project, query, request),
        ProjectRead::Files => documents::get_file(project, query, request),
        ProjectRead::Logo => logo::get_logo(project, request),
        ProjectRead::Worktrees => claims::get_worktrees(project, request),
        ProjectRead::Claims => claims::get_claims(project, request),
        ProjectRead::Reviews => reviews::get_reviews(project, query),
        ProjectRead::Review(review) => reviews::get_review(project, review, query, request),
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
        .and_then(|w| projects.source.workspace(w))
    else {
        return send_error(StatusCode::NOT_FOUND, "unknown workspace");
    };
    let workspace = workspace.as_ref();
    match parts[1..] {
        ["projects"] => {
            // Opening a workspace's list is opening the workspace: the last opened is the one `/` goes to. A choice
            // of view, the one thing a read records (ADR-003 §5); written only when it changes
            projects.source.opened(&workspace.id());
            workspaces::get_projects(workspace, request)
        }
        ["search"] => workspaces::get_search(workspace, query),
        ["projects", id, ref rest @ ..] if !id.is_empty() => match ProjectRead::named(rest) {
            Some(read) => serve_project_read(workspace, id, read, query, request),
            None => send_error(StatusCode::NOT_FOUND, "not found"),
        },
        _ => send_error(StatusCode::NOT_FOUND, "not found"),
    }
}

/// The reads past the page, its assets and whoami; everything else is 404.
#[must_use]
pub fn serve(projects: &Projects, path: &str, query: &str, request: &HeaderMap) -> Response {
    if path == "/api/v2/workspaces" {
        return workspaces::get_workspaces(projects);
    }
    if let Some(rest) = path.strip_prefix("/api/v2/workspaces/") {
        return serve_workspace_read(projects, rest, query, request);
    }
    // The v1 routes: the default workspace's
    let default = projects.source.default_workspace();
    let default = default.as_ref();
    if path == "/api/projects" {
        // The dashboard reads `default`'s list here: opening it, as `…/default/projects` would
        projects.source.opened(&default.id());
        return workspaces::get_projects(default, request);
    }
    if path == "/api/search" {
        return workspaces::get_search(default, query);
    }
    // `api/v1/projects/<p>/…`: its tasks, files and reviews; `p/<p>/…`: its claims, worktrees and logo
    if let Some(rest) = path.strip_prefix("/api/v1/projects/")
        && let Some((id, route)) = rest.split_once('/')
        && !id.is_empty()
        && let Some(
            read @ (ProjectRead::Tasks
            | ProjectRead::Documents
            | ProjectRead::Files
            | ProjectRead::Reviews
            | ProjectRead::Review(_)),
        ) = ProjectRead::named(&route.split('/').collect::<Vec<_>>())
    {
        return serve_project_read(default, id, read, query, request);
    }
    if let Some(rest) = path.strip_prefix("/p/")
        && let Some((id, name)) = rest.split_once('/')
        && !id.is_empty()
        && let Some(read) = match name {
            "claims.json" => Some(ProjectRead::Claims),
            "worktrees.json" => Some(ProjectRead::Worktrees),
            "logo" => Some(ProjectRead::Logo),
            _ => None,
        }
    {
        return serve_project_read(default, id, read, query, request);
    }
    send_error(StatusCode::NOT_FOUND, "not found")
}

#[cfg(test)]
mod tests {
    use atm_lib::query::Filters;

    use super::test_support::{Call, Fake, FakeProject, answer, having, plain, refusals};
    use super::*;

    #[test]
    fn an_etag_is_a_quoted_hash_that_changes_with_what_it_depends_on() {
        let tag = etag(&json!(["claims", 1]));
        assert_eq!(tag.len(), 18);
        assert!(tag.starts_with('"') && tag.ends_with('"'));
        assert!(tag[1..17].bytes().all(|b| b.is_ascii_hexdigit()));
        assert_eq!(tag, etag(&json!(["claims", 1])), "the same for the same");
        for other in [
            json!(["claims", 2]),
            json!(["claims"]),
            json!(["tasks", 1]),
            Value::Null,
        ] {
            assert_ne!(tag, etag(&other), "{other}");
        }
    }

    #[test]
    fn a_json_answer_starts_with_when_it_was_made_then_the_body_in_its_order() {
        let body = stamped(json!({"tasks": [], "meta": {}, "a": 1}));
        let keys: Vec<&str> = body
            .as_object()
            .expect("an object")
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(keys, ["generated_at", "tasks", "meta", "a"]);
        assert!(
            body["generated_at"]
                .as_str()
                .is_some_and(|t| t.ends_with('Z'))
        );
        // A body that isn't an object has nothing to add
        for other in [json!([1]), json!("x"), Value::Null] {
            let only = stamped(other.clone());
            assert_eq!(
                only.as_object().map(serde_json::Map::len),
                Some(1),
                "{other}"
            );
        }
    }

    #[test]
    fn a_request_is_unchanged_only_with_the_same_strong_tag() {
        assert!(unchanged(&having("\"a\""), "\"a\""));
        for (has, why) in [
            (having("\"b\""), "another version"),
            (having("W/\"a\""), "a weak tag"),
            (having("\"a\", \"b\""), "a list of tags"),
            (having("a"), "unquoted"),
            (plain(), "none"),
        ] {
            assert!(!unchanged(&has, "\"a\""), "{why}");
        }
    }

    #[test]
    fn a_304_has_its_tag_and_the_headers_asked_for_and_no_body() {
        for headers in [&[][..], &IMAGE_HEADERS[..]] {
            let answer = answer(not_modified("\"a\"", headers));
            assert_eq!(answer.status, StatusCode::NOT_MODIFIED);
            assert_eq!(answer.tag(), "\"a\"");
            assert_eq!(answer.body, b"");
            assert_eq!(answer.header("content-type"), "");
            let cache = if headers.is_empty() {
                "no-store"
            } else {
                "no-cache"
            };
            assert_eq!(answer.header("cache-control"), cache);
        }
    }

    #[test]
    fn a_json_answer_is_200_json_with_its_tag_if_any() {
        let tagged = answer(ok_json(&json!({"a": 1}), Some("\"t\"")));
        assert_eq!(tagged.status, StatusCode::OK);
        assert_eq!(tagged.header("content-type"), "application/json");
        assert_eq!(tagged.tag(), "\"t\"");
        assert_eq!(tagged.json(), json!({"a": 1}));
        let untagged = answer(ok_json(&json!({"a": 1}), None));
        assert_eq!(untagged.tag(), "");
        assert_eq!(untagged.header("cache-control"), "no-store");
    }

    #[test]
    fn each_refusal_of_atm_lib_is_its_http_status_with_its_message() {
        for (error, status) in refusals() {
            let answer = answer(error_response(&error));
            assert_eq!(answer.status, status, "{error}");
            assert_eq!(answer.json(), json!({"error": error.to_string()}));
        }
    }

    /// A fake whose project `p1` has a logo, so every read gets as far as its own call
    fn fake() -> Fake {
        Fake::with(FakeProject {
            logo: Some((json!([1, "1"]), "image/png", b"png".to_vec())),
            ..FakeProject::default()
        })
    }

    fn served(fake: Fake, path: &str, query: &str) -> (StatusCode, Value, Vec<Call>) {
        let (projects, calls) = fake.projects();
        let answer = answer(serve(&projects, path, query, &plain()));
        (answer.status, answer.json(), calls.taken())
    }

    /// Every read reaches its route, the same in v2 (any workspace) and v1 (`default`'s), with the query
    #[test]
    #[allow(clippy::too_many_lines)] // reason: one table, a row per read
    fn each_path_reaches_its_read_in_its_workspace() {
        let project = |w: &str| Call::Project(w.into(), "p1".into());
        let tasks = |w: &str| {
            vec![
                project(w),
                Call::OthersVersion,
                Call::Tasks(Filters {
                    status: vec!["todo".into()],
                    ..Filters::default()
                }),
            ]
        };
        let cases: Vec<(&str, &str, Vec<Call>)> = vec![
            ("/api/v2/workspaces", "", vec![]),
            (
                "/api/v2/workspaces/work/projects",
                "",
                vec![
                    Call::Opened("work".into()),
                    Call::ListedProjects("work".into()),
                ],
            ),
            (
                "/api/projects",
                "",
                vec![
                    Call::Opened("default".into()),
                    Call::ListedProjects("default".into()),
                ],
            ),
            (
                "/api/v2/workspaces/work/search",
                "q=x",
                vec![Call::Searched("work".into(), "x".into())],
            ),
            (
                "/api/search",
                "q=x",
                vec![Call::Searched("default".into(), "x".into())],
            ),
            (
                "/api/v2/workspaces/work/projects/p1/tasks",
                "status=todo",
                tasks("work"),
            ),
            ("/api/v1/projects/p1/tasks", "status=todo", tasks("default")),
            (
                "/api/v2/workspaces/work/projects/p1/documents",
                "path=a.md",
                vec![project("work"), Call::FileVersion("a.md".into())],
            ),
            (
                "/api/v1/projects/p1/documents",
                "path=a.md",
                vec![project("default"), Call::FileVersion("a.md".into())],
            ),
            (
                "/api/v2/workspaces/work/projects/p1/files",
                "path=a.png",
                vec![project("work"), Call::FileVersion("a.png".into())],
            ),
            (
                "/api/v1/projects/p1/files",
                "path=a.png",
                vec![project("default"), Call::FileVersion("a.png".into())],
            ),
            (
                "/api/v2/workspaces/work/projects/p1/claims",
                "",
                vec![project("work"), Call::Claims],
            ),
            (
                "/p/p1/claims.json",
                "",
                vec![project("default"), Call::Claims],
            ),
            (
                "/api/v2/workspaces/work/projects/p1/worktrees",
                "",
                vec![project("work"), Call::Worktrees],
            ),
            (
                "/p/p1/worktrees.json",
                "",
                vec![project("default"), Call::Worktrees],
            ),
            (
                "/api/v2/workspaces/work/projects/p1/logo",
                "",
                vec![project("work"), Call::ReadLogo],
            ),
            ("/p/p1/logo", "", vec![project("default"), Call::ReadLogo]),
            (
                "/api/v2/workspaces/work/projects/p1/documents",
                "path=a.md&at=t1",
                vec![
                    project("work"),
                    Call::DocumentAt("a.md".into(), "t1".into()),
                ],
            ),
            (
                "/api/v1/projects/p1/documents",
                "path=a.md&at=t1",
                vec![
                    project("default"),
                    Call::DocumentAt("a.md".into(), "t1".into()),
                ],
            ),
            (
                "/api/v2/workspaces/work/projects/p1/reviews",
                "q=x",
                vec![
                    project("work"),
                    Call::Reviews(atm_lib::review_pages::ReviewQuery {
                        q: "x".into(),
                        ..atm_lib::review_pages::ReviewQuery::default()
                    }),
                ],
            ),
            (
                "/api/v1/projects/p1/reviews",
                "",
                vec![
                    project("default"),
                    Call::Reviews(atm_lib::review_pages::ReviewQuery::default()),
                ],
            ),
            (
                "/api/v2/workspaces/work/projects/p1/reviews/CR-7",
                "round=2&against=1",
                vec![
                    project("work"),
                    Call::Review(atm_lib::reviews::ReviewRef("CR-7".into()), Some(2), Some(1)),
                ],
            ),
            (
                "/api/v1/projects/p1/reviews/CR-7",
                "",
                vec![
                    project("default"),
                    Call::Review(atm_lib::reviews::ReviewRef("CR-7".into()), None, None),
                ],
            ),
        ];
        for (path, query, expected) in cases {
            let (_, _, calls) = served(fake(), path, query);
            assert_eq!(calls, expected, "{path}?{query}");
        }
    }

    /// Anything that isn't a read is 404, before any project is looked at
    #[test]
    fn a_path_no_read_has_is_not_found_and_asks_nothing() {
        for (path, error) in [
            ("/api/v2/workspaces/", "unknown workspace"),
            ("/api/v2/workspaces/nope/projects", "unknown workspace"),
            ("/api/v2/workspaces//projects", "unknown workspace"),
            ("/api/v2/workspaces/work", "not found"),
            ("/api/v2/workspaces/work/", "not found"),
            ("/api/v2/workspaces/work/other", "not found"),
            ("/api/v2/workspaces/work/projects/", "not found"),
            ("/api/v2/workspaces/work/projects/p1", "not found"),
            ("/api/v2/workspaces/work/projects/p1/reviews/", "not found"),
            (
                "/api/v2/workspaces/work/projects/p1/reviews/CR-1/comments",
                "not found",
            ),
            ("/api/v2/workspaces/work/projects/p1/tasks/T-1", "not found"),
            ("/api/v1/projects/p1/reviews/", "not found"),
            ("/api/v1/projects/p1/reviews/CR-1/x", "not found"),
            ("/p/p1/reviews", "not found"),
            ("/api/v2/workspaces/work/projects//tasks", "not found"),
            ("/api/v1/projects/p1/claims", "not found"),
            ("/api/v1/projects/p1/logo", "not found"),
            ("/api/v1/projects//tasks", "not found"),
            ("/api/v1/projects/p1", "not found"),
            ("/p/p1/tasks.json", "not found"),
            ("/p/p1/claims", "not found"),
            ("/p//claims.json", "not found"),
            ("/api/v3/workspaces", "not found"),
            ("/elsewhere", "not found"),
            ("", "not found"),
        ] {
            let (status, body, calls) = served(fake(), path, "");
            assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
            assert_eq!(body, json!({"error": error}), "{path}");
            assert_eq!(calls, [], "{path}");
        }
    }

    /// A project the workspace doesn't list, or whose data atm-lib won't open: its refusal, nothing read
    #[test]
    fn an_unknown_or_unreadable_project_is_atm_libs_refusal() {
        let (status, body, calls) =
            served(fake(), "/api/v2/workspaces/work/projects/nope/tasks", "");
        assert_eq!(
            (status, body),
            (StatusCode::NOT_FOUND, json!({"error": "unknown project"}))
        );
        assert_eq!(calls, [Call::Project("work".into(), "nope".into())]);
        for (error, status) in refusals() {
            let mut broken = fake();
            broken.workspaces[1].1.members = vec![("p1".into(), Err(error.clone()))];
            let (got, body, calls) =
                served(broken, "/api/v2/workspaces/work/projects/p1/claims", "");
            assert_eq!(got, status, "{error}");
            assert_eq!(body, json!({"error": error.to_string()}));
            assert_eq!(
                calls,
                [Call::Project("work".into(), "p1".into())],
                "{error}"
            );
        }
    }

    /// A project in one workspace isn't found through another
    #[test]
    fn a_read_looks_in_its_own_workspace_only() {
        let mut only_work = fake();
        only_work.workspaces[0].1.members.clear();
        let (status, _, _) = served(only_work.clone(), "/api/v1/projects/p1/tasks", "");
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (status, _, _) = served(only_work, "/api/v2/workspaces/work/projects/p1/tasks", "");
        assert_eq!(status, StatusCode::OK);
    }

    /// Search and the workspaces' summary are reads that open nothing
    #[test]
    fn only_listing_a_workspaces_projects_opens_it() {
        for (path, query) in [
            ("/api/v2/workspaces", ""),
            ("/api/v2/workspaces/work/search", "q=x"),
            ("/api/search", "q=x"),
            ("/api/v2/workspaces/work/projects/p1/claims", ""),
        ] {
            let (_, _, calls) = served(fake(), path, query);
            assert!(
                !calls.iter().any(|c| matches!(c, Call::Opened(_))),
                "{path}: {calls:?}"
            );
        }
    }
}
