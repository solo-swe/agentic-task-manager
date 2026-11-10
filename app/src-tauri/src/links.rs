//! The app's deep links (T-459): `atm://projects/<id>[/<tab>]?task=…&doc=…` (and `atm://w/<w>/…`, `atm://board`, …)
//! open the app at that page. A link is only ever an address for the dashboard's own router: its path and query,
//! checked here and parsed by the page's address code (`dashboard/src/lib/address.ts`), never fetched or run. macOS
//! hands the app its links (`RunEvent::Opened`; the scheme is in `Info.plist`); the page gets them on a channel
//! (`subscribe_links`), and a link that started the app waits for it.

use std::collections::HashMap;
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

/// The addresses that reach each window's page (T-511): on its channel once it has subscribed, the last one kept for
/// it until then (a link that started the app, a new window's first page).
#[derive(Default)]
pub struct Links {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    /// Each window's page's channel, by the window's label
    channels: HashMap<String, Channel<String>>,
    /// The address waiting for each window's page
    waiting: HashMap<String, String>,
}

impl Links {
    fn inner(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// A link the app was opened with, for `window`'s page: sent now, or kept for it. Whether it was a link the app
    /// opens.
    pub fn open(&self, window: &str, link: &str) -> bool {
        let Some(address) = link_address(link) else {
            return false;
        };
        self.send(window, address);
        true
    }

    /// An address (already checked) for `window`'s page: sent now, or kept until it subscribes.
    pub fn send(&self, window: &str, address: String) {
        let mut inner = self.inner();
        let sent = inner
            .channels
            .get(window)
            .is_some_and(|channel| channel.send(address.clone()).is_ok());
        if !sent {
            inner.waiting.insert(window.to_owned(), address);
        }
    }

    /// `window`'s page subscribes: its later addresses go to `channel`; the one waiting for it, if any, is the answer.
    pub fn subscribe(&self, window: &str, channel: Channel<String>) -> Option<String> {
        let mut inner = self.inner();
        inner.channels.insert(window.to_owned(), channel);
        inner.waiting.remove(window)
    }

    /// `window` closed: nothing more goes to it.
    pub fn forget(&self, window: &str) {
        let mut inner = self.inner();
        inner.channels.remove(window);
        inner.waiting.remove(window);
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

    /// A channel that hands what it is sent to the returned receiver.
    fn recorded() -> (
        Channel<String>,
        std::sync::mpsc::Receiver<tauri::ipc::InvokeResponseBody>,
    ) {
        let (sent, received) = std::sync::mpsc::channel();
        let channel = Channel::new(move |body| {
            let _ = sent.send(body);
            Ok(())
        });
        (channel, received)
    }

    fn text(body: tauri::ipc::InvokeResponseBody) -> String {
        let tauri::ipc::InvokeResponseBody::Json(text) = body else {
            panic!("a JSON message");
        };
        text
    }

    #[test]
    fn a_link_before_the_page_subscribes_waits_for_it_and_one_after_goes_to_its_channel() {
        let links = Links::default();
        assert!(!links.open("main", "https://example.com"));
        assert!(links.open("main", "atm://projects/abc"));
        assert!(links.open("main", "atm://projects/def/list"));
        let (channel, received) = recorded();
        // The latest link is the one the app opens at
        assert_eq!(
            links.subscribe("main", channel).as_deref(),
            Some("/projects/def/list")
        );
        assert!(links.open("main", "atm://board"));
        assert_eq!(text(received.recv().expect("a link")), "\"/board\"");
    }

    /// T-511: each window's page gets its own addresses, and a closed window none
    #[test]
    fn each_window_gets_its_own_links() {
        let links = Links::default();
        let (main, to_main) = recorded();
        assert_eq!(links.subscribe("main", main), None);
        // A new window's first page waits for its page to subscribe
        links.send("window-1", "/projects/abc".into());
        let (other, to_other) = recorded();
        assert_eq!(
            links.subscribe("window-1", other).as_deref(),
            Some("/projects/abc")
        );
        assert!(links.open("window-1", "atm://board"));
        assert_eq!(text(to_other.recv().expect("a link")), "\"/board\"");
        assert!(
            to_main.try_recv().is_err(),
            "main got another window's link"
        );
        links.forget("window-1");
        links.send("window-1", "/projects/def".into());
        assert!(to_other.try_recv().is_err(), "a closed window got a link");
    }
}
