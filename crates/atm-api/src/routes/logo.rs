//! A project's logo (docs/multi-project-api.md §2.3).

use atm_lib::workspaces::{Member, file_stamp};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;

use super::{IMAGE_HEADERS, etag};
use crate::{send_conditional, send_error};

/// `GET …/projects/<p>/logo`: the project's logo (the store checks it is inside the project and small enough), kept
/// by the browser and asked for again each time (no-cache), a 304 while it is unchanged.
pub(super) fn get_logo(member: &Member, request: &HeaderMap) -> Response {
    let Some((file, kind)) = member.logo() else {
        return send_error(StatusCode::NOT_FOUND, "no logo");
    };
    let stamp = file_stamp(&file);
    if stamp.is_null() {
        return send_error(StatusCode::NOT_FOUND, "no logo");
    }
    let read = || std::fs::read(&file).unwrap_or_default();
    send_conditional(request, &etag(&stamp), read, kind, &IMAGE_HEADERS)
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{PIXEL, a_state_with_a_project, answer, having, member};
    use super::*;

    #[test]
    fn the_logo_is_sent_as_a_sandboxed_image_and_a_304_while_unchanged() {
        let home = a_state_with_a_project("logo");
        let member = member(&home);
        let first = answer(get_logo(&member, &HeaderMap::new()));
        assert_eq!(first.status, StatusCode::OK);
        assert_eq!(first.body, PIXEL);
        assert_eq!(first.header("content-type"), "image/png");
        assert!(first.header("content-security-policy").ends_with("sandbox"));
        assert_eq!(first.header("cache-control"), "no-cache");
        let tag = first.header("etag").to_string();
        let again = answer(get_logo(&member, &having(&tag)));
        assert_eq!(again.status, StatusCode::NOT_MODIFIED);
        assert_eq!(again.body, b"");
        std::fs::remove_dir_all(&home).expect("cleaned up");
    }

    #[test]
    fn no_logo_or_a_missing_one_is_not_found() {
        let home = a_state_with_a_project("no-logo");
        std::fs::remove_file(home.join("project/logo.png")).expect("the logo removed");
        let answer = answer(get_logo(&member(&home), &HeaderMap::new()));
        assert_eq!(answer.status, StatusCode::NOT_FOUND);
        assert_eq!(answer.json()["error"], "no logo");
        std::fs::remove_dir_all(&home).expect("cleaned up");
    }
}
