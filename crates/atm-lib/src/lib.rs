//! atm's domain: tasks and their states, claims, the loop, sprints, releases, components, the archive, settings, projects, documents and the agents' skills. Takes the store through its interface.
//!
//! Part of atm's Rust port (T-415): docs/porting-to-rust.md, docs/layered-architecture.md, docs/guides/rust.md.

pub mod actor;
pub mod agent_hooks;
pub mod agents;
pub mod archive;
pub mod changes;
pub mod check;
pub mod claims;
pub mod code;
pub mod components;
/// A project's files for the document viewer: the atm-docs component (docs/code-review-design.md §3.1).
pub use atm_docs as documents;
pub mod edit;
pub mod foreign;
pub mod leases;
pub mod lifecycle;
mod lookup;
pub mod loop_rules;
pub mod maintenance;
pub mod project;
pub mod query;
pub mod releases;
pub mod review_drafts;
pub mod review_keys;
pub mod review_listen;
pub mod review_pages;
pub mod review_requests;
pub mod reviews;
pub mod settings;
pub mod sprints;
pub mod state;
pub mod status;
pub mod summary;
pub mod tasks;
pub mod vocabulary;
pub mod waiting;
pub mod work;
pub mod workspaces;
pub mod worktrees;

/// Being told of changes (a workspace's, a project's): the store's subscriptions, how it hears of them its own.
pub use atm_db::subscription::{OnChange, Subscription};
pub use project::Project;
pub use workspaces::{Workspace, Workspaces};
