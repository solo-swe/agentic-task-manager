// @vitest-environment node
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { buildId, setBuildId } from '../../build/build-id';

const page = (body: string, id = '') =>
  `<html><head><meta name="atm-dashboard-build" content="${id}" /></head><body>${body}</body></html>`;

describe('build ID', () => {
  it('is 12 hex digits, the same for the same page, different for any change', () => {
    expect(buildId(page('a'))).toMatch(/^[0-9a-f]{12}$/);
    expect(buildId(page('a'))).toBe(buildId(page('a')));
    expect(buildId(page('a'))).not.toBe(buildId(page('b')));
  });

  it("ignores the tag's own content, so stamping it again gives the same ID", () => {
    const stamped = setBuildId(page('a'));
    expect(stamped).toContain(`content="${buildId(page('a'))}"`);
    expect(setBuildId(stamped)).toBe(stamped);
    expect(buildId(page('a', 'old'))).toBe(buildId(page('a')));
  });

  it('needs the tag', () => {
    expect(() => buildId('<html></html>')).toThrow(/atm-dashboard-build meta tag not found/);
  });

  it('the built dashboard carries its own build ID', () => {
    const html = readFileSync(new URL('../../dist/tasks-dashboard.html', import.meta.url), 'utf8');
    const id = /<meta name="atm-dashboard-build" content="([0-9a-f]{12})"/.exec(html)?.[1];
    expect(id).toBe(buildId(html));
  });
});
