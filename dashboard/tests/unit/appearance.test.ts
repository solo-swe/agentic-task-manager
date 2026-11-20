import { describe, expect, it } from 'vitest';
import {
  applyAppearance,
  DOC_SIZES,
  DOC_STYLES,
  isModePref,
  knownDocSize,
  knownDocStyle,
  knownTheme,
  resolveMode,
  THEMES,
} from '../../src/lib/appearance';

describe('appearance', () => {
  it('System follows the OS; Light and Dark override it', () => {
    expect(resolveMode('system', true)).toBe('light');
    expect(resolveMode('system', false)).toBe('dark');
    expect(resolveMode('light', false)).toBe('light');
    expect(resolveMode('dark', true)).toBe('dark');
  });

  it('accepts only known mode choices', () => {
    for (const ok of ['system', 'light', 'dark']) expect(isModePref(ok)).toBe(true);
    for (const bad of ['auto', '', null, 1]) expect(isModePref(bad)).toBe(false);
  });

  it('unknown themes fall back to Default', () => {
    expect(knownTheme('default')).toBe('default');
    expect(knownTheme('vaporwave')).toBe('default');
    expect(knownTheme(undefined)).toBe('default');
    expect(THEMES[0]).toMatchObject({ id: 'default', name: 'Default' });
    // Each theme shows a preview in the menu: background, accent, green, red
    for (const t of THEMES) expect(t.swatch).toHaveLength(4);
  });

  it('document styles: Auto first, then GitHub, Docs and Magazine; an unknown one is Auto (T-404)', () => {
    expect(DOC_STYLES.map((s) => s.id)).toEqual(['auto', 'github', 'docs', 'magazine']);
    expect(knownDocStyle('magazine')).toBe('magazine');
    for (const bad of ['terminal', '', null, 3]) expect(knownDocStyle(bad)).toBe('auto');
  });

  it('text sizes: Tiny to Huge, growing, Medium as written; an unknown one is Medium (ATM-691)', () => {
    expect(DOC_SIZES.map((s) => s.id)).toEqual(['tiny', 'small', 'medium', 'large', 'huge']);
    const scales = DOC_SIZES.map((s) => s.scale);
    expect([...scales].sort((a, b) => a - b)).toEqual(scales);
    expect(DOC_SIZES.find((s) => s.id === 'medium')?.scale).toBe(1);
    expect(knownDocSize('tiny')).toBe('tiny');
    for (const bad of ['gigantic', '', null, 2]) expect(knownDocSize(bad)).toBe('medium');
  });

  it('applies theme, mode and text size as attributes on the root element', () => {
    const root = document.createElement('html');
    applyAppearance(root, 'default', 'light');
    expect(root.dataset).toMatchObject({ theme: 'default', mode: 'light', docSize: 'medium' });
    applyAppearance(root, 'default', 'dark', 'large');
    expect(root.dataset).toMatchObject({ mode: 'dark', docSize: 'large' });
  });
});
