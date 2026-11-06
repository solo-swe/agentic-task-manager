import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';
import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';
import { changelogModule } from './build/changelog.ts';
import { compmdModule } from './build/compmd.ts';
import { devApi } from './build/dev-api.ts';
import { PAGES } from './build/finish-page.ts';
import { helpModule } from './build/help.ts';
import { pluginInfoModule } from './build/plugin-info.ts';
import { securityHeaders } from './build/security-headers.ts';
import { sizeReport } from './build/size-report.ts';

const pluginJson = fileURLToPath(new URL('../plugin/.claude-plugin/plugin.json', import.meta.url));
const changelog = fileURLToPath(new URL('../plugin/CHANGELOG.md', import.meta.url));
// What's new shows the short, end-user notes; the changelog (for developers) only dates the copyright
const whatsNew = fileURLToPath(new URL('../plugin/WHATS-NEW.md', import.meta.url));
const help = fileURLToPath(new URL('../docs/help.md', import.meta.url));
const ownTasks = fileURLToPath(new URL('../.agentic-task-manager', import.meta.url));

export default defineConfig({
  plugins: [
    // SvelteKit (T-428), its options here (SvelteKit 3 has no svelte.config.js): a single-page app from the static
    // adapter's fallback page, with every script and style inlined, so the build is one self-contained file
    sveltekit({
      preprocess: vitePreprocess(),
      // Dev server: reload the whole page on component changes. In-place component swaps can miss changes to the
      // shared dashboard state; a reload restores everything from storage anyway.
      compilerOptions: { hmr: false },
      // Its fallback page becomes plugin/templates/tasks-dashboard.html (build/finish-page.ts, after the build)
      adapter: adapter({ pages: PAGES, assets: PAGES, fallback: 'index.html', strict: false }),
      output: { bundleStrategy: 'inline' },
      // The dashboard keeps its own state in location.hash (T-429 moves the views to routes)
      router: { type: 'pathname' },
    }),
    changelogModule(whatsNew),
    compmdModule(),
    helpModule(help),
    pluginInfoModule(pluginJson, changelog),
    devApi(ownTasks),
    securityHeaders(),
    sizeReport(),
  ],
});
