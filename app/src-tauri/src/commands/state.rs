//! What every command shares (the app's state), their one error type, the filters and answers they take and give.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use crate::badge::NeedsYou;
use crate::links::Links;
use crate::menu::Menus;
use crate::session::Session;
use atm_lib::query::Filters;
use atm_lib::{OnChange, Subscription, Workspaces};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use specta::Type;
use tauri::ipc::Channel;

/// A live subscription: the window whose page made it, and atm-lib's subscription.
struct Live {
    window: String,
    /// Held only: it tells until it is dropped
    _subscription: Subscription,
}

/// What every command shares: the user's workspaces, opened when the app starts, the live subscriptions (each its
/// window's, T-511), the links for each window's page, and which window is in front.
pub struct AppState {
    pub(super) workspaces: Workspaces,
    subscriptions: Mutex<HashMap<u32, Live>>,
    next: AtomicU32,
    /// The deep links the app was opened with (T-459), and each new window's first page
    pub(super) links: Links,
    /// The window focused last
    front: Mutex<Option<String>>,
    /// The next `window-<n>`'s number
    next_window: AtomicU32,
    /// Each window's page's menu channel and what its menu shows (T-512)
    menus: Menus,
    /// The windows' frames and tabs, kept for the next start (T-518)
    session: Session,
    /// The needs-you counts seen last, for the notifications (T-520)
    needs_you: NeedsYou,
}

impl AppState {
    /// The app's state on `workspaces`.
    #[must_use]
    pub fn new(workspaces: Workspaces) -> Self {
        Self {
            workspaces,
            subscriptions: Mutex::new(HashMap::new()),
            next: AtomicU32::new(1),
            links: Links::default(),
            front: Mutex::new(None),
            next_window: AtomicU32::new(1),
            menus: Menus::default(),
            session: Session::default(),
            needs_you: NeedsYou::default(),
        }
    }

    /// The needs-you counts seen last.
    #[must_use]
    pub fn needs_you(&self) -> &NeedsYou {
        &self.needs_you
    }

    /// The windows as they are, kept for the next start.
    #[must_use]
    pub fn session(&self) -> &Session {
        &self.session
    }

    /// Each window's menu channel and state.
    #[must_use]
    pub fn menus(&self) -> &Menus {
        &self.menus
    }

    /// A label for a new window, `window-<n>`, never one used before.
    #[must_use]
    pub fn new_window_label(&self) -> String {
        format!(
            "{}{}",
            crate::windows::PREFIX,
            self.next_window.fetch_add(1, Ordering::Relaxed)
        )
    }

    /// The window focused last, if it is still open.
    #[must_use]
    pub fn front(&self) -> Option<String> {
        self.front
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// `window` came to the front.
    pub fn focused(&self, window: &str) {
        *self
            .front
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(window.to_owned());
    }

    /// `window` closed: its subscriptions end, its links go nowhere, and it is no longer in front. How many
    /// subscriptions ended.
    pub fn window_closed(&self, window: &str) -> usize {
        self.links.forget(window);
        self.menus.forget(window);
        self.needs_you().forget(window);
        let mut front = self
            .front
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if front.as_deref() == Some(window) {
            *front = None;
        }
        drop(front);
        self.unwatch_window(window)
    }

    /// Its deep links.
    #[must_use]
    pub fn links(&self) -> &Links {
        &self.links
    }

    /// Its workspaces.
    #[must_use]
    pub fn workspaces(&self) -> &Workspaces {
        &self.workspaces
    }

    /// Subscribe for `window`'s page with `subscribe` (atm-lib's, a workspace's or a project's), sending `message` on
    /// `channel` on each change it tells (on the store's thread). The subscription's ID, for `unsubscribe`.
    pub(super) fn watch(
        &self,
        window: &str,
        subscribe: impl FnOnce(OnChange) -> atm_common::Result<Subscription>,
        channel: Channel<Changed>,
        message: Changed,
    ) -> Result<u32, AppError> {
        let subscription = subscribe(Arc::new(move || {
            let _ = channel.send(message.clone());
        }))?;
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        self.subscriptions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(
                id,
                Live {
                    window: window.to_owned(),
                    _subscription: subscription,
                },
            );
        Ok(id)
    }

    /// End `window`'s subscription `id` (it stops when dropped); another window's stays. Whether there was one.
    pub(super) fn unwatch(&self, window: &str, id: u32) -> bool {
        let mut subscriptions = self
            .subscriptions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if subscriptions.get(&id).is_some_and(|s| s.window == window) {
            subscriptions.remove(&id).is_some()
        } else {
            false
        }
    }

    /// End every subscription of `window`: its page is loading again (a reload, ⌘R) or the window closed, and the page
    /// that made them is gone without saying so (a channel to it doesn't fail, its messages only find no callback).
    /// Other windows' stay. How many ended.
    pub fn unwatch_window(&self, window: &str) -> usize {
        let mut subscriptions = self
            .subscriptions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let before = subscriptions.len();
        subscriptions.retain(|_, s| s.window != window);
        let ended = before - subscriptions.len();
        drop(subscriptions);
        ended
    }
}

/// A command's error, as the UI sees it: atm's error kind, with its message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(tag = "kind", content = "message", rename_all = "snake_case")]
pub enum AppError {
    /// No such workspace, task, sprint, release, component or project.
    NotFound(String),
    /// The request itself is wrong.
    Invalid(String),
    /// It exists, but atm won't hand it out.
    Refused(String),
    /// Larger than atm hands out.
    TooLarge(String),
    /// Another holds it: a task claimed by another agent (ATM-664).
    Conflict(String),
    /// Anything else atm refuses or can't do.
    Failed(String),
}

impl From<atm_common::Error> for AppError {
    fn from(error: atm_common::Error) -> Self {
        use atm_common::Error as E;
        match error {
            E::NotFound(m) => Self::NotFound(m),
            E::Invalid(m) => Self::Invalid(m),
            E::Refused(m) => Self::Refused(m),
            E::TooLarge(m) => Self::TooLarge(m),
            E::Conflict(m) => Self::Conflict(m),
            E::Failed { message, .. } => Self::Failed(message),
        }
    }
}

/// Which of a project's reviews are listed: the REST route's query parameters; what isn't given keeps everything.
#[derive(Debug, Clone, Default, Deserialize, Type)]
#[serde(default)]
pub struct ReviewFilters {
    /// Every word in the title, summary or comments.
    pub q: String,
    /// One of these statuses.
    pub status: Vec<String>,
    /// This author.
    pub author: Option<String>,
    /// This reviewer among them.
    pub reviewer: Option<String>,
    /// It reviews this task.
    pub task: Option<String>,
    /// On this branch.
    pub branch: Option<String>,
}

impl From<ReviewFilters> for atm_lib::review_pages::ReviewQuery {
    fn from(f: ReviewFilters) -> Self {
        Self {
            q: f.q,
            status: f.status,
            author: f.author,
            reviewer: f.reviewer,
            task: f.task,
            branch: f.branch,
        }
    }
}

/// What a project's tasks are filtered by: the REST route's query parameters (`atm task list`'s options); what isn't
/// given keeps everything.
#[derive(Debug, Clone, Default, Deserialize, Type)]
#[serde(default)]
#[allow(clippy::struct_excessive_bools)] // reason: each is one of `atm task list`'s flags
pub struct TaskFilters {
    /// In these sprints.
    pub sprint: Vec<String>,
    /// In no sprint.
    pub backlog: bool,
    /// In this release.
    pub release: Option<String>,
    /// In this component.
    pub component: Option<String>,
    /// With these statuses.
    pub status: Vec<String>,
    /// Of this type.
    #[serde(rename = "type")]
    pub type_: Option<String>,
    /// Of this priority.
    pub priority: Option<String>,
    /// With all these labels.
    pub label: Vec<String>,
    /// Worked on by this agent.
    pub agent: Option<String>,
    /// Ready to start.
    pub ready: bool,
    /// Waiting on a dependency.
    pub blocked: bool,
    /// In an open pull request.
    pub in_pr: bool,
    /// Done together with the user.
    pub with_user: bool,
    /// The archive's instead.
    pub archived: bool,
}

impl From<TaskFilters> for Filters {
    fn from(f: TaskFilters) -> Self {
        Self {
            sprint: f.sprint,
            backlog: f.backlog,
            release: f.release,
            component: f.component,
            status: f.status,
            type_: f.type_,
            priority: f.priority,
            label: f.label,
            agent: f.agent,
            ready: f.ready,
            blocked: f.blocked,
            in_pr: f.in_pr,
            with_user: f.with_user,
            archived: f.archived,
        }
    }
}

/// A file from a project (an image, a logo): its content type and bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct FileData {
    /// Its content type (`image/png`, …).
    pub content_type: String,
    /// Its bytes.
    pub data: Vec<u8>,
}

/// The shape of [`Json`] for TypeScript: any JSON value, its numbers JavaScript's (atm's counts and sizes fit).
#[derive(Serialize, Type)]
#[allow(dead_code)] // reason: only its type is used, for the generated bindings
#[serde(untagged)]
pub(super) enum JsonShape {
    Null(()),
    Bool(bool),
    Number(f64),
    Text(String),
    List(Vec<JsonShape>),
    Object(HashMap<String, JsonShape>),
}

/// A command's JSON answer: what the REST route answers, without `generated_at`.
#[derive(Debug, Clone, PartialEq, Serialize, Type)]
#[serde(transparent)]
pub struct Json(#[specta(type = JsonShape)] pub Value);

/// What a subscription's channel says: something it watches changed, so the UI reads it again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct Changed {
    /// The workspace.
    pub workspace: String,
    /// The project, for a project's subscription.
    pub project: Option<String>,
}
