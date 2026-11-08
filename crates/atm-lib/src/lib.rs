//! atm's domain: tasks and their states, claims, the loop, sprints, releases, components, the archive, settings, projects, documents and the agents' skills. Takes the store through its interface.
//!
//! Part of atm's Rust port (T-415): docs/porting-to-rust.md, docs/layered-architecture.md, docs/guides/rust.md.

pub mod agents;
pub mod archive;
pub mod changes;
pub mod check;
pub mod claims;
pub mod code;
pub mod components;
pub mod documents;
pub mod edit;
pub mod leases;
pub mod loop_rules;
pub mod maintenance;
pub mod project;
pub mod query;
pub mod releases;
pub mod settings;
pub mod sprints;
pub mod state;
pub mod status;
pub mod summary;
pub mod tasks;
pub mod workspaces;
pub mod worktrees;

pub use project::Project;
pub use workspaces::{Workspace, Workspaces};
