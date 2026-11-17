//! The app's commands (docs/guides/tauri.md): thin mappings to atm-lib's `Workspaces`, `Workspace` and `Member`, the
//! REST API's v2 routes as commands (ADR-003 §5), answering what those routes answer (without `generated_at`). Their
//! arguments and results are serde types, their errors one tagged enum the UI matches on. Live changes come through
//! one channel per subscription, from atm-lib's subscriptions to a workspace or a project (the stores', T-555). (In a
//! module of their own: Tauri's command macros can't be defined at a library's root.)

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use crate::badge::NeedsYou;
use crate::links::Links;
use crate::menu::Menus;
use crate::session::Session;
use atm_lib::query::Filters;
use atm_lib::review_requests;
use atm_lib::workspaces::{Member, Workspace};
use atm_lib::{OnChange, Subscription, Workspaces};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use specta::Type;
use tauri::ipc::Channel;
use tauri::{Runtime, State, WebviewWindow};

/// A live subscription: the window whose page made it, and atm-lib's subscription.
struct Live {
    window: String,
    /// Held only: it tells until it is dropped
    _subscription: Subscription,
}

/// What every command shares: the user's workspaces, opened when the app starts, the live subscriptions (each its
/// window's, T-511), the links for each window's page, and which window is in front.
pub struct AppState {
    workspaces: Workspaces,
    subscriptions: Mutex<HashMap<u32, Live>>,
    next: AtomicU32,
    /// The deep links the app was opened with (T-459), and each new window's first page
    links: Links,
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
    fn watch(
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
    fn unwatch(&self, window: &str, id: u32) -> bool {
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
enum JsonShape {
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

fn workspace(workspaces: &Workspaces, id: &str) -> Result<Arc<Workspace>, AppError> {
    workspaces
        .get(id)
        .ok_or_else(|| AppError::NotFound("unknown workspace".into()))
}

fn member(workspaces: &Workspaces, w: &str, p: &str) -> Result<Member, AppError> {
    Ok(workspace(workspaces, w)?.member(p)?)
}

/// Workspace `w`'s projects as `list_projects` lists them, without making it the one opened last (the badge and the
/// notifications read them, T-520).
///
/// # Errors
/// No such workspace.
pub fn listed_projects(workspaces: &Workspaces, w: &str) -> Result<Vec<Value>, AppError> {
    Ok(workspace(workspaces, w)?.listing().projects())
}

/// `GET /api/v2/workspaces`: the workspaces, how many projects each lists, and the one opened last.
#[must_use]
pub fn workspaces_of(workspaces: &Workspaces) -> Value {
    workspaces.summary()
}

/// `GET /api/v2/workspaces/<w>/projects`: its projects with their summaries, worktrees and logos; opening it records
/// it as the last opened.
///
/// # Errors
/// No such workspace.
pub fn projects_of(workspaces: &Workspaces, w: &str) -> Result<Value, AppError> {
    let found = workspace(workspaces, w)?;
    let _ = workspaces.opened(w);
    Ok(json!({"projects": found.listing().projects()}))
}

/// `GET /api/v2/workspaces/<w>/search?q=`.
///
/// # Errors
/// No such workspace.
pub fn search_in(workspaces: &Workspaces, w: &str, q: &str) -> Result<Value, AppError> {
    Ok(json!(workspace(workspaces, w)?.search(q)))
}

/// `GET …/projects/<p>/tasks`.
///
/// # Errors
/// No such workspace or project, task data too large, a filter naming no such sprint or release.
pub fn tasks_of(
    workspaces: &Workspaces,
    w: &str,
    p: &str,
    filters: TaskFilters,
) -> Result<Value, AppError> {
    Ok(member(workspaces, w, p)?.tasks(&filters.into())?)
}

/// `GET …/projects/<p>/claims`.
///
/// # Errors
/// No such workspace or project.
pub fn claims_of(workspaces: &Workspaces, w: &str, p: &str) -> Result<Value, AppError> {
    Ok(json!({"claims": member(workspaces, w, p)?.claims()}))
}

/// `GET …/projects/<p>/worktrees`.
///
/// # Errors
/// No such workspace or project.
pub fn worktrees_of(workspaces: &Workspaces, w: &str, p: &str) -> Result<Value, AppError> {
    Ok(json!({"worktrees": member(workspaces, w, p)?.worktrees()}))
}

/// `GET …/projects/<p>/documents?path=` (`&at=`: as a commit or tree has it, a review round's side).
///
/// # Errors
/// No such workspace or project; a path atm refuses or can't find; a revision that isn't one.
pub fn document_of(
    workspaces: &Workspaces,
    w: &str,
    p: &str,
    path: &str,
    at: Option<&str>,
) -> Result<Value, AppError> {
    let member = member(workspaces, w, p)?;
    Ok(match at.filter(|a| !a.is_empty()) {
        Some(revision) => member.document_at(path, revision)?,
        None => member.document(path)?,
    })
}

/// `GET …/projects/<p>/reviews`: its reviews the filters keep, which need the user.
///
/// # Errors
/// No such workspace or project; a status that isn't one; reviews that can't be read.
pub fn reviews_of(
    workspaces: &Workspaces,
    w: &str,
    p: &str,
    filters: ReviewFilters,
) -> Result<Value, AppError> {
    Ok(member(workspaces, w, p)?.reviews(&filters.into())?)
}

/// `GET …/projects/<p>/reviews/<CR>?round=&against=`: a review's page for one view.
///
/// # Errors
/// No such workspace, project, review or round; a round's tree that can't be had here.
pub fn review_of(
    workspaces: &Workspaces,
    w: &str,
    p: &str,
    review: &str,
    round: Option<u32>,
    against: Option<u32>,
) -> Result<Value, AppError> {
    Ok(member(workspaces, w, p)?.review(&review_ref(review), round, against)?)
}

/// `GET …/projects/<p>/files?path=`.
///
/// # Errors
/// As [`document_of`], and a file that isn't an image.
pub fn file_of(
    workspaces: &Workspaces,
    w: &str,
    p: &str,
    path: &str,
) -> Result<FileData, AppError> {
    let (data, content_type) = member(workspaces, w, p)?.image(path)?;
    Ok(FileData {
        content_type: content_type.into(),
        data,
    })
}

/// `GET …/projects/<p>/logo`: none without one.
///
/// # Errors
/// No such workspace or project; a logo that can't be read.
pub fn logo_of(workspaces: &Workspaces, w: &str, p: &str) -> Result<Option<FileData>, AppError> {
    let Some((file, content_type)) = member(workspaces, w, p)?.logo() else {
        return Ok(None);
    };
    let data = std::fs::read(&file).map_err(|e| AppError::Failed(e.to_string()))?;
    Ok(Some(FileData {
        content_type: content_type.into(),
        data,
    }))
}

/// A review request's body, as the page sends it and the REST route takes it (ATM-572; atm-lib's `review_requests`
/// reads it, refusing what doesn't fit).
#[derive(Debug, Clone, PartialEq, Deserialize, Type)]
#[serde(transparent)]
pub struct ReviewRequest(#[specta(type = JsonShape)] pub Value);

/// Project `p` of workspace `w`, opened to act as the user (never an agent's identity).
fn as_user(workspaces: &Workspaces, w: &str, p: &str) -> Result<atm_lib::Project, AppError> {
    Ok(member(workspaces, w, p)?.project_as_user()?)
}

fn header_json(header: &atm_lib::reviews::ReviewHeader) -> Result<Value, AppError> {
    serde_json::to_value(header).map_err(|e| AppError::Failed(e.to_string()))
}

fn review_ref(review: &str) -> atm_lib::reviews::ReviewRef {
    atm_lib::reviews::ReviewRef(review.to_string())
}

fn comment_id(comment: &str) -> atm_lib::reviews::CommentId {
    atm_lib::reviews::CommentId(comment.to_string())
}

// The app's review commands (docs/code-review-design.md §3.12, ATM-572): each the REST route of the same name, the same
// atm-lib call as its `atm review …` command, acting as the user. Only ever on the user's action in the page (a new
// exception to I27, beside connecting agents and installing an update); CompMD's frames, which show the comments,
// still call nothing (I25).

/// `POST …/reviews`: a review opened (or a draft, or a live review).
///
/// # Errors
/// No such workspace or project; a body that isn't one; what the review's rules refuse.
#[tauri::command]
#[specta::specta]
pub async fn create_review(
    state: State<'_, AppState>,
    workspace: String,
    project: String,
    opening: ReviewRequest,
) -> Result<Json, AppError> {
    let mut project = as_user(&state.workspaces, &workspace, &project)?;
    let opening = review_requests::request_from(opening.0)?;
    header_json(&review_requests::open_review(&mut project, opening)?).map(Json)
}

/// `PATCH …/reviews/<CR>`: the review closed.
///
/// # Errors
/// As [`create_review`].
#[tauri::command]
#[specta::specta]
pub async fn close_review(
    state: State<'_, AppState>,
    workspace: String,
    project: String,
    review: String,
    closing: ReviewRequest,
) -> Result<Json, AppError> {
    let mut project = as_user(&state.workspaces, &workspace, &project)?;
    let closing = review_requests::request_from(closing.0)?;
    header_json(&review_requests::close_review(
        &mut project,
        &review_ref(&review),
        &closing,
    )?)
    .map(Json)
}

/// `PUT …/reviews/<CR>/tasks`: the tasks it reviews, as a whole.
///
/// # Errors
/// As [`create_review`].
#[tauri::command]
#[specta::specta]
pub async fn set_review_tasks(
    state: State<'_, AppState>,
    workspace: String,
    project: String,
    review: String,
    tasks: Vec<String>,
) -> Result<Json, AppError> {
    let mut project = as_user(&state.workspaces, &workspace, &project)?;
    let wanted = review_requests::ReviewTasks { tasks };
    header_json(&review_requests::set_review_tasks(
        &mut project,
        &review_ref(&review),
        &wanted,
    )?)
    .map(Json)
}

/// `POST …/reviews/<CR>/decisions`: a reviewer's decision.
///
/// # Errors
/// As [`create_review`].
#[tauri::command]
#[specta::specta]
pub async fn submit_review_decision(
    state: State<'_, AppState>,
    workspace: String,
    project: String,
    review: String,
    decision: ReviewRequest,
) -> Result<Json, AppError> {
    let mut project = as_user(&state.workspaces, &workspace, &project)?;
    let decision = review_requests::request_from(decision.0)?;
    header_json(&review_requests::submit_decision(
        &mut project,
        &review_ref(&review),
        &decision,
    )?)
    .map(Json)
}

/// `POST …/reviews/<CR>/comments`: a new chain.
///
/// # Errors
/// As [`create_review`].
#[tauri::command]
#[specta::specta]
pub async fn add_review_comment(
    state: State<'_, AppState>,
    workspace: String,
    project: String,
    review: String,
    comment: ReviewRequest,
) -> Result<Json, AppError> {
    let project = as_user(&state.workspaces, &workspace, &project)?;
    let comment = review_requests::request_from(comment.0)?;
    let id = review_requests::add_comment(&project, &review_ref(&review), comment)?;
    Ok(Json(json!({ "review": review, "comment": id })))
}

/// `PATCH …/reviews/<CR>/comments/<c>`: a comment's text (one's own), or its resolved box.
///
/// # Errors
/// As [`create_review`].
#[tauri::command]
#[specta::specta]
pub async fn update_review_comment(
    state: State<'_, AppState>,
    workspace: String,
    project: String,
    review: String,
    comment: String,
    update: ReviewRequest,
) -> Result<Json, AppError> {
    let project = as_user(&state.workspaces, &workspace, &project)?;
    let update = review_requests::request_from(update.0)?;
    review_requests::update_comment(
        &project,
        &review_ref(&review),
        &comment_id(&comment),
        update,
    )?;
    Ok(Json(json!({ "review": review, "comment": comment })))
}

/// `POST …/reviews/<CR>/comments/<c>/replies`: a reply.
///
/// # Errors
/// As [`create_review`].
#[tauri::command]
#[specta::specta]
pub async fn reply_to_review_comment(
    state: State<'_, AppState>,
    workspace: String,
    project: String,
    review: String,
    comment: String,
    body: String,
) -> Result<Json, AppError> {
    let project = as_user(&state.workspaces, &workspace, &project)?;
    let reply = review_requests::NewReply { body };
    let id = review_requests::reply_to_comment(
        &project,
        &review_ref(&review),
        &comment_id(&comment),
        &reply,
    )?;
    Ok(Json(
        json!({ "review": review, "comment": id, "reply_to": comment }),
    ))
}

/// atm's version.
#[tauri::command]
#[specta::specta]
#[must_use]
pub fn version() -> String {
    atm_common::VERSION.to_string()
}

/// The workspaces, how many projects each lists, and the one opened last.
///
/// # Errors
/// None: a `Result`, as every async command with state is.
#[tauri::command]
#[specta::specta]
pub async fn list_workspaces(state: State<'_, AppState>) -> Result<Json, AppError> {
    Ok(Json(workspaces_of(&state.workspaces)))
}

/// A workspace's projects, with their summaries; it becomes the last opened.
///
/// # Errors
/// No such workspace.
#[tauri::command]
#[specta::specta]
pub async fn list_projects(
    state: State<'_, AppState>,
    workspace: String,
) -> Result<Json, AppError> {
    projects_of(&state.workspaces, &workspace).map(Json)
}

/// Search a workspace's projects' tasks.
///
/// # Errors
/// No such workspace.
#[tauri::command]
#[specta::specta]
pub async fn search(
    state: State<'_, AppState>,
    workspace: String,
    q: String,
) -> Result<Json, AppError> {
    search_in(&state.workspaces, &workspace, &q).map(Json)
}

/// A project's tasks, with what atm derives for them.
///
/// # Errors
/// As [`tasks_of`].
#[tauri::command]
#[specta::specta]
pub async fn project_tasks(
    state: State<'_, AppState>,
    workspace: String,
    project: String,
    filters: TaskFilters,
) -> Result<Json, AppError> {
    tasks_of(&state.workspaces, &workspace, &project, filters).map(Json)
}

/// A project's live claims.
///
/// # Errors
/// As [`claims_of`].
#[tauri::command]
#[specta::specta]
pub async fn claims(
    state: State<'_, AppState>,
    workspace: String,
    project: String,
) -> Result<Json, AppError> {
    claims_of(&state.workspaces, &workspace, &project).map(Json)
}

/// A project's working copies.
///
/// # Errors
/// As [`worktrees_of`].
#[tauri::command]
#[specta::specta]
pub async fn worktrees(
    state: State<'_, AppState>,
    workspace: String,
    project: String,
) -> Result<Json, AppError> {
    worktrees_of(&state.workspaces, &workspace, &project).map(Json)
}

/// A document of a project, as the viewer shows it: from the working copy, or `at` a commit or tree.
///
/// # Errors
/// As [`document_of`].
#[tauri::command]
#[specta::specta]
pub async fn document(
    state: State<'_, AppState>,
    workspace: String,
    project: String,
    path: String,
    at: Option<String>,
) -> Result<Json, AppError> {
    document_of(
        &state.workspaces,
        &workspace,
        &project,
        &path,
        at.as_deref(),
    )
    .map(Json)
}

/// A project's reviews, for the reviews page.
///
/// # Errors
/// As [`reviews_of`].
#[tauri::command]
#[specta::specta]
pub async fn project_reviews(
    state: State<'_, AppState>,
    workspace: String,
    project: String,
    filters: ReviewFilters,
) -> Result<Json, AppError> {
    reviews_of(&state.workspaces, &workspace, &project, filters).map(Json)
}

/// A review's page, for one view of it.
///
/// # Errors
/// As [`review_of`].
#[tauri::command]
#[specta::specta]
pub async fn project_review(
    state: State<'_, AppState>,
    workspace: String,
    project: String,
    review: String,
    round: Option<u32>,
    against: Option<u32>,
) -> Result<Json, AppError> {
    review_of(
        &state.workspaces,
        &workspace,
        &project,
        &review,
        round,
        against,
    )
    .map(Json)
}

/// An image of a project.
///
/// # Errors
/// As [`file_of`].
#[tauri::command]
#[specta::specta]
pub async fn file(
    state: State<'_, AppState>,
    workspace: String,
    project: String,
    path: String,
) -> Result<FileData, AppError> {
    file_of(&state.workspaces, &workspace, &project, &path)
}

/// A project's logo, if it has one.
///
/// # Errors
/// As [`logo_of`].
#[tauri::command]
#[specta::specta]
pub async fn logo(
    state: State<'_, AppState>,
    workspace: String,
    project: String,
) -> Result<Option<FileData>, AppError> {
    logo_of(&state.workspaces, &workspace, &project)
}

/// Be told on `channel` when a workspace's list or any of its projects changes. The subscription's ID.
///
/// # Errors
/// No such workspace; a list or project that can't be watched.
#[tauri::command]
#[specta::specta]
pub async fn subscribe_workspace<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
    workspace: String,
    channel: Channel<Changed>,
) -> Result<u32, AppError> {
    let found = self::workspace(&state.workspaces, &workspace)?;
    let message = Changed {
        workspace,
        project: None,
    };
    state.watch(window.label(), |c| found.subscribe(c), channel, message)
}

/// Be told on `channel` when a project's data changes. The subscription's ID.
///
/// # Errors
/// No such workspace or project; a project that can't be watched.
#[tauri::command]
#[specta::specta]
pub async fn subscribe_project<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
    workspace: String,
    project: String,
    channel: Channel<Changed>,
) -> Result<u32, AppError> {
    let found = member(&state.workspaces, &workspace, &project)?;
    let message = Changed {
        workspace,
        project: Some(project),
    };
    state.watch(window.label(), |c| found.subscribe(c), channel, message)
}

/// `atm agents status`: per harness atm knows, whether it is installed here, and each of atm's skills' state in the
/// user's own folder for it (linked, copied, missing, stale, not atm's).
///
/// # Errors
/// None: a `Result`, as every async command is.
#[tauri::command]
#[specta::specta]
pub async fn agents_status() -> Result<Json, AppError> {
    Ok(Json(json!(atm_lib::agents::skills_status(
        None,
        atm_common::VERSION
    ))))
}

/// `atm agents install --harness <harness>` (T-458): this version's skills where the harness reads the user's, the
/// same atm-lib code as the command; `{written: [{harness, skill, path, how, action}], refused: [{path, reason}]}`. Only
/// ever run by the user's click on the app's button: nothing calls it on its own.
///
/// # Errors
/// A harness atm doesn't know; a folder or file that can't be written.
#[tauri::command]
#[specta::specta]
pub async fn connect_agents(harness: String) -> Result<Json, AppError> {
    let installed =
        atm_lib::agents::install_skills(Some(&harness), None, false, atm_common::VERSION)?;
    Ok(Json(
        json!({"written": installed.written, "refused": installed.refused}),
    ))
}

/// Be told on `channel` the dashboard address of each `atm://` link the app opens in the calling window (T-459,
/// T-511); the answer is the one waiting for it, if any (a link the app was started with, a new window's first page).
///
/// # Errors
/// None: a `Result`, as every async command with state is.
#[tauri::command]
#[specta::specta]
pub async fn subscribe_links<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
    channel: Channel<String>,
) -> Result<Option<String>, AppError> {
    Ok(state.links.subscribe(window.label(), channel))
}

/// End one of the calling window's subscriptions. Whether there was one.
///
/// # Errors
/// None: a `Result`, as every async command with state is.
#[tauri::command]
#[specta::specta]
pub async fn unsubscribe<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
    id: u32,
) -> Result<bool, AppError> {
    Ok(state.unwatch(window.label(), id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn a_subscription_says_when_a_watched_folder_changes_and_ends_with_unsubscribe() {
        let dir = std::env::temp_dir().join(format!("atm-app-watch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("workspaces")).expect("a folder");
        let state = AppState::new(Workspaces::at(&dir));
        let (sent, received) = std::sync::mpsc::channel();
        let channel = Channel::new(move |body| {
            let _ = sent.send(body);
            Ok(())
        });
        let message = Changed {
            workspace: "work".into(),
            project: Some("abcd1234".into()),
        };
        // The default workspace's: its list, in the state folder
        let workspace = state.workspaces.default_workspace();
        let id = state
            .watch("main", |c| workspace.subscribe(c), channel, message)
            .expect("watching");
        std::thread::sleep(Duration::from_millis(200));
        std::fs::write(dir.join("workspaces").join("default.json"), "{}").expect("a change");
        let body = received
            .recv_timeout(Duration::from_secs(10))
            .expect("told of the change");
        let tauri::ipc::InvokeResponseBody::Json(text) = body else {
            panic!("a JSON message");
        };
        assert_eq!(
            serde_json::from_str::<Value>(&text).expect("json"),
            json!({"workspace": "work", "project": "abcd1234"})
        );
        // Only its own window ends it
        assert!(!state.unwatch("window-1", id));
        assert!(state.unwatch("main", id) && !state.unwatch("main", id));
        std::fs::remove_dir_all(&dir).expect("clean up");
    }

    /// T-506, T-511: a page that reloads, or a window that closes, leaves its subscriptions behind; they end, and
    /// another window's stay
    #[test]
    fn a_window_loading_again_or_closing_ends_its_own_subscriptions() {
        let dir = std::env::temp_dir().join(format!("atm-app-unwatch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a folder");
        let state = AppState::new(Workspaces::at(&dir));
        let message = Changed {
            workspace: "default".into(),
            project: None,
        };
        let workspace = state.workspaces.default_workspace();
        let subscribe = |window: &str| {
            let channel = Channel::new(|_| Ok(()));
            state
                .watch(window, |c| workspace.subscribe(c), channel, message.clone())
                .expect("watching")
        };
        let mains = [subscribe("main"), subscribe("main")];
        let other = subscribe("window-1");
        assert_eq!(state.unwatch_window("main"), 2);
        assert!(mains.iter().all(|id| !state.unwatch("main", *id)));
        assert_eq!(state.unwatch_window("main"), 0);
        // The other window's is still there, until it closes
        state.focused("window-1");
        assert_eq!(state.front().as_deref(), Some("window-1"));
        assert_eq!(state.window_closed("window-1"), 1);
        assert!(!state.unwatch("window-1", other));
        assert_eq!(state.front(), None);
        std::fs::remove_dir_all(&dir).expect("clean up");
    }

    #[test]
    fn new_windows_are_numbered_and_never_reuse_a_label() {
        let state = AppState::new(Workspaces::at(&std::env::temp_dir()));
        assert_eq!(state.new_window_label(), "window-1");
        assert_eq!(state.new_window_label(), "window-2");
        state.focused("main");
        state.window_closed("window-2");
        assert_eq!(state.front().as_deref(), Some("main"));
    }
}
