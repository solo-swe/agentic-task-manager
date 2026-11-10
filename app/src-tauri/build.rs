//! The app's commands are listed in its manifest, so a window may call one only when its capability allows it
//! (`capabilities/`, least privilege: docs/guides/tauri.md).
//!
//! On Windows the app needs Common Controls v6 (`windows-app-manifest.xml`). Tauri's build embeds that manifest in
//! the app's binary only, so the tests' executable would start without it and stop at once
//! (`STATUS_ENTRYPOINT_NOT_FOUND`); the linker embeds it in every executable of the crate instead.

fn main() {
    let manifest = tauri_build::AppManifest::new().commands(&[
        "version",
        "list_workspaces",
        "list_projects",
        "search",
        "project_tasks",
        "claims",
        "worktrees",
        "document",
        "file",
        "logo",
        "subscribe_workspace",
        "subscribe_project",
        "unsubscribe",
        "agents_status",
        "connect_agents",
        "subscribe_links",
        "check_update",
        "install_update",
    ]);
    let mut attributes = tauri_build::Attributes::new().app_manifest(manifest);
    let msvc = std::env::var("CARGO_CFG_TARGET_ENV").is_ok_and(|env| env == "msvc");
    if msvc {
        attributes = attributes
            .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
        let xml = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed={}", xml.display());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", xml.display());
    }
    tauri_build::try_build(attributes).expect("the app's Tauri build");
    embed_assets();
}

/// What the app serves on its `atm:` scheme (`src/assets.rs`, ADR-004): the files of the dashboard's `CompMD` build
/// that the page loads itself, `compmd.js`, its `WebAssembly` and the packs' built modules, at the paths `atm serve`
/// gives them. Not the sandbox's frames, which `CompMD`'s plugin serves. Written to `OUT_DIR/atm_assets.rs` as
/// `ATM_ASSETS`.
///
/// # Panics
/// When the dashboard's `CompMD` build is missing: the app would show no Markdown formatted, so the build says so.
fn embed_assets() {
    use std::fmt::Write as _;
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dashboard/dist/assets/compmd");
    println!("cargo:rerun-if-changed={}", root.display());
    let mut found = Vec::new();
    for version in std::fs::read_dir(&root).into_iter().flatten().flatten() {
        let name = version.file_name().to_string_lossy().into_owned();
        let dir = version.path();
        for file in ["compmd.js", "compmd_wasm_bg.wasm"] {
            if dir.join(file).is_file() {
                found.push((format!("assets/compmd/{name}/{file}"), dir.join(file)));
            }
        }
        walk(
            &dir.join("compmd/packs"),
            &format!("assets/compmd/{name}/compmd/packs"),
            &mut found,
        );
    }
    assert!(
        found.iter().any(|(path, _)| path.ends_with("/compmd.js")),
        "{} has no CompMD build (compmd.js): run `pnpm run build` in dashboard/ first",
        root.display()
    );
    found.sort();
    let mut code = String::from(
        "/// The files the app serves on its `atm:` scheme, embedded by `build.rs`: (path, bytes).\n\
         pub static ATM_ASSETS: &[(&str, &[u8])] = &[\n",
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
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    std::fs::write(out.join("atm_assets.rs"), code).expect("the build folder is writable");
}

/// The `.js` files under `dir` (regular files, reached through no symlink), served under `prefix`
fn walk(dir: &std::path::Path, prefix: &str, found: &mut Vec<(String, std::path::PathBuf)>) {
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let served = format!("{prefix}/{name}");
        if kind.is_dir() {
            walk(&entry.path(), &served, found);
        } else if kind.is_file() && entry.path().extension().is_some_and(|ext| ext == "js") {
            found.push((served, entry.path()));
        }
    }
}
