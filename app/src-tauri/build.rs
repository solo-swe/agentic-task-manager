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
}
