//! The dashboard's pages (T-429; `dashboard/src/lib/address.ts` `PAGE_PATH`): the same page at each, which shows the
//! one its path names. Nothing else is served under them: a project's ID or tab is only text the page reads.

/// A page's path without its leading `/` and an optional trailing `/`: the root, `board`, `progress`, `agents`,
/// `reviews` (every project's, ATM-575), `projects`, `projects/<id>`, `projects/<id>/<tab>` (a tab in lowercase
/// letters) or a review's page, `projects/<id>/reviews/<CR>` (its reference: letters, digits, `-` and `.`, as
/// `CR-8.2`).
fn is_form(rest: &str) -> bool {
    if matches!(
        rest,
        "" | "board" | "progress" | "agents" | "reviews" | "projects"
    ) {
        return true;
    }
    let Some(after) = rest.strip_prefix("projects/") else {
        return false;
    };
    let parts: Vec<&str> = after.split('/').collect();
    let tab = |t: &str| !t.is_empty() && t.bytes().all(|b| b.is_ascii_lowercase());
    let reference = |r: &str| {
        !r.is_empty()
            && r.len() <= 64
            && r.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.')
    };
    match parts.as_slice() {
        [id] => !id.is_empty(),
        [id, t] => !id.is_empty() && tab(t),
        [id, "reviews", r] => !id.is_empty() && reference(r),
        _ => false,
    }
}

/// Whether `path` is one of the dashboard's pages: one of the forms, or one in a workspace (`/w/<w>`, the workspace's
/// landing page, or `/w/<w>/<form>`; ADR-003 §5).
pub fn is_page(path: &str) -> bool {
    let Some(mut rest) = path.strip_prefix('/') else {
        return false;
    };
    if let Some(after) = rest.strip_prefix("w/") {
        let (workspace, page) = after.split_once('/').unwrap_or((after, ""));
        if workspace.is_empty() {
            return false;
        }
        rest = page;
    }
    is_form(rest) || rest.strip_suffix('/').is_some_and(is_form)
}

#[cfg(test)]
mod tests {
    use super::is_page;

    #[test]
    fn the_pages_are_the_dashboards_paths_and_nothing_under_them() {
        for page in [
            "/",
            "/projects",
            "/projects/",
            "/projects/abcd1234",
            "/projects/abcd1234/list",
            "/projects/any-id/",
            "/board",
            "/progress/",
            "/agents",
            "/reviews",
            "/w/work/reviews/",
            "/w/work",
            "/w/work/",
            "/w/work/board",
            "/w/work/projects/abcd1234/list",
            "/projects/abcd1234/reviews",
            "/projects/abcd1234/reviews/CR-7",
            "/projects/abcd1234/reviews/CR-8.2/",
            "/projects/abcd1234/reviews/0193c0de-77aa",
            "/w/work/projects/abcd1234/reviews/CR-7",
        ] {
            assert!(is_page(page), "{page}");
        }
        for other in [
            "/projects/abcd1234/tasks.json",
            "/projects/abcd1234/list/x",
            "/projects/abcd1234/List",
            "/projects//",
            "/boards",
            "/board/x",
            "/reviews/CR-7",
            "/tasks.json",
            "/w",
            "/w/",
            "/w//projects",
            "/w/work/tasks.json",
            "/w/work/projects/abcd1234/list/x",
            "/projects/abcd1234/reviews//",
            "/projects/abcd1234/reviews/CR-7/comments",
            "/projects/abcd1234/reviews/CR 7",
            "/projects/abcd1234/reviews/CR_7",
            "/projects/abcd1234/reviews/<x>",
            "/projects/abcd1234/list/CR-7",
            "/projects//reviews/CR-7",
            "",
        ] {
            assert!(!is_page(other), "{other}");
        }
    }
}
