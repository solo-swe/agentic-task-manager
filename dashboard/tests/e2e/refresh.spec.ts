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

test.describe('changes (T-507)', () => {
  const requests = (tasksServer: { requests: number }) => expect.poll(() => tasksServer.requests);
  /** The change streams the page opened, and whether each is still open */
  const streams = (page: Page) =>
    page.evaluate(() =>
      (window as unknown as { __atmStreams: { path: string; closed: boolean }[] }).__atmStreams.map((s) => [
        s.path,
        s.closed,
      ]),
    );

  test('the page watches its workspace on one stream, and polls on no timer', async ({ page, tasksServer }) => {
    await page.clock.install();
    await openDashboard(page, tasksServer, 'standard');
    expect(await streams(page)).toEqual([['/api/v2/workspaces/default/changes', false]]);
    await page.clock.runFor(120_000);
    await requests(tasksServer).toBe(1);
    // Nothing to set: no interval on the button, no auto-refresh in the settings
    await expect(page.locator('#reload-btn .refresh-state')).toHaveCount(0);
    await openMenu(page, '#settings-btn');
    await expect(page.locator('#auto-refresh-check')).toHaveCount(0);
    await expect(page.locator('#last-loaded')).toContainText('Updates as tasks change');
  });

  test('a change reloads the page and shows', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard');
    tasksServer.use(standardWithT005Done());
    await tasksServer.pushChange();
    await expect(progress(page)).toHaveAccessibleName('Progress 55%, 6 of 11 tasks done');
    await requests(tasksServer).toBe(2);
  });

  test('an unchanged file is a 304: nothing is parsed or re-rendered; a change still shows', async ({
    page,
    tasksServer,
  }) => {
    await openDashboard(page, tasksServer, 'standard', { tab: 'list' });
    // A marker on a rendered card survives only if the card isn't rendered again
    await page.locator('.task-card[data-id="T-005"]').evaluate((el) => el.setAttribute('data-marker', '1'));
    await tasksServer.pushChange();
    await requests(tasksServer).toBe(2);
    expect(tasksServer.notModified).toBe(1);
    await expect(page.locator('.task-card[data-id="T-005"][data-marker="1"]')).toHaveCount(1);

    tasksServer.use(standardWithT005Done());
    await tasksServer.pushChange();
    await expect(progress(page)).toHaveAccessibleName('Progress 55%, 6 of 11 tasks done');
    expect(tasksServer.notModified).toBe(1);
  });

  test('another workspace: its own stream, the old one closed', async ({ page, tasksServer }) => {
    tasksServer.workspaces(['work']);
    await openDashboard(page, tasksServer, 'standard');
    await page.locator('#workspace-selector').click();
    await page.locator('.workspace-option[data-id="work"]').click();
    await expect(page.locator('#workspace-selector')).toHaveText('work');
    await expect
      .poll(() => streams(page))
      .toEqual([
        ['/api/v2/workspaces/default/changes', true],
        ['/api/v2/workspaces/work/changes', false],
      ]);
  });
});

test('the ⋯ settings menu: mode and theme; closes on Escape and on a click outside (T-323)', async ({
  page,
  tasksServer,
}) => {
  await openDashboard(page, tasksServer, 'standard');
  const button = page.locator('#settings-btn');
  await button.click();
  const settings = page.locator('#settings-menu');
  await expect(settings).toBeVisible();
  await expect(settings.locator('.menu-label')).toHaveText(['Mode', 'Theme', 'Document style', 'This project']);
  await expect(button).toHaveAttribute('aria-expanded', 'true');
  await page.keyboard.press('Escape');
  await expect(settings).toHaveCount(0);
  await button.click();
  await page.locator('h1').click();
  await expect(settings).toHaveCount(0);
});
