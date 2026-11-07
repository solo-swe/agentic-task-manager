import { readFileSync } from 'node:fs';
import { TARGET } from '../../build/finish-page';
import { readVersion } from '../../build/workspace';
import { expect, test } from './helpers';

// Checks on the built artifact itself (dist/tasks-dashboard.html, which atm embeds), not on its behavior.
const BUILT = TARGET;
// 500 KiB = 512,000 bytes (Vite's build output counts kB as 1,000 bytes, so it shows this as 512 kB). Raised from
// 400 KiB in 0.23.0 for dagre, the dependency graph's layout (about 48 kB; the file is served locally, 134 kB gzipped),
// then from 450 KiB in 0.24.0 for the sprint and release views (decided with Hakan in sprint planning, T-305/T-309),
// then from 500 KiB in 0.26.0 for the graph's auto-fit and animated fits (decided with Hakan, T-341), and from 525 KiB
// in 0.26.0 for the overview's Agents, Activity and Progress (decided with Hakan, T-329); T-342 looks for what makes
// the file grow, to trim it. Raised from 550 KiB in 0.29.0 for the document viewer and CompMD's coming upgrade
// (decided with Hakan, 2026-11-03), and from 600 KiB in 0.31.0 for SvelteKit's runtime (about 60 kB, 23 kB gzipped;
// decided with Hakan, T-428). Raised from 650 KiB in 0.31.0 for Bits UI (about 150 kB; T-431): the size is no longer
// a target, until the move to Rust and Tauri sets the new ones (decided with Hakan, 2026-11-06); this only catches
// a runaway build
const MAX_BYTES = 850 * 1024;

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

  test("carries atm's version", () => {
    expect(html()).toContain(`<meta name="atm-dashboard-version" content="${readVersion()}"`);
  });

  test("SvelteKit's app version is atm's, not the build time: the same sources build the same page (T-428)", () => {
    const version = readVersion();
    expect(html()).toMatch(new RegExp(`version: "${version.replace(/\./g, '\\.')}"`));
  });
});
