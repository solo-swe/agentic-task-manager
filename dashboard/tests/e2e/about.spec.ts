import { expect, openDashboard, test } from './helpers';

const overlay = '#about-overlay';

test.describe('About', () => {
  test('opens from the footer, closes with Escape and gives focus back', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard');
    await page.locator('#footer-about').click();
    await expect(page.locator(overlay)).toBeVisible();
    await expect(page.locator('#about-panel')).toContainText('Hakan Serce');
    await expect(page).toHaveURL(/\?(.*&)?about(&|$)/);
    await page.keyboard.press('Escape');
    await expect(page.locator(overlay)).toBeHidden();
    await expect(page.locator('#footer-about')).toBeFocused();
    expect(new URL(page.url()).search).not.toContain('about');
  });

  test('#about in the URL opens it, together with a view', async ({ page, tasksServer }) => {
    tasksServer.use('standard');
    await page.goto('/?view=kanban&about');
    await expect(page.locator(overlay)).toBeVisible();
    await expect(page.locator('#tab-kanban')).toHaveAttribute('aria-selected', 'true');
  });

  test('a tab switch keeps it open', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard');
    await page.locator('#footer-about').click();
    await page.locator('#tab-list').evaluate((tab: HTMLElement) => tab.click());
    await expect(page.locator('#tab-list')).toHaveAttribute('aria-selected', 'true');
    await expect(page.locator(overlay)).toBeVisible();
  });

  test('Back after closing it does not reopen it', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard');
    await page.locator('#footer-about').click();
    await page.locator('#about-panel .detail-close').click();
    await page.goBack();
    await expect(page.locator(overlay)).toBeHidden();
  });
});
