//! The app's commands (docs/guides/tauri.md): thin mappings to atm-lib's `Workspaces`, `Workspace` and `Member`, the
//! REST API's v2 routes as commands (ADR-003 §5), answering what those routes answer (without `generated_at`). Their
//! arguments and results are serde types, their errors one tagged enum the UI matches on. Live changes come through
//! one channel per subscription, from atm-lib's subscriptions to a workspace or a project (the stores', T-555). (In a
//! module of their own: Tauri's command macros can't be defined at a library's root.)
//!
//! By area (ATM-711): [`state`] (what the commands share, their errors and filters), [`reads`] (the workspaces,
//! projects, tasks, documents and files), [`reviews`] (the review and draft commands) and [`windows`] (the
//! subscriptions, the links and the agents' skills). Each is re-exported here, so every command keeps its path.

mod reads;
mod reviews;
mod state;
mod windows;

pub use reads::*;
pub use reviews::*;
pub use state::*;
pub use windows::*;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod integration_tests;
