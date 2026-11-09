//! The user's folders and atomic writes: the home folder, atm's state folder (ADR-003 §2: `ATM_STATE_DIR`, else the
//! platform's app data folder for atm's identifier), and replacing a file so readers never see half of it.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// The user's home folder (`HOME`, or the platform's own), or None when there isn't one. `HOME` comes first on
/// Windows too, where Rust's own reads only `USERPROFILE` (Git for Windows sets `HOME` to the same folder).
#[must_use]
pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
        .or_else(std::env::home_dir)
}

/// `~/<rest>` as a path: in the home folder, or the text as it is when there is none.
#[must_use]
pub fn in_home(rest: &str) -> PathBuf {
    home_dir().map_or_else(
        || PathBuf::from(format!("~/{rest}")),
        |home| home.join(rest),
    )
}

/// atm's identifier, the name of its state folder in the platform's app data folder; the app's `tauri.conf.json` has
/// the same (checked by the app's tests). Fixed: changing it would move everyone's data (ADR-003).
pub const APP_ID: &str = "com.solo-swe.atm";

/// Where atm kept its state before ADR-003, in the home folder.
pub const LEGACY_FOLDER: &str = ".agentic-task-manager";

/// The note left in the legacy folder once its content moved.
pub const MOVED_NOTE: &str = "MOVED.txt";

/// atm's state folder (the workspaces, the server's record, the installed skills; ADR-003 §2): `ATM_STATE_DIR` when
/// set, else `ATM_HOME` (its older name), else the platform's app data folder for [`APP_ID`]
/// (`~/Library/Application Support/…` on macOS, `$XDG_DATA_HOME/…` on Linux, `%APPDATA%\…` on Windows). Without
/// either variable, the content of `~/.agentic-task-manager` is moved there the first time (`move_legacy_state`).
#[must_use]
pub fn state_dir() -> PathBuf {
    let set = ["ATM_STATE_DIR", "ATM_HOME"]
        .iter()
        .find_map(|name| std::env::var_os(name).filter(|v| !v.is_empty()));
    if let Some(folder) = set {
        return absolute(&expand(Path::new(&folder)));
    }
    let state = platform_state_dir();
    move_legacy_state(&in_home(LEGACY_FOLDER), &state);
    state
}

/// The legacy folder an older atm may still write to, when `home` is the state folder its content moved to (T-501):
/// `~/.agentic-task-manager` once it holds the move's note, and only for the platform's state folder, never one set
/// with `ATM_STATE_DIR` or `ATM_HOME` (a test's or another setup's must never take in the user's projects).
#[must_use]
pub fn legacy_folder_of(home: &Path) -> Option<PathBuf> {
    let overridden = ["ATM_STATE_DIR", "ATM_HOME"]
        .iter()
        .any(|name| std::env::var_os(name).is_some_and(|v| !v.is_empty()));
    let legacy = in_home(LEGACY_FOLDER);
    (!overridden && home == platform_state_dir() && legacy.join(MOVED_NOTE).is_file())
        .then_some(legacy)
}

/// The platform's app data folder for atm (the user's home folder's when the platform names none).
fn platform_state_dir() -> PathBuf {
    dirs::data_dir().map_or_else(|| in_home(APP_ID), |data| data.join(APP_ID))
}

/// Move the legacy folder's content to `state`, once: only while `state` doesn't exist yet. The folder itself is
/// renamed (one atomic step, so two atm processes starting together can't both move it), then made again holding a
/// note that says where its content went. Nothing happens when it can't be moved (another volume, a file in use on
/// Windows): atm then starts with an empty state folder, and the note isn't written.
pub fn move_legacy_state(legacy: &Path, state: &Path) {
    if state.exists() || !legacy.is_dir() || legacy.join(MOVED_NOTE).exists() {
        return;
    }
    if let Some(parent) = state.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if fs::rename(legacy, state).is_err() {
        return;
    }
    if ensure_home(legacy).is_ok() {
        let note = format!(
            "atm's state (its workspaces, the dashboard server's record, the installed skills) moved to\n{}\n\
             Set ATM_STATE_DIR to use another folder.\n",
            state.display()
        );
        let _ = write_text_atomic(&legacy.join(MOVED_NOTE), &note);
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
    fn the_state_folder_is_named_by_the_identifier() {
        assert!(platform_state_dir().ends_with(APP_ID));
        assert!(absolute(Path::new("x")).is_absolute());
    }

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("atm-common-move-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_legacy_folder_moves_once_and_leaves_a_note() {
        let dir = scratch("once");
        let (legacy, state) = (dir.join("legacy"), dir.join("state/com.solo-swe.atm"));
        fs::create_dir_all(legacy.join("skills/claude-code")).unwrap();
        fs::write(legacy.join("projects.json"), "{}").unwrap();
        fs::write(legacy.join("server.json"), "{}").unwrap();
        move_legacy_state(&legacy, &state);
        assert!(state.join("projects.json").exists() && state.join("server.json").exists());
        assert!(state.join("skills/claude-code").is_dir());
        let note = fs::read_to_string(legacy.join(MOVED_NOTE)).unwrap();
        assert!(note.contains("com.solo-swe.atm"));
        assert_eq!(
            fs::read_dir(&legacy).unwrap().count(),
            1,
            "only the note is left"
        );
        // A second run: the state folder exists, nothing moves again
        fs::write(legacy.join("projects.json"), "{\"again\": 1}").unwrap();
        move_legacy_state(&legacy, &state);
        assert_eq!(
            fs::read_to_string(state.join("projects.json")).unwrap(),
            "{}"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    /// T-501: only the platform's state folder looks at the legacy one; any other (a test's, an `ATM_STATE_DIR`) never
    #[test]
    fn only_the_platforms_state_folder_has_a_legacy_folder() {
        assert_eq!(legacy_folder_of(&scratch("other-home")), None);
        assert_eq!(legacy_folder_of(Path::new("relative")), None);
    }

    #[test]
    fn nothing_moves_without_a_legacy_folder_or_into_an_existing_state() {
        let dir = scratch("none");
        let (legacy, state) = (dir.join("legacy"), dir.join("state"));
        move_legacy_state(&legacy, &state);
        assert!(!state.exists(), "no legacy folder: nothing made");
        fs::create_dir_all(&legacy).unwrap();
        fs::write(legacy.join("projects.json"), "{}").unwrap();
        fs::create_dir_all(&state).unwrap();
        move_legacy_state(&legacy, &state);
        assert!(legacy.join("projects.json").exists() && !legacy.join(MOVED_NOTE).exists());
        fs::remove_dir_all(&dir).unwrap();
    }
}
