// Runs every tests/scripts/*.test.mjs file with node:test, against this checkout's atm (`cargo build` first; T-478).
// The tests drive atm as a user would. `pnpm run test:scripts` in dashboard/ builds the binary and runs it.
// Usage: node tests/scripts/run.mjs [--file-limit=<seconds>] [--quiet] [<file>.test.mjs …]
//
// Each file runs in its own `node --test` process, four at a time, its TAP read: a test's identity is its file and the
// names of the suites it is in, "claims.test.mjs > claims > start claims the task …". A test waiting on something
// that never comes (a server, a watcher) can leave a process behind that keeps its file running for ever, so each
// file has a watchdog: past --file-limit (180 s by default) its process group is killed, and
// "<file> > (did not finish)" fails.
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { createInterface } from 'node:readline';
import { fileURLToPath } from 'node:url';

const dir = path.dirname(fileURLToPath(import.meta.url));
// `run.mjs merge.test.mjs serve.test.mjs`: only those files
const only = process.argv.slice(2).filter((a) => a.endsWith('.test.mjs'));
const files = fs
  .readdirSync(dir)
  .filter((f) => f.endsWith('.test.mjs'))
  .filter((f) => !only.length || only.includes(f))
  .sort();

const limitArg = process.argv.find((a) => a.startsWith('--file-limit='));
// A watchdog for hangs, not a time budget: Windows starts processes several times slower (scenarios.test.mjs takes
// about 160 s on CI's Windows runner), so its default is longer
const DEFAULT_LIMIT = process.platform === 'win32' ? 400 : 180;
const FILE_LIMIT_MS = (limitArg ? Number(limitArg.slice('--file-limit='.length)) : DEFAULT_LIMIT) * 1000;
const TEST_LIMIT_MS = 60000;
const results = new Map();
const timings = [];
// `--quiet`: only each file's time and the summary, not the TAP
const quiet = process.argv.includes('--quiet');

/** End a file's process and whatever it started (its process group; on Windows, the process itself) */
function killAll(child) {
  try {
    if (process.platform === 'win32') child.kill('SIGKILL');
    else process.kill(-child.pid, 'SIGKILL');
  } catch {
    // already gone
  }
}

const runFile = (file) =>
  new Promise((resolve) => {
    const started = Date.now();
    const out = [];
    const stack = [];
    const child = spawn(
      process.execPath,
      ['--test', '--test-reporter=tap', `--test-timeout=${TEST_LIMIT_MS}`, path.join(dir, file)],
      { env: process.env, stdio: ['ignore', 'pipe', 'pipe'], detached: process.platform !== 'win32' },
    );
    let timedOut = false;
    const watchdog = setTimeout(() => {
      timedOut = true;
      killAll(child);
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
      killAll(child);
      if (timedOut) results.set(`${file} > (did not finish)`, false);
      const seconds = Math.round((Date.now() - started) / 1000);
      // A failing file's TAP is printed even with --quiet: it says why
      const failed = timedOut || out.some((line) => /^\s*not ok /.test(line));
      if (!quiet || failed) process.stdout.write(out.join(''));
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
const failed = leaves.filter((k) => !results.get(k)).sort();
console.log(`\natm: ${leaves.length - failed.length} pass, ${failed.length} fail`);
if (failed.length) console.log(`\nFailing:\n  ${failed.join('\n  ')}`);
process.exitCode = failed.length ? 1 : 0;
