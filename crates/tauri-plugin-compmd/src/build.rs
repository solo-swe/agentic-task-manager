//! For an app's `build.rs`: embeds `CompMD`'s built folder in the app, as the file table [`crate::init`] serves.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// The file the table is written to, in Cargo's `OUT_DIR`; it defines `COMPMD_FILES`.
pub const OUT_FILE: &str = "compmd_files.rs";

/// Embeds the files of `folder` (`CompMD`'s built folder, from the crate's manifest folder) that the plugin serves
/// (`.js`, `.wasm` and `.html`; regular files reached through no symlink, each name plain), served under `prefix`
/// (no leading or trailing `/`), and writes the table to `OUT_DIR/compmd_files.rs` as `COMPMD_FILES`. Cargo builds
/// again when the folder changes.
///
/// # Panics
/// When the folder is missing or holds no frame document (`strict.html`): an app whose every frame is a `404` is a
/// build that failed, so the build says so, naming the folder.
pub fn embed(folder: &str, prefix: &str) {
    let manifest =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
    let root = manifest.join(folder);
    println!("cargo:rerun-if-changed={}", root.display());
    let mut found = Vec::new();
    walk(&root, "", &mut found);
    assert!(
        found.iter().any(|(path, _)| path.ends_with("/strict.html")),
        "tauri-plugin-compmd: {} has no CompMD build (no frame document, strict.html): build CompMD's folder first",
        root.display()
    );
    let mut code = String::from(
        "/// `CompMD`'s built files, embedded by `tauri-plugin-compmd`'s `build::embed`: (path, bytes).\n\
         pub static COMPMD_FILES: &[(&str, &[u8])] = &[\n",
    );
    for (served, path) in &found {
        let absolute = std::fs::canonicalize(path).expect("a file just listed");
        let _ = writeln!(
            code,
            "    ({:?}, include_bytes!({:?})),",
            format!("{prefix}/{served}"),
            absolute.display().to_string()
        );
    }
    code.push_str("];\n");
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR")).join(OUT_FILE);
    std::fs::write(out, code).expect("the build folder is writable");
}

fn walk(dir: &Path, relative: &str, found: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_symlink() || !super::plain(&name) {
            continue;
        }
        let served = if relative.is_empty() {
            name.clone()
        } else {
            format!("{relative}/{name}")
        };
        if kind.is_dir() {
            walk(&entry.path(), &served, found);
        } else if kind.is_file() && super::content_type(&name).is_some() {
            found.push((served, entry.path()));
        }
    }
}
