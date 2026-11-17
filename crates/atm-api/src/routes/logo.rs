//! A project's logo (docs/multi-project-api.md §2.3).

use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;

use super::source::ProjectSource;
use super::{IMAGE_HEADERS, etag};
use crate::{send_conditional, send_error};

/// `GET …/projects/<p>/logo`: the project's logo (the store checks it is inside the project and small enough), kept
/// by the browser and asked for again each time (no-cache), a 304 while it is unchanged.
pub(super) fn get_logo(project: &dyn ProjectSource, request: &HeaderMap) -> Response {
    let Some((stamp, kind)) = project.logo() else {
        return send_error(StatusCode::NOT_FOUND, "no logo");
    };
    if stamp.is_null() {
        return send_error(StatusCode::NOT_FOUND, "no logo");
    }
    let read = || project.read_logo();
    send_conditional(request, &etag(&stamp), read, kind, &IMAGE_HEADERS)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::super::test_support::{Call, FakeProject, answer, bound, having, plain};
    use super::*;

    fn with_logo(stamp: serde_json::Value, kind: &'static str) -> FakeProject {
        FakeProject {
            logo: Some((stamp, kind, b"<logo>".to_vec())),
            ..FakeProject::default()
        }
    }

    #[test]
    fn a_logo_is_its_bytes_as_its_type_sandboxed_and_asked_for_each_time() {
        for kind in ["image/png", "image/svg+xml", "image/jpeg"] {
            let (project, calls) = bound(with_logo(json!([6, "1"]), kind));
            let answer = answer(get_logo(project.as_ref(), &plain()));
            assert_eq!(answer.status, StatusCode::OK, "{kind}");
            assert_eq!(answer.body, b"<logo>");
            assert_eq!(answer.header("content-type"), kind);
            // An SVG could carry a script: shown, never run
            assert_eq!(
                answer.header("content-security-policy"),
                "default-src 'none'; style-src 'unsafe-inline'; sandbox"
            );
            assert_eq!(answer.header("x-content-type-options"), "nosniff");
            assert_eq!(answer.header("cache-control"), "no-cache");
            assert_ne!(answer.tag(), "");
            assert_eq!(calls.taken(), [Call::ReadLogo]);
        }
    }

    #[test]
    fn a_logo_is_a_304_without_being_read_until_its_file_changes() {
        let (project, calls) = bound(with_logo(json!([6, "1"]), "image/png"));
        let tag = answer(get_logo(project.as_ref(), &plain())).tag();
        calls.taken();
        let again = answer(get_logo(project.as_ref(), &having(&tag)));
        assert_eq!(again.status, StatusCode::NOT_MODIFIED);
        assert_eq!(again.body, b"");
        assert!(again.header("content-security-policy").ends_with("sandbox"));
        assert_eq!(calls.taken(), [], "not read");
        for changed in [json!([7, "1"]), json!([6, "2"])] {
            let (other, _) = bound(with_logo(changed.clone(), "image/png"));
            assert_eq!(
                answer(get_logo(other.as_ref(), &having(&tag))).status,
                StatusCode::OK,
                "{changed}"
            );
        }
    }

    #[test]
    fn no_logo_named_or_its_file_gone_is_not_found_and_nothing_is_read() {
        for fake in [
            FakeProject::default(),
            with_logo(serde_json::Value::Null, "image/png"),
        ] {
            let (project, calls) = bound(fake);
            let answer = answer(get_logo(project.as_ref(), &plain()));
            assert_eq!(answer.status, StatusCode::NOT_FOUND);
            assert_eq!(answer.json(), json!({"error": "no logo"}));
            assert_eq!(answer.tag(), "");
            assert_eq!(calls.taken(), []);
        }
    }
}
