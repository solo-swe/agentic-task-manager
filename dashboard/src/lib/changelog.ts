// Parses WHATS-NEW.md and CHANGELOG.md (Keep a Changelog subset) into releases, for picking them by
// version. Items keep their inline Markdown (backticks, links), which CompMD renders (entryMarkdown).

export interface ChangelogSection {
  title: string;
  items: string[];
}

export interface ChangelogEntry {
  /** "0.7.0", or "Unreleased" */
  version: string;
  /** YYYY-MM-DD; absent for Unreleased */
  date?: string;
  sections: ChangelogSection[];
}

export const UNRELEASED = 'Unreleased';

const VERSION_HEADING = /^##\s+\[([^\]]+)\](?:\s+-\s+(\d{4}-\d{2}-\d{2}))?/;
const SECTION_HEADING = /^###\s+(.+?)\s*$/;
const ITEM = /^[-*]\s+(.*)$/;

/** Entries in file order (newest first by convention). */
export function parseChangelog(markdown: string): ChangelogEntry[] {
  const entries: ChangelogEntry[] = [];
  let entry: ChangelogEntry | undefined;
  let section: ChangelogSection | undefined;
  let item: { index: number } | undefined;

  for (const raw of markdown.split(/\r?\n/)) {
    const line = raw.trimEnd();
    const version = VERSION_HEADING.exec(line);
    if (version) {
      entry = { version: version[1].trim(), ...(version[2] ? { date: version[2] } : {}), sections: [] };
      entries.push(entry);
      section = item = undefined;
      continue;
    }
    if (!entry) continue;
    const heading = SECTION_HEADING.exec(line);
    if (heading) {
      section = { title: heading[1], items: [] };
      entry.sections.push(section);
      item = undefined;
      continue;
    }
    const bullet = ITEM.exec(line);
    if (bullet) {
      if (!section) {
        section = { title: '', items: [] };
        entry.sections.push(section);
      }
      section.items.push(bullet[1].trim());
      item = { index: section.items.length - 1 };
      continue;
    }
    if (item && section && /^\s+\S/.test(line)) {
      // Wrapped continuation of the previous bullet
      section.items[item.index] += ' ' + line.trim();
    } else if (!line.trim()) {
      item = undefined;
    }
  }
  return entries;
}

/** A release's notes as Markdown, without its version heading: a `####` heading per titled section, then its items */
export function entryMarkdown(entry: ChangelogEntry): string {
  return entry.sections
    .map((s) => (s.title ? `#### ${s.title}\n\n` : '') + s.items.map((item) => `- ${item}`).join('\n'))
    .join('\n\n');
}

function parts(version: string): number[] {
  return version.split('.').map((p) => parseInt(p, 10) || 0);
}

/** Compares x.y.z versions numerically: negative if a < b, 0 if equal, positive if a > b. */
export function compareVersions(a: string, b: string): number {
  const pa = parts(a);
  const pb = parts(b);
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const diff = (pa[i] ?? 0) - (pb[i] ?? 0);
    if (diff !== 0) return diff;
  }
  return 0;
}

/**
 * Released entries newer than `fromExclusive` and up to `toInclusive`, newest first.
 * A missing `fromExclusive` (e.g. a dashboard without a version tag) counts as 0.0.0.
 */
export function entriesBetween(
  entries: ChangelogEntry[],
  fromExclusive: string | undefined,
  toInclusive: string,
): ChangelogEntry[] {
  const from = fromExclusive || '0.0.0';
  return entries
    .filter((e) => e.version !== UNRELEASED)
    .filter((e) => compareVersions(e.version, from) > 0 && compareVersions(e.version, toInclusive) <= 0)
    .sort((a, b) => compareVersions(b.version, a.version));
}
