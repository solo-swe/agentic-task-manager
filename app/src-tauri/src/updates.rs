//! The app's own updates (T-483): Tauri's updater checks the update manifest (`latest.json`) on the latest GitHub
//! release, and installs an update only when the user says so, after checking its signature against the public key
//! built in here. The page gets three commands, `check_update`, `install_update` (with its progress on a channel,
//! T-526) and `restart_app`, not the plugin's own API: the endpoint, the key and the install are this module's, never
//! the page's to choose.
//!
//! Installed with the Homebrew cask, the app updates itself too (the cask says `auto_updates true`); the bundled `atm`
//! CLI is inside the app, so it updates with it.

use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Runtime, Url, ipc::Channel};
use tauri_plugin_updater::{Updater, UpdaterExt};

use crate::commands::AppError;

/// Where the update manifest is: the latest published release's `latest.json` (never a draft or a pre-release).
pub const ENDPOINT: &str =
    "https://github.com/solo-swe/agentic-task-manager/releases/latest/download/latest.json";

/// The updater's public key (`tauri signer generate`, the `.pub` file as Tauri writes it, base64; minisign key
/// `D2392A7664649507`, made by Hakan, 2026-11-09): an update whose signature it doesn't verify is refused. Its private
/// half is the release secret `TAURI_SIGNING_PRIVATE_KEY`, never in the repository. The same key is in tauri.conf.json
/// (`plugins.updater.pubkey`), which the bundler checks the archive's signature against (a test keeps them equal).
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

/// How an install goes, step by step, on the channel `install_update` takes (T-526): the bytes downloaded so far (of
/// the archive's size, when the server says it), then the install once the download is done.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum UpdateProgress {
    /// The archive is downloading (sizes in `u32`, which TypeScript's numbers hold: the archive is tens of MB).
    Downloading {
        /// Bytes downloaded so far.
        downloaded: u32,
        /// The archive's size, when the server says it.
        total: Option<u32>,
    },
    /// Downloaded and verified: the new version is being put in place.
    Installing,
}

/// The download's progress, sent at most once per `STEP` bytes (the updater reports every chunk, a few KB each) and
/// once at its end.
struct Progress {
    downloaded: u64,
    sent: Option<u64>,
}

impl Progress {
    const STEP: u64 = 256 * 1024;

    fn new() -> Self {
        Self {
            downloaded: 0,
            sent: None,
        }
    }

    /// A chunk of `len` bytes arrived: the message to send, if one is due.
    fn chunk(&mut self, len: usize, total: Option<u64>) -> Option<UpdateProgress> {
        self.downloaded += len as u64;
        let done = total.is_some_and(|t| self.downloaded >= t);
        let due = self
            .sent
            .is_none_or(|sent| self.downloaded - sent >= Self::STEP);
        let bytes = |n: u64| u32::try_from(n).unwrap_or(u32::MAX);
        (due || done).then(|| {
            self.sent = Some(self.downloaded);
            UpdateProgress::Downloading {
                downloaded: bytes(self.downloaded),
                total: total.map(bytes),
            }
        })
    }
}

/// Download the newer version, check its signature and install it: only on the user's click. The page hears how it
/// goes on `progress`, and restarts the app with `restart_app` once it returns `true`. Nothing to install (up to date
/// since the check, or a build without updates) is not an error: it returns `false`.
///
/// The restart is a command of its own (T-526): the install used to restart the app itself, from this command's
/// thread, and on macOS 0.35.0 stayed open on the old version instead.
///
/// # Errors
/// The download fails, its signature doesn't verify, or it can't be installed; the running app stays as it is.
#[tauri::command]
#[specta::specta]
pub async fn install_update(
    app: AppHandle,
    progress: Channel<UpdateProgress>,
) -> Result<bool, AppError> {
    let failed = |e: tauri_plugin_updater::Error| AppError::Failed(e.to_string());
    let Some(updater) = updater(&app)? else {
        return Ok(false);
    };
    let Some(update) = updater.check().await.map_err(failed)? else {
        return Ok(false);
    };
    let mut download = Progress::new();
    update
        .download_and_install(
            |len, total| {
                if let Some(message) = download.chunk(len, total) {
                    let _ = progress.send(message);
                }
            },
            || {
                let _ = progress.send(UpdateProgress::Installing);
            },
        )
        .await
        .map_err(failed)?;
    Ok(true)
}

/// Restart the app into the version just installed: the page calls it once `install_update` returned `true`. It asks
/// the app to quit as the user's Quit does (the windows' frames are written, the plugins' locks let go, the single
/// instance's among them) and starts it again only after that, not from a command's thread while the old app still
/// runs. It returns before the app quits; a page still there seconds later offers Restart Now again.
#[tauri::command]
#[specta::specta]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri hands a command its AppHandle by value"
)]
pub fn restart_app(app: AppHandle) {
    app.request_restart();
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

    /// The download's progress goes to the page every 256 KB and at its end, not on every chunk the updater reports
    #[test]
    fn progress_is_sent_in_steps_and_at_the_end() {
        let total = Some(600 * 1024);
        let mut progress = Progress::new();
        let sent: Vec<u32> = (0..75)
            .filter_map(|_| progress.chunk(8 * 1024, total))
            .map(|m| match m {
                UpdateProgress::Downloading { downloaded, .. } => downloaded / 1024,
                UpdateProgress::Installing => unreachable!(),
            })
            .collect();
        // The first chunk, then every 256 KB, then the last
        assert_eq!(sent, [8, 264, 520, 600]);
        // Without a size, every 256 KB still
        let mut unsized_download = Progress::new();
        let count = (0..100)
            .filter(|_| unsized_download.chunk(8 * 1024, None).is_some())
            .count();
        assert_eq!(count, 4);
    }

    /// What the page reads: `step`, and the bytes while downloading
    #[test]
    fn progress_is_tagged_by_its_step() {
        let downloading = UpdateProgress::Downloading {
            downloaded: 10,
            total: Some(20),
        };
        assert_eq!(
            serde_json::to_value(downloading).unwrap(),
            serde_json::json!({ "step": "downloading", "downloaded": 10, "total": 20 })
        );
        assert_eq!(
            serde_json::to_value(UpdateProgress::Installing).unwrap(),
            serde_json::json!({ "step": "installing" })
        );
    }

    /// The bundler checks the update archive's signature against `plugins.updater.pubkey` in tauri.conf.json, and the
    /// plugin starts from it: the same key as the app checks updates with
    #[test]
    fn the_config_has_the_same_key() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).expect("tauri.conf.json");
        assert_eq!(conf["plugins"]["updater"]["pubkey"], PUBKEY);
    }
}
