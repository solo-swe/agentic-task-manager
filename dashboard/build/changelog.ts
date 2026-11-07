import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import type { Plugin } from 'vite';

const ID = 'virtual:atm-changelog';
const RESOLVED_ID = '\0' + ID;
const CHANGELOG = fileURLToPath(new URL('../src/lib/changelog.ts', import.meta.url));

/** How many releases What's new shows: the newest; the rest are in the changelog, which What's new links to */
export const RECENT_RELEASES = 10;

/** The file up to its `count`-th release heading (`## [`), so only the newest releases are embedded */
export function recentReleases(text: string, count = RECENT_RELEASES): string {
  const headings = [...text.matchAll(/^## \[/gm)].map((m) => m.index);
  return headings.length > count ? text.slice(0, headings[count]) : text;
}

// Exposes the release notes (WHATS-NEW.md), parsed, as `import changelog from 'virtual:atm-changelog'`: the
// newest RECENT_RELEASES releases, so What's new stays short and the file doesn't grow with every release. They end up
// inlined in the single-file build: no runtime request. Embedded as text and parsed when the page loads: smaller than
// the parsed entries, and the parser is a few hundred bytes (T-342).
export function changelogModule(changelogPath: string): Plugin {
  return {
    name: 'atm-changelog',
    resolveId: (id) => (id === ID ? RESOLVED_ID : undefined),
    load(id) {
      if (id !== RESOLVED_ID) return;
      this.addWatchFile(changelogPath);
      const text = JSON.stringify(recentReleases(readFileSync(changelogPath, 'utf8')));
      return `import { parseChangelog } from ${JSON.stringify(CHANGELOG)};\nexport default parseChangelog(${text});\n`;
    },
  };
}
