// `node build/python-copy.ts`, the last step of `pnpm run build`: copies what the build made in dist/ (the page and
// CompMD's runtime) to where the Python atm reads them, plugin/templates/ and plugin/assets/ beside its
// plugin/scripts/atm.py. Neither is committed (T-477): the Rust atm embeds dist/. This step goes with the Python atm
// and plugin/ (T-454).
import { cpSync, mkdirSync, rmSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { DIST, TARGET } from './finish-page.ts';

const PLUGIN = fileURLToPath(new URL('../../plugin', import.meta.url));

mkdirSync(`${PLUGIN}/templates`, { recursive: true });
cpSync(TARGET, `${PLUGIN}/templates/tasks-dashboard.html`);
rmSync(`${PLUGIN}/assets`, { recursive: true, force: true });
cpSync(`${DIST}/assets`, `${PLUGIN}/assets`, { recursive: true });
