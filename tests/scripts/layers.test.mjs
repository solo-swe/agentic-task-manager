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
  // What the file stores share (ATM-563): git, the lock, the watcher
  'crates/atm-fs': ['atm-common'],
  'crates/atm-db': ['atm-common', 'atm-fs'],
  'crates/atm-vcs': ['atm-common', 'atm-db'],
  // The code review tool (docs/code-review-design.md §3.1, ATM-561): a component of its own, which knows a task only
  // as a key; never atm-db or atm-lib
  'crates/atm-review': ['atm-common', 'atm-fs', 'atm-vcs'],
  // The document viewer's files (§3.1, ATM-570): a component of its own, with no task model; never atm-db or atm-lib
  'crates/atm-docs': ['atm-common', 'atm-vcs'],
  // atm-review through its link to the tasks (docs/code-review-design.md §3.1, ATM-565); atm-docs re-exported
  'crates/atm-lib': ['atm-common', 'atm-db', 'atm-docs', 'atm-review', 'atm-vcs'],
  'crates/atm-api': ['atm-common', 'atm-lib'],
  // The CLI enters the server (`atm _serve`), never calls it otherwise
  'crates/atm-cli': ['atm-common', 'atm-lib', 'atm-api'],
  'app/src-tauri': ['atm-common', 'atm-lib'],
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
      'atm-docs',
      'atm-review',
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
