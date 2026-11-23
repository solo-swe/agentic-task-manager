//! Committing: the paths to commit (exactly the files named), the changes under a folder, the commit itself.

use std::path::Path;

use atm_common::py::repr_str;
use atm_common::{Error, Result};

use crate::{GitVcs, LITERAL, WHOLE_TREE, ask, real_path, relative_to, step};

impl GitVcs {
    /// Whether git knows the file (tracked, or staged).
    pub(crate) fn known(&self, relative: &str) -> bool {
        ask(
            &[LITERAL, "ls-files", "--error-unmatch", "--", relative],
            &self.root,
        )
        .is_some()
    }

    /// The files to commit, relative to the repository, from names relative to `cwd` (none: the repository).
    ///
    /// # Errors
    /// A name that would stage more than itself, one outside the repository, a folder, a missing file git doesn't
    /// track.
    pub fn paths(&self, files: &[String], cwd: Option<&Path>) -> Result<Vec<String>> {
        let mut picked: Vec<String> = Vec::new();
        for name in files {
            if WHOLE_TREE.contains(&name.as_str()) || name.starts_with(':') {
                return Err(Error::Invalid(format!(
                    "atm vcs commit stages only the files you name; {} would stage the whole tree",
                    repr_str(name)
                )));
            }
            let full = real_path(&cwd.unwrap_or(&self.root).join(name));
            let relative = relative_to(&full, &self.root);
            if relative == "." || relative == ".." || relative.starts_with("../") {
                return Err(Error::Invalid(format!("{name} is outside the repository")));
            }
            if full.is_dir() {
                return Err(Error::Invalid(format!(
                    "{name} is a directory: name the files in it"
                )));
            }
            // A deleted file is fine as long as git knows it: still in the index (deleted by hand) or in HEAD (`git rm`)
            if !full.exists()
                && !self.known(&relative)
                && ask(&["cat-file", "-e", &format!("HEAD:{relative}")], &self.root).is_none()
            {
                return Err(Error::failed(format!(
                    "{name}: no such file, and git doesn't track it"
                )));
            }
            if !picked.contains(&relative) {
                picked.push(relative);
            }
        }
        Ok(picked)
    }

    /// `atm vcs commit --dir` (T-405): every change git sees under each folder (named relative to `cwd`), as `git
    /// status` lists it (renames with both names, new files git doesn't ignore), relative to the repository, in order.
    ///
    /// # Errors
    /// The repository's root or above, a folder outside it, the task folder, a file, a folder with nothing changed.
    pub fn changes_under(&self, folders: &[String], cwd: Option<&Path>) -> Result<Vec<String>> {
        let mut picked: Vec<String> = Vec::new();
        for name in folders {
            let full = real_path(&cwd.unwrap_or(&self.root).join(name));
            let relative = relative_to(&full, &self.root);
            if relative == "." || relative == ".." || relative.starts_with("../") {
                return Err(Error::Invalid(format!(
                    "--dir {name}: name a folder inside the repository, not the repository itself or above it"
                )));
            }
            if relative == atm_common::TASK_DIR
                || relative.starts_with(&format!("{}/", atm_common::TASK_DIR))
            {
                return Err(Error::Invalid(format!(
                    "--dir {name}: the task folder isn't committed this way (tasks.json goes with every commit)"
                )));
            }
            if full.is_file() {
                return Err(Error::Invalid(format!(
                    "--dir {name} is a file: name it with --file"
                )));
            }
            let status = step(
                &[
                    LITERAL,
                    "status",
                    "--porcelain",
                    "-z",
                    "--untracked-files=all",
                    "--",
                    &relative,
                ],
                &self.root,
                true,
            )?;
            let entries: Vec<&str> = status.split('\0').filter(|e| !e.is_empty()).collect();
            let mut found: Vec<String> = Vec::new();
            let mut i = 0;
            while i < entries.len() {
                let entry = entries[i];
                i += 1;
                if entry.len() < 4 {
                    continue;
                }
                let (code, path) = (&entry[..2], &entry[3..]);
                found.push(path.to_string());
                // A rename or a copy: the next entry is the name it had, which goes in the commit too
                if code.contains('R') || code.contains('C') {
                    if let Some(old) = entries.get(i) {
                        found.push((*old).to_string());
                    }
                    i += 1;
                }
            }
            found.retain(|p| *p == relative || p.starts_with(&format!("{relative}/")));
            if found.is_empty() {
                return Err(Error::failed(format!(
                    "--dir {name}: nothing changed under it"
                )));
            }
            for path in found {
                if !picked.contains(&path) {
                    picked.push(path);
                }
            }
        }
        Ok(picked)
    }

    /// Commit exactly `files` (relative to the repository); whatever else is staged stays staged and out of this
    /// commit. Returns the new commit's SHA.
    ///
    /// # Errors
    /// What git says when it refuses (a hook, nothing to commit).
    pub fn commit(&self, files: &[String], message: &str) -> Result<String> {
        // A deletion already staged with `git rm` is in neither the tree nor the index: there is nothing to add for it
        let to_add: Vec<&str> = files
            .iter()
            .filter(|f| self.known(f) || std::fs::symlink_metadata(self.root.join(f)).is_ok())
            .map(String::as_str)
            .collect();
        if !to_add.is_empty() {
            let mut args = vec![LITERAL, "add", "-A", "--"];
            args.extend(to_add);
            step(&args, &self.root, false)?;
        }
        let mut args = vec![LITERAL, "commit", "-q", "-m", message, "--"];
        args.extend(files.iter().map(String::as_str));
        step(&args, &self.root, false)?;
        step(&["rev-parse", "HEAD"], &self.root, false)
    }
}
