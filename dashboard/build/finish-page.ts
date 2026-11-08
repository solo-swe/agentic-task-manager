// `node build/finish-page.ts`, after `vite build` (`pnpm run build`): the single-file dashboard (T-428). SvelteKit's
// static adapter writes the page last, once both of its builds are done, so no Vite hook of ours sees it: this step
// takes the adapter's fallback page (everything inlined, `output.bundleStrategy: 'inline'`), stamps atm's version,
// the favicon and the build ID into it, and writes it into dist/, where the Rust atm embeds it from (T-477).
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { setBuildId } from './build-id.ts';
import { faviconHref, setFavicon } from './favicon.ts';
import { setDashboardVersion } from './inject-version.ts';
import { readVersion } from './workspace.ts';

const at = (path: string) => fileURLToPath(new URL(path, import.meta.url));

/** The static adapter's output folder (not committed): vite.config.ts gives it to the adapter */
export const PAGES = at('../.svelte-kit/static');
/** What the build makes for atm to embed, never committed: the page, and CompMD's runtime under assets/ */
export const DIST = at('../dist');
/** The page: the binary embeds it (crates/atm-common/src/dashboard.rs), `atm serve` serves it */
export const TARGET = `${DIST}/tasks-dashboard.html`;
const LOGO = at('../src/assets/atm-logo.svg');

/** The served page from the adapter's: atm's version and the favicon in, then the build ID over all of it. */
export function finishedPage(html: string, version: string, logoSvg: string): string {
  return setBuildId(setFavicon(setDashboardVersion(html, version), faviconHref(logoSvg)));
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const html = readFileSync(`${PAGES}/index.html`, 'utf8');
  mkdirSync(DIST, { recursive: true });
  writeFileSync(TARGET, finishedPage(html, readVersion(), readFileSync(LOGO, 'utf8')));
}
