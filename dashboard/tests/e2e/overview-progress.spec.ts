import { expect, test } from './helpers';

// T-329: a Progress tab on the Projects Overview, every project's progress
test.describe('Projects Overview: the Progress tab', () => {
  test('a row per project, the burndown and velocity of all; a row opens the project’s Progress tab', async ({
    page,
    tasksServer,
  }) => {
    tasksServer.use('agents');
    tasksServer.addProject('second', 'sprints', 'Second Project');
    await page.goto('tasks-dashboard.html#overview');
    await page.locator('#overview-tab-progress').click();
    await expect(page).toHaveURL(/view=progress/);
    const rows = page.locator('#overview-progress .overview-progress-row');
    await expect(rows).toHaveCount(2);
    // The agents fixture: 6 of its 8 tasks done, no sprints
    await expect(rows.filter({ hasText: 'Agents Project' })).toContainText('6 / 8 · 75%');
    await expect(rows.filter({ hasText: 'Agents Project' })).toContainText('No active sprint');
    await expect(page.locator('#overview-progress .burndown-chart, #overview-progress svg').first()).toBeVisible();
    // A week per bar by default; per day or month as chosen; a colour per project in the legend (T-329 review)
    await expect(page.locator('#overview-velocity .velocity-column')).toHaveCount(10);
    await expect(page.locator('#overview-velocity .velocity-legend > span')).toHaveText([
      'Agents Project',
      'Second Project',
    ]);
    await page.locator('#overview-velocity-unit [data-unit="day"]').click();
    await expect(page.locator('#overview-velocity .velocity-column')).toHaveCount(14);
    await page.locator('#overview-velocity-unit [data-unit="month"]').click();
    await expect(page.locator('#overview-velocity .velocity-column')).toHaveCount(6);
    await rows.filter({ hasText: 'Second Project' }).click();
    await expect(page).toHaveURL(/project=second/);
    await expect(page.locator('#tab-progress')).toHaveAttribute('aria-selected', 'true');
  });

  test('a project’s own Progress tab does not change the overview’s tab', async ({ page, tasksServer }) => {
    tasksServer.use('agents');
    tasksServer.addProject('second', 'sprints', 'Second Project');
    await page.goto('tasks-dashboard.html#overview');
    await page.locator('#overview-tab-board').click();
    await page.goto('tasks-dashboard.html#project=second&view=progress');
    await expect(page.locator('#progress')).toBeVisible();
    await page.goto('tasks-dashboard.html#overview');
    await expect(page.locator('#overview-tab-board')).toHaveAttribute('aria-selected', 'true');
  });
});
