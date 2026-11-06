//! The model (task, sprint, release, …), the vocabularies, errors and the JSON schema: what every layer shares. Depends
//! on nothing of atm.
//!
//! Part of atm's Rust port (T-415): docs/porting-to-rust.md, docs/layered-architecture.md, docs/guides/rust.md.

/// atm's version: the workspace's, which is the plugin's (`plugin/.claude-plugin/plugin.json`, checked by
/// `dashboard/scripts/check-versions.mjs`).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::VERSION;

    #[test]
    fn version_is_a_semantic_version() {
        let parts: Vec<&str> = VERSION.split('.').collect();
        assert_eq!(parts.len(), 3, "{VERSION}");
        assert!(parts.iter().all(|p| p.parse::<u32>().is_ok()), "{VERSION}");
    }
}
