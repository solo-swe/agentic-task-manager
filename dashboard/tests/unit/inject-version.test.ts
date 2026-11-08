// @vitest-environment node
import { describe, expect, it } from 'vitest';
import { setDashboardVersion } from '../../build/inject-version.ts';
import { readVersion } from '../../build/workspace.ts';

const meta = (v: string) => `<meta name="atm-dashboard-version" content="${v}">`;

describe('setDashboardVersion', () => {
  it('replaces the version in the meta tag', () => {
    expect(setDashboardVersion(`<head>${meta('0.0.0')}</head>`, '1.2.3')).toBe(`<head>${meta('1.2.3')}</head>`);
  });

  it('throws when the meta tag is missing', () => {
    expect(() => setDashboardVersion('<head></head>', '1.2.3')).toThrow(/meta tag not found/);
  });
});

describe('readVersion', () => {
  it("reads the version from the real Cargo.toml's [workspace.package]", () => {
    expect(readVersion()).toMatch(/^\d+\.\d+\.\d+$/);
  });
});
