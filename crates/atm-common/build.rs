//! Embed the agent skills (`skills/atm-*/SKILL.md`, and Claude Code's pre-approvals in `skills/claude-code.json`) in
//! the binary, so `atm agents install` writes the skills of the version that runs (T-442, docs/porting-to-rust.md §2).
//! The Python atm's `build.py` makes `common/skills.py` the same way.

use std::fmt::Write as _;
use std::path::Path;
use std::{env, fs};

fn main() {
    let manifest = env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    let skills = Path::new(&manifest)
        .join("../../skills")
        .canonicalize()
        .expect("skills/ is at the repository's root");
    println!("cargo:rerun-if-changed={}", skills.display());
    let mut names: Vec<String> = fs::read_dir(&skills)
        .expect("skills/ can be read")
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    let mut code = String::from(
        "/// The skills, by name: each one's `SKILL.md`, the portable core.\npub static SKILLS: &[(&str, &str)] = &[\n",
    );
    for name in &names {
        let file = skills.join(name).join("SKILL.md");
        println!("cargo:rerun-if-changed={}", file.display());
        writeln!(
            code,
            "    ({name:?}, include_str!({:?})),",
            file.display().to_string()
        )
        .expect("writing to a String");
    }
    code.push_str("];\n\n/// `skills/claude-code.json`: Claude Code's pre-approved commands for each skill.\n");
    let tools = skills.join("claude-code.json");
    println!("cargo:rerun-if-changed={}", tools.display());
    writeln!(
        code,
        "pub static CLAUDE_CODE_JSON: &str = include_str!({:?});",
        tools.display().to_string()
    )
    .expect("writing to a String");
    let out = Path::new(&env::var("OUT_DIR").expect("cargo sets OUT_DIR")).join("skills.rs");
    fs::write(out, code).expect("OUT_DIR is writable");
}
