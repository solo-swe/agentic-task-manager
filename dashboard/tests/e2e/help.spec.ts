import { expect, openDashboard, test, addressOf } from './helpers';

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
    await expect(page).toHaveURL(/\?(.*&)?help(&|$)/);
    await page.keyboard.press('Escape');
    await expect(page.locator(overlay)).toBeHidden();
    await expect(page.locator('#footer-help')).toBeFocused();
  });

  test('the text scrolls under the title without a scrollbar, with hints that count sections and scroll', async ({
    page,
    tasksServer,
  }) => {
    await openDashboard(page, tasksServer, 'standard');
    await page.locator('#footer-help').click();
    await expect(page.locator('#help-panel .compmd')).toHaveAttribute('data-compmd', 'ready');
    const body = page.locator('#help-body');
    const sizes = await body.evaluate((el) => ({
      overflows: el.scrollHeight > el.clientHeight,
      scrollbar: (el as HTMLElement).offsetWidth - el.clientWidth,
      panelScrolls: el.closest('#help-panel')!.scrollHeight > el.closest('#help-panel')!.clientHeight,
    }));
    expect(sizes).toEqual({ overflows: true, scrollbar: 0, panelScrolls: false });

    const panel = page.locator('#help-panel');
    // A hairline under the title; at the top, the title at its full size
    const titleStyle = () =>
      panel.locator('> h3').evaluate((h) => {
        const s = getComputedStyle(h);
        return { line: s.borderBottomStyle + ' ' + s.borderBottomWidth, size: parseFloat(s.fontSize) };
      });
    expect(await titleStyle()).toEqual({ line: 'solid 1px', size: 20 });
    await expect(panel).not.toHaveClass(/scrolled/);
    await expect(panel.locator('.kanban-more.top')).toHaveCount(0);
    await expect(panel.locator('.kanban-more.bottom')).toHaveText(/^▼ \d+ more$/);
    await panel.locator('.kanban-more.bottom').click();
    await expect.poll(() => body.evaluate((el) => el.scrollTop)).toBeGreaterThan(100);
    await expect(panel.locator('.kanban-more.top')).toHaveText(/^▲ \d+ more$/);
    // Scrolled: the title compacts (as the dashboard's title bar does), keeping its hairline, and stays in view
    await expect(panel).toHaveClass(/scrolled/);
    await expect.poll(titleStyle).toEqual({ line: 'solid 1px', size: 16 }); // after its transition
    await expect(panel.locator('> h3')).toBeInViewport();
    // Back at the top, full size again
    await body.evaluate((el) => el.scrollTo({ top: 0 }));
    await expect(panel).not.toHaveClass(/scrolled/);
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

  test('?help in the address opens it; its links within the page keep it open', async ({ page, tasksServer }) => {
    tasksServer.use('standard');
    await page.goto('/projects/fixture/list?help');
    await expect(page.locator(overlay)).toBeVisible();
    await expect(page.locator('#tab-list')).toHaveAttribute('aria-selected', 'true');
    await page.locator('#help-panel').getByRole('link', { name: 'priorities and labels' }).click();
    await expect(page.locator('#help-panel h2', { hasText: 'Priorities and labels' })).toBeInViewport();
    await expect(page.locator(overlay)).toBeVisible();
    // The Sprint filter follows in the address (?sprint=, T-323)
    expect(addressOf(page)).toMatch(/^\/projects\/fixture\/list\?(sprint=[^&]+&)?help(&sprint=[^&]+)?$/);
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
