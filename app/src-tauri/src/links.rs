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
const PAGES: [&str; 6] = ["projects", "board", "progress", "agents", "reviews", "w"];

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
        crate::locked(&self.inner)
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

    /// `window`'s page started loading (a reload, ATM-677, audit A8): the old page's channel is dropped, so an address
    /// sent before the new page subscribes waits for it rather than going to a page that is gone. What already waits
    /// stays.
    pub fn page_started(&self, window: &str) {
        self.inner().channels.remove(window);
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
            // A task by its key, an earlier prefix's key or its UUID (ADR-007 §5, T-546)
            (
                "atm://projects/3f2a9c1e/kanban?task=ATM-527",
                "/projects/3f2a9c1e/kanban?task=ATM-527",
            ),
            (
                "atm://projects/abc?task=TskMan-12",
                "/projects/abc?task=TskMan-12",
            ),
            (
                "atm://projects/abc?task=0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b",
                "/projects/abc?task=0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b",
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

    /// Every one of the dashboard's pages opens, with or without a query; an empty query is none
    #[test]
    fn each_of_the_dashboards_pages_opens() {
        for page in PAGES {
            assert_eq!(
                link_address(&format!("atm://{page}")),
                Some(format!("/{page}")),
                "{page}"
            );
        }
        assert_eq!(link_address("atm://board?").as_deref(), Some("/board"));
        assert_eq!(
            link_address("atm://?task=T-1").as_deref(),
            Some("/?task=T-1")
        );
        assert_eq!(
            link_address("atm://progress?sprint=S+1&release=0.41.0").as_deref(),
            Some("/progress?sprint=S+1&release=0.41.0")
        );
        // A review's sub-tab and a comment in view (ATM-697), the Reviews list's filter
        assert_eq!(
            link_address("atm://projects/abc/reviews/CR-7/diffs?file=docs/x.md&comment=c12")
                .as_deref(),
            Some("/projects/abc/reviews/CR-7/diffs?file=docs/x.md&comment=c12")
        );
        assert_eq!(
            link_address("atm://projects/abc/reviews?status=waiting&q=tabs").as_deref(),
            Some("/projects/abc/reviews?status=waiting&q=tabs")
        );
    }

    /// A query holds only what an address does: no quotes, spaces, angle brackets or another `?`
    #[test]
    fn a_query_with_other_characters_is_refused() {
        for link in [
            "atm://board?q=\"x\"",
            "atm://board?q=a b",
            "atm://board?q=<",
            "atm://board?a=1?b=2",
            "atm://board?q='x'",
            "atm://board?q=;",
        ] {
            assert_eq!(link_address(link), None, "{link}");
        }
        // A path doesn't take the query's punctuation
        for link in [
            "atm://projects/a=b",
            "atm://projects/a&b",
            "atm://projects/a:b",
        ] {
            assert_eq!(link_address(link), None, "{link}");
        }
    }

    /// The longest link the app opens is `MAX_LINK` long; one character more is refused
    #[test]
    fn a_link_at_the_limit_opens_and_one_past_it_does_not() {
        let at = format!(
            "atm://board?q={}",
            "a".repeat(MAX_LINK - "atm://board?q=".len())
        );
        assert_eq!(at.len(), MAX_LINK);
        assert!(link_address(&at).is_some());
        assert_eq!(link_address(&format!("{at}a")), None);
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

    /// A page whose channel fails (it went away) doesn't lose the address: it waits for the page's next subscription
    #[test]
    fn an_address_its_channel_refuses_waits_for_the_next_subscription() {
        let links = Links::default();
        let broken = Channel::new(|_| Err(tauri::Error::WindowNotFound));
        assert_eq!(links.subscribe("main", broken), None);
        assert!(links.open("main", "atm://board"));
        let (channel, received) = recorded();
        assert_eq!(links.subscribe("main", channel).as_deref(), Some("/board"));
        assert!(received.try_recv().is_err(), "sent once, as the answer");
        // Answered once: the next subscription has nothing waiting
        assert_eq!(links.subscribe("main", recorded().0), None);
    }

    /// A page that subscribes again (a reload) gets the later addresses on its new channel, not the old one
    #[test]
    fn a_new_subscription_replaces_the_windows_channel() {
        let links = Links::default();
        let (old, to_old) = recorded();
        let (new, to_new) = recorded();
        links.subscribe("main", old);
        links.subscribe("main", new);
        assert!(links.open("main", "atm://board"));
        assert_eq!(text(to_new.recv().expect("a link")), "\"/board\"");
        assert!(to_old.try_recv().is_err());
    }

    /// ATM-677, audit A8: a link sent while the page reloads (after its load started, before the new page subscribed)
    /// waits for the new page, never reaching the old page's channel; one waiting already stays
    #[test]
    fn a_link_sent_while_the_page_reloads_waits_for_the_new_page() {
        let links = Links::default();
        let (old, to_old) = recorded();
        links.subscribe("main", old);
        links.page_started("main");
        assert!(links.open("main", "atm://board"));
        assert!(to_old.try_recv().is_err(), "the page that is gone got it");
        let (new, _) = recorded();
        assert_eq!(links.subscribe("main", new).as_deref(), Some("/board"));
        // Waiting before the page started: kept for it
        links.send("window-1", "/projects/abc".into());
        links.page_started("window-1");
        assert_eq!(
            links.subscribe("window-1", recorded().0).as_deref(),
            Some("/projects/abc")
        );
    }

    /// A link the app doesn't open changes nothing: the address waiting stays
    #[test]
    fn a_refused_link_leaves_the_waiting_address() {
        let links = Links::default();
        assert!(links.open("main", "atm://projects/abc"));
        assert!(!links.open("main", "atm://api/v2/workspaces"));
        assert_eq!(
            links.subscribe("main", recorded().0).as_deref(),
            Some("/projects/abc")
        );
        // Forgetting a window drops what waits for it too
        links.send("window-1", "/board".into());
        links.forget("window-1");
        assert_eq!(links.subscribe("window-1", recorded().0), None);
    }
}
