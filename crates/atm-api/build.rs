//! Embeds the dashboard's assets (the `CompMD` runtime, `dashboard/dist/assets/`, built by `pnpm run build` in
//! `dashboard/` and never committed: T-477) in the binary: the files the server may serve
//! at `/assets/…`, as a list of (path, bytes). Only regular files reached through no symlink, each path segment a plain
//! name, with the extensions the server serves (the Python atm's `api/server.py` `asset_file` checks). `CompMD`'s
//! bundle (`compmd/<version>/compmd/`, ADR-006) is embedded whole, apart, for `compmd-host` to serve.

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

/// Every regular file under `dir` (reached through no symlink, each segment a plain name), whatever its kind: the
/// `CompMD` bundle, which `compmd-host` reads and serves by its own rules
fn walk_all(dir: &Path, relative: &str, found: &mut Vec<(String, PathBuf)>) {
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
        let served = if relative.is_empty() {
            name.clone()
        } else {
            format!("{relative}/{name}")
        };
        if kind.is_dir() {
            walk_all(&entry.path(), &served, found);
        } else if kind.is_file() {
            found.push((served, entry.path()));
        }
    }
}

/// `pub static <name>: &[(&str, &[u8])]`, each file embedded
fn table(code: &mut String, doc: &str, name: &str, files: &[(String, PathBuf)]) {
    let _ = writeln!(code, "/// {doc}\npub static {name}: &[(&str, &[u8])] = &[");
    for (served, path) in files {
        let absolute = std::fs::canonicalize(path).expect("a file just listed");
        let _ = writeln!(
            code,
            "    ({served:?}, include_bytes!({:?})),",
            absolute.display().to_string()
        );
    }
    code.push_str("];\n");
}

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets it"));
    let assets = manifest.join("../../dashboard/dist/assets");
    println!("cargo:rerun-if-changed={}", assets.display());
    let mut found = Vec::new();
    walk(&assets, "", &mut found);
    // CompMD's bundle (ADR-006) is served by compmd-host's own rules, not as one of these files
    found.retain(|(served, _)| served.split('/').nth(2).is_none_or(|s| s != "compmd"));
    assert!(
        !found.is_empty(),
        "dashboard/dist/assets/ is missing: atm embeds CompMD's runtime, so build the dashboard first \
         (`pnpm install --frozen-lockfile && pnpm run build` in dashboard/)"
    );
    // The bundle: dist/assets/compmd/<version>/compmd/, its paths from its root (manifest.json, frames/…, packs/…)
    // Exactly one version folder (ATM-672, audit H13): an older build's left beside the new one would be taken in
    // whatever order the folder lists them
    let versions: Vec<String> = std::fs::read_dir(assets.join("compmd"))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        versions.len() == 1,
        "dashboard/dist/assets/compmd/ must hold one CompMD version folder, not {versions:?}: build the dashboard \
         again (`pnpm run build` in dashboard/ clears that folder first)"
    );
    let version = versions[0].clone();
    let mut bundle = Vec::new();
    walk_all(
        &assets.join("compmd").join(&version).join("compmd"),
        "",
        &mut bundle,
    );
    assert!(
        bundle.iter().any(|(path, _)| path == "manifest.json"),
        "dashboard/dist/assets/compmd/{version}/compmd/ has no CompMD bundle (manifest.json): build the dashboard \
         again (`pnpm run build` in dashboard/)"
    );
    let mut code = String::new();
    table(
        &mut code,
        "The embedded assets: (path under `/assets/`, bytes).",
        "ASSETS",
        &found,
    );
    let _ = writeln!(
        code,
        "/// The `CompMD` version whose bundle is embedded: served under `/assets/compmd/<version>/compmd/`.\n\
         pub const COMPMD_VERSION: &str = {version:?};"
    );
    table(
        &mut code,
        "`CompMD`'s bundle (ADR-006): (path from its root, bytes), for `compmd_host::Bundle::embedded`.",
        "COMPMD_BUNDLE",
        &bundle,
    );
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets it")).join("assets.rs");
    std::fs::write(out, code).expect("the build folder is writable");
}
