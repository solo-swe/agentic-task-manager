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
    let real_root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let joined = real_root.join(relative);
    // A missing file can't be resolved: its folder decides whether it is inside
    let full = std::fs::canonicalize(&joined).unwrap_or_else(|_| {
        let normal = normalize(&joined);
        match (normal.parent(), normal.file_name()) {
            (Some(parent), Some(name)) => {
                std::fs::canonicalize(parent).map_or(normal.clone(), |p| p.join(name))
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
