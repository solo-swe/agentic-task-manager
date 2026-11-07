//! The dashboard's pages (T-429; `dashboard/src/lib/address.ts` `PAGE_PATH`): the same page at each, which shows the
//! one its path names. Nothing else is served under them: a project's ID or tab is only text the page reads.

/// A page's path without its leading `/` and an optional trailing `/`: the root, `board`, `progress`, `agents`,
/// `projects`, `projects/<id>` or `projects/<id>/<tab>` (a tab in lowercase letters).
fn is_form(rest: &str) -> bool {
    if matches!(rest, "" | "board" | "progress" | "agents" | "projects") {
        return true;
    }
    let Some(after) = rest.strip_prefix("projects/") else {
        return false;
    };
    let mut parts = after.split('/');
    let id = parts.next().unwrap_or_default();
    let tab = parts.next();
    !id.is_empty()
        && parts.next().is_none()
        && tab.is_none_or(|t| !t.is_empty() && t.bytes().all(|b| b.is_ascii_lowercase()))
}

/// Whether `path` is one of the dashboard's pages.
pub fn is_page(path: &str) -> bool {
    let Some(rest) = path.strip_prefix('/') else {
        return false;
    };
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
            "/tasks.json",
            "",
        ] {
            assert!(!is_page(other), "{other}");
        }
    }
}
