//! A project's files for the document viewer (docs/multi-project-api.md §2.6): through atm-lib (atm-docs), which
//! checks the path and reads the file from the working copy.

use atm_common::py::to_json;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use serde_json::json;

use super::query::parameter;
use super::source::ProjectSource;
use super::{IMAGE_HEADERS, error_response, etag, not_modified, ok_json, unchanged};
use crate::send;

/// `GET …/projects/<p>/documents?path=`: a file as the viewer shows it (its JSON). The `ETag` is the file's size and
/// mtime and the current commit (which it names), so the viewer's poll gets a 304 without the file being read again.
/// With `&at=<revision>`, the file as that commit or tree has it (a review round's side). Named by its full object
/// ID, it never changes, so the browser keeps it (ATM-587: a round's sides aren't asked for again when a far file is
/// made again, or the review is opened again): tagged by the revision and the path, a 304 without reading anything.
/// Any other revision (`HEAD`, a branch, a short ID) can move: neither tagged nor kept.
pub(super) fn get_document(
    project: &dyn ProjectSource,
    query: &str,
    request: &HeaderMap,
) -> Response {
    let path = parameter(query, "path");
    let at = parameter(query, "at");
    if !at.is_empty() {
        // A full object ID only, as a round records its sides (ATM-671, audit H6): never a name or an expression git
        // would work out (`:/<regex>` searches every commit's message)
        if !is_object_id(&at) {
            return error_response(&atm_common::Error::Invalid(format!(
                "at={at}: a revision is a full object ID"
            )));
        }
        let tag = etag(&json!(["document-at", at, path]));
        if unchanged(request, &tag) {
            return not_modified(&tag, &[KEPT]);
        }
        return match project.document_at(&path, &at) {
            Ok(document) => send(
                StatusCode::OK,
                to_json(&document).into_bytes(),
                "application/json",
                Some(&tag),
                &[KEPT],
            ),
            Err(e) => error_response(&e),
        };
    }
    let (size, modified) = match project.file_version(&path) {
        Ok(version) => version,
        Err(e) => return error_response(&e),
    };
    let tag = etag(&json!([
        "document",
        [size, modified.to_string()],
        project.head()
    ]));
    if unchanged(request, &tag) {
        return not_modified(&tag, &[]);
    }
    match project.document(&path) {
        Ok(document) => ok_json(&document, Some(&tag)),
        Err(e) => error_response(&e),
    }
}

/// What a file at a full object ID is sent with: this browser only, for a year, never asked again
const KEPT: (&str, &str) = ("Cache-Control", "private, max-age=31536000, immutable");

/// The one rule for a revision a page names (ATM-671): atm-lib's, which the app's commands keep too
fn is_object_id(revision: &str) -> bool {
    atm_lib::workspaces::is_object_id(revision)
}

/// `GET …/projects/<p>/files?path=`: an image's bytes (whose bytes match its type), sandboxed. The `ETag` is the
/// file's size and mtime. With `&at=<revision>`, the image as that commit or tree has it (a review's image file's
/// sides, ATM-689), kept by the browser as `documents?at=` is when named by a full object ID.
pub(super) fn get_file(project: &dyn ProjectSource, query: &str, request: &HeaderMap) -> Response {
    let path = parameter(query, "path");
    let at = parameter(query, "at");
    if !at.is_empty() {
        // A full object ID only, as for a document (ATM-671)
        if !is_object_id(&at) {
            return error_response(&atm_common::Error::Invalid(format!(
                "at={at}: a revision is a full object ID"
            )));
        }
        let kept = true;
        let tag = etag(&json!(["image-at", at, path]));
        if kept && unchanged(request, &tag) {
            return not_modified(&tag, &[IMAGE_HEADERS[0], IMAGE_HEADERS[1], KEPT]);
        }
        return match project.image_at(&path, &at) {
            Ok((data, content_type)) if kept => send(
                StatusCode::OK,
                data,
                content_type,
                Some(&tag),
                &[IMAGE_HEADERS[0], IMAGE_HEADERS[1], KEPT],
            ),
            Ok((data, content_type)) => {
                send(StatusCode::OK, data, content_type, None, &IMAGE_HEADERS)
            }
            Err(e) => error_response(&e),
        };
    }
    let (size, modified) = match project.file_version(&path) {
        Ok(version) => version,
        Err(e) => return error_response(&e),
    };
    let tag = format!("\"{size:x}-{modified:x}\"");
    if unchanged(request, &tag) {
        return not_modified(&tag, &IMAGE_HEADERS);
    }
    match project.image(&path) {
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

/// Which paths atm-docs refuses (outside the project, hidden, too large, …) is atm-docs' and its tests': here, that
/// the routes ask it first, answer each refusal with its status, and tag and cache what it gives them.
#[cfg(test)]
mod tests {
    use atm_common::Error;

    use super::super::test_support::{
        Answer, Call, FakeFile, FakeProject, answer, bound, having, plain, refusals,
    };
    use super::*;

    fn file(version: (u64, u128)) -> FakeFile {
        FakeFile {
            version,
            document: Ok(json!({"path": "docs/a b.md", "kind": "markdown"})),
            image: Ok((b"\x89PNG".to_vec(), "image/png")),
        }
    }

    fn fake(path: &str, found: atm_common::Result<FakeFile>, head: Option<&str>) -> FakeProject {
        FakeProject {
            files: vec![(path.into(), found)],
            head: head.map(String::from),
            ..FakeProject::default()
        }
    }

    const QUERY: &str = "path=docs%2Fa+b.md";
    const PATH: &str = "docs/a b.md";

    #[test]
    fn a_document_is_atm_libs_json_tagged_with_its_version_and_the_commit() {
        let (project, calls) = bound(fake(PATH, Ok(file((10, 20))), Some("aaaa")));
        let answer = answer(get_document(project.as_ref(), QUERY, &plain()));
        assert_eq!(answer.status, StatusCode::OK);
        assert_eq!(
            answer.json(),
            json!({"path": "docs/a b.md", "kind": "markdown"})
        );
        assert_eq!(answer.header("content-type"), "application/json");
        assert_ne!(answer.tag(), "");
        // The path as the query has it, decoded; checked before it is read
        assert_eq!(
            calls.taken(),
            [
                Call::FileVersion(PATH.into()),
                Call::Head,
                Call::Document(PATH.into())
            ]
        );
    }

    #[test]
    fn a_document_is_a_304_without_being_read_until_the_file_or_the_commit_changes() {
        let (project, calls) = bound(fake(PATH, Ok(file((10, 20))), Some("aaaa")));
        let tag = answer(get_document(project.as_ref(), QUERY, &plain())).tag();
        calls.taken();
        let again = answer(get_document(project.as_ref(), QUERY, &having(&tag)));
        assert_eq!(again.status, StatusCode::NOT_MODIFIED);
        assert_eq!(again.body, b"");
        assert_eq!(calls.taken(), [Call::FileVersion(PATH.into()), Call::Head]);
        for (version, head) in [
            ((11, 20), Some("aaaa")),
            ((10, 21), Some("aaaa")),
            ((10, 20), Some("bbbb")),
            ((10, 20), None),
        ] {
            let (other, _) = bound(fake(PATH, Ok(file(version)), head));
            assert_eq!(
                answer(get_document(other.as_ref(), QUERY, &having(&tag))).status,
                StatusCode::OK,
                "{version:?} {head:?}"
            );
        }
    }

    #[test]
    fn an_image_is_its_bytes_sandboxed_tagged_with_its_version_alone() {
        let (project, calls) = bound(fake("pixel.png", Ok(file((10, 255))), Some("aaaa")));
        let answer = answer(get_file(project.as_ref(), "path=pixel.png", &plain()));
        assert_eq!(answer.status, StatusCode::OK);
        assert_eq!(answer.body, b"\x89PNG");
        assert_eq!(answer.header("content-type"), "image/png");
        assert!(
            answer
                .header("content-security-policy")
                .ends_with("sandbox")
        );
        assert_eq!(answer.header("cache-control"), "no-cache");
        assert_eq!(answer.tag(), "\"a-ff\"");
        // An image doesn't depend on the commit
        assert_eq!(
            calls.taken(),
            [
                Call::FileVersion("pixel.png".into()),
                Call::Image("pixel.png".into())
            ]
        );
        let again = answer_of(project.as_ref(), "\"a-ff\"");
        assert_eq!(again.status, StatusCode::NOT_MODIFIED);
        assert!(again.header("content-security-policy").ends_with("sandbox"));
        assert_eq!(calls.taken(), [Call::FileVersion("pixel.png".into())]);
        assert_eq!(
            answer_of(project.as_ref(), "\"a-fe\"").status,
            StatusCode::OK
        );
    }

    /// ATM-689: an image at a revision (a review's image file's side): at a full object ID, sandboxed, tagged and kept
    /// (a 304 without reading); at a name that can move, sandboxed, neither tagged nor kept; a refusal its status
    #[test]
    fn an_image_at_a_revision_is_its_bytes_kept_only_at_a_full_object_id() {
        let id = "ab".repeat(20);
        let (project, calls) = bound(FakeProject::default());
        let query = format!("path=docs%2Fpic.gif&at={id}");
        let first = answer(get_file(project.as_ref(), &query, &plain()));
        assert_eq!(first.status, StatusCode::OK);
        assert_eq!(first.body, b"GIF89a");
        assert_eq!(first.header("content-type"), "image/gif");
        assert!(first.header("content-security-policy").ends_with("sandbox"));
        assert_eq!(first.header("x-content-type-options"), "nosniff");
        assert_eq!(first.header("cache-control"), KEPT.1);
        assert_eq!(
            calls.taken(),
            [Call::ImageAt("docs/pic.gif".into(), id.clone())]
        );
        let again = answer(get_file(project.as_ref(), &query, &having(&first.tag())));
        assert_eq!(again.status, StatusCode::NOT_MODIFIED);
        assert!(again.header("content-security-policy").ends_with("sandbox"));
        assert_eq!(calls.taken(), []);
        // Anything but a full object ID (a name that moves): refused, unasked (ATM-671)
        let moving = answer(get_file(
            project.as_ref(),
            "path=pic.gif&at=HEAD",
            &having(&first.tag()),
        ));
        assert_eq!(moving.status, StatusCode::BAD_REQUEST);
        assert_eq!(moving.tag(), "");
        assert_eq!(calls.taken(), []);
        // What atm-docs refuses: its status
        for (error, status) in refusals() {
            let (project, _) = bound(FakeProject {
                image_at: Err(error.clone()),
                ..FakeProject::default()
            });
            let refused = answer(get_file(project.as_ref(), &query, &plain()));
            assert_eq!(refused.status, status, "{error}");
            assert_eq!(refused.tag(), "");
        }
    }

    fn answer_of(project: &dyn ProjectSource, tag: &str) -> Answer {
        answer(get_file(project, "path=pixel.png", &having(tag)))
    }

    /// What atm-docs refuses, for either route: its status, no tag, nothing read
    #[test]
    fn a_path_atm_docs_refuses_is_its_status_and_never_read() {
        for (error, status) in refusals() {
            let (project, calls) = bound(fake(PATH, Err(error.clone()), Some("aaaa")));
            let document = answer(get_document(project.as_ref(), QUERY, &plain()));
            let image = answer(get_file(project.as_ref(), QUERY, &plain()));
            for answer in [&document, &image] {
                assert_eq!(answer.status, status, "{error}");
                assert_eq!(answer.json(), json!({"error": error.to_string()}));
                assert_eq!(answer.tag(), "");
            }
            assert_eq!(
                calls.taken(),
                [
                    Call::FileVersion(PATH.into()),
                    Call::FileVersion(PATH.into())
                ]
            );
        }
    }

    /// No path, or one the project doesn't have: atm-docs is asked, and says no
    #[test]
    fn no_path_or_an_unknown_one_is_atm_docs_to_refuse() {
        for (query, path) in [
            ("", ""),
            ("path=", ""),
            ("other=x", ""),
            ("path=gone.md", "gone.md"),
        ] {
            let (project, calls) = bound(fake(PATH, Ok(file((1, 1))), None));
            let answer = answer(get_document(project.as_ref(), query, &plain()));
            assert_eq!(answer.status, StatusCode::NOT_FOUND, "{query}");
            assert_eq!(calls.taken(), [Call::FileVersion(path.into())], "{query}");
        }
    }

    /// The file checked, then refused when it is read (it isn't an image, it went in between): its status
    #[test]
    fn a_file_refused_when_read_is_its_status() {
        let refused = FakeFile {
            document: Err(Error::TooLarge("too large".into())),
            image: Err(Error::Invalid("not an image".into())),
            ..file((1, 1))
        };
        let (project, _) = bound(fake(PATH, Ok(refused), None));
        let document = answer(get_document(project.as_ref(), QUERY, &plain()));
        assert_eq!(document.status, StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(document.tag(), "");
        let image = answer(get_file(project.as_ref(), QUERY, &plain()));
        assert_eq!(image.status, StatusCode::BAD_REQUEST);
        assert_eq!(image.json(), json!({"error": "not an image"}));
    }

    /// A review round's side is named by a full object ID (ATM-671, audit H6): anything else (a short ID, a name that
    /// moves, an expression git would work out, an option) is refused before atm-lib is asked
    #[test]
    fn a_document_at_anything_but_a_full_object_id_is_refused_unasked() {
        for query in [
            "path=docs%2Fa+b.md&at=3fa1",
            "at=HEAD&path=a.md",
            "path=&at=t1",
            "path=a.md&at=--output%3Dx",
            "path=a.md&at=%3A%2Ffix",
        ] {
            let (project, calls) = bound(fake(PATH, Ok(file((1, 1))), Some("aaaa")));
            let answer = answer(get_document(project.as_ref(), query, &having("\"any\"")));
            assert_eq!(answer.status, StatusCode::BAD_REQUEST, "{query}");
            assert_eq!(answer.tag(), "", "{query}");
            assert_eq!(calls.taken(), [], "{query}");
        }
        // No revision: the working copy's
        let (project, calls) = bound(fake(PATH, Ok(file((1, 1))), None));
        let _ = get_document(project.as_ref(), "path=docs%2Fa+b.md&at=", &plain());
        assert_eq!(calls.taken()[0], Call::FileVersion(PATH.into()));
    }

    const SHA1: &str = "3fa1c0de3fa1c0de3fa1c0de3fa1c0de3fa1c0de";

    /// A side named by a full object ID never changes: tagged by the revision and the path, kept by the browser, a 304
    /// without reading it again (ATM-587)
    #[test]
    fn a_document_at_a_full_object_id_is_kept_and_a_304_once_seen() {
        let sha256 = "ab".repeat(32);
        for revision in [SHA1, sha256.as_str()] {
            let query = format!("path=a.md&at={revision}");
            let (project, calls) = bound(fake(PATH, Ok(file((1, 1))), Some("aaaa")));
            let first = answer(get_document(project.as_ref(), &query, &plain()));
            assert_eq!(first.status, StatusCode::OK, "{revision}");
            assert_eq!(first.json(), json!({"path": "a.md", "revision": "t1"}));
            assert_eq!(
                first.header("cache-control"),
                "private, max-age=31536000, immutable"
            );
            let tag = first.tag();
            assert_ne!(tag, "");
            assert_eq!(
                calls.taken(),
                [Call::DocumentAt("a.md".into(), revision.into())]
            );
            let again = answer(get_document(project.as_ref(), &query, &having(&tag)));
            assert_eq!(again.status, StatusCode::NOT_MODIFIED);
            assert_eq!(
                again.header("cache-control"),
                "private, max-age=31536000, immutable"
            );
            assert_eq!(calls.taken(), []);
            // Another path at the same revision, or another revision: another tag
            for other in [
                format!("path=b.md&at={revision}"),
                format!("path=a.md&at={}", "cd".repeat(20)),
            ] {
                let answer = answer(get_document(project.as_ref(), &other, &having(&tag)));
                assert_eq!(answer.status, StatusCode::OK, "{other}");
                assert_ne!(answer.tag(), tag, "{other}");
            }
        }
    }

    /// Only a full object ID in git's lower case is kept: a short one, a name, upper case or another length can move
    #[test]
    fn a_revision_that_could_move_is_never_kept() {
        let short = &SHA1[..39];
        let upper = SHA1.to_uppercase();
        let long = format!("{SHA1}0");
        let not_hex = format!("{}g", &SHA1[..39]);
        for revision in [
            short,
            upper.as_str(),
            long.as_str(),
            not_hex.as_str(),
            "HEAD",
            "main",
        ] {
            assert!(!is_object_id(revision), "{revision}");
            let (project, _) = bound(fake(PATH, Ok(file((1, 1))), None));
            let query = format!("path=a.md&at={revision}");
            let answer = answer(get_document(project.as_ref(), &query, &plain()));
            assert_eq!(answer.header("cache-control"), "no-store", "{revision}");
            assert_eq!(answer.tag(), "", "{revision}");
        }
    }

    /// A refusal is never kept: the file may be readable later (a submodule checked out, a fetch)
    #[test]
    fn a_refusal_at_a_full_object_id_is_not_kept() {
        for (error, status) in refusals() {
            let (project, _) = bound(FakeProject {
                at_revision: Err(error.clone()),
                ..FakeProject::default()
            });
            let answer = answer(get_document(
                project.as_ref(),
                &format!("path=a.md&at={SHA1}"),
                &plain(),
            ));
            assert_eq!(answer.status, status, "{error}");
            assert_eq!(answer.header("cache-control"), "no-store");
            assert_eq!(answer.tag(), "");
        }
    }

    #[test]
    fn a_document_at_a_revision_atm_lib_refuses_is_its_status() {
        for (error, status) in refusals() {
            let (project, _) = bound(FakeProject {
                at_revision: Err(error.clone()),
                ..FakeProject::default()
            });
            let answer = answer(get_document(
                project.as_ref(),
                &format!("path=a.md&at={SHA1}"),
                &plain(),
            ));
            assert_eq!(answer.status, status, "{error}");
            assert_eq!(answer.json(), json!({"error": error.to_string()}));
        }
    }
}
