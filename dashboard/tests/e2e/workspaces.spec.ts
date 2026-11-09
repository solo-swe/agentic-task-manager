// Workspaces (ADR-003 §5, T-457): the switcher next to the project selector when there are several, the /w/<w>
// addresses, and `/` opening the workspace opened last.
import { addressOf, expect, openDashboard, test } from './helpers';

test.describe('workspaces', () => {
  test('with one workspace there is no switcher, and the addresses have no /w/', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard');
    await expect(page.locator('#dashboard-title')).toHaveText('Standard Project');
    await expect(page.locator('#workspace-selector')).toHaveCount(0);
    expect(addressOf(page)).toMatch(new RegExp(`^/projects/${tasksServer.projectId}(\\?|$)`));
  });

  test('the switcher lists the workspaces; choosing one opens its page, and back again', async ({
    page,
    tasksServer,
  }) => {
    tasksServer.workspaces(['work']);
    await openDashboard(page, tasksServer, 'standard');
    await expect(page.locator('#dashboard-title')).toHaveText('Standard Project');
    await expect(page.locator('#workspace-selector')).toHaveText('default');
    await page.locator('#workspace-selector').click();
    await expect(page.locator('#workspace-menu .workspace-option')).toHaveText([/default/, /work\s*0 projects/]);
    await expect(page.locator('.workspace-option[data-id="default"]')).toHaveAttribute('aria-current', 'true');
    await page.locator('.workspace-option[data-id="work"]').click();
    // Its own projects (none here): how to add one, in the workspace's address
    await expect(page.locator('#workspace-selector')).toHaveText('work');
    await expect.poll(() => addressOf(page)).toBe('/w/work');
    await expect(page.locator('#workspace-menu')).toHaveCount(0);

    await page.locator('#workspace-selector').click();
    await page.locator('.workspace-option[data-id="default"]').click();
    await expect(page.locator('#dashboard-title')).toHaveText('Standard Project');
    await expect.poll(() => addressOf(page)).toMatch(new RegExp(`^/projects/${tasksServer.projectId}(\\?|$)`));
  });

  test('/ opens the workspace opened last; a /w/<w> address opens that workspace', async ({ page, tasksServer }) => {
    tasksServer.workspaces(['work'], 'work');
    await openDashboard(page, tasksServer, 'standard');
    await expect.poll(() => addressOf(page)).toBe('/w/work');
    await expect(page.locator('#workspace-selector')).toHaveText('work');

    await page.goto(`/w/default/projects/${tasksServer.projectId}`);
    await expect(page.locator('#dashboard-title')).toHaveText('Standard Project');
    await expect.poll(() => addressOf(page)).toMatch(new RegExp(`^/projects/${tasksServer.projectId}(\\?|$)`));
  });
});
