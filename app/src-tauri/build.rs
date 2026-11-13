//! The app's commands are listed in its manifest, so a window may call one only when its capability allows it
//! (`capabilities/`, least privilege: docs/guides/tauri.md). Without that list Tauri lets any page call every command,
//! a page on `CompMD`'s frames' scheme too (`CompMD`'s Tauri chapter).
//!
//! On Windows the app needs Common Controls v6 (`windows-app-manifest.xml`). Tauri's build embeds that manifest in
//! the app's binary only, so the tests' executable would start without it and stop at once
//! (`STATUS_ENTRYPOINT_NOT_FOUND`); the linker embeds it in every executable of the crate instead.

fn main() {
    // CompMD's bundle (the sandbox's frames and the packs' modules), which the dashboard's build wrote beside its
    // compmd.js (`dashboard/vite.compmd.config.ts`), embedded for CompMD's plugin to serve (ADR-006): before Tauri's
    // own step, as CompMD's guide has it. Without one it warns, and the app shows no diagram
    tauri_plugin_compmd::build::bundle(format!(
        "../../dashboard/dist/assets/compmd/{}/compmd",
        compmd_version().unwrap_or_default()
    ));
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
        "restart_app",
        "shell",
        "set_title",
        "open_window",
        "subscribe_menu",
        "menu",
        "menu_state",
        "minimize",
        "toggle_maximize",
        "close_window",
        "start_dragging",
        "save_tabs",
        "restored_tabs",
        "needs_you",
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

/// Where the dashboard's `CompMD` build is: `dashboard/dist/assets/compmd/<version>/`.
fn compmd_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dashboard/dist/assets/compmd")
}

/// The `CompMD` version the dashboard was built with: its build's one folder.
fn compmd_version() -> Option<String> {
    std::fs::read_dir(compmd_root())
        .ok()?
        .flatten()
        .find(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
}

/// What the app serves on its `atm:` scheme (`src/assets.rs`, ADR-006): `CompMD`'s processor and sandbox for the
/// page, `compmd.js` and its `WebAssembly`, at the paths `atm serve` gives them. Not the sandbox's frames or the
/// packs' modules: `CompMD`'s plugin serves those, from the bundle `bundle()` embeds. Written to
/// `OUT_DIR/atm_assets.rs` as `ATM_ASSETS`.
///
/// # Panics
/// When the dashboard's `CompMD` build is missing: the app would show no Markdown formatted, so the build says so.
fn embed_assets() {
    use std::fmt::Write as _;
    let root = compmd_root();
    println!("cargo:rerun-if-changed={}", root.display());
    let mut found = Vec::new();
    for version in std::fs::read_dir(&root).into_iter().flatten().flatten() {
        let name = version.file_name().to_string_lossy().into_owned();
        let dir = version.path();
        for file in ["compmd.js", "compmd_wasm_bg.wasm"] {
            if dir.join(file).is_file() {
                println!("cargo:rerun-if-changed={}", dir.join(file).display());
                found.push((format!("assets/compmd/{name}/{file}"), dir.join(file)));
            }
        }
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
