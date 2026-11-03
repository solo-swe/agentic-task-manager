// @vitest-environment node
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { parseChangelog } from '../../src/lib/changelog';
import { dashboardVersion, releaseNotes } from '../../src/lib/release-notes';

const read = (name: string) => readFileSync(new URL(`../../../plugin/${name}`, import.meta.url), 'utf8');

describe('releaseNotes', () => {
  it("is plugin/WHATS-NEW.md (the users' notes, not the changelog), parsed at build time", () => {
    expect(releaseNotes).toEqual(parseChangelog(read('WHATS-NEW.md')));
    expect(releaseNotes.length).toBeGreaterThan(5);
  });

  it('has a short entry for every release in the changelog, with the same date', () => {
    // Released ones: an Unreleased entry (work for the next release) is in both files until the release dates it
    const changelog = parseChangelog(read('CHANGELOG.md')).filter((e) => e.date);
    const released = releaseNotes.filter((e) => e.date);
    expect(released.map((e) => [e.version, e.date])).toEqual(changelog.map((e) => [e.version, e.date]));
    for (const entry of releaseNotes) {
      const items = entry.sections.flatMap((s) => s.items);
      expect(items.length, entry.version).toBeGreaterThan(0);
      expect(items.length, entry.version).toBeLessThanOrEqual(6);
      for (const item of items) expect(item.length, `${entry.version}: ${item}`).toBeLessThanOrEqual(180);
    }
  });
});

describe('dashboardVersion', () => {
  const docWithMeta = (content?: string) =>
    ({ querySelector: () => (content === undefined ? null : { content }) }) as unknown as Document;

  it('reads the version meta tag', () => {
    expect(dashboardVersion(docWithMeta('0.7.0'))).toBe('0.7.0');
  });

  it('is undefined without the tag or with an empty one', () => {
    expect(dashboardVersion(docWithMeta())).toBeUndefined();
    expect(dashboardVersion(docWithMeta(''))).toBeUndefined();
  });
});
