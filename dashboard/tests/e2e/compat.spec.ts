import { expect, openDashboard, openView, test } from './helpers';

// NFR-5: tasks.json files from before 0.7.0 (no started_at/completed_at; some without timestamps or meta) must
// render in every tab without errors, with sensible fallbacks.
for (const fixture of ['standard', 'minimal', 'no-meta', 'edge-cases', 'empty']) {
  test(`a pre-0.7.0 tasks.json ("${fixture}") renders every tab`, async ({ page, tasksServer }) => {
    const errors: string[] = [];
    page.on('pageerror', (e) => errors.push(e.message));
    await openDashboard(page, tasksServer, fixture);

    await openView(page, 'kanban');
    await expect(page.locator('#kanban')).toBeVisible();
    await expect(page.locator('.kanban-column[data-status="todo"]')).toBeVisible();

    await openView(page, 'progress');
    await expect(page.locator('#progress')).toBeVisible();
    if (fixture === 'empty') await expect(page.locator('#progress')).toContainText('No tasks match current filters');
    else await expect(page.locator('#sprint-table')).toBeVisible();

    await openView(page, 'graph');
    await expect(page.locator('#dep-graph')).toBeVisible();
    expect(errors).toEqual([]);
  });
}

test('without start/complete times, estimates fall back to gaps between updated_at values', async ({
  page,
  tasksServer,
}) => {
  await openDashboard(page, tasksServer, 'standard');
  await openView(page, 'progress');
  await expect(page.locator('#progress-basis')).toContainText('last 4 tasks');
});
