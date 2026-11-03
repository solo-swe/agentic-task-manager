// The page's security headers (T-226, docs/security-review.md F11). tests/e2e/serve.mjs sends what `atm serve` sends
// (tests/scripts/serve.test.mjs checks that side), and the cspGuard fixture fails any test the policy would break.
import { expect, openDashboard, test } from './helpers';

test('the dashboard is served with a strict Content-Security-Policy, nosniff and same-origin', async ({
  page,
  tasksServer,
}) => {
  const headers = (await page.request.get('tasks-dashboard.html')).headers();
  const csp = headers['content-security-policy'];
  expect(csp).toContain("default-src 'none'");
  // The page's own scripts by hash, and the CompMD runtime from its asset folder only, which may compile WebAssembly
  // (docs/compmd-rendering-design.md §2): no 'self', no eval, no inline scripts
  expect(csp).toMatch(
    /script-src 'sha256-[A-Za-z0-9+/=]+' 'sha256-[A-Za-z0-9+/=]+' 'wasm-unsafe-eval' http:\/\/localhost:\d+\/assets\/compmd\/(;|$)/,
  );
  // Its only frames: CompMD's sandbox frames, from the same folder (T-285)
  expect(csp).toMatch(/frame-src http:\/\/localhost:\d+\/assets\/compmd\/(;|$)/);
  expect(csp).toContain("connect-src 'self'");
  expect(csp).toContain("frame-ancestors 'none'");
  expect(csp).not.toContain("'unsafe-eval'");
  expect(csp).not.toMatch(/script-src[^;]*('unsafe-inline'|'self')/);
  expect(headers['x-content-type-options']).toBe('nosniff');
  expect(headers['cross-origin-resource-policy']).toBe('same-origin');

  // And the dashboard works under it: it loads its data and renders (the cspGuard checks nothing was refused)
  await openDashboard(page, tasksServer, 'standard', { tab: 'list' });
  await expect(page.locator('.task-card').first()).toBeVisible();
});
