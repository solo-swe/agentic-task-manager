// Runs every tests/scripts/*.test.mjs file with node:test. The tests drive atm as a user would: the Python atm through
// plugin/scripts/atm, or with ATM_IMPL=rust the Rust binary (T-443, docs/porting-to-rust.md §5).
// Usage: node tests/scripts/run.mjs [--rust [--write-expected] [--file-limit=<seconds>] [--quiet]] [<file>.test.mjs …]
// (works on Node 18+, unlike `node --test <dir>`
// whose argument rules changed); `pnpm run test:scripts:rust` in dashboard/ builds the binary and runs it
//
// With ATM_IMPL=rust, the tests the Rust atm doesn't pass yet are listed in rust-expected-failures.json (by file and
// name). The run fails on a failure not listed, and on a listed test that passes: the list only shrinks, as slices are
// ported. `--write-expected` writes the list from a run (the first, or after renaming tests), never to hide one.
import { spawn, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { createInterface } from 'node:readline';
import { fileURLToPath, pathToFileURL } from 'node:url';

const dir = path.dirname(fileURLToPath(import.meta.url));
// `run.mjs --rust merge.test.mjs serve.test.mjs`: only those files (not with --write-expected, which needs them all)
const only = process.argv.slice(2).filter((a) => a.endsWith('.test.mjs'));
const files = fs
  .readdirSync(dir)
  .filter((f) => f.endsWith('.test.mjs'))
  .filter((f) => !only.length || only.includes(f))
  .sort();

// `--rust` is ATM_IMPL=rust: the test files (and the processes node:test runs them in) see it from the environment
if (process.argv.includes('--rust')) process.env.ATM_IMPL = 'rust';

// What atm-cli/build.py generates (.generated/, not committed): atm-lib's tests import the generated modules
const generate = spawnSync('python3', [path.join(dir, '..', '..', 'atm-cli', 'build.py'), '--generated'], {
  stdio: 'inherit',
});
if (generate.status !== 0) {
  console.error('run.mjs: `python3 atm-cli/build.py --generated` failed');
  process.exit(1);
}

if (process.env.ATM_IMPL !== 'rust') {
  for (const file of files) await import(pathToFileURL(path.join(dir, file)).href);
} else {
  const EXPECTED = path.join(dir, 'rust-expected-failures.json');
  const expected = new Set(fs.existsSync(EXPECTED) ? JSON.parse(fs.readFileSync(EXPECTED, 'utf8')) : []);
  // Each file in its own `node --test` process, its TAP read: a test's identity is its file and the names of the
  // suites it is in, "claims.test.mjs > claims > start claims the task …". A test waiting on something the Rust atm
  // doesn't do yet (a server, a watcher) can leave a process behind that keeps its file running for ever, so each
  // file has a watchdog: past FILE_LIMIT_MS its process group is killed, and "<file> > (did not finish)" fails.
  const limitArg = process.argv.find((a) => a.startsWith('--file-limit='));
  const FILE_LIMIT_MS = (limitArg ? Number(limitArg.slice('--file-limit='.length)) : 180) * 1000;
  const TEST_LIMIT_MS = 60000;
  const results = new Map();
  const timings = [];
  // `--quiet`: only each file's time and the summary, not the TAP
  const quiet = process.argv.includes('--quiet');
  const runFile = (file) =>
    new Promise((resolve) => {
      const started = Date.now();
      const out = [];
      const stack = [];
      const child = spawn(
        process.execPath,
        ['--test', '--test-reporter=tap', `--test-timeout=${TEST_LIMIT_MS}`, path.join(dir, file)],
        { env: process.env, stdio: ['ignore', 'pipe', 'pipe'], detached: true },
      );
      let timedOut = false;
      const watchdog = setTimeout(() => {
        timedOut = true;
        try {
          process.kill(-child.pid, 'SIGKILL');
        } catch {
          // already gone
        }
      }, FILE_LIMIT_MS);
      child.stderr.on('data', (d) => out.push(String(d)));
      createInterface({ input: child.stdout }).on('line', (line) => {
        out.push(line + '\n');
        const depth = Math.floor((line.length - line.trimStart().length) / 4);
        const subtest = /^\s*# Subtest: (.*)$/.exec(line);
        if (subtest) {
          stack.length = depth;
          stack[depth] = subtest[1];
          return;
        }
        const result = /^\s*(not ok|ok) \d+ - (.*?)(?: # (?:SKIP|TODO).*)?$/.exec(line);
        if (result) results.set([file, ...stack.slice(0, depth), result[2]].join(' > '), result[1] === 'ok');
      });
      child.on('close', () => {
        clearTimeout(watchdog);
        // what the file's tests may have left running
        try {
          process.kill(-child.pid, 'SIGKILL');
        } catch {
          // nothing left
        }
        if (timedOut) results.set(`${file} > (did not finish)`, false);
        const seconds = Math.round((Date.now() - started) / 1000);
        if (!quiet) process.stdout.write(out.join(''));
        const timing = `${file}: ${seconds}s${timedOut ? `, killed after ${FILE_LIMIT_MS / 1000}s` : ''}`;
        timings.push(timing);
        console.log(`# ${timing}`);
        resolve();
      });
    });
  const queue = [...files];
  const workers = Array.from({ length: 4 }, async () => {
    while (queue.length) await runFile(queue.shift());
  });
  await Promise.all(workers);
  console.log(`\nTime per file:\n  ${timings.sort().join('\n  ')}`);
  // The tests themselves, not the suites and files that contain them
  const keys = [...results.keys()];
  const leaves = keys.filter((k) => !keys.some((other) => other.startsWith(`${k} > `)));
  const failed = new Set(leaves.filter((k) => !results.get(k)));
  const passed = new Set(leaves.filter((k) => results.get(k)));

  if (process.argv.includes('--write-expected') && only.length) {
    console.log('\n--write-expected needs every file: run it without file names');
    process.exitCode = 1;
  } else if (process.argv.includes('--write-expected')) {
    fs.writeFileSync(EXPECTED, JSON.stringify([...failed].sort(), null, 2) + '\n');
    console.log(`\nrust-expected-failures.json: ${failed.size} tests the Rust atm doesn't pass yet`);
  } else {
    const unexpected = [...failed].filter((t) => !expected.has(t)).sort();
    const nowPassing = [...passed].filter((t) => expected.has(t)).sort();
    console.log(`\nRust atm: ${passed.size} pass, ${failed.size} fail (${expected.size} expected to fail)`);
    if (unexpected.length)
      console.log(`\nFailing, and not in rust-expected-failures.json:\n  ${unexpected.join('\n  ')}`);
    if (nowPassing.length)
      console.log(`\nPassing now: take them out of rust-expected-failures.json:\n  ${nowPassing.join('\n  ')}`);
    process.exitCode = unexpected.length || nowPassing.length ? 1 : 0;
  }
}
