// Light/dark mode and theme (FR-2, FR-3): chosen per browser, applied as data-theme / data-mode on <html>, where the
// theme stylesheets (src/themes/*.css) pick them up. index.html applies the stored choice before the first paint
// with the same rules; keep resolveMode() and that script in step.

import { THEMES } from '../themes';

export type Mode = 'light' | 'dark';
export type ModePref = 'system' | Mode;
export const MODE_PREFS: ModePref[] = ['system', 'light', 'dark'];
export const MODE_LABELS: Record<ModePref, string> = { system: 'System', light: 'Light', dark: 'Dark' };

export interface ThemeInfo {
  id: string;
  name: string;
  /** A few colours of the theme's dark variant, to preview it in the menu */
  swatch: string[];
  /** Themes that change more than colours (the board, the cards) are listed apart, after a separator */
  group?: 'board';
}

export { THEMES };
export const DEFAULT_THEME = 'default';

export function isModePref(value: unknown): value is ModePref {
  return (MODE_PREFS as unknown[]).includes(value);
}

export function knownTheme(id: unknown): string {
  return THEMES.some((t) => t.id === id) ? (id as string) : DEFAULT_THEME;
}

/** How the document viewer's page looks (T-404). These are host styles, so the dashboard defines them
 * (themes/doc-styles.css), after the ideas of CompMD's editor's own; 'auto' is the theme's page */
export const DOC_STYLES = [
  { id: 'auto', name: 'Auto (the theme’s page)' },
  { id: 'github', name: 'GitHub' },
  { id: 'docs', name: 'Docs' },
  { id: 'magazine', name: 'Magazine' },
] as const;
export type DocStyle = (typeof DOC_STYLES)[number]['id'];
export const DEFAULT_DOC_STYLE: DocStyle = 'auto';

export function knownDocStyle(id: unknown): DocStyle {
  return DOC_STYLES.find((s) => s.id === id)?.id ?? DEFAULT_DOC_STYLE;
}

export function resolveMode(pref: ModePref, systemPrefersLight: boolean): Mode {
  return pref === 'system' ? (systemPrefersLight ? 'light' : 'dark') : pref;
}

export function applyAppearance(root: HTMLElement, theme: string, mode: Mode) {
  root.dataset.theme = theme;
  root.dataset.mode = mode;
}
