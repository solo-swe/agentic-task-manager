//! Paths worked out as Python does.

use super::*;
use crate::paths::normalize;

#[test]
fn paths_are_made_relative_as_relpath_does() {
    assert_eq!(
        relative_to(Path::new("/r/a/b.txt"), Path::new("/r")),
        "a/b.txt"
    );
    assert_eq!(relative_to(Path::new("/x"), Path::new("/r")), "../x");
    assert_eq!(relative_to(Path::new("/r"), Path::new("/r")), ".");
    assert_eq!(normalize(Path::new("/r/a/../b")), PathBuf::from("/r/b"));
}
