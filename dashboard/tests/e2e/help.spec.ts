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

  test('the diagram example shows its source, then the diagram drawn from it', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard');
    await page.locator('#footer-help').click();
    await expect(page.locator('#help-panel pre', { hasText: 'flowchart LR' })).toHaveCount(1);
    const frame = page.locator('#help-panel iframe');
    await expect(frame).toHaveCount(1);
    await expect(frame).toHaveAttribute('sandbox', 'allow-scripts');
    const svg = page.frameLocator('#help-panel iframe').locator('svg').first();
    await expect(svg).toBeVisible({ timeout: 15_000 });
    await expect(svg).toContainText('In progress');
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

  test('links off the page go to the repository, CompMD and Mermaid, in a new tab, and are never fetched', async ({
    page,
    tasksServer,
  }) => {
    await openDashboard(page, tasksServer, 'standard');
    await page.locator('#footer-help').click();
    await expect(page.locator('#help-panel .compmd')).toHaveAttribute('data-compmd', 'ready');
    const hrefs = await page
      .locator('#help-panel a[target="_blank"]')
      .evaluateAll((links) => links.map((a) => a.getAttribute('href')));
    expect(hrefs).toEqual([
      'https://compmd.hakanserce.com/',
      'https://mermaid.js.org/',
      expect.stringMatching(/^https:\/\/github\.com\/solo-swe\/agentic-task-manager/),
    ]);
  });
});
