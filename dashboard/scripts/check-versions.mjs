// Fails if CHANGELOG.md or WHATS-NEW.md has no entry for atm's version, the Rust workspace's (Cargo.toml's
// [workspace.package], T-477).
import { readFileSync } from 'node:fs';

const text = (path) => readFileSync(new URL(path, import.meta.url), 'utf8');
const cargo = text('../../Cargo.toml');
const version = /\[workspace\.package\][^[]*?\nversion = "([^"]+)"/.exec(cargo)?.[1];
if (!version) {
  console.error("Cargo.toml's [workspace.package] has no version");
  process.exit(1);
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
console.log(`atm ${version}: CHANGELOG.md and WHATS-NEW.md have its entries`);
