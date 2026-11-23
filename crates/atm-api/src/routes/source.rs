//! What the routes ask of atm-lib, as traits: the server's answers are atm-lib's ([`AtmLib`]), a unit test's a fake's
//! (`test_support`), so each route is tested alone (AGENTS.md, Testing conventions). Each method is one atm-lib call.

use std::path::Path;
use std::sync::Arc;

use atm_common::{Error, Result};
use atm_lib::Project;
use atm_lib::query::Filters;
use atm_lib::review_drafts::{self, Draft, DraftUpdate, NewDraft};
use atm_lib::review_pages::ReviewQuery;
use atm_lib::review_requests::{
    self, CommentUpdate, Decision, NewComment, NewPoke, NewReply, ReviewClosing, ReviewOpening,
    ReviewTasks,
};
use atm_lib::reviews::{CommentId, ReviewHeader, ReviewRef};
use atm_lib::stats::StatsQuery;
use atm_lib::workspaces::{Member, Workspace, Workspaces, file_stamp};
use serde_json::{Map, Value};

/// The workspaces of the state folder.
pub trait Source: Send + Sync {
    /// `{workspaces: [{id, projects: how many}], recent: the one opened last}`.
    fn summary(&self) -> Value;
    /// The workspace `id`, if there is one.
    fn workspace(&self, id: &str) -> Option<Box<dyn WorkspaceSource>>;
    /// The `default` workspace, always there.
    fn default_workspace(&self) -> Box<dyn WorkspaceSource>;
    /// Record `id` as the workspace opened last; a record that can't be written isn't the read's failure.
    fn opened(&self, id: &str);
}

/// A workspace's projects as the dashboard lists them: what the list depends on, and the list, made only when asked.
pub struct ProjectList<'a> {
    /// What the list depends on, for its `ETag`.
    pub stamp: Value,
    /// The list.
    pub projects: Box<dyn FnOnce() -> Vec<Value> + 'a>,
}

/// One workspace.
pub trait WorkspaceSource {
    /// Its ID.
    fn id(&self) -> String;
    /// Its projects.
    fn projects(&self) -> ProjectList<'_>;
    /// Per project, the IDs of the tasks in which every word of `query` appears.
    fn search(&self, query: &str) -> Map<String, Value>;
    /// Its project `id`, there and readable.
    ///
    /// # Errors
    /// `NotFound`: no such project, or its task data missing or refused.
    fn project(&self, id: &str) -> Result<Box<dyn ProjectSource>>;
}

/// One project of a workspace.
pub trait ProjectSource {
    /// Its store's stamp (`{tasks, archive, claims, head, copies}`), for `ETag`s.
    fn stamp(&self) -> &Value;
    /// Its tasks the filters keep, with their derived values, its meta and its size.
    ///
    /// # Errors
    /// Task data too large; a filter naming no such sprint or release (`Invalid`); unreadable task data.
    fn tasks(&self, filters: &Filters) -> Result<Value>;
    /// Its stats, progress and forecast for a slice and a period (ATM-722).
    ///
    /// # Errors
    /// Task data too large; unreadable task data.
    fn stats(&self, query: &StatsQuery) -> Result<Value>;
    /// What the other projects its tasks name are now, for the tasks' `ETag`.
    fn others_version(&self) -> Value;
    /// The claims of all its worktrees.
    fn claims(&self) -> Vec<Value>;
    /// Its working copies, main first.
    fn worktrees(&self) -> Vec<Value>;
    /// Its logo's version (null when the file isn't there) and content type, when it names one.
    fn logo(&self) -> Option<(Value, &'static str)>;
    /// Its logo's bytes.
    /// The logo's bytes; none when it can't be read (gone since it was found, or unreadable: ATM-671).
    fn read_logo(&self) -> Option<Vec<u8>>;
    /// The size and mtime of the file at `path` in its working copy, once atm-docs has checked the path.
    ///
    /// # Errors
    /// No working copy (`NotFound`), or a path atm-docs refuses: malformed, not there, too large, not shown.
    fn file_version(&self, path: &str) -> Result<(u64, u128)>;
    /// Its working copy's current commit.
    fn head(&self) -> Option<String>;
    /// The document at `path`, as the viewer shows it.
    ///
    /// # Errors
    /// As [`ProjectSource::file_version`].
    fn document(&self, path: &str) -> Result<Value>;
    /// The image at `path`, and its content type.
    ///
    /// # Errors
    /// As [`ProjectSource::file_version`], and a file that isn't an image.
    fn image(&self, path: &str) -> Result<(Vec<u8>, &'static str)>;
    /// The image at `path` as the commit or tree `revision` has it, and its content type (a review's image file's
    /// side, ATM-689).
    ///
    /// # Errors
    /// As [`ProjectSource::document_at`], and a file that isn't an image.
    fn image_at(&self, path: &str, revision: &str) -> Result<(Vec<u8>, &'static str)>;
    /// The document at `path` as the commit or tree `revision` has it (a review round's side).
    ///
    /// # Errors
    /// As [`ProjectSource::file_version`], and a revision that isn't one or doesn't have the file.
    fn document_at(&self, path: &str, revision: &str) -> Result<Value>;
    /// Its reviews the query keeps, read as the user (which need them).
    ///
    /// # Errors
    /// A status that isn't one; reviews that can't be read.
    fn reviews(&self, query: &ReviewQuery) -> Result<Value>;
    /// A review's page for one view: a round (the latest when none), against an earlier one when given.
    ///
    /// # Errors
    /// No such review or round; a round's tree that can't be had here.
    fn review(&self, review: &ReviewRef, round: Option<u32>, against: Option<u32>)
    -> Result<Value>;
    /// What a live review's page of its working copy now is made from, cheap to read (ATM-607): its `ETag`'s parts.
    /// None for any other page, which isn't tagged.
    ///
    /// # Errors
    /// No such review; the project isn't in git.
    fn live_review_stamp(
        &self,
        review: &ReviewRef,
        round: Option<u32>,
        against: Option<u32>,
    ) -> Result<Option<Value>>;
    /// Its reviews, to change as the user at the machine (never an agent's identity).
    ///
    /// # Errors
    /// The project can't be opened.
    fn reviews_as_user(&self) -> Result<Box<dyn ReviewChanges>>;
}

/// A project's reviews, changed (atm-lib's `review_requests`).
pub trait ReviewChanges {
    /// A new review.
    ///
    /// # Errors
    /// What atm-lib refuses.
    fn open_review(&mut self, opening: ReviewOpening) -> Result<ReviewHeader>;
    /// The review closed.
    ///
    /// # Errors
    /// What atm-lib refuses.
    fn close_review(&mut self, review: &ReviewRef, closing: &ReviewClosing)
    -> Result<ReviewHeader>;
    /// The tasks it reviews, as a whole.
    ///
    /// # Errors
    /// What atm-lib refuses.
    fn set_review_tasks(&mut self, review: &ReviewRef, tasks: &ReviewTasks)
    -> Result<ReviewHeader>;
    /// A reviewer's decision.
    ///
    /// # Errors
    /// What atm-lib refuses.
    fn submit_decision(&mut self, review: &ReviewRef, decision: &Decision) -> Result<ReviewHeader>;
    /// A new chain: its first comment's ID.
    ///
    /// # Errors
    /// What atm-lib refuses.
    fn add_comment(&mut self, review: &ReviewRef, comment: NewComment) -> Result<CommentId>;
    /// A comment's text or resolved box.
    ///
    /// # Errors
    /// What atm-lib refuses.
    fn update_comment(
        &mut self,
        review: &ReviewRef,
        comment: &CommentId,
        update: CommentUpdate,
    ) -> Result<()>;
    /// A reply: its ID.
    ///
    /// # Errors
    /// What atm-lib refuses.
    fn reply_to_comment(
        &mut self,
        review: &ReviewRef,
        comment: &CommentId,
        reply: &NewReply,
    ) -> Result<CommentId>;
    /// A poke (ATM-611): its ID.
    ///
    /// # Errors
    /// What atm-lib refuses.
    fn poke_review(&mut self, review: &ReviewRef, poke: &NewPoke) -> Result<CommentId>;
    /// The user's drafts on the review (ATM-612), the oldest first.
    ///
    /// # Errors
    /// What atm-lib refuses.
    fn list_drafts(&self, review: &ReviewRef) -> Result<Vec<Draft>>;
    /// A new draft of the user's.
    ///
    /// # Errors
    /// What atm-lib refuses.
    fn add_draft(&mut self, review: &ReviewRef, draft: NewDraft) -> Result<Draft>;
    /// A draft's text.
    ///
    /// # Errors
    /// What atm-lib refuses.
    fn update_draft(
        &mut self,
        review: &ReviewRef,
        draft: &str,
        update: &DraftUpdate,
    ) -> Result<Draft>;
    /// A draft thrown away.
    ///
    /// # Errors
    /// What atm-lib refuses.
    fn remove_draft(&mut self, review: &ReviewRef, draft: &str) -> Result<()>;
}

/// atm-lib's answers: the server's.
pub struct AtmLib(Workspaces);

impl AtmLib {
    /// The workspaces of the state folder `home`.
    #[must_use]
    pub fn at(home: &Path) -> Self {
        Self(Workspaces::at(home))
    }
}

impl Source for AtmLib {
    fn summary(&self) -> Value {
        self.0.summary()
    }

    fn workspace(&self, id: &str) -> Option<Box<dyn WorkspaceSource>> {
        self.0
            .get(id)
            .map(|w| Box::new(w) as Box<dyn WorkspaceSource>)
    }

    fn default_workspace(&self) -> Box<dyn WorkspaceSource> {
        Box::new(self.0.default_workspace())
    }

    fn opened(&self, id: &str) {
        let _ = self.0.opened(id);
    }
}

impl WorkspaceSource for Arc<Workspace> {
    fn id(&self) -> String {
        Workspace::id(self).to_string()
    }

    fn projects(&self) -> ProjectList<'_> {
        let listing = self.listing();
        ProjectList {
            stamp: listing.stamp.clone(),
            projects: Box::new(move || listing.projects()),
        }
    }

    fn search(&self, query: &str) -> Map<String, Value> {
        Workspace::search(self, query)
    }

    fn project(&self, id: &str) -> Result<Box<dyn ProjectSource>> {
        Ok(Box::new(self.member(id)?))
    }
}

impl ProjectSource for Member {
    fn stamp(&self) -> &Value {
        &self.stamp
    }

    fn tasks(&self, filters: &Filters) -> Result<Value> {
        Member::tasks(self, filters)
    }

    fn stats(&self, query: &StatsQuery) -> Result<Value> {
        Member::stats(self, query)
    }

    fn others_version(&self) -> Value {
        Member::others_version(self)
    }

    fn claims(&self) -> Vec<Value> {
        Member::claims(self)
    }

    fn worktrees(&self) -> Vec<Value> {
        Member::worktrees(self)
    }

    fn logo(&self) -> Option<(Value, &'static str)> {
        Member::logo(self).map(|(file, kind)| (file_stamp(&file), kind))
    }

    fn read_logo(&self) -> Option<Vec<u8>> {
        Member::logo(self).and_then(|(file, _)| std::fs::read(file).ok())
    }

    fn file_version(&self, path: &str) -> Result<(u64, u128)> {
        let vcs = self
            .project()
            .ok()
            .and_then(|p| p.vcs())
            .ok_or_else(|| Error::NotFound("not found".into()))?;
        atm_lib::documents::document_stamp(Some(&vcs), path)
    }

    fn head(&self) -> Option<String> {
        self.project()
            .ok()
            .and_then(|p| p.vcs())
            .and_then(|v| v.head())
    }

    fn document(&self, path: &str) -> Result<Value> {
        Member::document(self, path)
    }

    fn image(&self, path: &str) -> Result<(Vec<u8>, &'static str)> {
        Member::image(self, path)
    }

    fn image_at(&self, path: &str, revision: &str) -> Result<(Vec<u8>, &'static str)> {
        Member::image_at(self, path, revision)
    }

    fn document_at(&self, path: &str, revision: &str) -> Result<Value> {
        Member::document_at(self, path, revision)
    }

    fn reviews(&self, query: &ReviewQuery) -> Result<Value> {
        Member::reviews(self, query)
    }

    fn review(
        &self,
        review: &ReviewRef,
        round: Option<u32>,
        against: Option<u32>,
    ) -> Result<Value> {
        Member::review(self, review, round, against)
    }

    fn live_review_stamp(
        &self,
        review: &ReviewRef,
        round: Option<u32>,
        against: Option<u32>,
    ) -> Result<Option<Value>> {
        Member::live_review_stamp(self, review, round, against)
    }

    fn reviews_as_user(&self) -> Result<Box<dyn ReviewChanges>> {
        Ok(Box::new(self.project_as_user()?))
    }
}

impl ReviewChanges for Project {
    fn open_review(&mut self, opening: ReviewOpening) -> Result<ReviewHeader> {
        review_requests::open_review(self, opening)
    }

    fn close_review(
        &mut self,
        review: &ReviewRef,
        closing: &ReviewClosing,
    ) -> Result<ReviewHeader> {
        review_requests::close_review(self, review, closing)
    }

    fn set_review_tasks(
        &mut self,
        review: &ReviewRef,
        tasks: &ReviewTasks,
    ) -> Result<ReviewHeader> {
        review_requests::set_review_tasks(self, review, tasks)
    }

    fn submit_decision(&mut self, review: &ReviewRef, decision: &Decision) -> Result<ReviewHeader> {
        review_requests::submit_decision(self, review, decision)
    }

    fn add_comment(&mut self, review: &ReviewRef, comment: NewComment) -> Result<CommentId> {
        review_requests::add_comment(self, review, comment)
    }

    fn update_comment(
        &mut self,
        review: &ReviewRef,
        comment: &CommentId,
        update: CommentUpdate,
    ) -> Result<()> {
        review_requests::update_comment(self, review, comment, update)
    }

    fn reply_to_comment(
        &mut self,
        review: &ReviewRef,
        comment: &CommentId,
        reply: &NewReply,
    ) -> Result<CommentId> {
        review_requests::reply_to_comment(self, review, comment, reply)
    }

    fn poke_review(&mut self, review: &ReviewRef, poke: &NewPoke) -> Result<CommentId> {
        review_requests::poke_review(self, review, poke)
    }

    fn list_drafts(&self, review: &ReviewRef) -> Result<Vec<Draft>> {
        review_drafts::list_drafts(self, review)
    }

    fn add_draft(&mut self, review: &ReviewRef, draft: NewDraft) -> Result<Draft> {
        review_drafts::add_draft(self, review, draft)
    }

    fn update_draft(
        &mut self,
        review: &ReviewRef,
        draft: &str,
        update: &DraftUpdate,
    ) -> Result<Draft> {
        review_drafts::update_draft(self, review, draft, update)
    }

    fn remove_draft(&mut self, review: &ReviewRef, draft: &str) -> Result<()> {
        review_drafts::remove_draft(self, review, draft)
    }
}
