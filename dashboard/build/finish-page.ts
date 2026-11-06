// `node build/finish-page.ts`, after `vite build` (`pnpm run build`): the single-file dashboard (T-428). SvelteKit's
// static adapter writes the page last, once both of its builds are done, so no Vite hook of ours sees it: this step
// takes the adapter's fallback page (everything inlined, `output.bundleStrategy: 'inline'`), stamps the plugin's
// version, the favicon and the build ID into it, and writes it where the plugin ships it.
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { setBuildId } from './build-id.ts';
import { faviconHref, setFavicon } from './favicon.ts';
import { readPluginVersion, setDashboardVersion } from './inject-version.ts';

const at = (path: string) => fileURLToPath(new URL(path, import.meta.url));

/** The static adapter's output folder (not committed): vite.config.ts gives it to the adapter */
export const PAGES = at('../.svelte-kit/static');
/** The committed build the plugin ships */
export const TARGET = at('../../plugin/templates/tasks-dashboard.html');
const PLUGIN_JSON = at('../../plugin/.claude-plugin/plugin.json');
const LOGO = at('../src/assets/atm-logo.svg');

/** The shipped page from the adapter's: the plugin's version and the favicon in, then the build ID over all of it. */
export function finishedPage(html: string, version: string, logoSvg: string): string {
  return setBuildId(setFavicon(setDashboardVersion(html, version), faviconHref(logoSvg)));
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const html = readFileSync(`${PAGES}/index.html`, 'utf8');
  writeFileSync(TARGET, finishedPage(html, readPluginVersion(PLUGIN_JSON), readFileSync(LOGO, 'utf8')));
}
