//! The user's folders and atomic writes, as the Python atm's `common/core.py` has them: the home folder, atm's own
//! folder (`ATM_HOME`, else `~/.agentic-task-manager`), and replacing a file so readers never see half of it.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// The user's home folder (`HOME`, or the platform's own), or None when there isn't one.
#[must_use]
pub fn home_dir() -> Option<PathBuf> {
    std::env::home_dir()
}

/// `~/<rest>` as a path: in the home folder, or the text as it is when there is none.
#[must_use]
pub fn in_home(rest: &str) -> PathBuf {
    home_dir().map_or_else(
        || PathBuf::from(format!("~/{rest}")),
        |home| home.join(rest),
    )
}

/// The user's atm folder: `ATM_HOME` when set, else `~/.agentic-task-manager` (the registry, the server's record,
/// the installed skills).
#[must_use]
pub fn atm_home() -> PathBuf {
    match std::env::var_os("ATM_HOME").filter(|v| !v.is_empty()) {
        Some(home) => absolute(&expand(Path::new(&home))),
        None => in_home(".agentic-task-manager"),
    }
}

/// A leading `~` as the home folder.
fn expand(path: &Path) -> PathBuf {
    match (path.strip_prefix("~"), home_dir()) {
        (Ok(rest), Some(home)) => home.join(rest),
        _ => path.to_path_buf(),
    }
}

/// The path from the current folder, without resolving links (Python's `os.path.abspath`).
#[must_use]
pub fn absolute(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Create atm's folder, readable by its owner only: it lists the user's projects and the server's pid and port
/// (docs/security-review.md F18). A folder that exists already keeps its permissions.
///
/// # Errors
/// When the folder can't be created.
pub fn ensure_home(home: &Path) -> std::io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(home)
}

/// Replace a file so readers only ever see the complete old or the complete new content.
///
/// # Errors
/// When the file can't be written or moved into place (the partial file is removed).
pub fn write_text_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    let name = path
        .file_name()
        .map_or_else(|| "file".into(), |n| n.to_string_lossy().into_owned());
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    let partial = path.with_file_name(format!("{name}.{}.{nanos:08x}.tmp", std::process::id()));
    let written = (|| {
        let mut file = fs::File::create(&partial)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        fs::rename(&partial, path)
    })();
    if written.is_err() {
        let _ = fs::remove_file(&partial);
    }
    written
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_is_replaced_whole() {
        let dir = std::env::temp_dir().join(format!("atm-common-home-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("x.json");
        write_text_atomic(&file, "one\n").unwrap();
        write_text_atomic(&file, "two\n").unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "two\n");
        assert_eq!(
            fs::read_dir(&dir).unwrap().count(),
            1,
            "no partial file is left"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn atm_home_is_in_the_home_folder_by_default() {
        assert!(in_home(".agentic-task-manager").ends_with(".agentic-task-manager"));
        assert!(absolute(Path::new("x")).is_absolute());
    }
}
