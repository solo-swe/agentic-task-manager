import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { viteSingleFile } from 'vite-plugin-singlefile';
import { stampBuildId } from './build/build-id.ts';
import { changelogModule } from './build/changelog.ts';
import { devApi } from './build/dev-api.ts';
import { favicon } from './build/favicon.ts';
import { helpModule } from './build/help.ts';
import { injectVersion } from './build/inject-version.ts';
import { pluginInfoModule } from './build/plugin-info.ts';
import { renameHtmlOutput } from './build/rename-output.ts';
import { securityHeaders } from './build/security-headers.ts';

const pluginJson = fileURLToPath(new URL('../plugin/.claude-plugin/plugin.json', import.meta.url));
const changelog = fileURLToPath(new URL('../plugin/CHANGELOG.md', import.meta.url));
// What's new shows the short, end-user notes; the changelog (for developers) only dates the copyright
const whatsNew = fileURLToPath(new URL('../plugin/WHATS-NEW.md', import.meta.url));
const help = fileURLToPath(new URL('../docs/help.md', import.meta.url));
const ownTasks = fileURLToPath(new URL('../.agentic-task-manager', import.meta.url));
const logo = fileURLToPath(new URL('./src/assets/atm-logo.svg', import.meta.url));

export default defineConfig({
  plugins: [
    // Dev server: reload the whole page on component changes. In-place component swaps can miss changes to the
    // shared dashboard state; a reload restores everything from storage anyway.
    svelte({ compilerOptions: { hmr: false } }),
    changelogModule(whatsNew),
    helpModule(help),
    pluginInfoModule(pluginJson, changelog),
    devApi(ownTasks),
    securityHeaders(),
    viteSingleFile(),
    injectVersion(pluginJson),
    favicon(logo),
    // After the page is complete, before it is renamed: plugins run their writeBundle hooks in this order
    stampBuildId(),
    renameHtmlOutput('tasks-dashboard.html'),
  ],
  build: {
    // The build output is what the plugin ships: plugin/templates/tasks-dashboard.html (committed; CI checks it is fresh).
    // emptyOutDir must stay false: templates/ also holds the tasks.json example.
    outDir: '../plugin/templates',
    emptyOutDir: false,
  },
});
