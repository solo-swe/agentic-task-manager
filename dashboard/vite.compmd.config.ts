import { readdirSync, rmSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { compmd } from '@compmd/vite';
import { defineConfig, type Plugin } from 'vite';
import { compmdVersion } from './build/compmd.ts';

// The CompMD runtime the dashboard loads (docs/compmd-rendering-design.md §2, §3c), in plugin/assets/compmd/<version>/,
// served by atm serve at /assets/compmd/<version>/:
// - compmd.js, compmd/entry.ts built, with the processor's WebAssembly as its own file beside it. An app build, not
//   library mode: library mode would inline the WebAssembly as a data: URL, which the page's CSP (connect-src 'self')
//   rightly refuses;
// - compmd/, @compmd/vite's: the sandbox's frame documents, and the built modules of the packs compmd.config.json
//   lists (the Mermaid pack, ambient), whose versions and hashes it pins.
// `base` is where atm serve serves the folder: @compmd/vite gives the frames' and modules' URLs from it. Committed, like
// tasks-dashboard.html; `npm run check` fails when it is stale (scripts/check-compmd-assets.mjs).

const assets = fileURLToPath(new URL('../plugin/assets/compmd/', import.meta.url));
const version = compmdVersion();

/** Only this version's folder is kept: an update removes the last one's files (not when building elsewhere, as the
 * freshness check does) */
function removeOtherVersions(): Plugin {
  let outDir = '';
  return {
    name: 'atm-compmd-other-versions',
    apply: 'build',
    configResolved(config) {
      outDir = config.build.outDir;
    },
    buildStart() {
      if (!outDir.startsWith(assets)) return;
      let entries: string[];
      try {
        entries = readdirSync(assets);
      } catch {
        return;
      }
      for (const entry of entries) if (entry !== version) rmSync(assets + entry, { recursive: true, force: true });
    },
  };
}

export default defineConfig({
  root: fileURLToPath(new URL('.', import.meta.url)),
  publicDir: false,
  base: `/assets/compmd/${version}/`,
  plugins: [removeOtherVersions(), compmd()],
  build: {
    outDir: assets + version,
    emptyOutDir: true,
    copyPublicDir: false,
    modulePreload: false,
    target: 'es2022',
    // Never inline: the WebAssembly must stay its own file
    assetsInlineLimit: 0,
    reportCompressedSize: false,
    rollupOptions: {
      input: fileURLToPath(new URL('./compmd/entry.ts', import.meta.url)),
      preserveEntrySignatures: 'exports-only',
      output: {
        format: 'es',
        entryFileNames: 'compmd.js',
        chunkFileNames: '[name].js',
        assetFileNames: '[name][extname]',
      },
    },
  },
});
