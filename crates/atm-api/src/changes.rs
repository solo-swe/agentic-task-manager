//! The change streams (T-507; docs/multi-project-api.md §2.8): server-sent events (`text/event-stream`) that say when
//! a workspace (`/api/v2/workspaces/<w>/changes`) or one of its projects (`…/<w>/projects/<p>/changes`) changed, as
//! the app's subscriptions say it (docs/guides/tauri.md, Streams). Each stream watches the folders the app's
//! subscription would ([`atm_lib::watch`]) and sends one `changed` event per settled burst of changes, naming the
//! workspace and the project, never their data: the page reads them again through the read routes. At most
//! [`MAX_STREAMS`] are open at once; each sends a comment every [`KEEP_ALIVE`], and ends, its watch with it, when its
//! client goes.

use std::convert::Infallible;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll};
use std::time::Duration;

use atm_lib::watch::{Watch, watch, workspace_folders};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use futures_core::Stream;
use serde_json::json;
use tokio::sync::mpsc;

use super::routes::Projects;
use super::{Served, send_error};

/// How many change streams may be open at once, all clients together; one more is refused (503). A browser tab holds
/// one (the dashboard watches its workspace), so this is far more tabs than anyone keeps open, and few enough
/// watchers that a page opening streams in a loop can't exhaust the system's.
pub const MAX_STREAMS: usize = 16;

/// How often an open stream sends a comment: it shows the connection is alive, and finds a client that is gone.
pub const KEEP_ALIVE: Duration = Duration::from_secs(15);

/// The change streams open now.
#[derive(Default)]
pub struct Streams {
    open: Arc<AtomicUsize>,
}

impl Streams {
    /// A place for one more stream, if there is one: held while the stream is open.
    fn take(&self) -> Option<Slot> {
        self.open
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                (n < MAX_STREAMS).then_some(n + 1)
            })
            .ok()
            .map(|_| Slot(Arc::clone(&self.open)))
    }
}

/// One stream's place among the [`MAX_STREAMS`]: given back when it is dropped.
struct Slot(Arc<AtomicUsize>);

impl Drop for Slot {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// What a request for a change stream names: the workspace, and the project for a project's stream.
pub struct Target {
    workspace: String,
    project: Option<String>,
}

/// The change stream `path` names, if it names one: `/api/v2/workspaces/<w>/changes` or
/// `/api/v2/workspaces/<w>/projects/<p>/changes`.
#[must_use]
pub fn target(path: &str) -> Option<Target> {
    let rest = path.strip_prefix("/api/v2/workspaces/")?;
    let parts: Vec<&str> = rest.split('/').collect();
    match parts[..] {
        [w, "changes"] if !w.is_empty() => Some(Target {
            workspace: w.to_string(),
            project: None,
        }),
        [w, "projects", p, "changes"] if !w.is_empty() && !p.is_empty() => Some(Target {
            workspace: w.to_string(),
            project: Some(p.to_string()),
        }),
        _ => None,
    }
}

/// Why a stream wasn't opened: the status and the error's message.
type Refusal = (StatusCode, String);

/// The folders `target`'s stream watches; a 404 for no such workspace or project.
fn folders(projects: &Projects, target: &Target) -> Result<Vec<PathBuf>, Refusal> {
    let workspaces = projects.workspaces();
    let Some(workspace) = workspaces.get(&target.workspace) else {
        return Err((StatusCode::NOT_FOUND, "unknown workspace".into()));
    };
    match &target.project {
        None => Ok(workspace_folders(workspaces, &workspace)),
        Some(id) => workspace
            .member(id)
            .map(|member| member.watch_folders())
            .map_err(|e| (StatusCode::NOT_FOUND, e.to_string())),
    }
}

/// An open stream: told by its watch, it sends a `changed` event each time; dropped (its client gone), the watch
/// stops and its place is given back.
struct Changes {
    changed: mpsc::Receiver<()>,
    /// What each event says
    data: String,
    /// Whether the first comment, that it watches, was sent
    started: bool,
    _watch: Watch,
    _slot: Slot,
}

impl Stream for Changes {
    type Item = Result<Event, Infallible>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        if !this.started {
            this.started = true;
            return Poll::Ready(Some(Ok(Event::default().comment("watching"))));
        }
        this.changed
            .poll_recv(cx)
            .map(|told| told.map(|()| Ok(Event::default().event("changed").data(&this.data))))
    }
}

/// Open `target`'s change stream: 404 for no such workspace or project, 503 when [`MAX_STREAMS`] are open.
pub async fn open(served: Arc<Served>, target: Target) -> Response {
    let Some(slot) = served.streams.take() else {
        let mut refused = send_error(StatusCode::SERVICE_UNAVAILABLE, "too many change streams");
        refused
            .headers_mut()
            .insert(header::RETRY_AFTER, HeaderValue::from_static("30"));
        return refused;
    };
    // Finding the folders reads the workspace's and the project's files, and starting a watch is the system's: off the
    // async workers
    let started = tokio::task::spawn_blocking(move || {
        let folders = folders(&served.projects, &target)?;
        // A burst told while one is waiting is the same news: the channel holds one
        let (tell, changed) = mpsc::channel(1);
        let watching = watch(&folders, move || {
            let _ = tell.try_send(());
        })
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let data = json!({"workspace": target.workspace, "project": target.project}).to_string();
        Ok::<_, Refusal>((watching, changed, data))
    })
    .await;
    let (watching, changed, data) = match started {
        Ok(Ok(started)) => started,
        Ok(Err((status, message))) => return send_error(status, &message),
        Err(_) => return send_error(StatusCode::INTERNAL_SERVER_ERROR, "internal error"),
    };
    let stream = Changes {
        changed,
        data,
        started: false,
        _watch: watching,
        _slot: slot,
    };
    let mut response = Sse::new(stream)
        .keep_alive(KeepAlive::new().interval(KEEP_ALIVE))
        .into_response();
    // As every answer of atm's (send): never kept, never sniffed, never another site's
    let headers = response.headers_mut();
    for (name, value) in [
        (header::CACHE_CONTROL, "no-store"),
        (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        (
            header::HeaderName::from_static("cross-origin-resource-policy"),
            "same-origin",
        ),
    ] {
        headers.insert(name, HeaderValue::from_static(value));
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_change_stream_is_a_workspace_or_a_project_of_one() {
        let named = |path: &str| target(path).map(|t| (t.workspace, t.project));
        assert_eq!(
            named("/api/v2/workspaces/work/changes"),
            Some(("work".into(), None))
        );
        assert_eq!(
            named("/api/v2/workspaces/work/projects/abcd1234/changes"),
            Some(("work".into(), Some("abcd1234".into())))
        );
        for other in [
            "/api/v2/workspaces//changes",
            "/api/v2/workspaces/work/projects//changes",
            "/api/v2/workspaces/work/projects",
            "/api/v2/workspaces/work/changes/more",
            "/api/changes",
        ] {
            assert!(named(other).is_none(), "{other}");
        }
    }

    #[test]
    fn at_most_max_streams_are_open_and_a_closed_one_makes_room() {
        let streams = Streams::default();
        let mut slots: Vec<Slot> = (0..MAX_STREAMS)
            .map(|_| streams.take().expect("room"))
            .collect();
        assert!(streams.take().is_none());
        slots.pop();
        assert!(streams.take().is_some());
    }
}
