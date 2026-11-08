//! The JSON-in-git store's working copies (the Python atm's `db/jsongit/worktrees.py`): a clone's git worktrees, each
//! with its own `tasks.json` (the main worktree's is the project's; the others show on the board as worktree copies).
//! Read from the git folder, without running git, so the server can ask on every poll.

use std::path::Path;

use atm_common::Shown;

use atm_common::py::strip;
use serde_json::{Value, json};

use crate::git::head_branch;

/// The git worktrees of the project whose main worktree is `root`, main first: `[{path, branch, main}]`. Linked
/// worktrees whose folder is gone are left out; outside git (or a bare repository) the list is empty.
#[must_use]
pub fn worktrees(root: &Path) -> Vec<Value> {
    let git_dir = root.join(".git");
    if !git_dir.is_dir() {
        return Vec::new();
    }
    let mut found =
        vec![json!({"path": root.shown(), "branch": head_branch(&git_dir), "main": true})];
    let linked = git_dir.join("worktrees");
    let mut names: Vec<String> = std::fs::read_dir(&linked)
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    for name in names {
        let admin = linked.join(&name);
        let Ok(text) = std::fs::read_to_string(admin.join("gitdir")) else {
            continue;
        };
        // The worktree's `.git` file
        let line = strip(text.split('\n').next().unwrap_or_default());
        let Some(path) = Path::new(line)
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
        else {
            continue;
        };
        if path.is_dir() {
            found.push(json!({"path": path.shown(), "branch": head_branch(&admin), "main": false}));
        }
    }
    found
}
