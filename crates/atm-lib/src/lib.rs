//! atm's domain: tasks and their states, claims, the loop, sprints, releases, components, the archive, settings, projects, documents and the agents' skills. Takes the store through its interface.
//!
//! Part of atm's Rust port (T-415): docs/porting-to-rust.md, docs/layered-architecture.md, docs/guides/rust.md.

pub mod agents;
pub mod changes;
pub mod claims;
pub mod edit;
pub mod leases;
pub mod loop_rules;
pub mod project;
pub mod query;
pub mod state;
pub mod status;
pub mod tasks;

pub use project::Project;
