import { readdirSync, rmSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { defineConfig, type Plugin } from 'vite';
import { compmdVersion } from './build/compmd.ts';

// The CompMD runtime the dashboard loads (docs/compmd-rendering-design.md §2): compmd/entry.ts built into
// plugin/assets/compmd/<version>/ as compmd.js, with the processor's WebAssembly as its own file beside it (the glue
// finds it with `new URL(…, import.meta.url)`). An app build, not library mode: library mode would inline the
// WebAssembly as a data: URL, which the page's CSP (connect-src 'self') rightly refuses. Committed, like
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
  publicDir: false,
  // Files find each other relative to the module, wherever the folder is served
  base: './',
  plugins: [removeOtherVersions()],
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
