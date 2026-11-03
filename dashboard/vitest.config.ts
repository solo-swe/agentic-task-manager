import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { svelteTesting } from '@testing-library/svelte/vite';
import { changelogModule } from './build/changelog.ts';
import { compmdModule } from './build/compmd.ts';
import { helpModule } from './build/help.ts';
import { pluginInfoModule } from './build/plugin-info.ts';

const changelog = fileURLToPath(new URL('../plugin/CHANGELOG.md', import.meta.url));
const whatsNew = fileURLToPath(new URL('../plugin/WHATS-NEW.md', import.meta.url));
const help = fileURLToPath(new URL('../docs/help.md', import.meta.url));
const pluginJson = fileURLToPath(new URL('../plugin/.claude-plugin/plugin.json', import.meta.url));

export default defineConfig({
  plugins: [
    svelte(),
    svelteTesting(),
    changelogModule(whatsNew),
    compmdModule(),
    helpModule(help),
    pluginInfoModule(pluginJson, changelog),
  ],
  test: {
    environment: 'jsdom',
    include: ['tests/unit/**/*.test.ts'],
    setupFiles: ['tests/unit/setup.ts'],
    coverage: {
      provider: 'v8',
      include: ['src/lib/**'],
      thresholds: { lines: 90 },
    },
  },
});
