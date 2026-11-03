import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import type { Plugin } from 'vite';

const ID = 'virtual:atm-changelog';
const RESOLVED_ID = '\0' + ID;
const CHANGELOG = fileURLToPath(new URL('../src/lib/changelog.ts', import.meta.url));

// Exposes the release notes (plugin/WHATS-NEW.md), parsed, as `import changelog from 'virtual:atm-changelog'`. They
// end up inlined in the single-file build: no runtime request, no Markdown library. Embedded as text and parsed when
// the page loads: smaller than the parsed entries, and the parser is a few hundred bytes (T-342).
export function changelogModule(changelogPath: string): Plugin {
  return {
    name: 'atm-changelog',
    resolveId: (id) => (id === ID ? RESOLVED_ID : undefined),
    load(id) {
      if (id !== RESOLVED_ID) return;
      this.addWatchFile(changelogPath);
      const text = JSON.stringify(readFileSync(changelogPath, 'utf8'));
      return `import { parseChangelog } from ${JSON.stringify(CHANGELOG)};\nexport default parseChangelog(${text});\n`;
    },
  };
}
