// @vitest-environment node
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { RECENT_RELEASES, recentReleases } from '../../build/changelog';
import { parseChangelog } from '../../src/lib/changelog';
import { dashboardVersion, releaseNotes } from '../../src/lib/release-notes';

const read = (name: string) => readFileSync(new URL(`../../../${name}`, import.meta.url), 'utf8');

describe('releaseNotes', () => {
  it("is WHATS-NEW.md's newest releases (the users' notes, not the changelog), parsed at build time", () => {
    expect(releaseNotes).toEqual(parseChangelog(read('WHATS-NEW.md')).slice(0, RECENT_RELEASES));
    expect(releaseNotes.length).toBe(RECENT_RELEASES);
  });

  it('recentReleases keeps the file up to the given release heading', () => {
    const text = '# What’s new\n\n## [3] - x\n- c\n\n## [2] - x\n- b\n\n## [1] - x\n- a\n';
    expect(parseChangelog(recentReleases(text, 2)).map((e) => e.version)).toEqual(['3', '2']);
    expect(recentReleases(text, 5)).toBe(text);
  });

  it('the file has a short entry for every release in the changelog, with the same date', () => {
    // Released ones: an Unreleased entry (work for the next release) is in both files until the release dates it.
    // The file keeps every release; the dashboard embeds the newest (above)
    const all = parseChangelog(read('WHATS-NEW.md'));
    const changelog = parseChangelog(read('CHANGELOG.md')).filter((e) => e.date);
    const released = all.filter((e) => e.date);
    expect(released.map((e) => [e.version, e.date])).toEqual(changelog.map((e) => [e.version, e.date]));
    for (const entry of all) {
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
