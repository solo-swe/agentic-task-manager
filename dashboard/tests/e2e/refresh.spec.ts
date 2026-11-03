import type { Page } from '@playwright/test';
import { expect, filterButton, loadFixture, matchCount, openDashboard, openFilters, openMenu, test } from './helpers';

/** standard.json with T-005 marked done. */
function standardWithT005Done() {
  const data = loadFixture('standard');
  data.tasks.find((t) => t.id === 'T-005')!.status = 'done';
  return data;
}

/** The progress pill in the top bar, named "Progress 42%, 5 of 12 tasks done" */
const progress = (page: Page) => page.locator('#progress-card');

test.describe('reload', () => {
  test('the Reload button re-fetches and re-renders', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard', { tab: 'list' });
    await expect(progress(page)).toHaveAccessibleName('Progress 45%, 5 of 11 tasks done');
    tasksServer.use(standardWithT005Done());
    await page.locator('#reload-btn').click();
    await expect(progress(page)).toHaveAccessibleName('Progress 55%, 6 of 11 tasks done');
    await expect(page.locator('.task-card[data-id="T-005"]')).toHaveClass(/status-done/);
    expect(tasksServer.requests).toBe(2);
  });

  test('active filters survive a reload', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard', { tab: 'list' });
    await openFilters(page);
    await filterButton(page, 'sprint', 'Phase 1').click();
    tasksServer.use(standardWithT005Done());
    await page.locator('#reload-btn').click();
    await expect(page.locator('.task-card[data-id="T-005"]')).toHaveClass(/status-done/);
    await expect(matchCount(page)).toHaveText('4 of 12 tasks');
    await expect(filterButton(page, 'sprint', 'Phase 1')).toHaveClass(/active/);
  });
});

test.describe('auto-refresh', () => {
  const requests = (tasksServer: { requests: number }) => expect.poll(() => tasksServer.requests);
  // The settings are in the ⋯ menu next to the Reload button (T-323)
  const menu = (page: Page) => openMenu(page, '#settings-btn');
  const toggle = (page: Page) => page.locator('.auto-refresh-toggle');
  const check = (page: Page) => page.locator('#auto-refresh-check');
  const interval = (page: Page) => page.locator('#auto-refresh-interval');
  const state = (page: Page) => page.locator('#reload-btn .refresh-state');

  test.beforeEach(async ({ page, tasksServer }) => {
    await page.clock.install();
    // A fresh browser profile: no stored preferences, so the dashboard's defaults apply
    await openDashboard(page, tasksServer, 'standard', { autoRefresh: true });
    await menu(page);
  });

  test('is on by default and polls every 5 seconds', async ({ page, tasksServer }) => {
    await expect(state(page)).toHaveText('5s');
    await expect(check(page)).toBeChecked();
    await expect(interval(page)).toHaveValue('5');
    await page.clock.runFor(5_000);
    await requests(tasksServer).toBe(2);
    await page.clock.runFor(5_000);
    await requests(tasksServer).toBe(3);
  });

  test('stops polling when switched off', async ({ page, tasksServer }) => {
    await toggle(page).click();
    await expect(check(page)).not.toBeChecked();
    await expect(state(page)).toHaveText(''); // a pause icon instead of the interval
    await page.clock.runFor(30_000);
    await requests(tasksServer).toBe(1);
  });

  test('switching it off is remembered across reloads', async ({ page, tasksServer }) => {
    await toggle(page).click();
    await page.reload();
    await menu(page);
    await expect(check(page)).not.toBeChecked();
    const afterReload = tasksServer.requests;
    await page.clock.runFor(30_000);
    expect(tasksServer.requests).toBe(afterReload);
  });

  test('picks up changes on the next tick', async ({ page, tasksServer }) => {
    tasksServer.use(standardWithT005Done());
    await page.clock.runFor(5_000);
    await expect(progress(page)).toHaveAccessibleName('Progress 55%, 6 of 11 tasks done');
  });

  test('an unchanged file is a 304: nothing is parsed or re-rendered; a change still shows', async ({
    page,
    tasksServer,
  }) => {
    // A marker on a rendered card survives only if the card isn't rendered again
    await page.keyboard.press('Escape'); // the settings menu
    await page.getByRole('tab', { name: 'Task List' }).click();
    await page.locator('.task-card[data-id="T-005"]').evaluate((el) => el.setAttribute('data-marker', '1'));
    await page.clock.runFor(10_000);
    await requests(tasksServer).toBe(3);
    expect(tasksServer.notModified).toBe(2);
    await expect(page.locator('.task-card[data-id="T-005"][data-marker="1"]')).toHaveCount(1);

    tasksServer.use(standardWithT005Done());
    await page.clock.runFor(5_000);
    await expect(progress(page)).toHaveAccessibleName('Progress 55%, 6 of 11 tasks done');
    expect(tasksServer.notModified).toBe(2);
  });

  test('a changed interval applies immediately and is remembered', async ({ page, tasksServer }) => {
    await interval(page).fill('2');
    await interval(page).press('Tab');
    await page.clock.runFor(2_000);
    await requests(tasksServer).toBe(2);
    await page.clock.runFor(2_000);
    await requests(tasksServer).toBe(3);
    await page.reload();
    await expect(state(page)).toHaveText('2s');
    await menu(page);
    await expect(interval(page)).toHaveValue('2');
    await expect(check(page)).toBeChecked();
  });
});

test('the ⋯ settings menu: auto-refresh, mode and theme; closes on Escape and on a click outside (T-323)', async ({
  page,
  tasksServer,
}) => {
  await openDashboard(page, tasksServer, 'standard');
  const button = page.locator('#settings-btn');
  await button.click();
  const settings = page.locator('#settings-menu');
  await expect(settings).toBeVisible();
  await expect(settings.locator('.menu-label')).toHaveText([
    'Auto-refresh',
    'Mode',
    'Theme',
    'Document style',
    'This project',
  ]);
  await expect(button).toHaveAttribute('aria-expanded', 'true');
  await page.keyboard.press('Escape');
  await expect(settings).toHaveCount(0);
  await button.click();
  await page.locator('h1').click();
  await expect(settings).toHaveCount(0);
});
