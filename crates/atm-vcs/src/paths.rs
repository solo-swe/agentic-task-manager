//! Paths worked out as Python's `os.path` does, without git.

use std::path::{Component, Path, PathBuf};

/// `..` and `.` worked out without reading links, as Python's `normpath` does.
pub(super) fn normalize(path: &Path) -> PathBuf {
    let mut normal = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                normal.pop();
            }
            Component::CurDir => {}
            other => normal.push(other),
        }
    }
    normal
}

/// The path with its links resolved, as Python's `realpath`: for one that doesn't exist, its nearest existing folder's.
pub(super) fn real_path(path: &Path) -> PathBuf {
    // atm-common's `canonical`: on Windows without `\\?\`, so it compares with the root
    let normal = normalize(path);
    if let Ok(real) = atm_common::pathtext::canonical(&normal) {
        return real;
    }
    let mut missing = Vec::new();
    let mut existing = normal.as_path();
    while let Some(parent) = existing.parent() {
        missing.push(existing.file_name().map(PathBuf::from).unwrap_or_default());
        existing = parent;
        if let Ok(real) = atm_common::pathtext::canonical(existing) {
            return missing.iter().rev().fold(real, |p, part| p.join(part));
        }
    }
    normal
}

/// `path` relative to `base`, `/`-separated, as Python's `relpath` gives it (`..` for each step up).
pub(super) fn relative_to(path: &Path, base: &Path) -> String {
    let (path, base): (Vec<Component>, Vec<Component>) =
        (path.components().collect(), base.components().collect());
    let common = path.iter().zip(&base).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<String> = vec!["..".into(); base.len() - common];
    parts.extend(
        path[common..]
            .iter()
            .map(|c| c.as_os_str().to_string_lossy().into_owned()),
    );
    if parts.is_empty() {
        ".".into()
    } else {
        parts.join("/")
    }
}
