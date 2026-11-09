#!/usr/bin/env node
// The atm app's update manifest for a release (T-483): `latest.json`, which the app's updater reads from the latest
// published release (app/src-tauri/src/updates.rs). It names the release's signed app archive (Tauri's updater
// artifact, `atm.app.tar.gz`, one universal archive for both Mac architectures) with its signature, and the version's
// What's new as its notes, which the app shows before it asks to install. The release workflow writes it next to the
// archive; nothing in it is trusted without the signature, which the app checks against its built-in public key.
//
// Usage: node packaging/updater-manifest.mjs <version> <archive file name> <signature file> <WHATS-NEW.md>
import { readFileSync } from 'node:fs';

const [version, archive, signatureFile, whatsNew] = process.argv.slice(2);
if (!version || !archive || !signatureFile || !whatsNew) {
  console.error('usage: updater-manifest.mjs <version> <archive file name> <signature file> <WHATS-NEW.md>');
  process.exit(1);
}
if (!/^\d+\.\d+\.\d+$/.test(version)) {
  console.error(`updater-manifest.mjs: ${version} isn't a version (MAJOR.MINOR.PATCH)`);
  process.exit(1);
}

/** The version's entry in What's new: its bullets, without the heading */
function notesOf(text, wanted) {
  const lines = text.split('\n');
  const start = lines.findIndex((line) => line.startsWith(`## [${wanted}]`));
  if (start < 0) return null;
  const end = lines.findIndex((line, i) => i > start && line.startsWith('## '));
  return lines
    .slice(start + 1, end < 0 ? undefined : end)
    .join('\n')
    .trim();
}

const notes = notesOf(readFileSync(whatsNew, 'utf8'), version);
if (!notes) {
  console.error(`updater-manifest.mjs: ${whatsNew} has no entry for ${version}`);
  process.exit(1);
}
const signature = readFileSync(signatureFile, 'utf8').trim();
const url = `https://github.com/solo-swe/agentic-task-manager/releases/download/v${version}/${archive}`;
// One universal archive serves both architectures
const platform = { signature, url };
const manifest = {
  version,
  notes,
  pub_date: new Date().toISOString().replace(/\.\d{3}Z$/, 'Z'),
  platforms: { 'darwin-aarch64': platform, 'darwin-x86_64': platform },
};
process.stdout.write(`${JSON.stringify(manifest, null, 2)}\n`);
