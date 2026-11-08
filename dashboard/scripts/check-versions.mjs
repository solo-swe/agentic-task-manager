// Fails if the plugin manifest, the marketplace entry and the Rust workspace (Cargo.toml) disagree on the atm version,
// or if plugin/CHANGELOG.md or plugin/WHATS-NEW.md has no entry for that version.
import { readFileSync } from 'node:fs';

const read = (path) => JSON.parse(readFileSync(new URL(path, import.meta.url), 'utf8'));
const plugin = read('../../plugin/.claude-plugin/plugin.json');
const marketplace = read('../../.claude-plugin/marketplace.json');
const entry = marketplace.plugins.find((p) => p.name === plugin.name);

if (!entry) {
  console.error(`marketplace.json has no entry for plugin "${plugin.name}"`);
  process.exit(1);
}
if (entry.version !== plugin.version) {
  console.error(`Version mismatch: plugin.json ${plugin.version}, marketplace.json ${entry.version}`);
  process.exit(1);
}
// The Rust atm's version (T-443): the workspace's, in Cargo.toml
const cargo = readFileSync(new URL('../../Cargo.toml', import.meta.url), 'utf8');
const rust = /\[workspace\.package\][^[]*?\nversion = "([^"]+)"/.exec(cargo)?.[1];
if (rust !== plugin.version) {
  console.error(`Version mismatch: plugin.json ${plugin.version}, Cargo.toml's [workspace.package] ${rust}`);
  process.exit(1);
}
const escaped = plugin.version.replace(/\./g, '\\.');
// Every release has both: the detailed changelog (developers) and the short What's New (users, shown in the
// dashboard)
for (const [file, what] of [
  ['CHANGELOG.md', 'the detailed changes'],
  ['WHATS-NEW.md', 'a short, end-user summary'],
]) {
  const text = readFileSync(new URL(`../../plugin/${file}`, import.meta.url), 'utf8');
  if (!new RegExp(`^## \\[${escaped}\\]`, 'm').test(text)) {
    console.error(`plugin/${file} has no "## [${plugin.version}]" entry. Add ${what} for this version.`);
    process.exit(1);
  }
}
console.log(`atm ${plugin.version}: plugin.json, marketplace.json, Cargo.toml, CHANGELOG.md and WHATS-NEW.md agree`);
