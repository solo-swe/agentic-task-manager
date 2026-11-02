import { expect, test } from './helpers';

// T-328: the title bar's Activity on the Projects Overview, every project's tasks together, by project
test.describe('Projects Overview: Activity across projects', () => {
  test.beforeEach(async ({ page, tasksServer }) => {
    tasksServer.use('agents');
    tasksServer.addProject('second', 'agents', 'Second Project');
    tasksServer.addProject('third', 'minimal', 'Third Project');
    await page.goto('tasks-dashboard.html#overview');
  });

  test('the chart sums every project; the popup has a row per project; a row opens it', async ({ page }) => {
    const card = page.locator('#activity-card');
    // The agents fixture finishes 6 tasks; twice over, plus none in the minimal one
    await expect(card).toHaveAttribute('aria-label', 'Activity: 12 tasks completed');
    await card.click();
    await expect(page.locator('#activity-panel')).toBeVisible();
    const rows = page.locator('#activity-projects .activity-project');
    await expect(rows.locator('.activity-project-name')).toHaveText([
      'Agents Project',
      'Second Project',
      'Third Project',
    ]);
    await expect(rows.locator('.activity-project-count')).toHaveText(['6 done', '6 done', '0 done']);
    // Where each project's time went; the minimal project has none
    await expect(rows.first().locator('.activity-project-split .time-split-working')).toHaveCount(1);
    await expect(rows.last()).toContainText('No work in this period');
    await rows.filter({ hasText: 'Second Project' }).click();
    await expect(page).toHaveURL(/project=second/);
  });

  test('coloured by project by default, by type as a switch (T-328 review)', async ({ page }) => {
    await page.locator('#activity-card').click();
    const legend = page.locator('#dive-completed .type-legend .type-chip');
    await expect(page.locator('#dive-completed h4')).toContainText('by project');
    await expect(legend).toHaveText([/Agents Project\s*6/, /Second Project\s*6/]);
    // Who was working, stacked by project too
    await expect(page.locator('#dive-agents .type-chip[data-kind="Agents Project"]')).toHaveCount(1);
    await page.locator('#activity-color-by [data-color-by="type"]').click();
    await expect(page.locator('#dive-completed h4')).toContainText('by type');
    await expect(legend.first()).not.toContainText('Project');
    await expect(page.locator('#dive-agents .type-chip[data-kind="agents"]')).toHaveCount(1);
  });

  test('its period is its own: a project’s Activity keeps the one it had', async ({ page }) => {
    await page.locator('#activity-card').click();
    await page.locator('#activity-panel [data-range="week"]').click();
    await expect(page.locator('#activity-panel [data-range="week"]')).toHaveAttribute('aria-pressed', 'true');
    await page.goto('tasks-dashboard.html#project=second');
    await page.locator('#activity-card').click();
    await expect(page.locator('#activity-panel [data-range="all"]')).toHaveAttribute('aria-pressed', 'true');
  });
});
