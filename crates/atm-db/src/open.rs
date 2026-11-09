//! Opening a project's store from a folder in it (ADR-003 §4): the project's settings file says which store it uses,
//! and this factory builds it. Finding the project (walking up to `.agentic-task-manager/`, a linked worktree's main
//! copy) is the store's own business, so it happens here, behind the factory.

use std::path::{Path, PathBuf};

use atm_common::{Error, Result, Shown, TASK_DIR};
use serde_json::Value;

use crate::Store;
use crate::jsongit::JsonGitStore;
use crate::jsongit::paths::{check_task_folder, find_paths};

/// The project's settings file, in its `.agentic-task-manager/` folder: atm's, read before any store is chosen.
pub const SETTINGS_FILE: &str = "settings.json";
/// The store a project without a settings file uses.
pub const DEFAULT_STORE: &str = "json-git";
/// The version control a project without a settings file uses.
pub const DEFAULT_VCS: &str = "git";
/// The stores atm knows.
pub const STORES: [&str; 1] = [DEFAULT_STORE];
/// A settings file larger than this is refused unread.
const MAX_SETTINGS_BYTES: u64 = 64 * 1024;

/// What a project's settings file says: which store and which version control it uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectSettings {
    /// The store (`json-git`).
    pub store: String,
    /// The version control (`git`).
    pub vcs: String,
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            store: DEFAULT_STORE.into(),
            vcs: DEFAULT_VCS.into(),
        }
    }
}

/// The settings in the task folder `dir`, the defaults when it has none.
///
/// # Errors
/// A settings file that is a link, too large, not a JSON object, or names a store or vcs that isn't text.
pub fn read_settings(dir: &Path) -> Result<ProjectSettings> {
    let file = dir.join(SETTINGS_FILE);
    let Ok(meta) = std::fs::symlink_metadata(&file) else {
        return Ok(ProjectSettings::default());
    };
    let refuse = |why: &str| Error::Invalid(format!("{}: {why}", file.shown()));
    if meta.file_type().is_symlink() || !meta.is_file() {
        return Err(refuse(
            "not a plain file (atm doesn't follow links in the task folder)",
        ));
    }
    if meta.len() > MAX_SETTINGS_BYTES {
        return Err(refuse("too large for a settings file"));
    }
    let text = std::fs::read_to_string(&file).map_err(|e| refuse(&e.to_string()))?;
    let Ok(Value::Object(settings)) = serde_json::from_str::<Value>(&text) else {
        return Err(refuse("not a JSON object"));
    };
    let field = |key: &str, default: &str| match settings.get(key) {
        None | Some(Value::Null) => Ok(default.to_string()),
        Some(Value::String(value)) if !value.is_empty() => Ok(value.clone()),
        Some(_) => Err(refuse(&format!("\"{key}\" isn't a name"))),
    };
    Ok(ProjectSettings {
        store: field("store", DEFAULT_STORE)?,
        vcs: field("vcs", DEFAULT_VCS)?,
    })
}

/// A project's store, opened from a folder in it, with where the project is and what its settings say.
pub struct Opened {
    /// The store.
    pub store: Box<dyn Store>,
    /// The project's root: its working copy's top folder.
    pub root: PathBuf,
    /// Its settings.
    pub settings: ProjectSettings,
}

/// Open the store of the project at or above `cwd`, as its settings file says (JSON-in-git without one). With
/// `tolerant`, a malformed task file is read as far as it can be, saying what is wrong instead of refusing it (T-377).
/// Nothing is read but the settings until the project's data is asked for.
///
/// # Errors
/// The task folder's checks (a symlink, or one leading outside the project), an unusable settings file, a store atm
/// doesn't know.
pub fn open(cwd: &Path, tolerant: bool) -> Result<Opened> {
    let paths = find_paths(cwd)?;
    // The folder's checks run on every open, so a file linked after the paths were found is refused too
    check_task_folder(&paths.root, &paths.dir)?;
    let settings = read_settings(&paths.dir)?;
    if !STORES.contains(&settings.store.as_str()) {
        return Err(Error::Invalid(format!(
            "{}: the store {:?} isn't one atm knows ({})",
            paths.dir.join(SETTINGS_FILE).shown(),
            settings.store,
            STORES.join(", ")
        )));
    }
    let root = paths.root.clone();
    let store: Box<dyn Store> = if tolerant {
        Box::new(JsonGitStore::tolerant(paths))
    } else {
        Box::new(JsonGitStore::new(paths))
    };
    Ok(Opened {
        store,
        root,
        settings,
    })
}

/// The task folder's name, for messages: where a project's settings file is.
#[must_use]
pub fn settings_path(root: &Path) -> PathBuf {
    root.join(TASK_DIR).join(SETTINGS_FILE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_dir::TestDir;

    #[test]
    fn without_a_settings_file_a_project_is_json_git_on_git() {
        let dir = TestDir::new();
        std::fs::create_dir_all(dir.path().join(TASK_DIR)).unwrap();
        let opened = open(dir.path(), false).unwrap();
        assert_eq!(opened.settings, ProjectSettings::default());
        assert_eq!(opened.root, dir.path());
    }

    #[test]
    fn the_settings_file_names_the_store_and_one_atm_doesnt_know_is_refused() {
        let dir = TestDir::new();
        let task_dir = dir.path().join(TASK_DIR);
        std::fs::create_dir_all(&task_dir).unwrap();
        std::fs::write(
            task_dir.join(SETTINGS_FILE),
            r#"{"store": "json-git", "vcs": "git"}"#,
        )
        .unwrap();
        assert!(open(dir.path(), true).is_ok());
        std::fs::write(task_dir.join(SETTINGS_FILE), r#"{"store": "cloud"}"#).unwrap();
        let refused = open(dir.path(), false).err().unwrap().to_string();
        assert!(
            refused.contains("isn't one atm knows (json-git)"),
            "{refused}"
        );
        for bad in ["[1]", "{\"store\": 3}", "not json"] {
            std::fs::write(task_dir.join(SETTINGS_FILE), bad).unwrap();
            assert!(
                matches!(read_settings(&task_dir), Err(Error::Invalid(_))),
                "{bad}"
            );
        }
    }
}
