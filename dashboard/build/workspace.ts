// What the dashboard's build reads from outside dashboard/ about atm itself (T-477): its version, name, author,
// licence and links, from the Rust workspace's Cargo.toml (`[workspace.package]`), and the release notes, at the
// repository's root.
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const at = (path: string) => fileURLToPath(new URL(path, import.meta.url));

export const CARGO_TOML = at('../../Cargo.toml');
/** The detailed record, for developers */
export const CHANGELOG = at('../../CHANGELOG.md');
/** What users see in the dashboard's What's new */
export const WHATS_NEW = at('../../WHATS-NEW.md');

export type WorkspacePackage = Record<string, string | string[]>;

/** Cargo.toml's `[workspace.package]`: its keys with a string or a list of strings (the subset atm's file uses; other
 * values are left out). */
export function workspacePackage(toml: string): WorkspacePackage {
  // CRLF on a Windows checkout
  const lines = toml.replace(/\r\n/g, '\n');
  const table = /^\[workspace\.package\]\n([\s\S]*?)(?=^\[|$(?![\s\S]))/m.exec(lines)?.[1];
  if (table === undefined) throw new Error('Cargo.toml has no [workspace.package]');
  const values: WorkspacePackage = {};
  for (const line of table.split('\n')) {
    const entry = /^([a-z][a-z0-9_-]*)\s*=\s*(.+?)\s*$/.exec(line);
    if (!entry) continue;
    const [, key, value] = entry;
    if (/^"(?:[^"\\]|\\.)*"$/.test(value)) values[key] = JSON.parse(value);
    else if (/^\[\s*(?:"(?:[^"\\]|\\.)*"\s*,?\s*)*\]$/.test(value))
      values[key] = JSON.parse(value.replace(/,\s*\]$/, ']'));
  }
  return values;
}

/** atm's version: the workspace's. */
export function readVersion(cargoToml = CARGO_TOML): string {
  const { version } = workspacePackage(readFileSync(cargoToml, 'utf8'));
  if (typeof version !== 'string' || !version) throw new Error('No version in ' + cargoToml);
  return version;
}
