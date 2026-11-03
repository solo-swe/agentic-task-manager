// @vitest-environment node
import { describe, expect, it } from 'vitest';
import { markdownPlainText } from '../../src/lib/plain-text';

describe('markdownPlainText: the text without markup, for tooltips and excerpts (T-219)', () => {
  it('takes inline marks away', () => {
    expect(markdownPlainText('Use `atm show` for **all** of it, *now* or _later_')).toBe(
      'Use atm show for all of it, now or later',
    );
    expect(markdownPlainText('See [the docs](https://x.dev/a) and ![a chart](data:image/png;base64,x)')).toBe(
      'See the docs and a chart',
    );
    expect(markdownPlainText('~~old~~ and __new__')).toBe('old and new');
  });

  it('keeps snake_case, lone markers and HTML as written', () => {
    expect(markdownPlainText('review_requested_at and in_review')).toBe('review_requested_at and in_review');
    expect(markdownPlainText('a * b, `open, **half')).toBe('a * b, `open, **half');
    expect(markdownPlainText('<b>bold</b>')).toBe('<b>bold</b>');
  });

  it('a line per line of text: without headings, list and quote marks, fences or blank lines', () => {
    expect(markdownPlainText('## Why\nIt *helps*.\n\n- one\n- [x] two\n1. three\n> said\n\n```\nx = 1\n```')).toBe(
      'Why\nIt helps.\none\ntwo\nthree\nsaid\nx = 1',
    );
    expect(markdownPlainText('a\r\nb')).toBe('a\nb');
    expect(markdownPlainText('')).toBe('');
  });
});
