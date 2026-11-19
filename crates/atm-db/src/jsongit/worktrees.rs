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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_dir::TestDir;

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn outside_git_or_in_a_bare_repository_there_are_no_worktrees() {
        let dir = TestDir::new();
        assert_eq!(worktrees(dir.path()), Vec::<Value>::new());
        write(&dir.path().join("HEAD"), "ref: refs/heads/main\n");
        assert_eq!(worktrees(dir.path()), Vec::<Value>::new());
        // A `.git` file (the root is itself a linked worktree) isn't a main worktree
        write(&dir.path().join(".git"), "gitdir: elsewhere\n");
        assert_eq!(worktrees(dir.path()), Vec::<Value>::new());
    }

    #[test]
    fn the_main_worktree_comes_first_then_the_linked_ones_by_name_without_those_gone() {
        let dir = TestDir::new();
        let root = dir.path().join("repo");
        let git = root.join(".git");
        write(&git.join("HEAD"), "ref: refs/heads/main\n");
        let linked = |name: &str, gitdir: &str, head: &str| {
            let admin = git.join("worktrees").join(name);
            write(&admin.join("gitdir"), gitdir);
            write(&admin.join("HEAD"), head);
        };
        let (b, a) = (dir.path().join("b"), dir.path().join("a"));
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        linked(
            "b",
            &format!("{}\n", b.join(".git").display()),
            "ref: refs/heads/feature-b\n",
        );
        linked(
            "a",
            &format!("  {}  \n", a.join(".git").display()),
            "0123456789abcdef\n",
        );
        linked(
            "gone",
            &format!("{}\n", dir.path().join("gone/.git").display()),
            "ref: refs/heads/x\n",
        );
        linked("empty", "", "ref: refs/heads/y\n");
        std::fs::create_dir_all(git.join("worktrees/no-gitdir")).unwrap();
        assert_eq!(
            worktrees(&root),
            [
                json!({"path": root.shown(), "branch": "main", "main": true}),
                json!({"path": a.shown(), "branch": "0123456", "main": false}),
                json!({"path": b.shown(), "branch": "feature-b", "main": false}),
            ]
        );
    }

    #[test]
    fn a_main_worktree_without_linked_ones_or_a_head_is_listed_alone_with_no_branch() {
        let dir = TestDir::new();
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        assert_eq!(
            worktrees(dir.path()),
            [json!({"path": dir.path().shown(), "branch": null, "main": true})]
        );
    }
}
