//! The project's logo for the dashboard (the Python atm's `db/jsongit/logo.py`): the image `meta.logo` names (a path
//! relative to the project root), or else one found in the usual places. `atm service` serves it at `/p/<id>/logo`
//! (docs/multi-project-api.md).

use std::path::{Path, PathBuf};

use atm_common::{Error, Result};
use serde_json::{Map, Value};

/// The image types a logo can be, by extension, with their content types.
const LOGO_TYPES: [(&str, &str); 4] = [
    (".svg", "image/svg+xml"),
    (".png", "image/png"),
    (".webp", "image/webp"),
    (".ico", "image/x-icon"),
];
/// The largest logo served.
const MAX_LOGO_BYTES: u64 = 512 * 1024;

/// Where a logo usually is, first match wins: logo.svg and logo.png in the root, then `.github/logo.*`,
/// `public/favicon.*` and `assets/logo.*`, each in the order of [`LOGO_TYPES`].
fn candidates() -> Vec<String> {
    let mut list = vec!["logo.svg".to_string(), "logo.png".to_string()];
    for base in [".github/logo", "public/favicon", "assets/logo"] {
        list.extend(LOGO_TYPES.iter().map(|(ext, _)| format!("{base}{ext}")));
    }
    list
}

fn refused(message: String) -> Error {
    Error::Failed { message, code: 2 }
}

/// A logo the project has: its path in the project, the file, its content type, and whether `meta.logo` set it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Logo {
    /// Its path, relative to the project root.
    pub path: String,
    /// The file.
    pub file: PathBuf,
    /// Its content type.
    pub content_type: &'static str,
    /// `set` (`meta.logo`) or `found` (in the usual places).
    pub source: &'static str,
}

/// The file and content type of the image at `relative` in the project at `root`.
///
/// # Errors
/// Exit code 2 when it isn't a supported image type, is outside the project (symbolic links followed), is missing or
/// too large.
pub fn logo_file(root: &Path, relative: &str) -> Result<(PathBuf, &'static str)> {
    if relative.trim().is_empty() {
        return Err(refused("no logo path given".into()));
    }
    let lower = relative.to_lowercase();
    let extension = lower
        .rfind('.')
        .map(|i| &lower[i..])
        .filter(|e| !e.contains('/'));
    let Some((_, content_type)) = LOGO_TYPES.iter().find(|(ext, _)| Some(*ext) == extension) else {
        return Err(refused(format!(
            "{relative}: a logo is an SVG, PNG, WebP or ICO image"
        )));
    };
    let real_root = atm_common::pathtext::canonical(root).unwrap_or_else(|_| root.to_path_buf());
    let joined = real_root.join(relative);
    // A missing file can't be resolved: its folder decides whether it is inside
    let full = atm_common::pathtext::canonical(&joined).unwrap_or_else(|_| {
        let normal = normalize(&joined);
        match (normal.parent(), normal.file_name()) {
            (Some(parent), Some(name)) => {
                atm_common::pathtext::canonical(parent).map_or(normal.clone(), |p| p.join(name))
            }
            _ => normal,
        }
    });
    if !full.starts_with(&real_root) {
        return Err(refused(format!(
            "{relative}: the logo must be inside the project folder"
        )));
    }
    if !full.is_file() {
        return Err(refused(format!("{relative}: no such file")));
    }
    if std::fs::metadata(&full).map_or(0, |m| m.len()) > MAX_LOGO_BYTES {
        return Err(refused(format!(
            "{relative}: the logo is larger than {} KB",
            MAX_LOGO_BYTES / 1024
        )));
    }
    Ok((full, content_type))
}

/// `..` and `.` worked out without reading links, as Python's `normpath` does.
#[must_use]
pub fn normalize(path: &Path) -> PathBuf {
    let mut normal = PathBuf::new();
    for part in path.components() {
        match part {
            std::path::Component::ParentDir => {
                normal.pop();
            }
            std::path::Component::CurDir => {}
            other => normal.push(other),
        }
    }
    normal
}

/// The project's logo: `meta.logo` wins when it is set (one it names that is unusable means no logo); otherwise the
/// first usable file of the usual places.
#[must_use]
pub fn find_logo(root: &Path, meta: &Map<String, Value>) -> Option<Logo> {
    if let Some(set) = meta.get("logo").filter(|l| atm_common::py::truthy(l)) {
        let path = atm_common::py::py_str(set);
        return logo_file(root, &path)
            .ok()
            .map(|(file, content_type)| Logo {
                path,
                file,
                content_type,
                source: "set",
            });
    }
    candidates().into_iter().find_map(|path| {
        logo_file(root, &path)
            .ok()
            .map(|(file, content_type)| Logo {
                path,
                file,
                content_type,
                source: "found",
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_dir::TestDir;
    use serde_json::json;

    fn put(root: &Path, relative: &str, bytes: usize) {
        let file = root.join(relative);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, vec![b'x'; bytes]).unwrap();
    }

    fn meta(value: Value) -> Map<String, Value> {
        match value {
            Value::Object(map) => map,
            other => panic!("not an object: {other}"),
        }
    }

    #[test]
    fn a_logo_is_one_of_four_image_types_by_extension_whatever_its_case() {
        let project = TestDir::new();
        for (name, content_type) in [
            ("a.svg", "image/svg+xml"),
            ("b.PNG", "image/png"),
            ("c.webp", "image/webp"),
            ("d.ico", "image/x-icon"),
        ] {
            put(project.path(), name, 1);
            let (file, found) = logo_file(project.path(), name).unwrap();
            assert_eq!(found, content_type, "{name}");
            assert_eq!(file, project.path().join(name));
        }
    }

    #[test]
    fn a_logo_path_that_is_empty_not_an_image_outside_missing_or_too_large_is_refused_with_code_2()
    {
        let project = TestDir::new();
        put(project.path(), "logo.jpg", 1);
        put(project.path(), "logo", 1);
        put(project.path(), "big.png", 512 * 1024 + 1);
        put(project.path(), "limit.png", 512 * 1024);
        assert!(
            logo_file(project.path(), "limit.png").is_ok(),
            "at the limit"
        );
        let outside = TestDir::new();
        put(outside.path(), "logo.svg", 1);
        let escape = format!(
            "../{}/logo.svg",
            outside.path().file_name().unwrap().to_string_lossy()
        );
        // (the path, the message)
        for (path, message) in [
            ("", "no logo path given".to_string()),
            ("  ", "no logo path given".to_string()),
            (
                "logo.jpg",
                "logo.jpg: a logo is an SVG, PNG, WebP or ICO image".to_string(),
            ),
            (
                "logo",
                "logo: a logo is an SVG, PNG, WebP or ICO image".to_string(),
            ),
            (
                "a.png/logo",
                "a.png/logo: a logo is an SVG, PNG, WebP or ICO image".to_string(),
            ),
            ("missing.svg", "missing.svg: no such file".to_string()),
            (
                "no/folder/missing.svg",
                "no/folder/missing.svg: no such file".to_string(),
            ),
            (
                "big.png",
                "big.png: the logo is larger than 512 KB".to_string(),
            ),
            (
                escape.as_str(),
                format!("{escape}: the logo must be inside the project folder"),
            ),
            (
                "../missing.svg",
                "../missing.svg: the logo must be inside the project folder".to_string(),
            ),
        ] {
            let error = logo_file(project.path(), path).unwrap_err();
            assert_eq!(error.to_string(), message, "{path:?}");
            assert_eq!(error.code(), 2, "{path:?}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_logo_linked_from_outside_the_project_is_refused() {
        let (project, outside) = (TestDir::new(), TestDir::new());
        put(outside.path(), "logo.svg", 1);
        std::os::unix::fs::symlink(
            outside.path().join("logo.svg"),
            project.path().join("logo.svg"),
        )
        .unwrap();
        let error = logo_file(project.path(), "logo.svg").unwrap_err();
        assert_eq!(
            error.to_string(),
            "logo.svg: the logo must be inside the project folder"
        );
    }

    #[test]
    fn normalize_works_out_dots_without_reading_the_disk() {
        for (path, normal) in [
            ("/a/./b/../c", "/a/c"),
            ("/a/b/..", "/a"),
            ("a/../../b", "b"),
            ("/a/b", "/a/b"),
        ] {
            assert_eq!(normalize(Path::new(path)), PathBuf::from(normal), "{path}");
        }
    }

    #[test]
    fn the_logo_meta_logo_names_wins_and_an_unusable_one_means_no_logo() {
        let project = TestDir::new();
        put(project.path(), "logo.svg", 1);
        put(project.path(), "brand/mark.png", 1);
        let set = find_logo(project.path(), &meta(json!({"logo": "brand/mark.png"}))).unwrap();
        assert_eq!(
            set,
            Logo {
                path: "brand/mark.png".into(),
                file: project.path().join("brand/mark.png"),
                content_type: "image/png",
                source: "set",
            }
        );
        assert_eq!(
            find_logo(project.path(), &meta(json!({"logo": "missing.png"}))),
            None
        );
        // An empty `meta.logo` counts as not set
        let found = find_logo(project.path(), &meta(json!({"logo": ""}))).unwrap();
        assert_eq!((found.path.as_str(), found.source), ("logo.svg", "found"));
    }

    #[test]
    fn without_meta_logo_the_first_usable_file_of_the_usual_places_is_the_logo() {
        let project = TestDir::new();
        assert_eq!(find_logo(project.path(), &Map::new()), None);
        // Each one found while it is the first there; a too-large one is passed over
        put(project.path(), "assets/logo.ico", 1);
        put(project.path(), "public/favicon.png", 1);
        put(project.path(), ".github/logo.webp", 1);
        put(project.path(), ".github/logo.svg", 512 * 1024 + 1);
        let first = |root: &Path| find_logo(root, &Map::new()).map(|l| l.path);
        assert_eq!(first(project.path()).as_deref(), Some(".github/logo.webp"));
        put(project.path(), "logo.png", 1);
        assert_eq!(first(project.path()).as_deref(), Some("logo.png"));
        put(project.path(), "logo.svg", 1);
        assert_eq!(first(project.path()).as_deref(), Some("logo.svg"));
    }
}
