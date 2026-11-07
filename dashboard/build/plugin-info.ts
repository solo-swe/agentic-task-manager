import { readFileSync } from 'node:fs';
import type { Plugin } from 'vite';
import { parseChangelog } from '../src/lib/changelog.ts';
import type { PluginInfo } from '../src/lib/plugin-info.ts';
import { workspacePackage } from './workspace.ts';

const ID = 'virtual:atm-plugin-info';
/** The command's name, `atm`; the About dialog shows it with the full name (D-1.4) */
const NAME = 'atm';
const FULL_NAME = 'Agentic Task Manager';
const RESOLVED_ID = '\0' + ID;

/**
 * What the footer and the About dialog show, from Cargo.toml's `[workspace.package]` (T-477; plugin.json before). The
 * copyright year is the year of the newest dated release in the changelog rather than the build date, so rebuilding
 * the same sources gives the same file.
 */
export function readPluginInfo(cargoToml: string, changelog: string): PluginInfo {
  const workspace = workspacePackage(cargoToml);
  const text = (key: string) => {
    const value = workspace[key];
    if (typeof value !== 'string' || !value) throw new Error(`Cargo.toml's [workspace.package] has no ${key}`);
    return value;
  };
  const author = Array.isArray(workspace.authors) ? workspace.authors[0] : undefined;
  if (!author) throw new Error("Cargo.toml's [workspace.package] has no authors");
  const released = parseChangelog(changelog).find((entry) => entry.date);
  if (!released?.date) throw new Error('No dated release in the changelog');
  return {
    name: NAME,
    fullName: FULL_NAME,
    description: text('description'),
    version: text('version'),
    // Cargo's form, `Name <email>`: the name only
    author: author.replace(/\s*<[^>]*>\s*$/, ''),
    license: text('license'),
    repository: text('repository'),
    year: released.date.slice(0, 4),
  };
}

// Exposes readPluginInfo's result as `import info from 'virtual:atm-plugin-info'`, inlined in the build.
export function pluginInfoModule(cargoTomlPath: string, changelogPath: string): Plugin {
  return {
    name: 'atm-plugin-info',
    resolveId: (id) => (id === ID ? RESOLVED_ID : undefined),
    load(id) {
      if (id !== RESOLVED_ID) return;
      this.addWatchFile(cargoTomlPath);
      this.addWatchFile(changelogPath);
      const info = readPluginInfo(readFileSync(cargoTomlPath, 'utf8'), readFileSync(changelogPath, 'utf8'));
      return `export default ${JSON.stringify(info)};`;
    },
  };
}
