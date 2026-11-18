//! The routes' unit tests' fake atm-lib (AGENTS.md, Testing conventions): it answers what a test sets, and records
//! each call a route makes, so a test checks the route's own work (the call, the status and body it maps each answer
//! to, its `ETag`s and headers) and nothing below it.

use std::sync::{Arc, Mutex};

use atm_common::{Error, Result};
use atm_lib::query::Filters;
use atm_lib::review_pages::ReviewQuery;
use atm_lib::review_requests::{
    CommentUpdate, Decision, NewComment, NewReply, ReviewClosing, ReviewOpening, ReviewTasks,
};
use atm_lib::reviews::{CommentId, ReviewHeader, ReviewId, ReviewRef, Status};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::Response;
use serde_json::{Map, Value, json};

use super::Projects;
use super::source::{ProjectList, ProjectSource, ReviewChanges, Source, WorkspaceSource};

/// One call a route made of atm-lib.
#[derive(Debug, Clone)]
#[allow(dead_code)] // reason: the fields are compared through Debug (PartialEq below), which dead-code analysis ignores
pub(super) enum Call {
    Opened(String),
    ListedProjects(String),
    Searched(String, String),
    Project(String, String),
    Tasks(Filters),
    OthersVersion,
    Claims,
    Worktrees,
    ReadLogo,
    FileVersion(String),
    Head,
    Document(String),
    Image(String),
    DocumentAt(String, String),
    Reviews(ReviewQuery),
    Review(ReviewRef, Option<u32>, Option<u32>),
    AsUser,
    OpenReview(ReviewOpening),
    CloseReview(ReviewRef, ReviewClosing),
    SetTasks(ReviewRef, ReviewTasks),
    Decide(ReviewRef, Decision),
    Comment(ReviewRef, NewComment),
    UpdateComment(ReviewRef, CommentId, CommentUpdate),
    Reply(ReviewRef, CommentId, NewReply),
}

impl PartialEq for Call {
    /// As they print: `Filters` has no `PartialEq` of its own.
    fn eq(&self, other: &Self) -> bool {
        format!("{self:?}") == format!("{other:?}")
    }
}

/// The calls made so far, in order.
#[derive(Clone, Default)]
pub(super) struct Calls(Arc<Mutex<Vec<Call>>>);

impl Calls {
    fn note(&self, call: Call) {
        self.0.lock().expect("the calls").push(call);
    }

    /// The calls made since the last time this was asked.
    pub fn taken(&self) -> Vec<Call> {
        std::mem::take(&mut *self.0.lock().expect("the calls"))
    }
}

/// A file of a fake project: its version, and what reading it as a document and as an image answer.
#[derive(Clone)]
pub(super) struct FakeFile {
    pub version: (u64, u128),
    pub document: Result<Value>,
    pub image: Result<(Vec<u8>, &'static str)>,
}

/// What a fake project's reviews answer.
#[derive(Clone)]
pub(super) struct FakeReviews {
    pub header: Result<ReviewHeader>,
    pub comment: Result<CommentId>,
    pub updated: Result<()>,
}

/// A review's header, `CR-1`, open.
pub(super) fn header() -> ReviewHeader {
    ReviewHeader {
        id: ReviewId("r1".into()),
        reference: ReviewRef("CR-1".into()),
        number: 1,
        title: "Look".into(),
        status: Status::Waiting,
        tasks: Vec::new(),
        author: None,
        reviewers: Vec::new(),
        created: "2026-11-17T10:00:00Z".into(),
        updated: "2026-11-17T10:00:00Z".into(),
    }
}

impl Default for FakeReviews {
    fn default() -> Self {
        Self {
            header: Ok(header()),
            comment: Ok(CommentId("c1".into())),
            updated: Ok(()),
        }
    }
}

/// A fake project.
#[derive(Clone)]
pub(super) struct FakeProject {
    pub stamp: Value,
    pub tasks: Result<Value>,
    pub others: Value,
    pub claims: Vec<Value>,
    pub worktrees: Vec<Value>,
    /// Its logo's version, content type and bytes
    pub logo: Option<(Value, &'static str, Vec<u8>)>,
    /// Its files by path; any other path is `NotFound`
    pub files: Vec<(String, Result<FakeFile>)>,
    pub head: Option<String>,
    /// What its reviews answer; an error refuses to open the project as the user
    pub reviews: Result<FakeReviews>,
    /// A file at a revision: what any is
    pub at_revision: Result<Value>,
    /// What listing its reviews answers
    pub listed: Result<Value>,
    /// What a review's page answers
    pub page: Result<Value>,
}

impl Default for FakeProject {
    fn default() -> Self {
        Self {
            stamp: json!({"tasks": [1, 1], "archive": null, "claims": [1, 1], "copies": [1]}),
            tasks: Ok(json!({"tasks": [], "meta": {}})),
            others: json!({}),
            claims: Vec::new(),
            worktrees: Vec::new(),
            logo: None,
            files: Vec::new(),
            head: Some("aaaa".into()),
            reviews: Ok(FakeReviews::default()),
            at_revision: Ok(json!({"path": "a.md", "revision": "t1"})),
            listed: Ok(json!({"reviews": [], "you": "Ana"})),
            page: Ok(json!({"review": {"reference": "CR-1"}, "view": {"files": []}})),
        }
    }
}

/// A fake workspace.
#[derive(Clone, Default)]
pub(super) struct FakeWorkspace {
    pub stamp: Value,
    pub projects: Vec<Value>,
    pub search: Map<String, Value>,
    /// Its projects by ID; any other is `NotFound("unknown project")`
    pub members: Vec<(String, Result<FakeProject>)>,
}

/// A fake atm-lib: its workspaces by ID (`default` among them), and the calls made of it.
#[derive(Clone, Default)]
pub(super) struct Fake {
    pub summary: Value,
    pub workspaces: Vec<(String, FakeWorkspace)>,
    pub calls: Calls,
}

impl Fake {
    /// `default` and `work`, each listing `project` as `p1`.
    #[allow(clippy::needless_pass_by_value)] // reason: the tests build the project inline
    pub fn with(project: FakeProject) -> Self {
        let workspace = |id: &str| FakeWorkspace {
            stamp: json!([id]),
            projects: vec![json!({"id": "p1", "in": id})],
            members: vec![("p1".into(), Ok(project.clone()))],
            ..FakeWorkspace::default()
        };
        Self {
            summary: json!({"workspaces": [{"id": "default", "projects": 1}], "recent": "default"}),
            workspaces: vec![
                ("default".into(), workspace("default")),
                ("work".into(), workspace("work")),
            ],
            calls: Calls::default(),
        }
    }

    /// The server's routes on this fake; and its calls, to look at.
    pub fn projects(self) -> (Projects, Calls) {
        let calls = self.calls.clone();
        (Projects::of(self), calls)
    }
}

/// `fake` as a route gets it, its calls recorded from here on.
pub(super) fn bound(fake: FakeProject) -> (Box<dyn ProjectSource>, Calls) {
    let calls = Calls::default();
    let project = Box::new(Bound {
        fake,
        id: "p1".into(),
        calls: calls.clone(),
    });
    (project, calls)
}

/// A fake, with the calls it records.
struct Bound<T> {
    fake: T,
    id: String,
    calls: Calls,
}

impl Source for Fake {
    fn summary(&self) -> Value {
        self.summary.clone()
    }

    fn workspace(&self, id: &str) -> Option<Box<dyn WorkspaceSource>> {
        self.workspaces
            .iter()
            .find(|(w, _)| w == id)
            .map(|(w, fake)| {
                Box::new(Bound {
                    fake: fake.clone(),
                    id: w.clone(),
                    calls: self.calls.clone(),
                }) as Box<dyn WorkspaceSource>
            })
    }

    fn default_workspace(&self) -> Box<dyn WorkspaceSource> {
        self.workspace("default").expect("the fake has default")
    }

    fn opened(&self, id: &str) {
        self.calls.note(Call::Opened(id.into()));
    }
}

impl WorkspaceSource for Bound<FakeWorkspace> {
    fn id(&self) -> String {
        self.id.clone()
    }

    fn projects(&self) -> ProjectList<'_> {
        ProjectList {
            stamp: self.fake.stamp.clone(),
            projects: Box::new(|| {
                self.calls.note(Call::ListedProjects(self.id.clone()));
                self.fake.projects.clone()
            }),
        }
    }

    fn search(&self, query: &str) -> Map<String, Value> {
        self.calls
            .note(Call::Searched(self.id.clone(), query.into()));
        self.fake.search.clone()
    }

    fn project(&self, id: &str) -> Result<Box<dyn ProjectSource>> {
        self.calls.note(Call::Project(self.id.clone(), id.into()));
        let (_, found) = self
            .fake
            .members
            .iter()
            .find(|(p, _)| p == id)
            .ok_or_else(|| Error::NotFound("unknown project".into()))?;
        Ok(Box::new(Bound {
            fake: found.clone()?,
            id: id.into(),
            calls: self.calls.clone(),
        }))
    }
}

impl Bound<FakeProject> {
    fn file(&self, path: &str) -> Result<FakeFile> {
        self.fake.files.iter().find(|(p, _)| p == path).map_or_else(
            || Err(Error::NotFound(format!("no {path}"))),
            |(_, f)| f.clone(),
        )
    }
}

impl ProjectSource for Bound<FakeProject> {
    fn stamp(&self) -> &Value {
        &self.fake.stamp
    }

    fn tasks(&self, filters: &Filters) -> Result<Value> {
        self.calls.note(Call::Tasks(filters.clone()));
        self.fake.tasks.clone()
    }

    fn others_version(&self) -> Value {
        self.calls.note(Call::OthersVersion);
        self.fake.others.clone()
    }

    fn claims(&self) -> Vec<Value> {
        self.calls.note(Call::Claims);
        self.fake.claims.clone()
    }

    fn worktrees(&self) -> Vec<Value> {
        self.calls.note(Call::Worktrees);
        self.fake.worktrees.clone()
    }

    fn logo(&self) -> Option<(Value, &'static str)> {
        self.fake
            .logo
            .as_ref()
            .map(|(stamp, kind, _)| (stamp.clone(), *kind))
    }

    fn read_logo(&self) -> Vec<u8> {
        self.calls.note(Call::ReadLogo);
        self.fake
            .logo
            .as_ref()
            .map(|(_, _, bytes)| bytes.clone())
            .unwrap_or_default()
    }

    fn file_version(&self, path: &str) -> Result<(u64, u128)> {
        self.calls.note(Call::FileVersion(path.into()));
        self.file(path).map(|f| f.version)
    }

    fn head(&self) -> Option<String> {
        self.calls.note(Call::Head);
        self.fake.head.clone()
    }

    fn document(&self, path: &str) -> Result<Value> {
        self.calls.note(Call::Document(path.into()));
        self.file(path)?.document
    }

    fn document_at(&self, path: &str, revision: &str) -> Result<Value> {
        self.calls
            .note(Call::DocumentAt(path.into(), revision.into()));
        self.fake.at_revision.clone()
    }

    fn reviews(&self, query: &ReviewQuery) -> Result<Value> {
        self.calls.note(Call::Reviews(query.clone()));
        self.fake.listed.clone()
    }

    fn review(
        &self,
        review: &ReviewRef,
        round: Option<u32>,
        against: Option<u32>,
    ) -> Result<Value> {
        self.calls
            .note(Call::Review(review.clone(), round, against));
        self.fake.page.clone()
    }

    fn image(&self, path: &str) -> Result<(Vec<u8>, &'static str)> {
        self.calls.note(Call::Image(path.into()));
        self.file(path)?.image
    }

    fn reviews_as_user(&self) -> Result<Box<dyn ReviewChanges>> {
        self.calls.note(Call::AsUser);
        Ok(Box::new(Bound {
            fake: self.fake.reviews.clone()?,
            id: self.id.clone(),
            calls: self.calls.clone(),
        }))
    }
}

impl ReviewChanges for Bound<FakeReviews> {
    fn open_review(&mut self, opening: ReviewOpening) -> Result<ReviewHeader> {
        self.calls.note(Call::OpenReview(opening));
        self.fake.header.clone()
    }

    fn close_review(
        &mut self,
        review: &ReviewRef,
        closing: &ReviewClosing,
    ) -> Result<ReviewHeader> {
        self.calls
            .note(Call::CloseReview(review.clone(), closing.clone()));
        self.fake.header.clone()
    }

    fn set_review_tasks(
        &mut self,
        review: &ReviewRef,
        tasks: &ReviewTasks,
    ) -> Result<ReviewHeader> {
        self.calls
            .note(Call::SetTasks(review.clone(), tasks.clone()));
        self.fake.header.clone()
    }

    fn submit_decision(&mut self, review: &ReviewRef, decision: &Decision) -> Result<ReviewHeader> {
        self.calls
            .note(Call::Decide(review.clone(), decision.clone()));
        self.fake.header.clone()
    }

    fn add_comment(&mut self, review: &ReviewRef, comment: NewComment) -> Result<CommentId> {
        self.calls.note(Call::Comment(review.clone(), comment));
        self.fake.comment.clone()
    }

    fn update_comment(
        &mut self,
        review: &ReviewRef,
        comment: &CommentId,
        update: CommentUpdate,
    ) -> Result<()> {
        self.calls
            .note(Call::UpdateComment(review.clone(), comment.clone(), update));
        self.fake.updated.clone()
    }

    fn reply_to_comment(
        &mut self,
        review: &ReviewRef,
        comment: &CommentId,
        reply: &NewReply,
    ) -> Result<CommentId> {
        self.calls
            .note(Call::Reply(review.clone(), comment.clone(), reply.clone()));
        self.fake.comment.clone()
    }
}

/// Each of atm-lib's refusals, with the status a route answers it with.
pub(super) fn refusals() -> [(Error, StatusCode); 5] {
    [
        (Error::Invalid("malformed".into()), StatusCode::BAD_REQUEST),
        (Error::NotFound("no such".into()), StatusCode::NOT_FOUND),
        (
            Error::TooLarge("too large".into()),
            StatusCode::PAYLOAD_TOO_LARGE,
        ),
        (Error::Refused("not shown".into()), StatusCode::FORBIDDEN),
        (Error::failed("broken"), StatusCode::INTERNAL_SERVER_ERROR),
    ]
}

/// No headers.
pub(super) fn plain() -> HeaderMap {
    HeaderMap::new()
}

/// A request that has the version `tag` already.
pub(super) fn having(tag: &str) -> HeaderMap {
    let mut request = HeaderMap::new();
    request.insert(
        header::IF_NONE_MATCH,
        HeaderValue::from_str(tag).expect("a tag"),
    );
    request
}

/// An answer, read back.
pub(super) struct Answer {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

impl Answer {
    /// The body as JSON (Null when it isn't JSON).
    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or(Value::Null)
    }

    /// The body as JSON, without `generated_at`.
    pub fn unstamped(&self) -> Value {
        let mut body = self.json();
        if let Some(map) = body.as_object_mut() {
            map.remove("generated_at");
        }
        body
    }

    /// The header `name`, or an empty text.
    pub fn header(&self, name: &str) -> &str {
        self.headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
    }

    /// The `ETag`.
    pub fn tag(&self) -> String {
        self.header("etag").to_string()
    }
}

/// `response`, read.
pub(super) fn answer(response: Response) -> Answer {
    let status = response.status();
    let headers = response.headers().clone();
    let body = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime")
        .block_on(axum::body::to_bytes(response.into_body(), usize::MAX))
        .expect("the body")
        .to_vec();
    Answer {
        status,
        headers,
        body,
    }
}
