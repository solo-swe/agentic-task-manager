//! A round's patch, cut into its files, a submodule's move known by its commits (ATM-599).

/// One file of a patch: its paths, how it changed, and its part of the patch (from its `diff --git` line).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePatch {
    /// Its path after.
    pub path: String,
    /// Its path before (the same unless it moved).
    pub old_path: String,
    /// `added`, `modified`, `deleted` or `renamed`.
    pub status: &'static str,
    /// Binary: listed, no lines.
    pub binary: bool,
    /// A submodule this change moved (ATM-583): the commits it was at and is at (none where it was added or
    /// removed), a `-dirty` mark dropped; none for a file.
    pub submodule: Option<(Option<String>, Option<String>)>,
    /// Its part of the patch.
    pub patch: String,
}

/// A submodule's move, from its part of the patch: git writes it as `-Subproject commit <old>` and `+Subproject
/// commit <new>`, no file lines.
pub(super) fn moved_submodule(lines: &[&str]) -> Option<(Option<String>, Option<String>)> {
    let commit = |sign: char| {
        lines.iter().find_map(|l| {
            let rest = l.trim_end_matches('\n').strip_prefix(sign)?;
            let sha = rest.strip_prefix("Subproject commit ")?;
            Some(sha.trim_end_matches("-dirty").to_string())
        })
    };
    let gitlink = lines
        .iter()
        .take_while(|l| !l.starts_with("@@"))
        .any(|l| l.contains(" 160000"));
    let (old, new) = (commit('-'), commit('+'));
    (gitlink && (old.is_some() || new.is_some())).then_some((old, new))
}

/// The two paths of a `diff --git a/<p> b/<p>` line, when it names one path twice (a path with a space included).
pub(super) fn paths_of_header(rest: &str) -> Option<String> {
    let len = rest.len().checked_sub(5)?;
    if len % 2 != 0 {
        return None;
    }
    let path = rest.get(2..2 + len / 2)?;
    (rest == format!("a/{path} b/{path}")).then(|| path.to_string())
}

/// A patch, cut per file.
#[must_use]
pub fn file_patches(patch: &str) -> Vec<FilePatch> {
    let mut out: Vec<FilePatch> = Vec::new();
    let mut lines: Vec<&str> = Vec::new();
    let mut finish = |lines: &mut Vec<&str>| {
        if let Some(file) = one_file(lines) {
            out.push(file);
        }
        lines.clear();
    };
    for line in patch.split_inclusive('\n') {
        if line.starts_with("diff --git ") && !lines.is_empty() {
            finish(&mut lines);
        }
        lines.push(line);
    }
    finish(&mut lines);
    out
}

pub(super) fn one_file(lines: &[&str]) -> Option<FilePatch> {
    let header = lines
        .first()?
        .trim_end_matches('\n')
        .strip_prefix("diff --git ")?;
    let (mut old, mut new) = (None::<String>, None::<String>);
    let (mut status, mut binary) = ("modified", false);
    for line in lines.iter().map(|l| l.trim_end_matches('\n')) {
        if line.starts_with("@@") {
            break;
        }
        if line.starts_with("new file mode") {
            status = "added";
        } else if line.starts_with("deleted file mode") {
            status = "deleted";
        } else if let Some(from) = line.strip_prefix("rename from ") {
            (old, status) = (Some(from.to_string()), "renamed");
        } else if let Some(to) = line.strip_prefix("rename to ") {
            new = Some(to.to_string());
        } else if let Some(a) = line.strip_prefix("--- a/") {
            old.get_or_insert_with(|| a.to_string());
        } else if let Some(b) = line.strip_prefix("+++ b/") {
            new.get_or_insert_with(|| b.to_string());
        } else if line.starts_with("Binary files ") || line == "GIT binary patch" {
            binary = true;
        }
    }
    let same = paths_of_header(header);
    let path = new
        .clone()
        .or_else(|| old.clone())
        .or_else(|| same.clone())?;
    Some(FilePatch {
        old_path: old.or(same).unwrap_or_else(|| path.clone()),
        path,
        status,
        binary,
        submodule: moved_submodule(lines),
        patch: lines.concat(),
    })
}

/// A patch's files, a submodule only moved among them known by its sides' trees: renamed with its commit the same, it
/// has no `Subproject commit` lines (ATM-599: `markdown-rs`, moved into `crates/`, showed as a file not in its
/// commit).
pub(super) fn with_links(
    repo: &atm_vcs::GitVcs,
    patches: Vec<FilePatch>,
    before: Option<&String>,
    after: &String,
) -> Vec<FilePatch> {
    // A side is a round's commit or a tree atm rebuilt: a full object ID, or nothing to look up (ATM-659)
    let linked = |object: Option<&String>, path: &str| {
        let object = atm_vcs::Rev::object_id(object?).ok()?;
        repo.gitlink_in(&object, path)
    };
    patches
        .into_iter()
        .map(|mut file| {
            // Only an entry with no lines can be a submodule git didn't mark: one whose commit changed has its
            // `Subproject commit` lines, and `submodule` already. No lines is a pure rename (the moved submodule), a
            // mode change, an empty or a binary file: only those are looked up in the trees, a git process each,
            // never the files with lines, so a large review costs nothing more
            let no_lines = !file.patch.contains("\n@@");
            if file.submodule.is_none() && no_lines {
                let (old, new) = (
                    linked(before, &file.old_path),
                    linked(Some(after), &file.path),
                );
                if old.is_some() || new.is_some() {
                    file.submodule = Some((old, new));
                }
            }
            file
        })
        .collect()
}
