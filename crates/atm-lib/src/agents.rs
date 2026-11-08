//! Installing atm's skills into the agent harnesses (T-442, docs/better-way-to-distribute-atm.md §3, §5): the Python
//! atm's `lib/agents.py`.
//!
//! The skills of this atm's version are written once per harness into the user's atm folder
//! (`<ATM_HOME>/skills/<harness>/<skill>/SKILL.md`) and linked from the folder each harness reads its skills from;
//! copied there instead with `copy`, where a link can't be made, and always for a project (whose copy is committed for
//! a team, so it can't point into one person's home folder). What install wrote is recorded (`<ATM_HOME>/agents.json`),
//! so uninstall removes exactly that and status can say what is missing or stale. A folder atm didn't write is never
//! touched. Harnesses are a table: Claude Code for now; another is one more entry (T-413).

use std::fs;

use atm_common::Shown;
use std::path::{Path, PathBuf};

use atm_common::home::{absolute, atm_home, ensure_home, in_home, write_text_atomic};
use atm_common::py::to_json;
use atm_common::skills::{SKILLS, claude_code_allowed_tools};
use atm_common::{Error, Result};
use serde_json::{Value, json};

use crate::project::Project;

const RECORD: &str = "agents.json";

/// An agent harness atm can install its skills into.
#[derive(Debug)]
pub struct Harness {
    /// Its ID (`--harness`).
    pub id: &'static str,
    /// Its name, for people.
    pub label: &'static str,
    /// Where it is installed, in the home folder (atm says so; installing doesn't need it).
    detect: &'static str,
    /// Where it reads the user's skills, in the home folder.
    user_dir: &'static str,
    /// Where it reads a project's skills, from the project's root.
    project_dir: &'static str,
    /// Whether it reads `allowed-tools` in a skill's frontmatter.
    allowed_tools: bool,
    /// Whether it follows symlinked skill folders.
    links: bool,
}

/// The harnesses atm knows.
pub static HARNESSES: &[Harness] = &[Harness {
    id: "claude",
    label: "Claude Code",
    detect: ".claude",
    user_dir: ".claude/skills",
    project_dir: ".claude/skills",
    allowed_tools: true,
    links: true,
}];

/// The project's folder, for `--project`: its working copy (where its task folder is).
#[must_use]
pub fn project_folder(project: &Project) -> Option<PathBuf> {
    project.store().working_copy().map(Path::to_path_buf)
}

fn harnesses(harness: Option<&str>) -> Result<Vec<&'static Harness>> {
    match harness {
        None => Ok(HARNESSES.iter().collect()),
        Some(id) => HARNESSES
            .iter()
            .find(|h| h.id == id)
            .map(|h| vec![h])
            .ok_or_else(|| {
                let known: Vec<&str> = HARNESSES.iter().map(|h| h.id).collect();
                Error::Invalid(format!(
                    "no harness '{id}' (atm knows: {})",
                    known.join(", ")
                ))
            }),
    }
}

/// A skill's `SKILL.md` as this harness gets it: the portable core, with Claude Code's pre-approved commands added to
/// its frontmatter for Claude Code.
#[must_use]
pub fn skill_text(harness: &Harness, name: &str, text: &str) -> String {
    match claude_code_allowed_tools(name).filter(|t| harness.allowed_tools && !t.is_empty()) {
        Some(tools) => match text.split_once("\n---\n") {
            Some((head, body)) => format!("{head}\nallowed-tools: {tools}\n---\n{body}"),
            None => text.to_string(),
        },
        None => text.to_string(),
    }
}

fn record_path() -> PathBuf {
    atm_home().join(RECORD)
}

/// What install wrote: the record's entries, each with its `path`.
fn read_record() -> Vec<serde_json::Map<String, Value>> {
    let data = fs::read_to_string(record_path())
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok());
    data.and_then(|d| d.get("installed").cloned())
        .and_then(|i| i.as_array().cloned())
        .into_iter()
        .flatten()
        .filter_map(|e| e.as_object().cloned())
        .filter(|e| e.get("path").is_some_and(Value::is_string))
        .collect()
}

fn entry_path(entry: &serde_json::Map<String, Value>) -> &str {
    entry
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or_default()
}

fn write_record(mut entries: Vec<serde_json::Map<String, Value>>) -> Result<()> {
    let home = atm_home();
    ensure_home(&home).map_err(|e| Error::failed(format!("can't make {}: {e}", home.shown())))?;
    entries.sort_by(|a, b| entry_path(a).cmp(entry_path(b)));
    let installed: Vec<Value> = entries.into_iter().map(Value::Object).collect();
    write_text_atomic(
        &record_path(),
        &(to_json(&json!({"installed": installed})) + "\n"),
    )
    .map_err(|e| Error::failed(format!("can't write {}: {e}", record_path().shown())))
}

fn io(path: &Path) -> impl Fn(std::io::Error) -> Error + '_ {
    move |e| Error::failed(format!("{}: {e}", path.shown()))
}

fn write_skill(folder: &Path, text: &str) -> Result<()> {
    fs::create_dir_all(folder).map_err(io(folder))?;
    let file = folder.join("SKILL.md");
    fs::write(&file, text).map_err(io(&file))
}

fn read_skill(folder: &Path) -> Option<String> {
    fs::read_to_string(folder.join("SKILL.md")).ok()
}

fn is_link(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink())
}

fn lexists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

fn remove(path: &Path) -> Result<()> {
    if is_link(path) || path.is_file() {
        // A link to a folder is a directory entry on Windows
        fs::remove_file(path)
            .or_else(|_| fs::remove_dir(path))
            .map_err(io(path))
    } else if path.is_dir() {
        fs::remove_dir_all(path).map_err(io(path))
    } else {
        Ok(())
    }
}

fn link(source: &Path, path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(source, path)
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(source, path)
    }
}

/// Where a harness reads skills: the folder, and the scope (`user` or `project`).
fn target(harness: &Harness, project_dir: Option<&Path>) -> (PathBuf, &'static str) {
    match project_dir {
        Some(project) => (absolute(project).join(harness.project_dir), "project"),
        None => (in_home(harness.user_dir), "user"),
    }
}

fn same_place(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// What `install_skills` did: `written` [{harness, skill, path, how, action}] (action: installed, updated or
/// unchanged) and `refused` [{path, reason}] (a folder in the way that atm didn't write).
#[derive(Debug, Clone, PartialEq)]
pub struct Installed {
    /// The skills written or found as they should be.
    pub written: Vec<Value>,
    /// The folders left alone.
    pub refused: Vec<Value>,
}

/// Write the skills for the harnesses (one, or all) and link or copy them where each reads them: the user's folder,
/// or `project_dir`'s with a project. Idempotent; a folder in the way that atm didn't write is refused, not replaced.
///
/// # Errors
/// `Invalid` for a harness atm doesn't know; a folder or file that can't be written.
pub fn install_skills(
    harness: Option<&str>,
    project_dir: Option<&Path>,
    copy: bool,
    version: &str,
) -> Result<Installed> {
    let mut record = read_record();
    let mut written = Vec::new();
    let mut refused = Vec::new();
    for h in harnesses(harness)? {
        let (folder, scope) = target(h, project_dir);
        let source_root = atm_home().join("skills").join(h.id);
        for (name, core) in SKILLS {
            let text = skill_text(h, name, core);
            let source = source_root.join(name);
            if scope == "user" && read_skill(&source).as_deref() != Some(text.as_str()) {
                write_skill(&source, &text)?;
            }
            let path = folder.join(name);
            let key = path.shown();
            let mut how = if copy || scope == "project" || !h.links {
                "copy"
            } else {
                "link"
            };
            let ours = record.iter().any(|e| entry_path(e) == key);
            let exists = lexists(&path);
            if exists && !ours {
                refused.push(
                    json!({"path": key, "reason": "is there already, and atm didn't put it there"}),
                );
                continue;
            }
            let action = if (how == "link" && is_link(&path) && same_place(&path, &source))
                || (how == "copy"
                    && !is_link(&path)
                    && read_skill(&path).as_deref() == Some(text.as_str()))
            {
                "unchanged"
            } else {
                remove(&path)?;
                fs::create_dir_all(&folder).map_err(io(&folder))?;
                // No links here (a file system or platform without them): a copy does
                if how == "link" && link(&source, &path).is_err() {
                    how = "copy";
                }
                if how == "copy" {
                    write_skill(&path, &text)?;
                }
                if exists { "updated" } else { "installed" }
            };
            let entry = json!({"path": key, "harness": h.id, "skill": name, "how": how, "scope": scope, "version": version});
            record.retain(|e| entry_path(e) != key);
            record.push(entry.as_object().cloned().unwrap_or_default());
            written.push(
                json!({"harness": h.id, "skill": name, "path": key, "how": how, "action": action}),
            );
        }
    }
    write_record(record)?;
    Ok(Installed { written, refused })
}

/// Remove what install wrote for the harnesses (and, with a project, only that project's): the links or copies, and
/// the user's copies in atm's folder. Returns the paths removed.
///
/// # Errors
/// `Invalid` for a harness atm doesn't know; something that can't be removed.
pub fn uninstall_skills(harness: Option<&str>, project_dir: Option<&Path>) -> Result<Vec<String>> {
    let wanted: Vec<&str> = harnesses(harness)?.into_iter().map(|h| h.id).collect();
    let project =
        // `/`: the paths recorded are as atm shows them, on every platform
        project_dir.map(|p| format!("{}/", absolute(p).shown()));
    let mut removed = Vec::new();
    let mut kept = Vec::new();
    for entry in read_record() {
        let in_harness = entry
            .get("harness")
            .and_then(Value::as_str)
            .is_some_and(|h| wanted.contains(&h));
        let in_scope = in_harness
            && match &project {
                None => entry.get("scope").and_then(Value::as_str) == Some("user"),
                Some(prefix) => entry_path(&entry).starts_with(prefix.as_str()),
            };
        if !in_scope {
            kept.push(entry);
            continue;
        }
        let path = PathBuf::from(entry_path(&entry));
        if lexists(&path) {
            remove(&path)?;
            removed.push(entry_path(&entry).to_string());
        }
    }
    if project.is_none() {
        for h in &wanted {
            let source_root = atm_home().join("skills").join(h);
            if source_root.is_dir() {
                fs::remove_dir_all(&source_root).map_err(io(&source_root))?;
            }
        }
    }
    write_record(kept)?;
    Ok(removed)
}

/// Per harness and scope: whether the harness is installed here, and each skill's state where it reads them:
/// linked, copied, missing, stale (a copy of another version, or a link whose skill is gone) or "not atm's".
/// `project_dir` adds that project's. Each report is {harness, label, detected, scope, folder, version, skills}.
#[must_use]
pub fn skills_status(project_dir: Option<&Path>, version: &str) -> Vec<Value> {
    let record = read_record();
    let mut report = Vec::new();
    for h in HARNESSES {
        let mut scopes: Vec<Option<&Path>> = vec![None];
        if project_dir.is_some() {
            scopes.push(project_dir);
        }
        for where_ in scopes {
            let (folder, scope) = target(h, where_);
            let skills: Vec<Value> = SKILLS
                .iter()
                .map(|(name, core)| {
                    let path = folder.join(name);
                    let key = path.shown();
                    let entry = record.iter().find(|e| entry_path(e) == key);
                    let state = if !lexists(&path) {
                        "missing"
                    } else if entry.is_none() {
                        "not atm's"
                    } else if read_skill(&path).as_deref()
                        != Some(skill_text(h, name, core).as_str())
                    {
                        "stale"
                    } else if is_link(&path) {
                        "linked"
                    } else {
                        "copied"
                    };
                    let installed = entry
                        .and_then(|e| e.get("version"))
                        .cloned()
                        .unwrap_or(Value::Null);
                    json!({"skill": name, "path": key, "state": state, "version": installed})
                })
                .collect();
            report.push(json!({
                "harness": h.id,
                "label": h.label,
                "detected": in_home(h.detect).is_dir(),
                "scope": scope,
                "folder": folder.shown(),
                "version": version,
                "skills": skills,
            }));
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_code_gets_its_pre_approvals_in_the_frontmatter() {
        let (name, core) = SKILLS[0];
        let text = skill_text(&HARNESSES[0], name, core);
        let tools = claude_code_allowed_tools(name).unwrap();
        assert!(text.contains(&format!("\nallowed-tools: {tools}\n---\n")));
        assert!(text.starts_with(&format!("---\nname: {name}\n")));
    }

    #[test]
    fn a_skill_without_a_closing_frontmatter_is_left_as_it_is() {
        assert_eq!(
            skill_text(&HARNESSES[0], SKILLS[0].0, "no frontmatter"),
            "no frontmatter"
        );
    }

    #[test]
    fn an_unknown_harness_names_the_known_ones() {
        assert_eq!(
            harnesses(Some("nope")).unwrap_err(),
            Error::Invalid("no harness 'nope' (atm knows: claude)".into())
        );
        assert_eq!(harnesses(None).unwrap().len(), HARNESSES.len());
    }
}
