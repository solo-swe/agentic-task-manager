import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';
import { svelte, vitePreprocess } from '@sveltejs/vite-plugin-svelte';
import { svelteTesting } from '@testing-library/svelte/vite';
import { changelogModule } from './build/changelog.ts';
import { compmd } from '@compmd/vite';
import { compmdModule } from './build/compmd.ts';
import { GENERATED } from './build/generated.ts';
import { helpModule } from './build/help.ts';
import { pluginInfoModule } from './build/plugin-info.ts';
import { CARGO_TOML, CHANGELOG, WHATS_NEW } from './build/workspace.ts';

const help = fileURLToPath(new URL('../docs/help.md', import.meta.url));

export default defineConfig({
  plugins: [
    // The components without SvelteKit (its options are vite.config.ts's, T-428): the same preprocessing
    svelte({ configFile: false, preprocess: vitePreprocess() }),
    svelteTesting(),
    changelogModule(WHATS_NEW),
    compmdModule(),
    // `virtual:compmd`, as the served runtime has it (compmd.config.json): tests process texts with the same packs
    compmd(),
    helpModule(help),
    pluginInfoModule(CARGO_TOML, CHANGELOG),
  ],
  // vite.config.ts's SvelteKit alias, without SvelteKit
  resolve: { alias: { $generated: GENERATED } },
  test: {
    environment: 'jsdom',
    include: ['tests/unit/**/*.test.ts'],
    setupFiles: ['tests/unit/setup.ts'],
    coverage: {
      provider: 'v8',
      include: ['src/lib/**'],
      // The 90% is for the logic; the components (in src/lib/<area>/ since T-430) are covered by their component
      // tests and the e2e suite
      exclude: ['src/lib/**/*.svelte'],
      thresholds: { lines: 90 },
    },
  },
});
