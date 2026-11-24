import type { Page } from '@playwright/test';
import { expect, filterButton, matchCount, openDashboard, openFilters, openView, test } from './helpers';

// A project's page by item (ATM-752, docs/sidebar-overhaul-design.md §5): its items as tabs, Tasks' and Analysis' views
// as sub-tabs under them

const ITEMS = ['overview', 'inbox', 'tasks', 'reviews', 'documents', 'analysis'];
const VIEWS: Record<string, string> = {
  project: 'overview',
  inbox: 'inbox',
  kanban: 'tasks',
  list: 'tasks',
  graph: 'tasks',
  reviews: 'reviews',
  documents: 'documents',
  progress: 'analysis',
  agents: 'analysis',
};
const SUB_TABS: Record<string, string[]> = { tasks: ['kanban', 'list', 'graph'], analysis: ['progress', 'agents'] };

const item = (page: Page, id: string) => page.locator(`#item-${id}`);
const subTab = (page: Page, id: string) => page.locator(`#tab-${id}`);
const panel = (page: Page, id: string) => page.locator(`#panel-${id}`);
const zoomLabel = (page: Page) => page.locator('#graph-zoom-level');

/** View `id` shown: its item selected (the only one in the tab order), its sub-tab where it has one, its panel alone */
async function expectSelected(page: Page, id: string) {
  const selected = VIEWS[id];
  for (const other of ITEMS) {
    await expect(item(page, other)).toHaveAttribute('aria-selected', String(other === selected));
    await expect(item(page, other)).toHaveAttribute('tabindex', other === selected ? '0' : '-1');
  }
  const subs = SUB_TABS[selected] ?? [];
  await expect(page.locator('.sub-tab')).toHaveCount(subs.length);
  for (const sub of subs) await expect(subTab(page, sub)).toHaveAttribute('aria-selected', String(sub === id));
  await expect(panel(page, id)).toBeVisible();
  for (const other of Object.keys(VIEWS).filter((x) => x !== id)) await expect(panel(page, other)).toBeHidden();
}

test.describe('the page by item', () => {
  test('the items as tabs; Tasks’ and Analysis’ views as sub-tabs; the Overview by default (ATM-751)', async ({
    page,
    tasksServer,
  }) => {
    await openDashboard(page, tasksServer, 'standard');
    await expect(page.getByRole('tablist', { name: 'Project' }).getByRole('tab')).toHaveText([
      'Overview',
      'Inbox',
      'Tasks',
      'Code Reviews',
      'Documents',
      'Analysis',
    ]);
    await expectSelected(page, 'project');
    await item(page, 'tasks').click();
    // The Kanban is the Board, the graph Dependencies (ATM-751); an item opens on its first view
    await expect(page.getByRole('tablist', { name: 'Tasks' }).getByRole('tab')).toHaveText([
      'Board',
      'List',
      'Dependencies',
    ]);
    await expectSelected(page, 'kanban');
    await expect(panel(page, 'kanban')).toHaveAttribute('role', 'tabpanel');
    await expect(panel(page, 'kanban')).toHaveAttribute('aria-labelledby', 'tab-kanban');
    await expect(subTab(page, 'kanban')).toHaveAttribute('aria-controls', 'panel-kanban');
    await item(page, 'analysis').click();
    await expect(page.getByRole('tablist', { name: 'Analysis' }).getByRole('tab')).toHaveText(['Progress', 'Agents']);
    await expectSelected(page, 'progress');
    await expect(panel(page, 'project')).toHaveAttribute('aria-labelledby', 'item-overview');
  });

  test('Inbox and Documents say their pages are coming', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard');
    await item(page, 'inbox').click();
    await expectSelected(page, 'inbox');
    await expect(page).toHaveURL(/\/projects\/[^/]+\/inbox(\?|$)/);
    await expect(page.locator('#inbox-placeholder')).toContainText('The Inbox is coming');
    await item(page, 'documents').click();
    await expectSelected(page, 'documents');
    await expect(page.locator('#documents-placeholder')).toContainText('Documents are coming');
  });

  test('a project’s address without a tab is its Overview, whatever tab was seen last (ATM-751)', async ({
    page,
    tasksServer,
  }) => {
    await openDashboard(page, tasksServer, 'standard', { tab: 'graph' });
    await expectSelected(page, 'graph');
    await page.goto(`/projects/${tasksServer.projectId}`);
    await expectSelected(page, 'project');
    // A tab in the address is still that tab
    await page.goto(`/projects/${tasksServer.projectId}/list`);
    await expectSelected(page, 'list');
  });

  test('#view=graph still opens the graph', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard', { tab: 'graph' });
    await expectSelected(page, 'graph');
    await expect(page.locator('#dep-graph')).toBeVisible();
  });

  test("an item or a sub-tab switches views and writes the project's address", async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard');
    await openView(page, 'graph');
    await expectSelected(page, 'graph');
    // /projects/<id>/<tab> (T-429); the query holds the Sprint filter (?sprint=, T-323)
    const view = () => new URL(page.url()).pathname.split('/')[3] ?? null;
    expect(view()).toBe('graph');
    await openView(page, 'agents');
    await expectSelected(page, 'agents');
    expect(view()).toBe('agents');
    // The item already shown: a click on it stays where it is
    await item(page, 'analysis').click();
    await expectSelected(page, 'agents');
  });

  test('arrow keys move between the items, and between an item’s sub-tabs, wrapping', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard', { tab: 'kanban' });
    await subTab(page, 'kanban').focus();
    await page.keyboard.press('ArrowRight');
    await expectSelected(page, 'list');
    await expect(subTab(page, 'list')).toBeFocused();
    await page.keyboard.press('ArrowLeft');
    await page.keyboard.press('ArrowLeft');
    await expectSelected(page, 'graph');
    await expect(subTab(page, 'graph')).toBeFocused();
    await item(page, 'tasks').focus();
    await page.keyboard.press('ArrowRight');
    await expectSelected(page, 'reviews');
    await expect(item(page, 'reviews')).toBeFocused();
    await page.keyboard.press('ArrowLeft');
    await page.keyboard.press('ArrowLeft');
    await page.keyboard.press('ArrowLeft');
    await expectSelected(page, 'project');
    await page.keyboard.press('ArrowLeft');
    await expectSelected(page, 'progress');
  });

  test('a deep link opens that view, and it survives a reload', async ({ page, tasksServer }) => {
    tasksServer.use('standard');
    await page.goto('/?view=kanban');
    await expectSelected(page, 'kanban');
    await page.reload();
    await expectSelected(page, 'kanban');
    // The landing page's List is the project's, though All projects has one too
    await page.goto('/?view=list');
    await expectSelected(page, 'list');
  });

  test('back and forward follow the items and the sub-tabs', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard');
    await openView(page, 'kanban');
    await subTab(page, 'graph').click();
    await openView(page, 'progress');
    await page.goBack();
    await expectSelected(page, 'graph');
    await page.goBack();
    await expectSelected(page, 'kanban');
    await page.goBack();
    await expectSelected(page, 'project');
    await page.goForward();
    await page.goForward();
    await expectSelected(page, 'graph');
  });

  // ATM-696: closing a task's details goes Back to the page under them, so Back after it goes to the page before
  test('Back after closing a task’s details goes to the page before, not the same page again', async ({
    page,
    tasksServer,
  }) => {
    await openDashboard(page, tasksServer, 'standard', { tab: 'list' });
    await subTab(page, 'kanban').click();
    await expectSelected(page, 'kanban');
    const depth = await page.evaluate(() => history.length);
    await page.locator('.kanban-card[data-id="T-004"]').click({ position: { x: 10, y: 10 } });
    await expect(page.locator('#detail-overlay')).toHaveClass(/open/);
    await page.keyboard.press('Escape');
    await expect(page.locator('#detail-overlay')).not.toHaveClass(/open/);
    await expect.poll(() => new URL(page.url()).searchParams.get('task')).toBeNull();
    await page.goBack();
    await expectSelected(page, 'list');
    expect(await page.evaluate(() => history.length)).toBe(depth + 1);
  });

  test('the graph keeps its zoom across a tab round trip', async ({ page, tasksServer }) => {
    // Without auto-fit (T-341), the graph opens at 60%
    await openDashboard(page, tasksServer, 'large-90', { tab: 'graph', graphAutoFit: false });
    await page.locator('#graph-container').focus();
    await page.keyboard.press('+');
    await expect(zoomLabel(page)).toHaveText('75%');
    const viewBox = await page.locator('#dep-graph').getAttribute('viewBox');
    await openView(page, 'progress');
    await openView(page, 'graph');
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
      await openView(page, id);
      await expect(page.locator('.topbar #filters-toggle')).toBeVisible();
      await expect(matchCount(page)).toHaveText('4 of 12 tasks');
    }
  });

  test('the List tab shows the task cards', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard');
    await expect(page.locator('.task-card').first()).toBeHidden();
    await openView(page, 'list');
    await expectSelected(page, 'list');
    await expect(page.locator('.task-card')).toHaveCount(12);
    await expect(page.locator('.task-card').first()).toBeVisible();
  });
});
