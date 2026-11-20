import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { defineConfig, devices } from '@playwright/test';

// One port per checkout, from its folder: agents testing in parallel git worktrees each get their own e2e server,
// instead of reusing another worktree's (reuseExistingServer) and testing its build
const checkout = fileURLToPath(new URL('.', import.meta.url));
const PORT = 4173 + (parseInt(createHash('sha1').update(checkout).digest('hex').slice(0, 6), 16) % 800);

export default defineConfig({
  testDir: 'tests/e2e',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? [['list'], ['html', { open: 'never' }]] : 'list',
  snapshotPathTemplate: '{testDir}/__screenshots__/{testFilePath}/{arg}-{platform}{ext}',
  // Strict on purpose: any visual change should be deliberate (regenerate baselines). Loosen per test, if ever.
  // The default per-pixel threshold (0.2) misses e.g. a green -> cyan text color change.
  // A few dozen pixels may differ (T-345): anti-aliasing moves a little from run to run; a real change (a colour, a
  // moved element) differs in many more
  expect: { toHaveScreenshot: { threshold: 0.05, maxDiffPixels: 50 } },
  use: {
    ...devices['Desktop Chrome'],
    viewport: { width: 1280, height: 900 },
    // The dashboard follows the OS color scheme; tests run in dark (the original look) unless they switch it
    colorScheme: 'dark',
    // The graph animates its fits (T-341) unless the OS asks for reduced motion: tests read the view at once
    // (graph-zoom.spec.ts turns motion on to test the animation)
    reducedMotion: 'reduce',
    // Dates are rendered with toLocale*String: pin them so text and screenshots are stable
    locale: 'en-US',
    timezoneId: 'UTC',
    // Only when CI retries a failed test: recording one for every test made the suite ~40% slower (T-345). Locally,
    // `pnpm exec playwright test <spec> --trace on` records them while looking into a failure
    trace: 'on-first-retry',
  },
  // The dashboard at its pages' paths, as atm serve serves it (T-429), in four projects, one after the other (ATM-641):
  // the tests that measure time against a budget (tagged @timed) must not share the machine with heavy ones, which made
  // them miss it (the long review's settle, 104 to 110 ms for 100, beside a review of 5,000 lines and a WebKit page
  // busy for seconds; alone, 26 to 61).
  // - `svelte`: every other test;
  // - `webkit`: the tests tagged @webkit again in WebKit, the app's engine on macOS (ATM-655: the pane jumped there
  //   and not in Chromium), once `svelte` is done;
  // - `timed`: the @timed tests, once `webkit` is done (no WebKit page beside them), one at a time (ATM-654: beside each other they missed their
  //   budgets too, the long review's first draw 443 ms for 200; alone, 50 to 100);
  // - `measure`: a real review measured in Chromium and WebKit (real-review.spec.ts), last; it runs only when given a
  //   review (a developer's real-review.local.json, or the environment).
  // A project runs only once the one before it passed: with a functional test failing, the timed ones wait for its fix.
  // One project alone: `pnpm exec playwright test --project timed --no-deps`
  projects: [
    {
      name: 'svelte',
      testIgnore: /real-review\.spec\.ts/,
      grepInvert: /@timed/,
      use: { baseURL: `http://localhost:${PORT}/` },
    },
    {
      name: 'webkit',
      testIgnore: /real-review\.spec\.ts/,
      grep: /@webkit/,
      grepInvert: /@timed/,
      dependencies: ['svelte'],
      use: {
        ...devices['Desktop Safari'],
        viewport: { width: 1280, height: 900 },
        baseURL: `http://localhost:${PORT}/`,
      },
    },
    {
      name: 'timed',
      testIgnore: /real-review\.spec\.ts/,
      grep: /@timed/,
      dependencies: ['webkit'],
      workers: 1,
      use: { baseURL: `http://localhost:${PORT}/` },
    },
    { name: 'measure', testMatch: /real-review\.spec\.ts/, dependencies: ['timed'] },
  ],
  webServer: {
    command: 'node tests/e2e/serve.mjs',
    url: `http://localhost:${PORT}/`,
    env: { PORT: String(PORT) },
    reuseExistingServer: !process.env.CI,
  },
});
