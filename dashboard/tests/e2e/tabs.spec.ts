import type { Page } from '@playwright/test';
import { expect, filterButton, matchCount, openDashboard, openFilters, test } from './helpers';

const tab = (page: Page, id: string) => page.locator(`#tab-${id}`);
const panel = (page: Page, id: string) => page.locator(`#panel-${id}`);
const zoomLabel = (page: Page) => page.locator('#graph-zoom-level');

async function expectSelected(page: Page, id: string) {
  await expect(tab(page, id)).toHaveAttribute('aria-selected', 'true');
  await expect(tab(page, id)).toHaveAttribute('tabindex', '0');
  await expect(panel(page, id)).toBeVisible();
  for (const other of ['graph', 'kanban', 'list', 'agents', 'progress', 'project'].filter((x) => x !== id)) {
    await expect(tab(page, other)).toHaveAttribute('aria-selected', 'false');
    await expect(tab(page, other)).toHaveAttribute('tabindex', '-1');
    await expect(panel(page, other)).toBeHidden();
  }
}

test.describe('view tabs', () => {
  test('a tablist: the task views, a divider, the project views; the Kanban selected by default (T-251)', async ({
    page,
    tasksServer,
  }) => {
    await openDashboard(page, tasksServer, 'standard');
    await expect(page.getByRole('tablist')).toBeVisible();
    await expect(page.getByRole('tab')).toHaveText([
      'Kanban Board',
      'Task List',
      'Dependency Graph',
      'Progress',
      'Agents',
      'Project',
    ]);
    // The divider sits between the graph and Progress
    await expect(page.locator('#tab-graph + .view-tab-divider + #tab-progress')).toHaveCount(1);
    await expectSelected(page, 'kanban');
    await expect(panel(page, 'kanban')).toHaveAttribute('role', 'tabpanel');
    await expect(panel(page, 'kanban')).toHaveAttribute('aria-labelledby', 'tab-kanban');
  });

  test('#view=graph still opens the graph', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard', { tab: 'graph' });
    await expectSelected(page, 'graph');
    await expect(page.locator('#dep-graph')).toBeVisible();
  });

  test('clicking a tab switches views and writes the URL hash', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard');
    await tab(page, 'graph').click();
    await expectSelected(page, 'graph');
    // The hash also holds the Sprint filter (#sprint=, T-323)
    const view = () => new URLSearchParams(new URL(page.url()).hash.slice(1)).get('view');
    expect(view()).toBe('graph');
    await tab(page, 'progress').click();
    await expectSelected(page, 'progress');
    expect(view()).toBe('progress');
  });

  test('arrow keys move between tabs and wrap', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard');
    await tab(page, 'kanban').focus();
    await page.keyboard.press('ArrowRight');
    await expectSelected(page, 'list');
    await expect(tab(page, 'list')).toBeFocused();
    await page.keyboard.press('ArrowLeft');
    await page.keyboard.press('ArrowLeft');
    await expectSelected(page, 'project');
    await expect(tab(page, 'project')).toBeFocused();
  });

  test('a deep link opens that view, and it survives a reload', async ({ page, tasksServer }) => {
    tasksServer.use('standard');
    await page.goto('tasks-dashboard.html#view=kanban');
    await expectSelected(page, 'kanban');
    await page.reload();
    await expectSelected(page, 'kanban');
  });

  test('back and forward follow the selected tab', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard');
    await tab(page, 'kanban').click();
    await tab(page, 'progress').click();
    await page.goBack();
    await expectSelected(page, 'kanban');
    await page.goForward();
    await expectSelected(page, 'progress');
  });

  test('the graph keeps its zoom across a tab round trip', async ({ page, tasksServer }) => {
    // Without auto-fit (T-341), the graph opens at 60%
    await openDashboard(page, tasksServer, 'large-90', { tab: 'graph', graphAutoFit: false });
    await page.locator('#graph-container').focus();
    await page.keyboard.press('+');
    await expect(zoomLabel(page)).toHaveText('75%');
    const viewBox = await page.locator('#dep-graph').getAttribute('viewBox');
    await tab(page, 'kanban').click();
    await tab(page, 'graph').click();
    await expect(zoomLabel(page)).toHaveText('75%');
    await expect(page.locator('#dep-graph')).toHaveAttribute('viewBox', viewBox!);
  });

  test('search and Filters stay in the title bar on every tab, with the count of matching tasks', async ({
    page,
    tasksServer,
  }) => {
    await openDashboard(page, tasksServer, 'standard');
    await openFilters(page);
    await filterButton(page, 'sprint', 'Phase 1').click();
    await page.keyboard.press('Escape');
    for (const id of ['graph', 'kanban', 'list', 'agents', 'progress']) {
      await tab(page, id).click();
      await expect(page.locator('.topbar #filters-toggle')).toBeVisible();
      await expect(matchCount(page)).toHaveText('4 of 12 tasks');
    }
  });

  test('the List tab shows the task cards', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard');
    await expect(page.locator('.task-card').first()).toBeHidden();
    await tab(page, 'list').click();
    await expectSelected(page, 'list');
    await expect(page.locator('.task-card')).toHaveCount(12);
    await expect(page.locator('.task-card').first()).toBeVisible();
  });
});
