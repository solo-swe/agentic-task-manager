import { readFileSync } from 'node:fs';
import { expect, test } from './helpers';

// Checks on the shipped artifact itself (plugin/templates/tasks-dashboard.html), not on its behavior.
const BUILT = new URL('../../../plugin/templates/tasks-dashboard.html', import.meta.url);
const PLUGIN_JSON = new URL('../../../plugin/.claude-plugin/plugin.json', import.meta.url);
// 500 KiB = 512,000 bytes (Vite's build output counts kB as 1,000 bytes, so it shows this as 512 kB). Raised from
// 400 KiB in 0.23.0 for dagre, the dependency graph's layout (about 48 kB; the file is served locally, 134 kB gzipped),
// then from 450 KiB in 0.24.0 for the sprint and release views (decided with Hakan in sprint planning, T-305/T-309),
// then from 500 KiB in 0.26.0 for the graph's auto-fit and animated fits (decided with Hakan, T-341; T-342 embeds
// only the recent changelog releases, the file's steady growth)
const MAX_BYTES = 525 * 1024;

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
