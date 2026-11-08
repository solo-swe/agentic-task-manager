//! The agent skills, built into atm (`build.rs` embeds `skills/`): what `atm agents install` writes is the version
//! that runs. The Python atm's generated `common/skills.py`.

use std::sync::LazyLock;

use serde_json::Value;

include!(concat!(env!("OUT_DIR"), "/skills.rs"));

static ALLOWED_TOOLS: LazyLock<Vec<(String, String)>> = LazyLock::new(|| {
    let data: Value =
        serde_json::from_str(CLAUDE_CODE_JSON).expect("skills/claude-code.json is valid JSON");
    data["allowed-tools"]
        .as_object()
        .expect("skills/claude-code.json has an allowed-tools object")
        .iter()
        .map(|(name, tools)| (name.clone(), tools.as_str().unwrap_or_default().to_string()))
        .collect()
});

/// Claude Code's pre-approved commands for a skill (its `allowed-tools`), if it has any.
#[must_use]
pub fn claude_code_allowed_tools(skill: &str) -> Option<&'static str> {
    ALLOWED_TOOLS
        .iter()
        .find(|(name, _)| name == skill)
        .map(|(_, tools)| tools.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_skill_is_built_in_by_name_in_order() {
        assert!(SKILLS.len() >= 7);
        assert!(SKILLS.windows(2).all(|w| w[0].0 < w[1].0));
        for (name, text) in SKILLS {
            assert!(name.starts_with("atm-"), "{name}");
            assert!(text.starts_with(&format!("---\nname: {name}\n")), "{name}");
        }
    }

    #[test]
    fn each_skill_has_its_claude_code_pre_approvals() {
        for (name, _) in SKILLS {
            assert!(
                claude_code_allowed_tools(name).is_some_and(|t| !t.is_empty()),
                "{name}"
            );
        }
        assert_eq!(claude_code_allowed_tools("nope"), None);
    }
}
