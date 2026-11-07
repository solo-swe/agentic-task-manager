//! Embeds the dashboard's assets (the `CompMD` runtime, `dashboard/dist/assets/`, built by `pnpm run build` in
//! `dashboard/` and never committed: T-477) in the binary: the files the server may serve
//! at `/assets/…`, as a list of (path, bytes). Only regular files reached through no symlink, each path segment a plain
//! name, with the extensions the server serves (the Python atm's `api/server.py` `asset_file` checks).

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const EXTENSIONS: [&str; 3] = ["js", "wasm", "html"];

fn plain(segment: &str) -> bool {
    let mut chars = segment.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '@')
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '@' | '-'))
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
        if kind.is_symlink() || !plain(&name) {
            continue;
        }
        let path = entry.path();
        let served = if relative.is_empty() {
            name.clone()
        } else {
            format!("{relative}/{name}")
        };
        if kind.is_dir() {
            walk(&path, &served, found);
        } else if kind.is_file()
            && path
                .extension()
                .is_some_and(|e| EXTENSIONS.iter().any(|x| e == *x))
        {
            found.push((served, path));
        }
    }
}

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets it"));
    let assets = manifest.join("../../dashboard/dist/assets");
    println!("cargo:rerun-if-changed={}", assets.display());
    let mut found = Vec::new();
    walk(&assets, "", &mut found);
    assert!(
        !found.is_empty(),
        "dashboard/dist/assets/ is missing: atm embeds CompMD's runtime, so build the dashboard first \
         (`pnpm install --frozen-lockfile && pnpm run build` in dashboard/)"
    );
    let mut code = String::from(
        "/// The embedded assets: (path under `/assets/`, bytes).\npub static ASSETS: &[(&str, &[u8])] = &[\n",
    );
    for (served, path) in &found {
        let absolute = std::fs::canonicalize(path).expect("a file just listed");
        let _ = writeln!(
            code,
            "    ({served:?}, include_bytes!({:?})),",
            absolute.display().to_string()
        );
    }
    code.push_str("];\n");
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets it")).join("assets.rs");
    std::fs::write(out, code).expect("the build folder is writable");
}
