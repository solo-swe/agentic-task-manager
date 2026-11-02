import { expect, test } from './helpers';

// T-328: the title bar's Activity on the Projects Overview, every project's tasks together
test.describe('Projects Overview: Activity across projects', () => {
  test('the chart sums every project; the popup counts each project; a project opens from it', async ({
    page,
    tasksServer,
  }) => {
    tasksServer.use('agents');
    tasksServer.addProject('second', 'agents', 'Second Project');
    tasksServer.addProject('third', 'minimal', 'Third Project');
    await page.goto('tasks-dashboard.html#overview');
    const card = page.locator('#activity-card');
    // The agents fixture finishes 6 tasks; twice over, plus none in the minimal one
    await expect(card).toHaveAttribute('aria-label', 'Activity: 12 tasks completed');
    await card.click();
    await expect(page.locator('#activity-panel')).toBeVisible();
    const rows = page.locator('#activity-projects .activity-project');
    await expect(rows).toHaveText([/Agents Project\s*6/, /Second Project\s*6/, /Third Project\s*0/]);
    await rows.filter({ hasText: 'Second Project' }).click();
    await expect(page).toHaveURL(/project=second/);
  });

  test('its period is its own: a project’s Activity keeps the one it had', async ({ page, tasksServer }) => {
    tasksServer.use('agents');
    tasksServer.addProject('second', 'agents', 'Second Project');
    await page.goto('tasks-dashboard.html#overview');
    await page.locator('#activity-card').click();
    await page.locator('#activity-panel [data-range="week"]').click();
    await expect(page.locator('#activity-panel [data-range="week"]')).toHaveAttribute('aria-pressed', 'true');
    await page.goto('tasks-dashboard.html#project=second');
    await page.locator('#activity-card').click();
    await expect(page.locator('#activity-panel [data-range="all"]')).toHaveAttribute('aria-pressed', 'true');
  });
});
