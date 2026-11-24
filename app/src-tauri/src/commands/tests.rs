//! The commands' unit tests: each command's mapping to atm-lib, with a faked store's subscription.

use super::*;
use atm_lib::query::Filters;
use atm_lib::{OnChange, Subscription, Workspaces};
use serde_json::{Value, json};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::ipc::Channel;

/// A store's subscription, faked: it keeps the callback it was given, for the test to call as a change would, and
/// says when it was dropped (the subscription ended).
#[derive(Default)]
struct FakeStore {
    told: Arc<Mutex<Vec<OnChange>>>,
    ended: Arc<AtomicBool>,
}

/// Set when the subscription holding it is dropped.
struct Ends(Arc<AtomicBool>);

impl Drop for Ends {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

impl FakeStore {
    fn subscribe(&self, on_change: OnChange) -> Subscription {
        self.told.lock().expect("the fake's lock").push(on_change);
        Subscription::new(Ends(self.ended.clone()))
    }

    /// The store says its data changed.
    fn change(&self) {
        for told in self.told.lock().expect("the fake's lock").iter() {
            told();
        }
    }

    fn ended(&self) -> bool {
        self.ended.load(Ordering::SeqCst)
    }
}

/// A channel that hands what it is sent to the returned receiver.
fn recorded() -> (
    Channel<Changed>,
    std::sync::mpsc::Receiver<tauri::ipc::InvokeResponseBody>,
) {
    let (sent, received) = std::sync::mpsc::channel();
    let channel = Channel::new(move |body| {
        let _ = sent.send(body);
        Ok(())
    });
    (channel, received)
}

fn json_of(body: tauri::ipc::InvokeResponseBody) -> Value {
    let tauri::ipc::InvokeResponseBody::Json(text) = body else {
        panic!("a JSON message");
    };
    serde_json::from_str(&text).expect("json")
}

/// Its workspaces are never read by these tests: the subscriptions are fakes
fn state() -> AppState {
    AppState::new(Workspaces::at(&std::env::temp_dir().join("atm-app-unread")))
}

fn changed(project: Option<&str>) -> Changed {
    Changed {
        workspace: "work".into(),
        project: project.map(str::to_owned),
    }
}

#[test]
fn a_subscription_sends_its_message_on_each_change_until_it_ends() {
    let state = state();
    let store = FakeStore::default();
    let (channel, received) = recorded();
    let id = state
        .watch(
            "main",
            |c| Ok(store.subscribe(c)),
            channel,
            changed(Some("abcd1234")),
        )
        .expect("watching");
    store.change();
    store.change();
    for _ in 0..2 {
        assert_eq!(
            json_of(received.try_recv().expect("told of the change")),
            json!({"workspace": "work", "project": "abcd1234"})
        );
    }
    assert!(!store.ended());
    assert!(state.unwatch("main", id));
    assert!(store.ended(), "unsubscribing drops atm-lib's subscription");
}

/// A workspace's subscription says only the workspace: `project` is there, as `null`
#[test]
fn a_workspaces_message_names_no_project() {
    assert_eq!(
        serde_json::to_value(changed(None)).expect("json"),
        json!({"workspace": "work", "project": null})
    );
}

#[test]
fn a_subscription_atm_lib_refuses_is_its_error_and_nothing_is_kept() {
    let state = state();
    let (channel, _) = recorded();
    let refused = state.watch(
        "main",
        |_| Err(atm_common::Error::NotFound("unknown project".into())),
        channel,
        changed(None),
    );
    assert_eq!(refused, Err(AppError::NotFound("unknown project".into())));
    assert_eq!(state.unwatch_window("main"), 0);
}

#[test]
fn each_subscription_has_its_own_id() {
    let state = state();
    let store = FakeStore::default();
    let ids: Vec<u32> = (0..3)
        .map(|_| {
            state
                .watch(
                    "main",
                    |c| Ok(store.subscribe(c)),
                    recorded().0,
                    changed(None),
                )
                .expect("watching")
        })
        .collect();
    assert_eq!(ids, [1, 2, 3]);
}

/// T-511: a window ends only its own subscriptions, each once; an ID never given ends nothing
#[test]
fn only_its_own_window_ends_a_subscription_and_only_once() {
    let state = state();
    let store = FakeStore::default();
    let id = state
        .watch(
            "main",
            |c| Ok(store.subscribe(c)),
            recorded().0,
            changed(None),
        )
        .expect("watching");
    assert!(!state.unwatch("window-1", id));
    assert!(!store.ended(), "another window's unsubscribe ends nothing");
    assert!(!state.unwatch("main", id + 1));
    assert!(state.unwatch("main", id));
    assert!(!state.unwatch("main", id));
}

/// T-506, T-511: a page that reloads, or a window that closes, leaves its subscriptions behind; they end, and
/// another window's stay
#[test]
fn a_window_loading_again_or_closing_ends_its_own_subscriptions() {
    let state = state();
    let (main_store, other_store) = (FakeStore::default(), FakeStore::default());
    let watch = |window: &str, store: &FakeStore| {
        state
            .watch(
                window,
                |c| Ok(store.subscribe(c)),
                recorded().0,
                changed(None),
            )
            .expect("watching")
    };
    let mains = [watch("main", &main_store), watch("main", &main_store)];
    let other = watch("window-1", &other_store);
    assert_eq!(state.unwatch_window("main"), 2);
    assert!(main_store.ended());
    assert!(mains.iter().all(|id| !state.unwatch("main", *id)));
    assert_eq!(state.unwatch_window("main"), 0);
    // The other window's is still there, until it closes
    assert!(!other_store.ended());
    state.focused("window-1");
    assert_eq!(state.front().as_deref(), Some("window-1"));
    assert_eq!(state.window_closed("window-1"), 1);
    assert!(other_store.ended());
    assert!(!state.unwatch("window-1", other));
    assert_eq!(state.front(), None);
}

/// A closed window's links and menu go with it: nothing waits for it, its menu shows nothing checked
#[test]
fn a_closed_windows_links_and_menu_are_forgotten() {
    let state = state();
    state.links().send("window-1", "/board".into());
    state.menus().set_state(
        "window-1",
        crate::menu::MenuState {
            sidebar: true,
            ..crate::menu::MenuState::default()
        },
    );
    assert_eq!(state.window_closed("window-1"), 0);
    assert_eq!(
        state
            .links()
            .subscribe("window-1", Channel::new(|_| Ok(()))),
        None
    );
    assert_eq!(
        state.menus().state("window-1"),
        crate::menu::MenuState::default()
    );
}

#[test]
fn new_windows_are_numbered_and_never_reuse_a_label() {
    let state = state();
    assert_eq!(state.front(), None, "no window focused yet");
    assert_eq!(state.new_window_label(), "window-1");
    assert_eq!(state.new_window_label(), "window-2");
    state.focused("main");
    state.window_closed("window-2");
    assert_eq!(state.front().as_deref(), Some("main"));
    assert_eq!(state.new_window_label(), "window-3");
}

/// Each of atm's error kinds is the UI's, with its message, tagged as the page matches on it
#[test]
fn errors_keep_their_kind_and_message() {
    use atm_common::Error as E;
    for (error, app, kind) in [
        (
            E::NotFound("T-1".into()),
            AppError::NotFound("T-1".into()),
            "not_found",
        ),
        (
            E::Invalid("bad".into()),
            AppError::Invalid("bad".into()),
            "invalid",
        ),
        (
            E::Refused("no".into()),
            AppError::Refused("no".into()),
            "refused",
        ),
        (
            E::TooLarge("big".into()),
            AppError::TooLarge("big".into()),
            "too_large",
        ),
        (
            E::Conflict("claimed".into()),
            AppError::Conflict("claimed".into()),
            "conflict",
        ),
        (E::failed("off"), AppError::Failed("off".into()), "failed"),
    ] {
        let message = match &app {
            AppError::NotFound(m)
            | AppError::Invalid(m)
            | AppError::Refused(m)
            | AppError::TooLarge(m)
            | AppError::Conflict(m)
            | AppError::Failed(m) => m.clone(),
        };
        assert_eq!(AppError::from(error), app);
        assert_eq!(
            serde_json::to_value(&app).expect("json"),
            json!({"kind": kind, "message": message})
        );
    }
}

/// The page's task filters are `atm task list`'s: each named as the REST route's query parameter (`type`, not
/// `type_`), each handed on to atm-lib as it came
#[test]
fn task_filters_reach_atm_lib_as_the_page_sends_them() {
    let filters: TaskFilters = serde_json::from_value(json!({
        "sprint": ["S1", "S2"], "backlog": true, "release": "0.41.0", "component": "app",
        "status": ["todo"], "type": "bug", "priority": "high", "label": ["ui", "mac"],
        "agent": "claude:x", "ready": true, "blocked": true, "in_pr": true, "with_user": true,
        "archived": true,
    }))
    .expect("the page's filters");
    let lib = Filters::from(filters);
    assert_eq!(lib.sprint, ["S1", "S2"]);
    assert!(lib.backlog && lib.ready && lib.blocked && lib.in_pr && lib.with_user && lib.archived);
    assert_eq!(lib.release.as_deref(), Some("0.41.0"));
    assert_eq!(lib.component.as_deref(), Some("app"));
    assert_eq!(lib.status, ["todo"]);
    assert_eq!(lib.type_.as_deref(), Some("bug"));
    assert_eq!(lib.priority.as_deref(), Some("high"));
    assert_eq!(lib.label, ["ui", "mac"]);
    assert_eq!(lib.agent.as_deref(), Some("claude:x"));
}

/// What the page leaves out keeps everything
#[test]
fn no_filters_filter_nothing() {
    let tasks = Filters::from(serde_json::from_value::<TaskFilters>(json!({})).expect("{}"));
    assert!(tasks.sprint.is_empty() && tasks.status.is_empty() && tasks.label.is_empty());
    assert!(!tasks.backlog && !tasks.ready && !tasks.blocked && !tasks.archived);
    assert!(tasks.release.is_none() && tasks.type_.is_none() && tasks.agent.is_none());
    let reviews = atm_lib::review_pages::ReviewQuery::from(
        serde_json::from_value::<ReviewFilters>(json!({})).expect("{}"),
    );
    assert!(reviews.q.is_empty() && reviews.status.is_empty());
    assert!(reviews.author.is_none() && reviews.reviewer.is_none());
    assert!(reviews.task.is_none() && reviews.branch.is_none());
}

#[test]
fn review_filters_reach_atm_lib_as_the_page_sends_them() {
    let filters: ReviewFilters = serde_json::from_value(json!({
        "q": "needle", "status": ["open", "draft"], "author": "hakan", "reviewer": "claude:x",
        "task": "T-1", "branch": "dev/x",
    }))
    .expect("the page's filters");
    let lib = atm_lib::review_pages::ReviewQuery::from(filters);
    assert_eq!(lib.q, "needle");
    assert_eq!(lib.status, ["open", "draft"]);
    assert_eq!(lib.author.as_deref(), Some("hakan"));
    assert_eq!(lib.reviewer.as_deref(), Some("claude:x"));
    assert_eq!(lib.task.as_deref(), Some("T-1"));
    assert_eq!(lib.branch.as_deref(), Some("dev/x"));
}

/// A filter of the wrong shape is refused before any command runs (Tauri's deserializing)
#[test]
fn a_filter_of_the_wrong_shape_is_refused() {
    for wrong in [
        json!({"status": "todo"}),
        json!({"ready": "yes"}),
        json!("todo"),
    ] {
        assert!(
            serde_json::from_value::<TaskFilters>(wrong.clone()).is_err(),
            "{wrong}"
        );
    }
}

/// A file's answer, as the page's `api/images.ts` reads it
#[test]
fn a_file_is_its_content_type_and_bytes() {
    let file = FileData {
        content_type: "image/png".into(),
        data: vec![0x89, 0x50],
    };
    assert_eq!(
        serde_json::to_value(file).expect("json"),
        json!({"content_type": "image/png", "data": [0x89, 0x50]})
    );
}

/// The JSON answers and request bodies are the JSON itself, not wrapped
#[test]
fn json_answers_and_bodies_are_transparent() {
    let value = json!({"projects": [], "n": 1});
    assert_eq!(
        serde_json::to_value(Json(value.clone())).expect("json"),
        value
    );
    assert_eq!(
        serde_json::from_value::<ReviewRequest>(value.clone()).expect("a body"),
        ReviewRequest(value)
    );
}

/// ATM-768: a reload's tag is what its answer depends on, hashed as atm serve's `ETag`; the body is worked out only
/// when the page's tag isn't it
#[test]
fn a_reload_answers_its_body_unless_the_page_has_it() {
    let parts = json!(["claims", [1, 2], []]);
    let worked = std::cell::Cell::new(0);
    let body = || {
        worked.set(worked.get() + 1);
        Ok(json!({"claims": []}))
    };
    // None known: the body, and its tag (16 hex digits, quoted)
    let first = Fresh::of(None, &parts, body).expect("an answer");
    assert_eq!(first.body, Some(Json(json!({"claims": []}))));
    let digest = atm_common::sha1::sha1_hex(atm_common::py::to_json(&parts).as_bytes());
    assert_eq!(first.tag, format!("\"{}\"", &digest[..16]));
    // Its tag known: no body, and it wasn't worked out
    let again = Fresh::of(Some(&first.tag), &parts, body).expect("an answer");
    assert_eq!((again.tag.as_str(), again.body), (first.tag.as_str(), None));
    assert_eq!(worked.get(), 1);
    // A stale tag, or one unquoted: the body
    for stale in ["\"0000000000000000\"", first.tag.trim_matches('"'), ""] {
        assert!(
            Fresh::of(Some(stale), &parts, body)
                .expect("an answer")
                .body
                .is_some(),
            "{stale}"
        );
    }
    // Other parts, another tag; the same parts, the same tag
    assert_ne!(
        Fresh::of(None, &json!(["claims", [1, 3], []]), body)
            .expect("an answer")
            .tag,
        first.tag
    );
    assert_eq!(
        Fresh::of(None, &parts, body).expect("an answer").tag,
        first.tag
    );
    // The body's error is the answer's, unless the page has it
    let failing = || Err(AppError::NotFound("gone".into()));
    assert!(matches!(
        Fresh::of(None, &parts, failing),
        Err(AppError::NotFound(_))
    ));
    assert!(Fresh::of(Some(&first.tag), &parts, failing).is_ok());
    // As the page reads it: the tag, and the body or null
    assert_eq!(
        serde_json::to_value(Fresh::of(Some(&first.tag), &parts, body).expect("an answer"))
            .expect("json"),
        json!({"tag": first.tag, "body": null})
    );
}

#[test]
fn version_is_atms() {
    assert_eq!(version(), atm_common::VERSION);
}
