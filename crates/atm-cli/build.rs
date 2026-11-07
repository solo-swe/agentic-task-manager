//! Embed `atm docs`' topics in the binary (T-440): each `docs/agent/<topic>.md`, whose first line is
//! `# <Title>: <summary>`, and `CompMD`'s guide for agents (`docs/agent/compmd-llms.txt`) as the topic `compmd`. Checked
//! as the Python atm's `build.py` checks them: a topic's name is lowercase letters, digits and `-`.

use std::fmt::Write as _;
use std::path::Path;
use std::{env, fs};

const COMPMD_SUMMARY: &str = "CompMD, the Markdown task text renders as: its guide for agents (what renders: atm docs tasks)";

fn main() {
    let manifest = env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    let docs = Path::new(&manifest)
        .join("../../docs/agent")
        .canonicalize()
        .expect("docs/agent/ is in the repository");
    println!("cargo:rerun-if-changed={}", docs.display());
    let mut topics: Vec<(String, String, String)> = Vec::new(); // (name, summary, file)
    for entry in fs::read_dir(&docs)
        .expect("docs/agent/ can be read")
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let name = path
            .file_stem()
            .expect("a .md file has a name")
            .to_string_lossy()
            .into_owned();
        let valid = name.chars().next().is_some_and(|c| c.is_ascii_lowercase())
            && name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        assert!(
            valid,
            "docs/agent/{name}.md: a topic's name is lowercase letters, digits and -"
        );
        // LF line ends, whatever the checkout made them (git on Windows writes CRLF)
        let text = fs::read_to_string(&path)
            .expect("a topic is UTF-8 text")
            .replace("\r\n", "\n");
        let first = text.split('\n').next().unwrap_or_default();
        let summary = first
            .strip_prefix("# ")
            .and_then(|title| title.split_once(": "))
            .map(|(_, summary)| summary.trim().to_string());
        let summary = summary.unwrap_or_else(|| {
            panic!("docs/agent/{name}.md: the first line is `# <Title>: <summary>`")
        });
        println!("cargo:rerun-if-changed={}", path.display());
        topics.push((name, summary, text));
    }
    assert!(!topics.is_empty(), "docs/agent/ has no topics");
    let compmd = docs.join("compmd-llms.txt");
    println!("cargo:rerun-if-changed={}", compmd.display());
    let compmd_text = fs::read_to_string(&compmd)
        .expect("compmd-llms.txt is UTF-8 text")
        .replace("\r\n", "\n");
    topics.push(("compmd".into(), COMPMD_SUMMARY.into(), compmd_text));
    topics.sort();
    let mut code = String::from(
        "/// `atm docs`' topics, by name: (topic, summary, Markdown as written).\npub static AGENT_DOCS: &[(&str, &str, &str)] = &[\n",
    );
    for (name, summary, text) in &topics {
        writeln!(code, "    ({name:?}, {summary:?}, {text:?}),").expect("writing to a String");
    }
    code.push_str("];\n");
    let out = Path::new(&env::var("OUT_DIR").expect("cargo sets OUT_DIR")).join("agent_docs.rs");
    fs::write(out, code).expect("OUT_DIR is writable");
}
