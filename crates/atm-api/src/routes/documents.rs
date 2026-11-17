//! A project's files for the document viewer (docs/multi-project-api.md §2.6): through atm-lib (atm-docs), which
//! checks the path and reads the file from the working copy.

use atm_lib::workspaces::Member;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use serde_json::json;

use super::query::parameter;
use super::{IMAGE_HEADERS, error_response, etag, not_modified, ok_json, unchanged};
use crate::{send, send_error};

/// The file the query's `path` names, after atm-docs' checks.
struct Checked {
    path: String,
    /// Its `(size, mtime)`
    stamp: (u64, u128),
    /// The working copy's current commit
    head: Option<String>,
}

/// The file the query names, checked; or the refusal to answer.
fn checked(member: &Member, query: &str) -> Result<Checked, Box<Response>> {
    let Some(vcs) = member.project().ok().and_then(|p| p.vcs()) else {
        return Err(Box::new(send_error(StatusCode::NOT_FOUND, "not found")));
    };
    let path = parameter(query, "path");
    match atm_lib::documents::document_stamp(Some(&vcs), &path) {
        Ok(stamp) => Ok(Checked {
            path,
            stamp,
            head: vcs.head(),
        }),
        Err(e) => Err(Box::new(error_response(&e))),
    }
}

/// `GET …/projects/<p>/documents?path=`: a file as the viewer shows it (its JSON). The `ETag` is the file's size and
/// mtime and the current commit (which it names), so the viewer's poll gets a 304 without the file being read again.
pub(super) fn get_document(member: &Member, query: &str, request: &HeaderMap) -> Response {
    let found = match checked(member, query) {
        Ok(found) => found,
        Err(refused) => return *refused,
    };
    let tag = etag(&json!([
        "document",
        [found.stamp.0, found.stamp.1.to_string()],
        found.head
    ]));
    if unchanged(request, &tag) {
        return not_modified(&tag, &[]);
    }
    match member.document(&found.path) {
        Ok(document) => ok_json(&document, Some(&tag)),
        Err(e) => error_response(&e),
    }
}

/// `GET …/projects/<p>/files?path=`: an image's bytes (whose bytes match its type), sandboxed. The `ETag` is the
/// file's size and mtime.
pub(super) fn get_file(member: &Member, query: &str, request: &HeaderMap) -> Response {
    let found = match checked(member, query) {
        Ok(found) => found,
        Err(refused) => return *refused,
    };
    let tag = format!("\"{:x}-{:x}\"", found.stamp.0, found.stamp.1);
    if unchanged(request, &tag) {
        return not_modified(&tag, &IMAGE_HEADERS);
    }
    match member.image(&found.path) {
        Ok((data, content_type)) => send(
            StatusCode::OK,
            data,
            content_type,
            Some(&tag),
            &IMAGE_HEADERS,
        ),
        Err(e) => error_response(&e),
    }
}
