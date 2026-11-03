import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import type { Plugin } from 'vite';

const ID = 'virtual:atm-help';
const RESOLVED_ID = '\0' + ID;
const MARKDOWN = fileURLToPath(new URL('../src/lib/markdown.ts', import.meta.url));

// Exposes docs/help.md, parsed (src/lib/markdown.ts), as `import help from 'virtual:atm-help'`. Help ends up inlined in
// the single-file build, so it works offline and matches the dashboard's version. It is embedded as its text and
// parsed when the page loads: the parsed tree is two and a half times the text, and the parser ships anyway (T-342).
export function helpModule(helpPath: string): Plugin {
  return {
    name: 'atm-help',
    resolveId: (id) => (id === ID ? RESOLVED_ID : undefined),
    load(id) {
      if (id !== RESOLVED_ID) return;
      this.addWatchFile(helpPath);
      const text = JSON.stringify(readFileSync(helpPath, 'utf8'));
      return `import { parseMarkdown } from ${JSON.stringify(MARKDOWN)};\nexport default parseMarkdown(${text});\n`;
    },
  };
}
