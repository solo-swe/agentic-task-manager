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
  expect: { toHaveScreenshot: { threshold: 0.05 } },
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
    trace: 'retain-on-failure',
  },
  projects: [{ name: 'svelte', use: { baseURL: `http://localhost:${PORT}/svelte/` } }],
  webServer: {
    command: 'node tests/e2e/serve.mjs',
    url: `http://localhost:${PORT}/svelte/tasks-dashboard.html`,
    env: { PORT: String(PORT) },
    reuseExistingServer: !process.env.CI,
  },
});
