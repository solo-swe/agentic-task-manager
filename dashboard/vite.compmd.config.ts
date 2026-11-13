import { readdirSync, rmSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { compmd } from '@compmd/vite';
import { defineConfig, type Plugin } from 'vite';
import { compmdVersion } from './build/compmd.ts';

// The CompMD the dashboard loads (docs/compmd-rendering-design.md §2, §3c; ADR-006), in dist/assets/compmd/<version>/
// (never committed: atm and the app embed it, T-477), one build for both hosts:
// - compmd.js, compmd/entry.ts built, with the processor's WebAssembly as its own file beside it. An app build, not
//   library mode: library mode would inline the WebAssembly as a data: URL, which the page's CSP (connect-src 'self')
//   rightly refuses. atm serve serves them at /assets/compmd/<version>/, the app on its atm: scheme at the same path;
// - compmd/, CompMD 0.7.0's bundle (`out`): manifest.json, the sandbox's frames under their hash (frames/<hash>/) and
//   the packs compmd.config.json lists (the Mermaid pack, ambient) by their place (packs/<name>@<version>/). Built for
//   the scheme CompMD's Tauri plugin serves it on (`source`: WebKit reads 'self' in a sandboxed frame on a custom scheme
//   as nothing, so the frames' policies name it; on http it is harmless), so the app's build.rs embeds this same
//   bundle. Both hosts serve it with compmd-host's `respond`: atm serve at /assets/compmd/<version>/compmd/, the app's
//   plugin on compmd:. `virtual:compmd` then names no origin: the app's plugin gives it, the page gives atm serve's.

const assets = fileURLToPath(new URL('./dist/assets/compmd/', import.meta.url));
const version = compmdVersion();

/** Only this version's folder, fresh: an update removes the last one's files, a rebuild this one's (the bundle is
 * written into it before Vite writes the rest, so Vite can't empty it). Not when building elsewhere, as the freshness
 * check does */
function freshVersionFolder(): Plugin {
  return {
    name: 'atm-compmd-version-folder',
    apply: 'build',
    configResolved(config) {
      if (!config.build.outDir.startsWith(assets)) return;
      let entries: string[];
      try {
        entries = readdirSync(assets);
      } catch {
        return;
      }
      for (const entry of entries) rmSync(assets + entry, { recursive: true, force: true });
    },
  };
}

export default defineConfig({
  root: fileURLToPath(new URL('.', import.meta.url)),
  publicDir: false,
  base: `/assets/compmd/${version}/`,
  plugins: [
    freshVersionFolder(),
    compmd({ out: `dist/assets/compmd/${version}/compmd`, source: 'compmd://localhost/' }),
  ],
  build: {
    outDir: assets + version,
    emptyOutDir: false,
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
