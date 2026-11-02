import js from '@eslint/js';
import svelte from 'eslint-plugin-svelte';
import globals from 'globals';
import ts from 'typescript-eslint';
import svelteConfig from './svelte.config.js';

export default ts.config(
  { ignores: ['dist/', 'coverage/', 'playwright-report/', 'test-results/', 'tests/legacy/'] },
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
    files: ['**/*.svelte', '**/*.svelte.ts'],
    languageOptions: {
      parserOptions: { projectService: true, extraFileExtensions: ['.svelte'], parser: ts.parser, svelteConfig },
    },
  },
);
