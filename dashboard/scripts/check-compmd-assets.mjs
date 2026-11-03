// Fails if the committed CompMD runtime (plugin/assets/compmd/<version>/, docs/compmd-rendering-design.md §2) isn't
// what the build makes now from the installed packages and compmd/entry.ts: it builds again into a temporary folder
// and compares every file. Run by `npm run check`, so CI catches a package update or an entry change committed
// without `npm run build`.
import { mkdtempSync, readdirSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { build } from 'vite';

const dashboard = fileURLToPath(new URL('..', import.meta.url));
const assets = fileURLToPath(new URL('../../plugin/assets/compmd/', import.meta.url));
const version = JSON.parse(
  readFileSync(join(dashboard, 'node_modules/@compmd/processor/package.json'), 'utf8'),
).version;

const fail = (message) => {
  console.error(`plugin/assets/compmd is stale: ${message}. Run 'npm run build' in dashboard/ and commit it.`);
  process.exit(1);
};

const folders = readdirSync(assets);
if (folders.join() !== version) fail(`it holds ${folders.join(', ') || 'nothing'}, not just ${version}`);

const out = mkdtempSync(join(tmpdir(), 'atm-compmd-'));
try {
  await build({
    configFile: join(dashboard, 'vite.compmd.config.ts'),
    root: dashboard,
    logLevel: 'error',
    build: { outDir: out },
  });
  const committed = join(assets, version);
  const built = readdirSync(out).sort();
  if (readdirSync(committed).sort().join() !== built.join())
    fail(`its files differ from the build's (${built.join(', ')})`);
  for (const file of built)
    if (!readFileSync(join(out, file)).equals(readFileSync(join(committed, file))))
      fail(`${file} differs from the build's`);
} finally {
  rmSync(out, { recursive: true, force: true });
}
console.log(`CompMD ${version}: plugin/assets/compmd matches the build`);
