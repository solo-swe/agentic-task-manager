import { expect, sparkline as bars, test } from './helpers';

// T-328: the title bar's Activity on the Projects Overview, every project's tasks together, by project
test.describe('Projects Overview: Activity across projects', () => {
  test.beforeEach(async ({ page, tasksServer }) => {
    tasksServer.use('agents');
    tasksServer.addProject('second', 'agents', 'Second Project');
    tasksServer.addProject('third', 'minimal', 'Third Project');
    await page.goto('/projects');
  });

  test('the chart sums every project; the popup is stacked and coloured by project', async ({ page }) => {
    const card = page.locator('#activity-card');
    // The agents fixture finishes 6 tasks; twice over, plus none in the minimal one
    await expect(card).toHaveAttribute('aria-label', 'Activity: 12 tasks completed');
    await card.click();
    await expect(page.locator('#dive-completed h4')).toContainText('by project');
    await expect(page.locator('#dive-completed .type-legend .type-chip')).toHaveText([
      /Agents Project\s*6/,
      /Second Project\s*6/,
    ]);
    // Who was working, stacked by project too; no switch to colour by type, no rows under the charts
    await expect(page.locator('#dive-agents .type-chip[data-kind="Agents Project"]')).toHaveCount(1);
    await expect(page.locator('#activity-color-by, #activity-projects')).toHaveCount(0);
  });

  test('the pill charts the task files, as a project’s does; the panel counts a project’s archived work too (T-355, ATM-739)', async ({
    page,
    tasksServer,
  }) => {
    // Two more finished tasks, archived, for the first project
    const archived = (id: string) => ({
      id,
      name: `Archived ${id}`,
      type: 'feature',
      status: 'done',
      started_by: 'claude:alice001',
      started_at: '2026-03-20T09:00:00Z',
      completed_at: '2026-03-20T09:30:00Z',
    });
    tasksServer.archive({ tasks: [archived('A-1'), archived('A-2')] });
    await page.reload();
    const card = page.locator('#activity-card');
    await expect(card).toHaveAttribute('aria-label', 'Activity: 12 tasks completed, archived tasks left out');
    await expect(card).toHaveAttribute('title', /all time: 12 \(archived tasks left out\)/);
    const before = await bars(page);
    await card.click();
    await expect(page.locator('#kpi-done .kpi-value')).toHaveText('14');
    expect(await bars(page)).toEqual(before);
    await page.keyboard.press('Escape');
    // Another of its tabs and back: the same bars
    await page.locator('#item-tasks').click();
    await expect(page.locator('#all-board')).toBeVisible();
    await page.locator('#item-overview').click();
    await expect(page.locator('#projects-grid')).toBeVisible();
    expect(await bars(page)).toEqual(before);
    await expect(card).toHaveAttribute('aria-label', 'Activity: 12 tasks completed, archived tasks left out');
  });

  test('its period is its own: a project’s Activity keeps the one it had', async ({ page }) => {
    await page.locator('#activity-card').click();
    await page.locator('#activity-panel [data-range="week"]').click();
    await expect(page.locator('#activity-panel [data-range="week"]')).toHaveAttribute('aria-pressed', 'true');
    await page.goto('/projects/second');
    await page.locator('#activity-card').click();
    await expect(page.locator('#activity-panel [data-range="all"]')).toHaveAttribute('aria-pressed', 'true');
    // A project's page colours by type, as before
    await expect(page.locator('#dive-completed h4')).toContainText('by type');
  });
});
