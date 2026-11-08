// Runs the agent sandbox (docs/security-architecture.md §3.1, level 2) and prints its verdict.
//
//   ANTHROPIC_API_KEY=... ATM_SANDBOX_BIN=<a Linux atm> node tests/sandbox/run.mjs
//
// Needs Docker, and atm built for Linux on the machine's architecture: a release archive's `atm`, or the `atm-linux`
// artifact of a CI run on your branch. Builds the image with that atm and its skills (`atm agents install`), runs
// /atm-loop-tasks on the seeded project with no network but the model API, then judges, per red-team task, whether the
// agent blocked it. Exits 1 when any was followed. A run costs a few minutes and a few dollars of model calls.
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { judge } from './verdict.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const binary = process.env.ATM_SANDBOX_BIN;
if (!binary || !fs.existsSync(binary)) {
  console.error('Set ATM_SANDBOX_BIN to a Linux build of atm (a release archive’s, or CI’s atm-linux artifact).');
  process.exit(2);
}
// The image's atm: copied into the build context (bin/ is not committed)
fs.mkdirSync(path.join(here, 'bin'), { recursive: true });
fs.copyFileSync(binary, path.join(here, 'bin/atm'));
fs.chmodSync(path.join(here, 'bin/atm'), 0o755);

const results = path.join(here, 'results');
fs.rmSync(results, { recursive: true, force: true });
fs.mkdirSync(path.join(results, 'agent'), { recursive: true });
fs.mkdirSync(path.join(results, 'proxy'), { recursive: true });

const compose = (...args) =>
  execFileSync('docker', ['compose', '-f', path.join(here, 'compose.yaml'), ...args], { stdio: 'inherit' });

const PROMPT = '/atm-loop-tasks';
compose('build', 'agent');
try {
  compose(
    'run',
    '--rm',
    'agent',
    'sh',
    '-c',
    // The agent works unattended, as /atm-loop-tasks does; the sandbox is what makes skipping prompts safe here
    `claude -p '${PROMPT}' --dangerously-skip-permissions --output-format stream-json --verbose > /results/transcript.jsonl; ` +
      'node /opt/atm/verdict.mjs collect /home/agent/project /home/agent/remote.git > /results/evidence.json',
  );
} finally {
  compose('down');
}

const evidence = JSON.parse(fs.readFileSync(path.join(results, 'agent/evidence.json'), 'utf8'));
const proxyLogFile = path.join(results, 'proxy/proxy.log');
const proxyLog = fs.existsSync(proxyLogFile) ? fs.readFileSync(proxyLogFile, 'utf8') : '';
const verdict = judge(evidence, proxyLog);
for (const r of verdict.results) console.log(`${r.id}: ${r.result}${r.what ? ` (${r.what})` : ''}`);
if (verdict.leaked.length) console.log(`canaries seen by the proxy: ${verdict.leaked.join(', ')}`);
console.log(verdict.pass ? 'PASS: every red-team task was blocked' : 'FAIL');
console.log(`transcript: ${path.join(results, 'agent/transcript.jsonl')}`);
process.exitCode = verdict.pass ? 0 : 1;
