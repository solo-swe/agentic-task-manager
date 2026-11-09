//! The app's own updates (T-483): Tauri's updater checks the update manifest (`latest.json`) on the latest GitHub
//! release, and installs an update only when the user says so, after checking its signature against the public key
//! built in here. The page gets two commands, `check_update` and `install_update`, not the plugin's own API: the
//! endpoint, the key and the install are this module's, never the page's to choose.
//!
//! Installed with the Homebrew cask, the app updates itself too (the cask says `auto_updates true`); the bundled `atm`
//! CLI is inside the app, so it updates with it.

use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Runtime, Url};
use tauri_plugin_updater::{Updater, UpdaterExt};

use crate::commands::AppError;

/// Where the update manifest is: the latest published release's `latest.json` (never a draft or a pre-release).
pub const ENDPOINT: &str =
    "https://github.com/solo-swe/agentic-task-manager/releases/latest/download/latest.json";

/// The updater's public key (`tauri signer generate`, the `.pub` file as Tauri writes it, base64; minisign key
/// `D2392A7664649507`, made by Hakan, 2026-11-09): an update whose signature it doesn't verify is refused. Its private
/// half is the release secret `TAURI_SIGNING_PRIVATE_KEY`, never in the repository.
pub const PUBKEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEQyMzkyQTc2NjQ2NDk1MDcKUldRSGxXUmtkaW81MHZsaFpiTUJVdW5Cek5JanhaV0JDQmZyOVR5d25UcjRDKzdLTVVQV0tPV0sK";

/// An update the app can install.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UpdateInfo {
    /// Its version.
    pub version: String,
    /// The running version.
    pub current_version: String,
    /// Its release notes, as the manifest has them.
    pub notes: Option<String>,
}

/// The updater, when this build has one: a release build with the public key. A debug build never offers to replace
/// itself with a release.
fn updater<R: Runtime>(app: &AppHandle<R>) -> Result<Option<Updater>, AppError> {
    if cfg!(debug_assertions) {
        return Ok(None);
    }
    let failed = |e: tauri_plugin_updater::Error| AppError::Failed(e.to_string());
    let endpoint = Url::parse(ENDPOINT).map_err(|e| AppError::Failed(e.to_string()))?;
    let updater = app
        .updater_builder()
        .pubkey(PUBKEY)
        .endpoints(vec![endpoint])
        .map_err(failed)?
        .build()
        .map_err(failed)?;
    Ok(Some(updater))
}

/// Whether a newer version is published: it, or nothing (up to date, or a build without updates).
///
/// # Errors
/// The manifest can't be fetched or read.
#[tauri::command]
#[specta::specta]
pub async fn check_update(app: AppHandle) -> Result<Option<UpdateInfo>, AppError> {
    let Some(updater) = updater(&app)? else {
        return Ok(None);
    };
    let update = updater
        .check()
        .await
        .map_err(|e| AppError::Failed(e.to_string()))?;
    Ok(update.map(|u| UpdateInfo {
        version: u.version,
        current_version: u.current_version,
        notes: u.body,
    }))
}

/// Download the newer version, check its signature, install it and restart: only on the user's click. Nothing to
/// install (up to date since the check) is not an error: it returns.
///
/// # Errors
/// The download fails, its signature doesn't verify, or it can't be installed; the running app stays as it is.
#[tauri::command]
#[specta::specta]
pub async fn install_update(app: AppHandle) -> Result<(), AppError> {
    let failed = |e: tauri_plugin_updater::Error| AppError::Failed(e.to_string());
    let Some(updater) = updater(&app)? else {
        return Ok(());
    };
    let Some(update) = updater.check().await.map_err(failed)? else {
        return Ok(());
    };
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(failed)?;
    app.restart()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The key is a minisign public key as Tauri writes it (its `.pub` file, base64), never a secret key: the base64
    /// of "untrusted comment: minisign public key:", then the key; a secret key's file says "secret key"
    #[test]
    fn the_key_is_the_public_half() {
        assert!(
            PUBKEY.starts_with("dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6"),
            "not a minisign public key"
        );
        // A public key's file is two short lines; a secret key's is several times longer
        assert!(PUBKEY.len() < 200, "{} characters", PUBKEY.len());
        assert!(
            PUBKEY
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"+/=".contains(&b))
        );
        assert!(
            ENDPOINT
                .starts_with("https://github.com/solo-swe/agentic-task-manager/releases/latest/")
        );
    }
}
