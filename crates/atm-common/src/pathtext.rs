//! How atm writes a path, in its output and its files, the same on every platform (T-480): `/` between the parts, as
//! git prints them (Windows takes `/` everywhere), and on Windows the real, long form of a path, never an 8.3 short
//! name (`RUNNER~1`), so that two spellings of one folder compare equal.

use std::path::{Path, PathBuf};

/// A path as atm prints and stores it: `/` between its parts. On Unix the path as it is.
pub trait Shown {
    /// The path's text, with `/` between its parts.
    fn shown(&self) -> String;
}

impl Shown for Path {
    fn shown(&self) -> String {
        slashed(&self.display().to_string())
    }
}

impl Shown for PathBuf {
    fn shown(&self) -> String {
        self.as_path().shown()
    }
}

/// `text` with `\` turned into `/` on Windows (where it is a separator); as it is elsewhere.
#[must_use]
pub fn slashed(text: &str) -> String {
    if cfg!(windows) {
        text.replace('\\', "/")
    } else {
        text.to_string()
    }
}

/// On Windows, `path`'s real form: the long names of its folders (no 8.3 short names) and its drive letter as the
/// system has it, without the `\\?\` prefix `canonicalize` adds, with `/` between the parts; the path as it is when
/// it doesn't exist. On Unix the path as it is: atm keeps the folders it is given (a symlinked folder stays itself).
#[must_use]
pub fn real(path: &Path) -> PathBuf {
    if !cfg!(windows) {
        return path.to_path_buf();
    }
    let Ok(canonical) = std::fs::canonicalize(path) else {
        return PathBuf::from(slashed(&path.display().to_string()));
    };
    let text = canonical.display().to_string();
    let plain = if let Some(unc) = text.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{unc}")
    } else {
        text.strip_prefix(r"\\?\").unwrap_or(&text).to_string()
    };
    PathBuf::from(slashed(&plain))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_shows_with_slashes() {
        let path = Path::new("a").join("b").join("c.json");
        assert_eq!(path.shown(), "a/b/c.json");
    }

    #[test]
    fn a_missing_path_is_kept() {
        let missing = Path::new("no/such/folder");
        assert_eq!(real(missing).shown(), "no/such/folder");
    }
}
