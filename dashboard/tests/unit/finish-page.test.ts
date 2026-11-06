// @vitest-environment node
// build/finish-page.ts (T-428): the static adapter's page becomes the shipped one, with the plugin's version, the
// favicon and the build ID over all of it
import { describe, expect, it } from 'vitest';
import { buildId } from '../../build/build-id';
import { finishedPage } from '../../build/finish-page';

const PAGE = [
  '<!doctype html><html><head>',
  '<meta name="atm-dashboard-version" content="0.0.0" />',
  '<meta name="atm-dashboard-build" content="" />',
  '</head><body><script>start()</script></body></html>',
].join('\n');
const LOGO = '<svg><!-- a comment --><path fill="currentColor" d="M0 0"/></svg>';

describe('finishedPage', () => {
  it('stamps the version and the favicon, then the build ID of the result', () => {
    const page = finishedPage(PAGE, '1.2.3', LOGO);
    expect(page).toContain('<meta name="atm-dashboard-version" content="1.2.3" />');
    expect(page).toMatch(/<link rel="icon" type="image\/svg\+xml" href="data:image\/svg\+xml,[^"]+" \/>\n {2}<\/head>/);
    const id = /<meta name="atm-dashboard-build" content="([0-9a-f]{12})"/.exec(page)?.[1];
    expect(id).toBe(buildId(page));
  });

  it('gives the same page for the same input, and another build ID for another version', () => {
    expect(finishedPage(PAGE, '1.2.3', LOGO)).toBe(finishedPage(PAGE, '1.2.3', LOGO));
    expect(buildId(finishedPage(PAGE, '1.2.4', LOGO))).not.toBe(buildId(finishedPage(PAGE, '1.2.3', LOGO)));
  });

  it('refuses a page without the meta tags it stamps', () => {
    expect(() => finishedPage('<html><head></head></html>', '1.2.3', LOGO)).toThrow(/atm-dashboard-version/);
  });
});
