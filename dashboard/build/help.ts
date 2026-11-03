import { readFileSync } from 'node:fs';
import type { Plugin } from 'vite';

const ID = 'virtual:atm-help';
const RESOLVED_ID = '\0' + ID;

// Exposes docs/help.md, as its text, as `import help from 'virtual:atm-help'`. Help ends up inlined in the single-file
// build, so it matches the dashboard's version; CompMD renders it when Help opens (HelpDialog.svelte).
export function helpModule(helpPath: string): Plugin {
  return {
    name: 'atm-help',
    resolveId: (id) => (id === ID ? RESOLVED_ID : undefined),
    load(id) {
      if (id !== RESOLVED_ID) return;
      this.addWatchFile(helpPath);
      return `export default ${JSON.stringify(readFileSync(helpPath, 'utf8'))};\n`;
    },
  };
}
