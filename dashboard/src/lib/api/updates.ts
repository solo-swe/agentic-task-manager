// The atm app's own updates (T-483): the app checks its release's update manifest and installs a signed update only on
// the user's click (app/src-tauri/src/updates.rs). The page asks through the app's two commands; the plugin's own API
// isn't the page's.
import { Channel, invoke } from '@tauri-apps/api/core';

/** An update the app can install: what `check_update` answers */
export interface AppUpdate {
  version: string;
  current_version: string;
  /** Its What's new, as plain text */
  notes: string | null;
}

/** How often the app looks for an update while it runs (on start, then at most this often, when its window is
 * focused) */
export const UPDATE_CHECK_MS = 6 * 60 * 60 * 1000;

/** How long the page waits for the app to restart before it offers Restart Now (the app quits within a second or
 * two; a page still here after this the restart didn't take) */
export const RESTART_WAIT_MS = 10_000;

/** A newer published version, or null (up to date, a build without updates, or the check failed: logged only) */
export async function checkAppUpdate(): Promise<AppUpdate | null> {
  try {
    return await invoke<AppUpdate | null>('check_update');
  } catch (e) {
    console.error('Checking for an update failed:', e);
    return null;
  }
}

/** How an install goes, as the app sends it while `install_update` runs (T-526) */
export type UpdateProgress = { step: 'downloading'; downloaded: number; total: number | null } | { step: 'installing' };

/** What an install came to: installed (the app is restarted next, `restartApp`), nothing to install, or why it
 * failed */
export type InstallResult = { installed: boolean } | { error: string };

/** Download, verify and install the update, telling `onProgress` how it goes; the running app stays as it is until
 * `restartApp` */
export async function installAppUpdate(onProgress: (p: UpdateProgress) => void): Promise<InstallResult> {
  const progress = new Channel<UpdateProgress>();
  progress.onmessage = onProgress;
  try {
    return { installed: await invoke<boolean>('install_update', { progress }) };
  } catch (e) {
    const error = e as { message?: unknown };
    return { error: typeof error?.message === 'string' ? error.message : String(e) };
  }
}

/** Restart the app into the version just installed: the app quits and starts again, so this returns before it does
 * (or, if it never does, the page stays and offers it again) */
export async function restartApp(): Promise<void> {
  try {
    await invoke('restart_app');
  } catch (e) {
    console.error('Restarting the app failed:', e);
  }
}
