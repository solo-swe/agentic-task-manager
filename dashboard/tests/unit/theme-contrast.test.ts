// @vitest-environment node
// FR-3.4: text and chart colours are readable in every theme and mode. Reads the token values straight from the
// theme stylesheets and checks WCAG contrast ratios. Targets (docs/theme-validation.md):
//   text and muted text on every surface: 4.5:1 (AA); High contrast: 7:1 (AAA)
//   coloured labels (badges, type/status colours) on their tint and on the surface: 3:1; High contrast: 4.5:1
//   text on accent buttons: 4.5:1; High contrast: 7:1
//   burndown lines on the surface: 3:1 (the series colours were also run through the dataviz palette validator)
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';
import { THEMES } from '../../src/lib/appearance';

const src = path.resolve(import.meta.dirname, '../../src');
type Tokens = Record<string, string>;

/** Custom properties of every rule in a stylesheet whose selector matches `selector`. */
function tokens(file: string, selector: (sel: string) => boolean): Tokens {
  const css = readFileSync(path.join(src, file), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
  const out: Tokens = {};
  for (const [, sel, body] of css.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    if (!selector(sel.trim())) continue;
    for (const [, name, value] of body.matchAll(/--([\w-]+)\s*:\s*([^;]+);/g)) out[name] = value.trim();
  }
  return out;
}

function palette(theme: string, mode: 'light' | 'dark'): Tokens {
  if (theme === 'default') {
    const dark = tokens('tokens.css', (s) => s === ':root');
    return mode === 'dark'
      ? dark
      : { ...dark, ...tokens('themes/default.css', (s) => s.includes("[data-mode='light']")) };
  }
  // The page's tokens: the :root rules only. A theme may set tokens inside an element too (the sticky-notes and
  // hand-drawn themes' board and cards, a `.class` in the selector); those are checked below.
  return tokens(
    `themes/${theme}.css`,
    (s) => s.includes(`'${theme}'`) && !s.includes('.') && s.includes("[data-mode='light']") === (mode === 'light'),
  );
}

/** The tokens a theme sets inside `element` (e.g. `.kanban-board`), over the page's, for one mode. */
function scoped(theme: string, mode: 'light' | 'dark', element: string): Tokens {
  const inside = (light: boolean) =>
    tokens(
      `themes/${theme}.css`,
      (s) => s.includes(`'${theme}'`) && s.endsWith(element) && s.includes("[data-mode='light']") === light,
    );
  // A light rule adds to the dark one, as in the browser; a token may name another (var(--note-ink))
  const page = mode === 'light' ? { ...palette(theme, 'dark'), ...palette(theme, 'light') } : palette(theme, 'dark');
  const t: Tokens = { ...page, ...inside(false), ...(mode === 'light' ? inside(true) : {}) };
  const resolve = (value: string): string => {
    const ref = /^var\(--([\w-]+)\)$/.exec(value);
    return ref ? resolve(t[ref[1]]) : value;
  };
  return Object.fromEntries(Object.entries(t).map(([name, value]) => [name, resolve(value)]));
}

const rgb = (hex: string) => {
  const h = hex.replace('#', '');
  if (/^[0-9a-f]{3}$/i.test(h)) return [...h].map((c) => parseInt(c + c, 16) / 255);
  if (!/^[0-9a-f]{6}$/i.test(h)) throw new Error(`not a #rgb or #rrggbb colour: ${hex}`);
  return [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16) / 255);
};
const luminance = (hex: string) => {
  const [r, g, b] = rgb(hex).map((c) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
};
export const contrast = (a: string, b: string) => {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
};
/** `color-mix(in srgb, fg pct%, transparent)` painted over `bg`. */
const tint = (fg: string, bg: string, pct: number) => {
  const [f, b] = [rgb(fg), rgb(bg)];
  return (
    '#' +
    f
      .map((c, i) =>
        Math.round((c * pct + b[i] * (1 - pct)) * 255)
          .toString(16)
          .padStart(2, '0'),
      )
      .join('')
  );
};

const COLOURED = ['accent', 'green', 'yellow', 'red', 'purple', 'pink', 'orange', 'cyan'];

for (const { id } of THEMES) {
  for (const mode of ['dark', 'light'] as const) {
    describe(`${id} ${mode}`, () => {
      const t = palette(id, mode);
      const hc = id === 'high-contrast';
      const TEXT = hc ? 7 : 4.5;
      const LABEL = hc ? 4.5 : 3;
      // The Default theme predates these targets for labels; it is GitHub's palette and stays as it is
      const surfaces = ['bg', 'surface', 'surface2'];

      it('defines every token as a colour', () => {
        for (const name of [
          ...surfaces,
          'border',
          'text',
          'text-muted',
          'on-accent',
          ...COLOURED,
          'series-remaining',
          'series-scope',
        ])
          expect(() => rgb(t[name]), name).not.toThrow();
      });

      it(`text: ${TEXT}:1 on every surface, muted text on the surface`, () => {
        for (const s of surfaces) expect(contrast(t.text, t[s]), `text on ${s}`).toBeGreaterThanOrEqual(TEXT);
        expect(contrast(t['text-muted'], t.surface), 'muted text').toBeGreaterThanOrEqual(TEXT);
        // Filter count badges: text on a 15% text tint
        expect(contrast(t.text, tint(t.text, t.surface, 0.15)), 'count badge').toBeGreaterThanOrEqual(TEXT);
      });

      it(`coloured labels: ${LABEL}:1 on their tint and on the surface`, () => {
        for (const c of COLOURED) {
          expect(contrast(t[c], tint(t[c], t.surface, 0.15)), `${c} on its badge`).toBeGreaterThanOrEqual(LABEL);
          expect(contrast(t[c], t.surface), `${c} on the surface`).toBeGreaterThanOrEqual(LABEL);
        }
      });

      it(`text on accent buttons: ${hc ? 7 : 4.5}:1`, () => {
        expect(contrast(t['on-accent'], t.accent)).toBeGreaterThanOrEqual(hc ? 7 : 4.5);
      });

      it('burndown lines stand out from the surface (3:1)', () => {
        for (const s of ['series-remaining', 'series-scope'])
          expect(contrast(t[s], t.surface), s).toBeGreaterThanOrEqual(3);
      });
    });
  }
}

// Themes that draw the board: on the board (its own --surface), the text, the headings (--board-ink) and the
// coloured labels read like on the page
for (const theme of ['sticky-notes', 'hand-drawn']) {
  for (const mode of ['dark', 'light'] as const) {
    it(`${theme} ${mode}: text, headings and labels on the board`, () => {
      const b = scoped(theme, mode, '.kanban-board');
      for (const name of ['text', 'text-muted', 'board-ink'])
        expect(contrast(b[name], b.surface), name).toBeGreaterThanOrEqual(4.5);
      for (const c of COLOURED) expect(contrast(b[c], b.surface), c).toBeGreaterThanOrEqual(3);
    });
  }
}

// The sticky notes are paper in both modes: inside a card, text reads at 4.5:1 and the coloured labels at 3:1 on
// every note colour, and on their badge's tint
const NOTES = ['yellow', 'pink', 'green', 'blue', 'orange', 'purple'];
for (const mode of ['dark', 'light'] as const) {
  it(`sticky-notes ${mode}: the inks read on every note`, () => {
    const card = scoped('sticky-notes', mode, '.kanban-card');
    for (const note of NOTES) {
      const paper = card[`note-${note}`];
      for (const name of ['text', 'text-muted'])
        expect(contrast(card[name], paper), `${name} on ${note}`).toBeGreaterThanOrEqual(4.5);
      for (const c of COLOURED) {
        expect(contrast(card[c], paper), `${c} on ${note}`).toBeGreaterThanOrEqual(3);
        expect(contrast(card[c], tint(card[c], paper, 0.15)), `${c} badge on ${note}`).toBeGreaterThanOrEqual(3);
      }
    }
  });
}
