// The layer rule (docs/layered-architecture.md §1, ADR-003 §4): which of atm's crates each crate may depend on. The
// compiler refuses a use of a crate a Cargo.toml doesn't name; this refuses a Cargo.toml naming one it may not, so a
// front end can never reach atm-db or atm-vcs past atm-lib.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { describe, test } from 'node:test';
import { REPO } from './helpers.mjs';

const ALLOWED = {
  'crates/atm-common': [],
  'crates/atm-db': ['atm-common'],
  'crates/atm-vcs': ['atm-common', 'atm-db'],
  'crates/atm-lib': ['atm-common', 'atm-db', 'atm-vcs'],
  'crates/atm-api': ['atm-common', 'atm-lib'],
  // The CLI enters the server (`atm _serve`), never calls it otherwise
  'crates/atm-cli': ['atm-common', 'atm-lib', 'atm-api'],
  'app/src-tauri': ['atm-common', 'atm-lib'],
  // CompMD's Tauri plugin (ADR-004): none of atm's crates, so it can move to CompMD as it is
  'crates/tauri-plugin-compmd': [],
};

/** The atm crates a Cargo.toml's `[dependencies]` (and target-specific ones) name. */
function atmDependencies(file) {
  const names = [];
  let inDependencies = false;
  for (const line of fs.readFileSync(file, 'utf8').split('\n')) {
    const section = line.match(/^\[(.+)\]\s*$/);
    if (section) {
      inDependencies = /(^|\.)dependencies$/.test(section[1]) && !section[1].includes('build-dependencies');
      continue;
    }
    const dependency = line.match(/^(atm-[a-z]+)\s*[.=]/);
    if (inDependencies && dependency) names.push(dependency[1]);
  }
  return names;
}

describe('layers', () => {
  test('each crate depends only on the atm crates its layer may use', () => {
    // The reading finds what is there (else the rule would pass on nothing)
    assert.deepEqual(atmDependencies(path.join(REPO, 'crates/atm-lib/Cargo.toml')), [
      'atm-common',
      'atm-db',
      'atm-vcs',
    ]);
    for (const [crate, allowed] of Object.entries(ALLOWED)) {
      for (const name of atmDependencies(path.join(REPO, crate, 'Cargo.toml')))
        assert.ok(allowed.includes(name), `${crate} may not depend on ${name}`);
    }
  });

  test('every crate is in the rule', () => {
    const crates = fs.readdirSync(path.join(REPO, 'crates')).map((n) => `crates/${n}`);
    for (const crate of crates)
      if (fs.existsSync(path.join(REPO, crate, 'Cargo.toml'))) assert.ok(crate in ALLOWED, `${crate} has no layer`);
  });
});
