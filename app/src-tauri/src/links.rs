//! The app's deep links (T-459): `atm://projects/<id>[/<tab>]?task=…&doc=…` (and `atm://w/<w>/…`, `atm://board`, …)
//! open the app at that page. A link is only ever an address for the dashboard's own router: its path and query,
//! checked here and parsed by the page's address code (`dashboard/src/lib/address.ts`), never fetched or run. macOS
//! hands the app its links (`RunEvent::Opened`; the scheme is in `Info.plist`); the page gets them on a channel
//! (`subscribe_links`), and a link that started the app waits for it.

use std::sync::Mutex;

use tauri::ipc::Channel;

/// The scheme the app registers.
pub const SCHEME: &str = "atm";
/// A link longer than this is refused: no page's address comes near it.
const MAX_LINK: usize = 4096;
/// The first part of a page's path: the dashboard's pages (`docs/multi-project-api.md` §2.7).
const PAGES: [&str; 5] = ["projects", "board", "progress", "agents", "w"];

/// The dashboard address an `atm://` link opens (`/projects/abc/list?task=T-1`), or none for anything else: another
/// scheme, a path that isn't one of the dashboard's pages, `.` or `..` segments, a fragment, characters an address
/// doesn't hold.
#[must_use]
pub fn link_address(link: &str) -> Option<String> {
    if link.len() > MAX_LINK {
        return None;
    }
    let rest = link.strip_prefix(SCHEME)?.strip_prefix("://")?;
    let (path, query) = match rest.split_once('?') {
        Some((path, query)) => (path, Some(query)),
        None => (rest, None),
    };
    let path = path.trim_end_matches('/');
    let safe = |text: &str, extra: &[u8]| {
        text.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-._~%".contains(&b) || extra.contains(&b))
    };
    let segments: Vec<&str> = path.split('/').collect();
    let page = segments.first().copied().unwrap_or_default();
    let ok = (path.is_empty() || PAGES.contains(&page))
        && segments
            .iter()
            .all(|s| (path.is_empty() || !s.is_empty()) && *s != "." && *s != "..")
        && safe(path, b"/")
        && query.is_none_or(|q| safe(q, b"=&+,:/@"));
    ok.then(|| match query {
        Some(query) if !query.is_empty() => format!("/{path}?{query}"),
        _ => format!("/{path}"),
    })
}

/// The links that reach the app: the page's channel once it has subscribed, the last link until then.
#[derive(Default)]
pub struct Links {
    inner: Mutex<(Option<Channel<String>>, Option<String>)>,
}

impl Links {
    /// A link the app was opened with: to the page now, or kept for it. Whether it was a link the app opens.
    pub fn open(&self, link: &str) -> bool {
        let Some(address) = link_address(link) else {
            return false;
        };
        let mut inner = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let sent = inner
            .0
            .as_ref()
            .is_some_and(|channel| channel.send(address.clone()).is_ok());
        if !sent {
            inner.1 = Some(address);
        }
        true
    }

    /// The page subscribes: later links go to `channel`; the one that came before, if any, is the answer.
    pub fn subscribe(&self, channel: Channel<String>) -> Option<String> {
        let mut inner = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        inner.0 = Some(channel);
        inner.1.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_link_is_one_of_the_dashboards_addresses_and_nothing_else() {
        for (link, address) in [
            (
                "atm://projects/abc/list?task=T-1",
                "/projects/abc/list?task=T-1",
            ),
            ("atm://projects/abc/", "/projects/abc"),
            (
                "atm://w/work/projects/abc?doc=docs/x.md:L3",
                "/w/work/projects/abc?doc=docs/x.md:L3",
            ),
            ("atm://board", "/board"),
            ("atm://", "/"),
            (
                "atm://projects/a%20b?sprint=Library,%20gallery&sprint=backlog",
                "/projects/a%20b?sprint=Library,%20gallery&sprint=backlog",
            ),
        ] {
            assert_eq!(link_address(link).as_deref(), Some(address), "{link}");
        }
        for link in [
            "https://projects/abc",
            "atmx://projects/abc",
            "atm:projects/abc",
            "atm://api/v2/workspaces",
            "atm://p/abc/claims.json",
            "atm://projects/../api/projects",
            "atm://projects/./abc",
            "atm://projects//abc",
            "atm://projects/abc#x",
            "atm://projects/abc?q=<script>",
            "atm://projects/a b",
            "atm://projects/abc\\..",
        ] {
            assert_eq!(link_address(link), None, "{link}");
        }
        assert_eq!(
            link_address(&format!("atm://projects/{}", "a".repeat(MAX_LINK))),
            None
        );
    }

    #[test]
    fn a_link_before_the_page_subscribes_waits_for_it_and_one_after_goes_to_its_channel() {
        let links = Links::default();
        assert!(!links.open("https://example.com"));
        assert!(links.open("atm://projects/abc"));
        assert!(links.open("atm://projects/def/list"));
        let (sent, received) = std::sync::mpsc::channel();
        let channel = Channel::new(move |body| {
            let _ = sent.send(body);
            Ok(())
        });
        // The latest link is the one the app opens at
        assert_eq!(
            links.subscribe(channel).as_deref(),
            Some("/projects/def/list")
        );
        assert!(links.open("atm://board"));
        let tauri::ipc::InvokeResponseBody::Json(text) = received.recv().expect("a link") else {
            panic!("a JSON message");
        };
        assert_eq!(text, "\"/board\"");
    }
}
