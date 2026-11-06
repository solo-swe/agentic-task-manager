// @vitest-environment node
// T-434 (docs/guides/svelte.md "Accessibility"): what can be clicked is a control. No click handler on a `div` or
// `span` (a `<button>` or `<a>` is reached and pressed by the keyboard), but on a card that is a button by ARIA and
// on the few containers below, each for its reason; and every button that shows only an icon has a name
// (`aria-label`).
import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

const lib = path.resolve(import.meta.dirname, '../../src');
const files = (dir: string): string[] =>
  readdirSync(dir, { withFileTypes: true }).flatMap((e) =>
    e.isDirectory() ? files(path.join(dir, e.name)) : [path.join(dir, e.name)],
  );

/** Clicks handled on a container for what is inside it, not a control of their own */
const ALLOWED = [
  // The details panel's backdrop: a click beside the panel closes it, as Escape and the × do
  'details/DetailPanel.svelte: div.detail-overlay',
  // CompMD's rendered text: its links, caught where they bubble up (they are <a> elements)
  'documents/CompMd.svelte: div.compmd',
  // The document viewer's text: a click anywhere in it only clears the text fragment's mark
  'documents/DocumentViewer.svelte: div.doc-body',
];

interface Tag {
  name: string;
  attrs: string;
  /** The text up to its closing tag (for a button), else '' */
  content: string;
}

/** The opening tags of a component's markup (outside <script> and <style>), read with their `{…}` attributes */
function tags(source: string): Tag[] {
  const markup = source.replace(/<(script|style)\b[\s\S]*?<\/\1>/g, '').replace(/<!--[\s\S]*?-->/g, '');
  const found: Tag[] = [];
  for (let i = markup.indexOf('<'); i >= 0; i = markup.indexOf('<', i + 1)) {
    const name = /^<([a-z][\w-]*)/.exec(markup.slice(i))?.[1];
    if (!name) continue;
    // To the tag's end: a `>` outside quotes and braces (an attribute can hold `() => …`)
    let depth = 0;
    let quote = '';
    let end = i + name.length + 1;
    for (; end < markup.length; end++) {
      const c = markup[end];
      if (quote) quote = c === quote ? '' : quote;
      else if (c === '"' || c === "'") quote = depth ? '' : c;
      else if (c === '{') depth++;
      else if (c === '}') depth--;
      else if (c === '>' && depth === 0) break;
    }
    const attrs = markup.slice(i + name.length + 1, end);
    const close = name === 'button' ? markup.indexOf('</button>', end) : -1;
    found.push({ name, attrs, content: close > 0 ? markup.slice(end + 1, close) : '' });
  }
  return found;
}

const classOf = (attrs: string) => /\bclass="([\w-]+)/.exec(attrs)?.[1] ?? '';

describe('controls are controls (T-434)', () => {
  it('reads the tags, attributes with arrows included (the check itself)', () => {
    const read = tags(
      '<div class="a" onclick={() => x > 1}><button aria-label="Close"><Icon name="x" /></button></div>',
    );
    // HTML elements; a component (`<Icon>`) is its own file's to check
    expect(read.map((t) => t.name)).toEqual(['div', 'button']);
    expect(read[0].attrs).toContain('onclick');
    expect(read[1].content).toBe('<Icon name="x" />');
  });

  for (const file of files(lib).filter((f) => f.endsWith('.svelte'))) {
    const rel = path
      .relative(lib, file)
      .replace(/^lib[\\/]/, '')
      .replaceAll('\\', '/');
    it(rel, () => {
      const all = tags(readFileSync(file, 'utf8'));
      // A card holding controls of its own can't be a <button> (its content would be): it is one by ARIA, a role
      // with the keyboard's focus and keys (the Kanban and Omniboard cards)
      const keyboardControl = (attrs: string) =>
        /\brole="button"/.test(attrs) && /\btabindex="0"/.test(attrs) && /\bonkeydown=/.test(attrs);
      const clicked = all
        .filter((t) => (t.name === 'div' || t.name === 'span') && /\bonclick=/.test(t.attrs))
        .filter((t) => !keyboardControl(t.attrs))
        .map((t) => `${rel}: ${t.name}.${classOf(t.attrs)}`)
        .filter((at) => !ALLOWED.includes(at));
      expect(clicked).toEqual([]);
      // A button whose content is only an icon (or ×) needs a name; one spreading Bits UI's props gets its label from them
      // only if it says so, so it is held to the same rule
      const unnamed = all
        .filter((t) => t.name === 'button' && /^\s*(<Icon\b[^>]*\/>|&times;|×)\s*$/.test(t.content))
        .filter((t) => !/\baria-label=/.test(t.attrs))
        .map((t) => `${rel}: button.${classOf(t.attrs)}`);
      expect(unnamed).toEqual([]);
    });
  }
});
