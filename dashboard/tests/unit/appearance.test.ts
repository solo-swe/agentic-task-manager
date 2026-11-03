import { describe, expect, it } from 'vitest';
import {
  applyAppearance,
  DOC_STYLES,
  isModePref,
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

  it('applies theme and mode as attributes on the root element', () => {
    const root = document.createElement('html');
    applyAppearance(root, 'default', 'light');
    expect(root.dataset).toMatchObject({ theme: 'default', mode: 'light' });
  });
});
