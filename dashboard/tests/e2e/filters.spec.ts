import {
  closeFilters,
  expect,
  filterButton,
  matchCount,
  openDashboard,
  openFilters,
  test,
  visibleCardIds,
} from './helpers';

test.describe('filters', () => {
  test.beforeEach(async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard', { tab: 'list' });
    await openFilters(page);
  });

  test("buttons are generated from the data, in the vocabulary's order and with its labels", async ({ page }) => {
    const labels = (group: string) =>
      page.locator(`.filter-btn[data-filter="${group}"] .filter-label`).allTextContents();
    // The project's phases, read as sprints (not migrated yet): the open ones, then the closed one (all done)
    // No "All" chip (T-323): nothing selected is all; a group with a selection gets "clear"
    expect(await labels('sprint')).toEqual(['Phase 1', 'Phase 2', 'Phase 0']);
    expect(await labels('type')).toEqual(['Feature', 'Bugfix', 'Test', 'Infra', 'Docs', 'Refactor', 'Polish']);
    expect(await labels('status')).toEqual(['To Do', 'In Progress', 'In Review', 'Blocked', 'Done', 'Skipped']);
    expect(await labels('priority')).toEqual(['P2 Medium']);
    await expect(page.locator('.filter-btn.active')).toHaveCount(0);
    await expect(page.locator('.filter-all')).toHaveCount(0);
  });

  test('selecting a value filters cards and updates the count', async ({ page }) => {
    await filterButton(page, 'sprint', 'Phase 1').click();
    expect(await visibleCardIds(page)).toEqual(['T-004', 'T-005', 'T-006', 'T-007']);
    await expect(matchCount(page)).toHaveText('4 of 12 tasks');
  });

  test('values within a group are OR-ed', async ({ page }) => {
    await filterButton(page, 'sprint', 'Phase 1').click();
    await filterButton(page, 'sprint', 'Phase 2').click();
    await expect(matchCount(page)).toHaveText('8 of 12 tasks');
  });

  test('groups are AND-ed', async ({ page }) => {
    await filterButton(page, 'sprint', 'Phase 1').click();
    await filterButton(page, 'sprint', 'Phase 2').click();
    await filterButton(page, 'type', 'test').click();
    expect(await visibleCardIds(page)).toEqual(['T-009']);
  });

  test('"clear" clears only its own group', async ({ page }) => {
    await filterButton(page, 'sprint', 'Phase 2').click();
    await filterButton(page, 'type', 'test').click();
    await filterButton(page, 'sprint', 'all').click();
    // Grouped by sprint (T-323): the open phase 2 first, the finished phase 0 last
    expect(await visibleCardIds(page)).toEqual(['T-009', 'T-003']);
  });

  test('clicking an active value deselects it', async ({ page }) => {
    await filterButton(page, 'status', 'done').click();
    await expect(matchCount(page)).toHaveText('5 of 12 tasks');
    await filterButton(page, 'status', 'done').click();
    await expect(matchCount(page)).toHaveText('12 of 12 tasks');
    await expect(filterButton(page, 'status', 'all')).toHaveCount(0);
  });

  test('active state stays in sync; "clear" shows only while the group has a selection', async ({ page }) => {
    const clear = filterButton(page, 'type', 'all');
    const feature = filterButton(page, 'type', 'feature');
    await expect(clear).toHaveCount(0);
    await feature.click();
    await expect(feature).toHaveClass(/active/);
    await expect(clear).toHaveText('clear');
    await clear.click();
    await expect(clear).toHaveCount(0);
    await expect(feature).not.toHaveClass(/active/);
  });

  test('filters also apply to the dependency graph', async ({ page }) => {
    await filterButton(page, 'sprint', 'Phase 0').click();
    await closeFilters(page);
    await page.locator('#tab-graph').click();
    await expect(page.locator('#dep-graph g.graph-node')).toHaveCount(4);
  });

  test('no matches shows a message in the grid and the graph', async ({ page }) => {
    await filterButton(page, 'status', 'blocked').click();
    await filterButton(page, 'type', 'docs').click();
    await expect(page.locator('.task-card')).toHaveCount(0);
    await expect(page.locator('#phases')).toContainText('No tasks match current filters');
    await expect(matchCount(page)).toHaveText('0 of 12 tasks');
    await expect(page.locator('#dep-graph')).toContainText('No tasks match current filters');
  });
});

test('unknown statuses sort after the known ones', async ({ page, tasksServer }) => {
  await openDashboard(page, tasksServer, 'edge-cases');
  await openFilters(page);
  const labels = await page.locator('.filter-btn[data-filter="status"] .filter-label').allTextContents();
  expect(labels).toEqual(['To Do', 'In Progress', 'In Review', 'Blocked', 'Done', 'Skipped', 'Review']);
});

test('the Status filter offers every status of the project, also when all tasks are done', async ({
  page,
  tasksServer,
}) => {
  const task = (id: string) => ({ id, name: id, phase: 0, type: 'feature', description: '', status: 'done' });
  await openDashboard(page, tasksServer, { meta: { project: 'Finished' }, tasks: [task('T-1'), task('T-2')] });
  await openFilters(page);
  const labels = await page.locator('.filter-btn[data-filter="status"] .filter-label').allTextContents();
  expect(labels).toEqual(['To Do', 'In Progress', 'In Review', 'Blocked', 'Done', 'Skipped']);
  await filterButton(page, 'status', 'todo').click();
  await expect(matchCount(page)).toHaveText('0 of 2 tasks');
});

test('with many sprints, the Sprint filter lists the open ones; a toggle shows the closed ones (SR-7.1)', async ({
  page,
  tasksServer,
}) => {
  // Phases 0–7 finished (closed sprints), 8 in progress, 9 to do (planned)
  const status = (phase: number) => (phase < 8 ? 'done' : phase === 8 ? 'in_progress' : 'todo');
  const tasks = Array.from({ length: 10 }, (_, phase) => ({
    id: `T-${phase}`,
    name: `Task ${phase}`,
    phase,
    type: 'feature',
    description: '',
    status: status(phase),
  }));
  await openDashboard(page, tasksServer, { meta: { project: 'Many phases' }, tasks });
  await openFilters(page);
  const sprints = page.locator('.filter-btn[data-filter="sprint"]');
  const toggle = page.locator('#filter-closed-sprints');
  await expect(sprints).toHaveCount(2); // Phase 8, Phase 9
  await expect(toggle).toHaveText('+ 8 closed');

  await toggle.click();
  await expect(sprints).toHaveCount(10);
  await expect(toggle).toHaveText('Hide closed');
  await filterButton(page, 'sprint', 'Phase 2').click();
  await expect(matchCount(page)).toHaveText('1 of 10 tasks');

  // Collapsed again, the selected closed sprint stays (with the group's "clear"); the choice is remembered
  await toggle.click();
  await expect(sprints).toHaveCount(4);
  await expect(toggle).toHaveText('+ 7 closed');
  await page.reload();
  await openFilters(page);
  await expect(sprints).toHaveCount(4);
  await expect(filterButton(page, 'sprint', 'Phase 2')).toHaveAttribute('aria-pressed', 'true');
});

test.describe('filter counts', () => {
  test.beforeEach(async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard');
    await openFilters(page);
  });

  const count = (page: import('@playwright/test').Page, group: 'sprint' | 'type' | 'status', value: string) =>
    filterButton(page, group, value).locator('.filter-count');

  test('every button shows how many tasks it would show', async ({ page }) => {
    await expect(count(page, 'sprint', 'Phase 1')).toHaveText('4');
    await expect(count(page, 'type', 'feature')).toHaveText('3');
    await expect(count(page, 'status', 'done')).toHaveText('5');
  });

  test('counts follow the selections in the other groups, not their own', async ({ page }) => {
    await filterButton(page, 'sprint', 'Phase 1').click();
    await expect(count(page, 'sprint', 'Phase 2')).toHaveText('4'); // its own group: unchanged
    await expect(count(page, 'type', 'feature')).toHaveText('2');
    await expect(count(page, 'status', 'todo')).toHaveText('1');
  });

  test('values with no tasks are dimmed but can still be selected', async ({ page }) => {
    await filterButton(page, 'sprint', 'Phase 0').click();
    const feature = filterButton(page, 'type', 'feature');
    await expect(feature.locator('.filter-count')).toHaveText('0');
    await expect(feature).toHaveClass(/\bempty\b/);
    await feature.click();
    await expect(feature).toHaveClass(/\bactive\b/);
    await expect(feature).not.toHaveClass(/\bempty\b/);
    await expect(matchCount(page)).toHaveText('0 of 12 tasks');
  });

  test('screen readers hear the count', async ({ page }) => {
    await expect(filterButton(page, 'type', 'feature')).toHaveAccessibleName('Feature, 3 tasks');
    await expect(filterButton(page, 'sprint', 'Phase 1')).toHaveAccessibleName('Phase 1, 4 tasks');
    await expect(filterButton(page, 'status', 'blocked')).toHaveAccessibleName('Blocked, 1 task');
    // "clear" says what it brings back
    await filterButton(page, 'status', 'blocked').click();
    await expect(filterButton(page, 'status', 'all')).toHaveAccessibleName('All statuses, 12 tasks');
    await expect(filterButton(page, 'type', 'feature')).toHaveAttribute('aria-pressed', 'false');
  });

  test('counts update on reload', async ({ page, tasksServer }) => {
    const data = (await import('./helpers')).loadFixture('standard');
    data.tasks.push({ ...data.tasks[0], id: 'T-099', type: 'feature' });
    tasksServer.use(data);
    await page.locator('#reload-btn').click();
    await expect(count(page, 'type', 'feature')).toHaveText('4');
    await expect(count(page, 'sprint', 'Phase 0')).toHaveText('5');
  });
});

test.describe('search and Filters in the title bar (T-323)', () => {
  test.beforeEach(async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard', { tab: 'list' });
  });

  test('the Filters button opens a popup: what matches and what is selected, a line per group; a badge counts them', async ({
    page,
  }) => {
    const toggle = page.locator('#filters-toggle');
    await expect(page.locator('#filter-panel')).toBeHidden();
    await expect(toggle).toHaveAttribute('aria-expanded', 'false');
    await expect(page.locator('#filters-badge')).toHaveCount(0);
    // No filter row: search and the button sit in the title bar
    await expect(page.locator('.topbar #task-search')).toBeVisible();
    await expect(page.locator('.topbar #filters-toggle')).toBeVisible();

    await openFilters(page);
    await expect(toggle).toHaveAttribute('aria-expanded', 'true');
    await expect(page.locator('#active-filters')).toHaveText('No filters');
    await expect(page.locator('.filter-group')).toHaveText([/^Sprint/, /^Type/, /^Status/, /^Priority/]);
    await filterButton(page, 'sprint', 'Phase 1').click();
    await filterButton(page, 'sprint', 'Phase 2').click();
    await filterButton(page, 'type', 'test').click();
    const chips = page.locator('.filter-chip');
    await expect(chips).toHaveText(['Sprint: Phase 1, Phase 2', 'Type: Test']);
    await expect(matchCount(page)).toHaveText('1 of 12 tasks');
    // The badge counts what narrows the board within its sprints: the Sprint choice is the scope bar's
    await expect(page.locator('#filters-badge')).toHaveText('1');
    await chips.filter({ hasText: 'Sprint' }).getByRole('button', { name: 'Clear Sprint filter' }).click();
    await expect(chips).toHaveText(['Type: Test']);
    await expect(matchCount(page)).toHaveText('2 of 12 tasks');

    // Escape closes it, and so does a click outside
    await page.keyboard.press('Escape');
    await expect(page.locator('#filter-panel')).toBeHidden();
    await toggle.click();
    await page.locator('#dashboard-title').click();
    await expect(page.locator('#filter-panel')).toBeHidden();
  });

  test('"/" focuses the search; Clear filters keeps the sprint', async ({ page }) => {
    // Once the page has loaded: a key pressed before it mounts is lost (CI's runner is slower)
    await expect(page.locator('.task-card').first()).toBeVisible();
    await page.keyboard.press('/');
    await expect(page.locator('#task-search')).toBeFocused();
    await page.keyboard.type('editor');
    await openFilters(page);
    await filterButton(page, 'sprint', 'Phase 2').click();
    await filterButton(page, 'type', 'feature').click();
    await page.locator('#clear-filters').click();
    await expect(page.locator('#task-search')).toHaveValue('');
    await expect(page.locator('.filter-chip')).toHaveText(['Sprint: Phase 2']);
    await expect(page.locator('#clear-filters')).toBeDisabled();
  });

  test('the keyword search narrows every view and the filter counts', async ({ page }) => {
    await page.locator('#task-search').fill('login');
    expect(await visibleCardIds(page)).toEqual(['T-004']);
    await expect(matchCount(page)).toHaveText('1 of 12 tasks');
    await openFilters(page);
    await expect(filterButton(page, 'type', 'feature').locator('.filter-count')).toHaveText('1');

    // Every word has to match, in the ID, name, description or type
    await page.locator('#task-search').fill('t-00 docs');
    expect(await visibleCardIds(page)).toEqual(['T-002']);

    await page.getByRole('button', { name: 'Clear search' }).click();
    await expect(page.locator('#task-search')).toHaveValue('');
    await expect(matchCount(page)).toHaveText('12 of 12 tasks');
  });
});
