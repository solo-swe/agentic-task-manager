import js from '@eslint/js';
import svelte from 'eslint-plugin-svelte';
import globals from 'globals';
import ts from 'typescript-eslint';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

// The Svelte options eslint needs (SvelteKit 3 keeps them in vite.config.ts, T-428): the preprocessing
const svelteConfig = { preprocess: vitePreprocess() };

// The task manager's own modules: its state, its task model, its areas and its document source. The document viewer
// and the review tool are components of their own (docs/code-review-design.md §3.1, ATM-571), which never import them
const TASK_AREAS = ['shell', 'kanban', 'tasks', 'graph', 'details', 'dialogs', 'progress', 'agents', 'overview'];
// (lib/state.svelte, lib/types and lib/project-documents, reached from lib/<component>/ as ../<module>; a component's
// own modules, ./types, are its own)
const TASK_MANAGER = [
  ...['state.svelte', 'types', 'project-documents'].flatMap((module) => [`**/lib/${module}`, `../${module}`]),
  ...TASK_AREAS.flatMap((area) => [`**/lib/${area}/**`, `../${area}/**`]),
];
const boundary = (who) => ({
  'no-restricted-imports': [
    'error',
    {
      patterns: [
        {
          group: TASK_MANAGER,
          message: `${who} is a component of its own (docs/code-review-design.md §3.1): it takes what it needs as props, never the task manager's state, task model or areas.`,
        },
      ],
    },
  ],
});

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
    // The document viewer: itself, lib/ui and the shared modules (CompMD, documents, scroll, prefs), never the task
    // manager (ATM-571)
    files: ['src/lib/documents/**'],
    rules: boundary('The document viewer'),
  },
  {
    // The review tool (when it comes): the task areas only through lib/reviews/links
    files: ['src/lib/reviews/**'],
    ignores: ['src/lib/reviews/links/**'],
    rules: boundary('The review tool'),
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
