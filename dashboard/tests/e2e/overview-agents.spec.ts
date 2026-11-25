import { expect, openView, test } from './helpers';

// T-333: the overview's Agents tab, every project's agents on one timeline
test.describe('Projects Overview: the Agents tab', () => {
  test.beforeEach(async ({ page, tasksServer }) => {
    tasksServer.use('agents');
    tasksServer.addProject('second', 'agents', 'Second Project');
    await page.goto('/projects');
  });

  test('a lane per agent across projects, naming them; a bar opens its task in its project', async ({ page }) => {
    await openView(page, 'agents', { overview: true });
    await expect(page).toHaveURL(/\/agents(\?|$)/);
    const lanes = page.locator('#overview-agents .agent-info');
    // alice and bob worked in both projects (the same fixture twice); T-004 has no agent
    await expect(lanes).toHaveCount(3);
    const alice = page.locator('#overview-agents .agent-info[data-agent="claude:alice001"]');
    await expect(alice.locator('.agent-projects')).toHaveText('Agents Project · Second Project');
    // Each project's T-001 is its own bar
    await expect(page.locator('#overview-agents .agent-bar[data-id="T-001"]')).toHaveCount(2);
    // The fixture's live task runs to now, months on, covering the short bars: click the bar itself
    await page.locator('#overview-agents .agent-bar[data-id="T-001"][data-project="second"]').dispatchEvent('click');
    await expect(page).toHaveURL(/\/projects\/second(\/|\?|$)/);
    await expect(page.locator('#detail-overlay.open')).toContainText('T-001');
  });

  test('the tab is remembered; the arrow keys move between the items, round from the last to the first, and between Analysis’ sub-tabs (ATM-752)', async ({
    page,
  }) => {
    await openView(page, 'agents', { overview: true });
    await expect(page.locator('#overview-agents .agent-info').first()).toBeVisible();
    await page.reload();
    await expect(page.locator('#item-analysis')).toHaveAttribute('aria-selected', 'true');
    await expect(page.locator('#overview-tab-agents')).toHaveAttribute('aria-selected', 'true');
    // Analysis' sub-tabs: Progress and Agents, round
    await page.locator('#overview-tab-agents').press('ArrowRight');
    await expect(page.locator('#overview-tab-progress')).toHaveAttribute('aria-selected', 'true');
    // The items: Analysis is the last, then round to Overview, and back; an item opens on its first view
    await page.locator('#item-analysis').press('ArrowRight');
    await expect(page.locator('#item-overview')).toHaveAttribute('aria-selected', 'true');
    await expect(page.locator('#projects-grid')).toBeVisible();
    await page.locator('#item-overview').press('ArrowLeft');
    await expect(page.locator('#item-analysis')).toHaveAttribute('aria-selected', 'true');
    await expect(page.locator('#overview-tab-progress')).toHaveAttribute('aria-selected', 'true');
  });

  test('a project’s own Agents tab does not change the overview’s tab', async ({ page }) => {
    await openView(page, 'board', { overview: true });
    await page.goto('/projects/second/agents');
    await expect(page.locator('#agents .agent-info').first()).toBeVisible();
    // The landing page: the overview on its last tab
    await page.goto('/');
    await expect(page.locator('#overview-tab-board')).toHaveAttribute('aria-selected', 'true');
  });
});
