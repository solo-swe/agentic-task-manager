import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import type { Plugin } from 'vite';

// CompMD in the details panel (docs/compmd-rendering-design.md): which version the dashboard loads from atm serve's
// asset folder (§2). Packs, their versions and the sandbox's frames are @compmd/vite's (compmd.config.json, built by
// vite.compmd.config.ts).

const ID = 'virtual:atm-compmd';
const RESOLVED_ID = '\0' + ID;
const PROCESSOR = fileURLToPath(new URL('../node_modules/@compmd/processor/package.json', import.meta.url));

/** The installed CompMD version: the asset folder's name (plugin/assets/compmd/<version>/), so a page left open
 * across an update never loads another version's files */
export function compmdVersion(): string {
  return JSON.parse(readFileSync(PROCESSOR, 'utf8')).version;
}

/** What About says of CompMD, which renders the dashboard's Markdown: from its processor's package.json */
export function compmdAbout(): { license: string; homepage: string } {
  const { license, homepage } = JSON.parse(readFileSync(PROCESSOR, 'utf8'));
  return { license, homepage };
}

/** `import { version, license, homepage } from 'virtual:atm-compmd'`, inlined in the build */
export function compmdModule(): Plugin {
  return {
    name: 'atm-compmd',
    resolveId: (id) => (id === ID ? RESOLVED_ID : undefined),
    load(id) {
      if (id !== RESOLVED_ID) return;
      this.addWatchFile(PROCESSOR);
      const { license, homepage } = compmdAbout();
      return (
        `export const version = ${JSON.stringify(compmdVersion())};\n` +
        `export const license = ${JSON.stringify(license)};\n` +
        `export const homepage = ${JSON.stringify(homepage)};\n`
      );
    },
  };
}
