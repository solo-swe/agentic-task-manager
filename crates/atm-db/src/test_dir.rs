//! A folder of its own for each test, removed when the test ends.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT: AtomicU32 = AtomicU32::new(0);

pub(crate) struct TestDir(PathBuf);

impl TestDir {
    pub(crate) fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let path = std::env::temp_dir().join(format!(
            "atm-db-test-{}-{}-{nanos}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).expect("the system's temporary folder is writable");
        Self(plain(
            &std::fs::canonicalize(&path).expect("the folder was just made"),
        ))
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

/// `path` without Windows' verbatim prefix (`\\?\C:\…`, which `canonicalize` gives there): the form atm and git name the
/// folder by (`C:\…`), so a path atm returns compares equal, and git takes it as a submodule's URL. Elsewhere, as it is.
fn plain(path: &Path) -> PathBuf {
    path.to_str()
        .and_then(|p| p.strip_prefix(r"\\?\"))
        .filter(|p| !p.starts_with("UNC\\"))
        .map_or_else(|| path.to_path_buf(), PathBuf::from)
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
