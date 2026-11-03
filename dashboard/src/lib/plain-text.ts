// The text of a Markdown string without its markup, for the places that can't show markup: tooltips, and a card's
// excerpt of its description. Not a renderer: CompMD renders all the Markdown the dashboard shows (CompMd.svelte);
// this only takes the common marks away, line by line.

const BLOCK_MARK = /^\s{0,3}(?:#{1,6}\s+|>\s?|[-*+]\s+(?:\[[ xX]\]\s+)?|\d+[.)]\s+)/;
const FENCE = /^\s*(```|~~~)/;

function inline(line: string): string {
  return line
    .replace(/!?\[([^\]]*)\]\([^)]*\)/g, '$1') // links and images: their text
    .replace(/`([^`]+)`/g, '$1')
    .replace(/(\*\*|__)(\S(?:.*?\S)?)\1/g, '$2')
    .replace(/~~(\S(?:.*?\S)?)~~/g, '$1')
    .replace(/\*(\S(?:.*?\S)?)\*/g, '$1')
    .replace(/(^|\W)_(\S(?:.*?\S)?)_(?=\W|$)/g, '$1$2'); // _em_ only at a word boundary: snake_case stays
}

/** "## Why\nIt *helps*." → "Why\nIt helps.": a line per line of text, without fences, block marks or inline marks */
export function markdownPlainText(src: string): string {
  return src
    .replace(/\r\n?/g, '\n')
    .split('\n')
    .filter((line) => !FENCE.test(line))
    .map((line) => inline(line.replace(BLOCK_MARK, '')).trim())
    .filter(Boolean)
    .join('\n');
}
