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

#[cfg(test)]
mod tests {
    use super::super::test_support::{PIXEL, a_state_with_a_project, answer, having, member};
    use super::*;

    #[test]
    fn a_document_is_its_json_and_a_304_while_unchanged() {
        let home = a_state_with_a_project("document");
        let member = member(&home);
        let first = answer(get_document(&member, "path=README.md", &HeaderMap::new()));
        assert_eq!(first.status, StatusCode::OK, "{}", first.json());
        assert_eq!(first.json()["path"], "README.md");
        let tag = first.header("etag").to_string();
        assert_eq!(
            answer(get_document(&member, "path=README.md", &having(&tag))).status,
            StatusCode::NOT_MODIFIED
        );
        // A change to the file is a new version
        std::fs::write(home.join("project/README.md"), "# Routes\n\nTwo needles.\n")
            .expect("README.md changed");
        assert_eq!(
            answer(get_document(&member, "path=README.md", &having(&tag))).status,
            StatusCode::OK
        );
        std::fs::remove_dir_all(&home).expect("cleaned up");
    }

    #[test]
    fn an_image_is_its_bytes_sandboxed() {
        let home = a_state_with_a_project("image");
        let member = member(&home);
        let first = answer(get_file(&member, "path=pixel.png", &HeaderMap::new()));
        assert_eq!(first.status, StatusCode::OK);
        assert_eq!(first.body, PIXEL);
        assert_eq!(first.header("content-type"), "image/png");
        assert!(first.header("content-security-policy").ends_with("sandbox"));
        let tag = first.header("etag").to_string();
        let again = answer(get_file(&member, "path=pixel.png", &having(&tag)));
        assert_eq!(again.status, StatusCode::NOT_MODIFIED);
        assert!(again.header("content-security-policy").ends_with("sandbox"));
        std::fs::remove_dir_all(&home).expect("cleaned up");
    }

    /// atm-docs' checks come first: nothing outside the project, nothing missing, nothing atm doesn't show
    #[test]
    fn a_path_atm_docs_refuses_is_never_read() {
        let home = a_state_with_a_project("refused");
        std::fs::write(home.join("secret.md"), "# Not the project's\n").expect("a file outside");
        let member = member(&home);
        for query in [
            "path=../secret.md",
            "path=%2E%2E%2Fsecret.md",
            "path=",
            "path=missing.md",
        ] {
            for route in [get_document, get_file] {
                let answer = answer(route(&member, query, &HeaderMap::new()));
                assert!(
                    answer.status.is_client_error(),
                    "{query}: {}",
                    answer.status
                );
                assert!(answer.header("etag").is_empty(), "{query}");
            }
        }
        // A document isn't an image, whatever it is asked for as
        let answer = answer(get_file(&member, "path=README.md", &HeaderMap::new()));
        assert!(answer.status.is_client_error(), "{}", answer.status);
        std::fs::remove_dir_all(&home).expect("cleaned up");
    }
}
