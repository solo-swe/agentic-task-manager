// @vitest-environment node
// T-433: spacing and radii come from the scales in src/tokens.css. Outside it and the themes (token data), a padding,
// margin, gap or border radius has no px literal but 0 and a layout size (over 36px: a component's own distance,
// not spacing); a value relative to the text (`em`) is fine. A custom property's definition (`--x: 4px`) is token
// data too. The colours are no-hardcoded-colours.test.ts's.
import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

const src = path.resolve(import.meta.dirname, '../../src');
const files = (dir: string): string[] =>
  readdirSync(dir, { withFileTypes: true }).flatMap((e) =>
    e.isDirectory() ? files(path.join(dir, e.name)) : [path.join(dir, e.name)],
  );
const tokenData = (file: string) => /[\\/]themes[\\/]/.test(file) || file.endsWith('tokens.css');
const LAYOUT = 36;

/** The stylesheet text of a file: a .css file, or a component's <style> blocks and inline styles */
const styles = (file: string) => {
  const text = readFileSync(file, 'utf8');
  if (!file.endsWith('.svelte')) return text;
  const blocks = text.match(/<style[^>]*>[\s\S]*?<\/style>/g) ?? [];
  const inline = [...text.matchAll(/\sstyle="([^"]*)"/g)].map((m) => `{${m[1]};}`);
  return [...blocks, ...inline].join('\n');
};

/** The declarations of spacing and radii whose value has a px literal off the scale's place */
function offending(text: string): string[] {
  const found: string[] = [];
  for (const m of text.matchAll(
    /(?:^|[;{\s])((?:padding|margin)(?:-[\w-]+)?|(?:row-|column-)?gap|border(?:-\w+)*-radius)\s*:\s*([^;{}]+)(?=[;}])/gm,
  )) {
    const literals = [...m[2].matchAll(/(?<![\w.-])-?(\d+(?:\.\d+)?)px\b/g)].map((n) => Number(n[1]));
    if (literals.some((v) => v !== 0 && v <= LAYOUT)) found.push(`${m[1]}: ${m[2].trim()}`);
  }
  return found;
}

describe('spacing and radii are tokens (src/tokens.css)', () => {
  it('the scales are there: ten spacing steps, four radii, a pill and a circle', () => {
    const tokens = readFileSync(path.join(src, 'tokens.css'), 'utf8');
    const value = (name: string) => new RegExp(`--${name}:\\s*([^;]+);`).exec(tokens)?.[1];
    expect(Array.from({ length: 10 }, (_, i) => value(`space-${i + 1}`))).toEqual([
      '2px',
      '4px',
      '6px',
      '8px',
      '10px',
      '12px',
      '16px',
      '20px',
      '24px',
      '32px',
    ]);
    expect(['sm', 'md', 'lg', 'xl', 'pill', 'round'].map((r) => value(`radius-${r}`))).toEqual([
      '4px',
      '6px',
      '8px',
      '12px',
      '999px',
      '50%',
    ]);
  });

  it('finds a literal where a token belongs (the check itself)', () => {
    expect(offending('.a { padding: 6px 0; } .b { border-radius: 3px; } .c { gap: 1em; margin: 0 64px; }')).toEqual([
      'padding: 6px 0',
      'border-radius: 3px',
    ]);
    expect(offending('.a { padding: var(--space-3) 0; --x-gap: 4px; }')).toEqual([]);
  });

  for (const file of files(src).filter((f) => /\.(css|svelte)$/.test(f) && !tokenData(f))) {
    it(path.relative(src, file), () => {
      // A custom property's definition is token data, not a declaration of spacing
      const text = styles(file)
        .replace(/\/\*[\s\S]*?\*\//g, '')
        .replace(/--[\w-]+\s*:[^;{}]*;/g, '');
      expect(offending(text)).toEqual([]);
    });
  }
});
