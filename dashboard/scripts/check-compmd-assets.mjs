// Fails if the committed CompMD runtime (plugin/assets/compmd/<version>/, docs/compmd-rendering-design.md §2, §3c)
// isn't what the build makes now from the installed packages, compmd.config.json and compmd/entry.ts: it builds again
// into a temporary folder and compares every file, in every folder (the sandbox's frames and the packs, which
// @compmd/vite writes). Run by `pnpm run check`, so CI catches a package or configuration change committed without
// `pnpm run build`.
import { mkdtempSync, readdirSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import { build } from 'vite';

const dashboard = fileURLToPath(new URL('..', import.meta.url));
const assets = fileURLToPath(new URL('../../plugin/assets/compmd/', import.meta.url));
const version = JSON.parse(
  readFileSync(join(dashboard, 'node_modules/@compmd/processor/package.json'), 'utf8'),
).version;

const fail = (message) => {
  console.error(`plugin/assets/compmd is stale: ${message}. Run 'pnpm run build' in dashboard/ and commit it.`);
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
  /** Every file under `dir`, by its path there, sorted */
  const files = (dir) =>
    readdirSync(dir, { recursive: true, withFileTypes: true })
      .filter((entry) => entry.isFile())
      .map((entry) => relative(dir, join(entry.parentPath, entry.name)))
      .sort();
  const built = files(out);
  const extra = files(committed).filter((file) => !built.includes(file));
  const missing = built.filter((file) => !files(committed).includes(file));
  if (extra.length || missing.length)
    fail(
      `it has ${extra.length} files the build doesn't make and lacks ${missing.length} it does (${[...extra, ...missing].slice(0, 5).join(', ')})`,
    );
  for (const file of built)
    if (!readFileSync(join(out, file)).equals(readFileSync(join(committed, file))))
      fail(`${file} differs from the build's`);
} finally {
  rmSync(out, { recursive: true, force: true });
}
console.log(`CompMD ${version}: plugin/assets/compmd matches the build`);
