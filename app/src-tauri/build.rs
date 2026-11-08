//! The app's commands are listed in its manifest, so a window may call one only when its capability allows it
//! (`capabilities/`, least privilege: docs/guides/tauri.md).

fn main() {
    let manifest = tauri_build::AppManifest::new().commands(&["version", "list_projects"]);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))
        .expect("the app's Tauri build");
}
