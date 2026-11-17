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
