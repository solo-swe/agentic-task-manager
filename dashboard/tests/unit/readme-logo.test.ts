import { existsSync, readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { FAVICON_COLOR } from '../../build/favicon';
import { ACCENT, brandLogo } from '../../../branding/generate.mjs';

// The logo outside the dashboard (README, GitHub's social preview, the org's avatar) is made by branding/generate.mjs
// from the dashboard's logo, in the accent blue.
// The path as an argument, never a literal or template string inside `new URL(…, import.meta.url)`: Vite would turn
// that into an asset import. Paths are relative to the repository root.
const file = (path: string) => new URL(path, import.meta.url);
const read = (path: string) => readFileSync(file('../../../' + path), 'utf8');

describe('the branding files', () => {
  it('are up to date with the logo source (run `npm run branding` after changing it)', () => {
    const logo = read('branding/atm-logo.svg');
    expect(logo).toBe(brandLogo(read('dashboard/src/assets/atm-logo.svg')));
    expect(logo).toContain(`stroke="${FAVICON_COLOR}"`);
    expect(ACCENT).toBe(FAVICON_COLOR);
  });

  it('head the README, with the PNGs next to the SVG', () => {
    expect(read('README.md')).toContain('src="branding/atm-logo.svg"');
    // The PNGs differ by platform, so only their presence is checked
    expect(existsSync(file('../../../branding/atm-logo.png'))).toBe(true);
    // The social preview, dark and light (T-279): one of them is uploaded
    expect(existsSync(file('../../../branding/social-preview-dark.png'))).toBe(true);
    expect(existsSync(file('../../../branding/social-preview-light.png'))).toBe(true);
    expect(existsSync(file('../../../branding/social-preview.png'))).toBe(false);
  });
});
