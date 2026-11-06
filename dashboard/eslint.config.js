import js from '@eslint/js';
import svelte from 'eslint-plugin-svelte';
import globals from 'globals';
import ts from 'typescript-eslint';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

// The Svelte options eslint needs (SvelteKit 3 keeps them in vite.config.ts, T-428): the preprocessing
const svelteConfig = { preprocess: vitePreprocess() };

export default ts.config(
  { ignores: ['dist/', 'coverage/', 'playwright-report/', 'test-results/', 'tests/legacy/', '.svelte-kit/'] },
  js.configs.recommended,
  ...ts.configs.recommended,
  ...svelte.configs.recommended,
  {
    languageOptions: { globals: { ...globals.browser, ...globals.node } },
  },
  {
    // Playwright fixtures must be destructured, even when unused: ({}, testInfo) => ...
    files: ['tests/e2e/**'],
    rules: { 'no-empty-pattern': 'off' },
  },
  {
    // The client layer (docs/layered-architecture.md §1): only src/lib/api/ makes requests; everything else takes a
    // Fetch from there
    files: ['src/**'],
    ignores: ['src/lib/api/**'],
    rules: {
      'no-restricted-globals': [
        'error',
        { name: 'fetch', message: 'Requests go through the client layer, src/lib/api/http.ts (browserFetch).' },
        { name: 'XMLHttpRequest', message: 'Requests go through the client layer, src/lib/api/http.ts.' },
        { name: 'WebSocket', message: 'Requests go through the client layer, src/lib/api/http.ts.' },
        { name: 'EventSource', message: 'Requests go through the client layer, src/lib/api/http.ts.' },
      ],
    },
  },
  {
    // The dashboard's links are external (a repository, a PR) or hash links (its views live in the hash until T-429
    // gives them routes): none goes to a SvelteKit route, so none needs resolve() yet
    files: ['**/*.svelte'],
    rules: { 'svelte/no-navigation-without-resolve': ['error', { ignoreLinks: true }] },
  },
  {
    files: ['**/*.svelte', '**/*.svelte.ts'],
    languageOptions: {
      parserOptions: { projectService: true, extraFileExtensions: ['.svelte'], parser: ts.parser, svelteConfig },
    },
  },
);
