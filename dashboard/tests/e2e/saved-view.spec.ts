import {
  closeFilters,
  expect,
  filterButton,
  loadFixture,
  matchCount,
  openDashboard,
  openFilters,
  test,
} from './helpers';

// FR-5: the dashboard remembers each project's view (filters, search, tab, the options in each tab and the zoom of
// the graph and the Agents timeline) and
// restores it when the dashboard is opened again. Projects are told apart by meta.id (else meta.project).
const project = (id: string) => {
  const data = loadFixture('standard');
  data.meta = { ...data.meta, id };
  return data;
};

test.describe('saved view', () => {
  test('filters, search, tab and Kanban options come back after a reload', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, project('p1'));
    // Grouping is offered while the board shows several sprints (T-323): all of them, before picking one
    await page.locator('#kanban-group-by-sprint').check();
    await page.locator('#kanban-show-skipped').check();
    await openFilters(page);
    await filterButton(page, 'sprint', 'Phase 1').click();
    await filterButton(page, 'type', 'feature').click();
    await page.locator('#task-search').fill('user');
    await closeFilters(page);
    await page.locator('#tab-graph').click();

    await page.goto('tasks-dashboard.html'); // no #view=: the saved tab applies, the graph (not the default Kanban)
    await expect(page.locator('.filter-chip')).toHaveText(['Sprint: Phase 1', 'Type: Feature']);
    await expect(page.locator('#task-search')).toHaveValue('user');
    await expect(matchCount(page)).toHaveText('1 of 12 tasks');
    await expect(page.locator('#tab-graph')).toHaveAttribute('aria-selected', 'true');
    // The saved Sprint filter goes into the hash too (#sprint=, T-323)
    await expect(page).toHaveURL(/#view=graph&sprint=Phase%201$/);
    await page.locator('#tab-kanban').click();
    await expect(page.locator('#kanban-show-skipped')).toBeChecked();
    await openFilters(page);
    await expect(filterButton(page, 'sprint', 'Phase 1')).toHaveClass(/\bactive\b/);
    await filterButton(page, 'sprint', 'all').click();
    await closeFilters(page);
    await expect(page.locator('#kanban-group-by-sprint')).toBeChecked();
  });

  test('"Show all" in the Kanban done column is remembered', async ({ page, tasksServer }) => {
    const data = loadFixture('kanban');
    data.meta = { ...data.meta, id: 'kan' };
    await openDashboard(page, tasksServer, data, { tab: 'kanban' });
    const done = page.locator('.kanban-column[data-status="done"]');
    await done.getByRole('button', { name: /Show all/ }).click();
    await page.reload();
    await expect(done.locator('.kanban-card')).toHaveCount(24);
    await expect(done.getByRole('button', { name: 'Show the latest 20' })).toBeVisible();
  });

  test('greying out filtered tasks in the graph is remembered', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, project('p1'), { tab: 'graph' });
    await page.locator('#task-search').fill('editor');
    await page.locator('#graph-filtered [data-mode="dim"]').click();
    await page.reload();
    await expect(page.locator('#graph-filtered [data-mode="dim"]')).toHaveAttribute('aria-pressed', 'true');
    await expect(page.locator('#dep-graph g.graph-dimmed')).toHaveCount(9);
  });

  test('the focused phase on the Progress tab is remembered', async ({ page, tasksServer }) => {
    await page.clock.setFixedTime(new Date('2026-04-06T12:00:00Z'));
    const data = loadFixture('progress');
    data.meta = { ...data.meta, id: 'prog' };
    await openDashboard(page, tasksServer, data);
    await page.locator('#tab-progress').click();
    await page.locator('#sprint-table tbody tr').nth(1).click();
    const focused = await page.locator('#burndown-focus').textContent();
    expect(focused).toMatch(/^Phase \d+$/);
    await page.reload();
    await expect(page.locator('#burndown-focus')).toHaveText(focused!);
  });

  test('each project keeps its own view on the same origin', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, project('p1'));
    await openFilters(page);
    await filterButton(page, 'sprint', 'Phase 2').click();
    // Opened fresh (no #sprint= from the other project's address, T-323)
    tasksServer.use(project('p2'));
    await page.goto('tasks-dashboard.html');
    await expect(page.locator('.filter-chip')).toHaveCount(0);
    tasksServer.use(project('p1'));
    await page.goto('tasks-dashboard.html');
    await expect(page.locator('.filter-chip')).toHaveText(['Sprint: Phase 2']);
  });

  test('a #view= link wins over the saved tab', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, project('p1'));
    await page.locator('#tab-progress').click();
    await page.goto('tasks-dashboard.html#view=kanban');
    await page.reload();
    await expect(page.locator('#tab-kanban')).toHaveAttribute('aria-selected', 'true');
  });

  test('values that no longer exist are dropped quietly', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, project('p1'));
    await openFilters(page);
    await filterButton(page, 'type', 'polish').click();
    await filterButton(page, 'sprint', 'Phase 2').click();
    const data = project('p1');
    data.tasks = data.tasks.filter((t) => t.type !== 'polish');
    tasksServer.use(data);
    await page.reload();
    await expect(page.locator('.filter-chip')).toHaveText(['Sprint: Phase 2']);
    await expect(matchCount(page)).toHaveText('3 of 11 tasks');
  });

  test('Reset view forgets the saved view', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, project('p1'));
    await openFilters(page);
    await filterButton(page, 'status', 'todo').click();
    await page.locator('#task-search').fill('editor');
    await closeFilters(page);
    await page.locator('#tab-progress').click();
    // In the ⋯ settings menu, apart from Clear filters (T-323)
    await page.locator('#settings-btn').click();
    await page.locator('#reset-view').click();
    await expect(page.locator('#settings-menu')).toHaveCount(0);
    await expect(page.locator('.filter-btn.active')).toHaveCount(0);
    await expect(page.locator('#task-search')).toHaveValue('');
    await expect(page.locator('#tab-kanban')).toHaveAttribute('aria-selected', 'true');
    await page.goto('tasks-dashboard.html');
    await expect(matchCount(page)).toHaveText('12 of 12 tasks');
  });

  test('the view is kept under the project id the server gives (meta.id, or its stand-in)', async ({
    page,
    tasksServer,
  }) => {
    await openDashboard(page, tasksServer, 'standard'); // no meta.id: the server's id stands in
    await openFilters(page);
    await filterButton(page, 'sprint', 'Phase 0').click();
    const stored = await page.evaluate(() => Object.keys(localStorage).filter((k) => k.includes(':view:')));
    expect(stored).toEqual([`atm-dashboard:view:id:${tasksServer.projectId}`]);
  });
});
