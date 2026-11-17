//! A project's files for the document viewer (docs/multi-project-api.md §2.6): through atm-lib (atm-docs), which
//! checks the path and reads the file from the working copy.

use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use serde_json::json;

use super::query::parameter;
use super::source::ProjectSource;
use super::{IMAGE_HEADERS, error_response, etag, not_modified, ok_json, unchanged};
use crate::send;

/// `GET …/projects/<p>/documents?path=`: a file as the viewer shows it (its JSON). The `ETag` is the file's size and
/// mtime and the current commit (which it names), so the viewer's poll gets a 304 without the file being read again.
/// With `&at=<revision>`, the file as that commit or tree has it (a review round's side), which never changes: no tag.
pub(super) fn get_document(
    project: &dyn ProjectSource,
    query: &str,
    request: &HeaderMap,
) -> Response {
    let path = parameter(query, "path");
    let at = parameter(query, "at");
    if !at.is_empty() {
        return match project.document_at(&path, &at) {
            Ok(document) => ok_json(&document, None),
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

/// `GET …/projects/<p>/files?path=`: an image's bytes (whose bytes match its type), sandboxed. The `ETag` is the
/// file's size and mtime.
pub(super) fn get_file(project: &dyn ProjectSource, query: &str, request: &HeaderMap) -> Response {
    let path = parameter(query, "path");
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

    /// A review round's side: the file as a commit or tree has it, asked of atm-lib with the revision as given (it
    /// checks it); never the working copy's version, never tagged
    #[test]
    fn a_document_at_a_revision_is_atm_libs_answer_untagged() {
        for (query, path, revision) in [
            ("path=docs%2Fa+b.md&at=3fa1", PATH, "3fa1"),
            ("at=HEAD&path=a.md", "a.md", "HEAD"),
            ("path=&at=t1", "", "t1"),
            ("path=a.md&at=--output%3Dx", "a.md", "--output=x"),
        ] {
            let (project, calls) = bound(fake(PATH, Ok(file((1, 1))), Some("aaaa")));
            let answer = answer(get_document(project.as_ref(), query, &having("\"any\"")));
            assert_eq!(answer.status, StatusCode::OK, "{query}");
            assert_eq!(answer.json(), json!({"path": "a.md", "revision": "t1"}));
            assert_eq!(answer.tag(), "", "{query}");
            assert_eq!(
                calls.taken(),
                [Call::DocumentAt(path.into(), revision.into())],
                "{query}"
            );
        }
        // No revision: the working copy's
        let (project, calls) = bound(fake(PATH, Ok(file((1, 1))), None));
        let _ = get_document(project.as_ref(), "path=docs%2Fa+b.md&at=", &plain());
        assert_eq!(calls.taken()[0], Call::FileVersion(PATH.into()));
    }

    #[test]
    fn a_document_at_a_revision_atm_lib_refuses_is_its_status() {
        for (error, status) in refusals() {
            let (project, _) = bound(FakeProject {
                at_revision: Err(error.clone()),
                ..FakeProject::default()
            });
            let answer = answer(get_document(project.as_ref(), "path=a.md&at=x", &plain()));
            assert_eq!(answer.status, status, "{error}");
            assert_eq!(answer.json(), json!({"error": error.to_string()}));
        }
    }
}
