import { existsSync, readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { FAVICON_COLOR } from '../../build/favicon';
import { ACCENT, appIcon, avatar, brandLogo, iconPages, squircle } from '../../../branding/generate.mjs';

// The logo outside the dashboard (README, GitHub's social preview, the org's avatar) is made by branding/generate.mjs
// from the dashboard's logo, in the accent blue.
// The path as an argument, never a literal or template string inside `new URL(…, import.meta.url)`: Vite would turn
// that into an asset import. Paths are relative to the repository root.
const file = (path: string) => new URL(path, import.meta.url);
const read = (path: string) => readFileSync(file('../../../' + path), 'utf8');

describe('the branding files', () => {
  it('are up to date with the logo source (run `pnpm run branding` after changing it)', () => {
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
    // The app icon's source (T-504), from which `pnpm tauri icon` made app/src-tauri/icons/
    expect(existsSync(file('../../../branding/app-icon.png'))).toBe(true);
    expect(existsSync(file('../../../app/src-tauri/icons/icon.icns'))).toBe(true);
  });
});

describe('the app icon and the avatar (T-504)', () => {
  const points = (path: string) =>
    path
      .replace(/^M|Z$/g, '')
      .split('L')
      .map((p) => p.split(' ').map(Number));

  it('the body is a squircle filling its box, its corners cut further in than a circle’s and less than a square’s', () => {
    const shape = points(squircle(100, 100, 824));
    const xs = shape.map(([x]) => x);
    const ys = shape.map(([, y]) => y);
    expect(Math.min(...xs)).toBeCloseTo(100, 0);
    expect(Math.max(...xs)).toBeCloseTo(924, 0);
    expect(Math.min(...ys)).toBeCloseTo(100, 0);
    expect(Math.max(...ys)).toBeCloseTo(924, 0);
    // Along the diagonal: a circle reaches 0.707 of the half side, a square 1; Apple's shape about 0.87
    const diagonal = shape[Math.round(shape.length / 8)];
    const reach = (diagonal[0] - 512) / 412;
    expect(reach).toBeGreaterThan(0.8);
    expect(reach).toBeLessThan(0.95);
  });

  it('the icon keeps Apple’s grid: a 1024 canvas, the body 824 in its middle, the logo in the accent blue', () => {
    const logo = brandLogo(read('dashboard/src/assets/atm-logo.svg'));
    const icon = appIcon(logo);
    expect(icon).toContain('viewBox="0 0 1024 1024"');
    expect(icon).toContain(`d="${squircle(100, 100, 824)}"`);
    // The logo is the README's (in the accent blue), embedded as a data: URL
    expect(icon).toContain(encodeURIComponent(logo));
    expect(logo).toContain(`stroke="${ACCENT}"`);
    // The avatar: the face edge to edge (GitHub rounds it), the same logo
    expect(avatar(logo)).toContain('<rect width="512" height="512" fill="url(#face)" />');
    expect(iconPages(logo).map((p) => [p.file, p.width, p.transparent])).toEqual([
      ['app-icon.png', 1024, true],
      ['atm-logo.png', 512, true],
    ]);
  });
});
