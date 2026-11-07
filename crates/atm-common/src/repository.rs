//! Repository web addresses (the Python atm's `common/repository.py`): `meta.repository`, e.g. `https://github.com/o/r`,
//! which the dashboard uses to link commits and pull requests; a git remote turned into one, and PR links moved when
//! the repository moves.

use serde_json::Value;

/// `scheme://[user@]host[:port][/path]`, the URL forms of a remote: (scheme, host, path).
fn url_parts(text: &str) -> Option<(&str, &str, &str)> {
    let (scheme, rest) = text.split_once("://")?;
    if !matches!(scheme, "http" | "https" | "ssh" | "git") {
        return None;
    }
    // Credentials: everything up to an `@` that comes before any `/`
    let rest = match (rest.find('@'), rest.find('/')) {
        (Some(at), slash) if at > 0 && slash.is_none_or(|s| at < s) => &rest[at + 1..],
        _ => rest,
    };
    let host_end = rest.find(['/', ':']).unwrap_or(rest.len());
    let host = &rest[..host_end];
    if host.is_empty() {
        return None;
    }
    let mut after = &rest[host_end..];
    if let Some(port) = after.strip_prefix(':') {
        let digits = port.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 {
            return None;
        }
        after = &port[digits..];
    }
    if !after.is_empty() && !after.starts_with('/') {
        return None;
    }
    Some((scheme, host, after))
}

fn word(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '.' | '-')
}

/// `[user@]host:path`, git's scp-like form: (host, path).
fn scp_parts(text: &str) -> Option<(&str, &str)> {
    let rest = match text.find('@') {
        Some(at) if at > 0 && text[..at].chars().all(word) => &text[at + 1..],
        _ => text,
    };
    let host_end = rest.find(|c: char| !word(c))?;
    let (host, after) = rest.split_at(host_end);
    let path = after.strip_prefix(':')?;
    if host.is_empty() || path.is_empty() || path.starts_with("//") || path.contains('\n') {
        return None;
    }
    Some((host, path))
}

/// The web address of a repository: an http(s) URL as given, or a git remote turned into one
/// (`git@github.com:o/r.git`, `ssh://git@github.com/o/r.git`, `https://github.com/o/r.git` → `https://github.com/o/r`).
/// Credentials, a trailing `.git` and a trailing slash go. None if it isn't one.
#[must_use]
pub fn repository_url(text: &str) -> Option<String> {
    let text = crate::py::strip(text);
    let (web, host, path) = if let Some((scheme, host, path)) = url_parts(text) {
        let web = if matches!(scheme, "ssh" | "git") {
            "https"
        } else {
            scheme
        };
        (web, host, path.to_string())
    } else {
        let (host, path) = scp_parts(text)?;
        ("https", host, format!("/{path}"))
    };
    let trimmed = path.trim_end_matches('/');
    let path = trimmed
        .strip_suffix(".git")
        .unwrap_or(trimmed)
        .trim_end_matches('/');
    if host.is_empty()
        || (!host.contains('.') && host != "localhost")
        || path.trim_matches('/').is_empty()
    {
        return None;
    }
    Some(format!("{web}://{host}{path}"))
}

/// Point the tasks' recorded PR links (`pr.url`) on the `old` address at the `new` one, after the repository moved.
/// Only the link changes: no timestamps or actors, so the history stays as it was. Returns how many moved.
pub fn move_pr_links(tasks: &mut [Value], old: &str, new: &str) -> usize {
    let mut moved = 0;
    for pr in tasks
        .iter_mut()
        .filter_map(|t| t.get_mut("pr"))
        .filter(|p| p.is_object())
    {
        let Some(link) = pr.get("url").and_then(Value::as_str) else {
            continue;
        };
        if link == old || link.starts_with(&format!("{old}/")) {
            let moved_link = format!("{new}{}", &link[old.len()..]);
            pr["url"] = Value::String(moved_link);
            moved += 1;
        }
    }
    moved
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn remotes_become_web_addresses() {
        let cases = [
            ("https://github.com/o/r", Some("https://github.com/o/r")),
            (
                "https://github.com/o/r.git/",
                Some("https://github.com/o/r"),
            ),
            (
                "https://user:token@github.com/o/r.git",
                Some("https://github.com/o/r"),
            ),
            ("git@github.com:o/r.git", Some("https://github.com/o/r")),
            (
                "ssh://git@git.example.com:2222/team/r.git",
                Some("https://git.example.com/team/r"),
            ),
            ("http://localhost/r", Some("http://localhost/r")),
            ("javascript:alert(1)", None),
            ("https://github.com/", None),
            ("not a remote", None),
            ("ftp://example.com/r", None),
        ];
        for (remote, web) in cases {
            assert_eq!(repository_url(remote).as_deref(), web, "{remote}");
        }
    }

    #[test]
    fn pr_links_move_with_the_repository() {
        let mut tasks = vec![
            json!({"pr": {"number": 1, "url": "https://a.com/o/r/pull/1"}}),
            json!({"pr": {"number": 2, "url": "https://b.com/x/pull/2"}}),
            json!({"id": "T-3"}),
        ];
        assert_eq!(
            move_pr_links(&mut tasks, "https://a.com/o/r", "https://c.com/o/r"),
            1
        );
        assert_eq!(tasks[0]["pr"]["url"], json!("https://c.com/o/r/pull/1"));
    }
}
