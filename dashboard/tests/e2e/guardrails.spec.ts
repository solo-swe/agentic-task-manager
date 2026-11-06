import { readFileSync } from 'node:fs';
import { expect, test } from './helpers';

// Checks on the shipped artifact itself (plugin/templates/tasks-dashboard.html), not on its behavior.
const BUILT = new URL('../../../plugin/templates/tasks-dashboard.html', import.meta.url);
const PLUGIN_JSON = new URL('../../../plugin/.claude-plugin/plugin.json', import.meta.url);
// 500 KiB = 512,000 bytes (Vite's build output counts kB as 1,000 bytes, so it shows this as 512 kB). Raised from
// 400 KiB in 0.23.0 for dagre, the dependency graph's layout (about 48 kB; the file is served locally, 134 kB gzipped),
// then from 450 KiB in 0.24.0 for the sprint and release views (decided with Hakan in sprint planning, T-305/T-309),
// then from 500 KiB in 0.26.0 for the graph's auto-fit and animated fits (decided with Hakan, T-341), and from 525 KiB
// in 0.26.0 for the overview's Agents, Activity and Progress (decided with Hakan, T-329); T-342 looks for what makes
// the file grow, to trim it. Raised from 550 KiB in 0.29.0 for the document viewer and CompMD's coming upgrade
// (decided with Hakan, 2026-11-03), and from 600 KiB in 0.31.0 for SvelteKit's runtime (about 60 kB, 23 kB gzipped;
// decided with Hakan, T-428)
const MAX_BYTES = 650 * 1024;

test.describe('shipped dashboard file', () => {
  const html = () => readFileSync(BUILT, 'utf8');

  test('is a single self-contained file', () => {
    const h = html();
    expect(h).not.toMatch(/<script[^>]+\bsrc=/i);
    expect(h).not.toMatch(/<link[^>]+rel=["']?(stylesheet|modulepreload)/i);
    // The favicon is inline (the logo as a data: URL), never a file next to the page
    const icons = [...h.matchAll(/<link[^>]+rel=["']?[^"'>]*icon[^>]*>/gi)].map(([tag]) => tag);
    expect(icons).toHaveLength(1);
    expect(icons[0]).toMatch(/\shref="data:image\/svg\+xml,/);
    expect(h).not.toMatch(/url\(\s*["']?https?:/i);
    expect(h).not.toMatch(/@import/i);
  });

  test(`stays under ${MAX_BYTES / 1024} KiB (${MAX_BYTES.toLocaleString('en-US')} bytes)`, () => {
    expect(Buffer.byteLength(html())).toBeLessThan(MAX_BYTES);
  });

  test('carries the plugin version', () => {
    const { version } = JSON.parse(readFileSync(PLUGIN_JSON, 'utf8'));
    expect(html()).toContain(`<meta name="atm-dashboard-version" content="${version}"`);
  });
});
