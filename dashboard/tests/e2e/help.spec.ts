import { expect, openDashboard, test } from './helpers';

// D-1.3, D-1.5: Help shows docs/help.md, inlined at build time and rendered with CompMD; it opens from the footer or
// #help.
const overlay = '#help-overlay';

test.describe('Help', () => {
  test('opens from the footer, closes with Escape and gives focus back', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard');
    await page.locator('#footer-help').click();
    await expect(page.locator(overlay)).toBeVisible();
    await expect(page.locator('#help-panel .compmd')).toHaveAttribute('data-compmd', 'ready');
    await expect(page.locator('#help-panel h2', { hasText: 'Statuses and kinds' })).toBeVisible();
    await expect(page).toHaveURL(/#.*help/);
    await page.keyboard.press('Escape');
    await expect(page.locator(overlay)).toBeHidden();
    await expect(page.locator('#footer-help')).toBeFocused();
  });

  test('#help in the URL opens it; its links within the page keep it open', async ({ page, tasksServer }) => {
    tasksServer.use('standard');
    await page.goto('tasks-dashboard.html#view=list&help');
    await expect(page.locator(overlay)).toBeVisible();
    await expect(page.locator('#tab-list')).toHaveAttribute('aria-selected', 'true');
    await page.locator('#help-panel').getByRole('link', { name: 'priorities and labels' }).click();
    await expect(page.locator('#help-panel h2', { hasText: 'Priorities and labels' })).toBeInViewport();
    await expect(page.locator(overlay)).toBeVisible();
    // The Sprint filter follows in the hash (#sprint=, T-323)
    expect(new URL(page.url()).hash).toMatch(/^#view=list&help(&sprint=[^&]+)?$/);
  });

  test('the only link off the page goes to the repository, in a new tab, and is never fetched', async ({
    page,
    tasksServer,
  }) => {
    await openDashboard(page, tasksServer, 'standard');
    await page.locator('#footer-help').click();
    await expect(page.locator('#help-panel .compmd')).toHaveAttribute('data-compmd', 'ready');
    const external = page.locator('#help-panel a[target="_blank"]');
    await expect(external).toHaveCount(1);
    await expect(external).toHaveAttribute('href', /^https:\/\/github\.com\/solo-swe\/agentic-task-manager/);
  });
});
