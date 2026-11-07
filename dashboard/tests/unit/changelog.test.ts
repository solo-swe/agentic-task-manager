// @vitest-environment node
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { CHANGELOG, readVersion } from '../../build/workspace';
import { compareVersions, entriesBetween, entryMarkdown, parseChangelog, UNRELEASED } from '../../src/lib/changelog';

const real = readFileSync(CHANGELOG, 'utf8');
const pluginVersion = readVersion();

describe('parseChangelog (CHANGELOG.md)', () => {
  const entries = parseChangelog(real);

  it('has an entry for every release, newest first', () => {
    const released = entries.filter((e) => e.version !== UNRELEASED).map((e) => e.version);
    expect(released.slice(-6)).toEqual(['0.6.0', '0.5.0', '0.4.0', '0.3.0', '0.2.0', '0.1.0']);
    expect([...released].sort((a, b) => compareVersions(b, a))).toEqual(released);
  });

  it('has an entry for the current plugin version', () => {
    expect(entries.map((e) => e.version)).toContain(pluginVersion);
  });

  it('gives every release a date and at least one item', () => {
    for (const e of entries.filter((x) => x.version !== UNRELEASED)) {
      expect(e.date, e.version).toMatch(/^\d{4}-\d{2}-\d{2}$/);
      expect(e.sections.flatMap((s) => s.items).length, e.version).toBeGreaterThan(0);
    }
  });

  it('joins wrapped bullet lines', () => {
    const v060 = entries.find((e) => e.version === '0.6.0')!;
    expect(v060.sections[0].items[0]).toMatch(/^Dependency graph zoom and pan: .* when the graph doesn't fit\.$/);
  });
});

describe('parseChangelog (edge cases)', () => {
  it('returns nothing for an empty file or one without versions', () => {
    expect(parseChangelog('')).toEqual([]);
    expect(parseChangelog('# Changelog\n\nSome intro\n- not an entry')).toEqual([]);
  });

  it('handles a missing date, unknown section names and bullets without a section', () => {
    const md = ['## [Unreleased]', '- loose item', '### Security', '* patched', '', '## [1.0.0] - 2026-01-02'].join(
      '\n',
    );
    expect(parseChangelog(md)).toEqual([
      {
        version: 'Unreleased',
        sections: [
          { title: '', items: ['loose item'] },
          { title: 'Security', items: ['patched'] },
        ],
      },
      { version: '1.0.0', date: '2026-01-02', sections: [] },
    ]);
  });

  it('keeps inline Markdown as text and does not join across blank lines', () => {
    const md = ['## [1.0.0] - 2026-01-02', '### Added', '- `code` and [link](x)', '', '  not a continuation'].join(
      '\n',
    );
    expect(parseChangelog(md)[0].sections[0].items).toEqual(['`code` and [link](x)']);
  });
});

describe('entryMarkdown: a release’s notes, for CompMD', () => {
  it('a list per section, a heading for each titled one; no version heading', () => {
    const [entry] = parseChangelog(
      ['## [1.0.0] - 2026-01-02', '- loose `item`', '### Fixed', '- one', '- two', '  wrapped'].join('\n'),
    );
    expect(entryMarkdown(entry)).toBe('- loose `item`\n\n#### Fixed\n\n- one\n- two wrapped');
  });
});

describe('compareVersions', () => {
  it('compares numerically, not as strings', () => {
    expect(compareVersions('0.10.0', '0.9.0')).toBeGreaterThan(0);
    expect(compareVersions('1.0.0', '1.0.0')).toBe(0);
    expect(compareVersions('1.0', '1.0.1')).toBeLessThan(0);
  });
});

describe('entriesBetween', () => {
  const entries = parseChangelog(real);
  const versions = (from: string | undefined, to: string) => entriesBetween(entries, from, to).map((e) => e.version);

  it('selects (from, to] newest first, without Unreleased', () => {
    expect(versions('0.4.0', '0.6.0')).toEqual(['0.6.0', '0.5.0']);
  });

  it('treats a missing old version as 0.0.0', () => {
    expect(versions(undefined, '0.2.0')).toEqual(['0.2.0', '0.1.0']);
  });

  it('is empty when already up to date', () => {
    expect(versions('0.6.0', '0.6.0')).toEqual([]);
  });
});
