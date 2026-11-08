//! The store interface, and its implementations: jsongit (the task files, their lock, the merge driver, migrations, the claims store) and memory.
//!
//! Part of atm's Rust port (T-415): docs/porting-to-rust.md, docs/layered-architecture.md, docs/guides/rust.md.

pub mod git;
pub mod interface;
pub mod jsongit;
pub mod memory;
pub mod open;
pub mod registry;
pub mod working;

#[cfg(test)]
mod test_dir;

pub use interface::Store;
pub use jsongit::JsonGitStore;
pub use memory::MemoryStore;
pub use open::{Opened, ProjectSettings, open};
