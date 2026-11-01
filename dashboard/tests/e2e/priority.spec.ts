import {
  expect,
  filterButton,
  loadFixture,
  matchCount,
  openDashboard,
  openFilters,
  test,
  visibleCardIds,
} from './helpers';

// D-2.3: a priority badge on every card and in the detail panel, a Priority filter group, and the List tab ordered
// by priority on request; the filter and the order are part of the project's saved view.
const withPriorities = () => {
  const data = loadFixture('standard');
  data.meta = { ...data.meta, id: 'prio' };
  const priorities: Record<string, string> = { 'T-006': 'P0', 'T-009': 'P0', 'T-002': 'P1', 'T-011': 'P3' };
  for (const t of data.tasks) if (priorities[String(t.id)]) t.priority = priorities[String(t.id)];
  return data;
};

test.describe('priority', () => {
  test('badges on cards, Kanban cards and the detail panel; the default for tasks without one', async ({
    page,
    tasksServer,
  }) => {
    await openDashboard(page, tasksServer, withPriorities(), { tab: 'list' });
    const badge = (id: string) => page.locator(`.task-card[data-id="${id}"] .priority-badge`);
    await expect(badge('T-006')).toHaveText('P0');
    await expect(badge('T-006')).toHaveAttribute('title', 'P0 Critical priority');
    await expect(badge('T-001')).toHaveText('P2');
    await expect(badge('T-001')).toHaveAttribute('title', 'P2 Medium priority (the default)');

    await page.locator('#tab-kanban').click();
    await expect(page.locator('.kanban-card[data-id="T-006"] .priority-badge')).toHaveText('P0');
    await page.locator('.kanban-card[data-id="T-006"]').click();
    await expect(page.locator('#detail-panel .priority-badge')).toHaveText('P0');
  });

  test('the Priority filter, with counts, most urgent first', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, withPriorities(), { tab: 'list' });
    await openFilters(page);
    await expect(page.locator('.filter-btn[data-filter="priority"] .filter-label')).toHaveText([
      'All',
      'P0 Critical',
      'P1 High',
      'P2 Medium',
      'P3 Low',
    ]);
    await expect(filterButton(page, 'priority', 'P0').locator('.filter-count')).toHaveText('2');
    await filterButton(page, 'priority', 'P0').click();
    await expect(matchCount(page)).toHaveText('2 of 12 tasks');
    expect(await visibleCardIds(page)).toEqual(['T-006', 'T-009']);
    await expect(page.locator('.filter-chip')).toHaveText(['Priority: P0 Critical']);
  });

  test('the List tab orders by priority on request; the filter and the order are remembered', async ({
    page,
    tasksServer,
  }) => {
    await openDashboard(page, tasksServer, withPriorities(), { tab: 'list' });
    // One list, not a group per sprint (T-323)
    await page.locator('#list-group').selectOption('none');
    expect((await visibleCardIds(page)).slice(0, 3)).toEqual(['T-001', 'T-002', 'T-003']);
    await page.locator('#list-sort').selectOption('priority');
    // P0 (by phase), P1, then the P2s by phase and ID
    expect((await visibleCardIds(page)).slice(0, 4)).toEqual(['T-006', 'T-009', 'T-002', 'T-001']);
    await openFilters(page);
    await filterButton(page, 'priority', 'P3').click();

    await page.goto('tasks-dashboard.html#view=list');
    await expect(page.locator('#list-sort')).toHaveValue('priority');
    await expect(page.locator('.filter-chip')).toHaveText(['Priority: P3 Low']);
    expect(await visibleCardIds(page)).toEqual(['T-011']);
  });
});
