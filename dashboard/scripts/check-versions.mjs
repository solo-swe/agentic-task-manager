// Fails if the Rust workspace (Cargo.toml, atm's version: T-477), the plugin manifest and the marketplace entry
// disagree on the atm version, or if CHANGELOG.md or WHATS-NEW.md has no entry for that version. plugin.json and
// marketplace.json go with the plugin (T-454); until then the Python atm reads its version from plugin.json.
import { readFileSync } from 'node:fs';

const text = (path) => readFileSync(new URL(path, import.meta.url), 'utf8');
const cargo = text('../../Cargo.toml');
const version = /\[workspace\.package\][^[]*?\nversion = "([^"]+)"/.exec(cargo)?.[1];
if (!version) {
  console.error("Cargo.toml's [workspace.package] has no version");
  process.exit(1);
}
const plugin = JSON.parse(text('../../plugin/.claude-plugin/plugin.json'));
const entry = JSON.parse(text('../../.claude-plugin/marketplace.json')).plugins.find((p) => p.name === plugin.name);
if (!entry) {
  console.error(`marketplace.json has no entry for plugin "${plugin.name}"`);
  process.exit(1);
}
for (const [file, other] of [
  ['plugin.json', plugin.version],
  ['marketplace.json', entry.version],
]) {
  if (other !== version) {
    console.error(`Version mismatch: Cargo.toml's [workspace.package] ${version}, ${file} ${other}`);
    process.exit(1);
  }
}
const escaped = version.replace(/\./g, '\\.');
// Every release has both: the detailed changelog (developers) and the short What's New (users, shown in the
// dashboard)
for (const [file, what] of [
  ['CHANGELOG.md', 'the detailed changes'],
  ['WHATS-NEW.md', 'a short, end-user summary'],
]) {
  if (!new RegExp(`^## \\[${escaped}\\]`, 'm').test(text(`../../${file}`))) {
    console.error(`${file} has no "## [${version}]" entry. Add ${what} for this version.`);
    process.exit(1);
  }
}
console.log(`atm ${version}: Cargo.toml, plugin.json, marketplace.json, CHANGELOG.md and WHATS-NEW.md agree`);
