import type { Page } from '@playwright/test';
import { expect, test, type TasksServer } from './helpers';

// T-350: the Activity popup's periods (Hakan, on a Monday: Day, Week and Month drew one bar across the chart) and its
// hover tooltip staying inside the popup. On the Projects Overview, the same popup as a project's.
const NOW = new Date('2026-04-13T10:30:00Z'); // a Monday morning
const at = (days: number, hour: number) =>
  new Date(Date.UTC(2026, 3, 13 - days, hour, 0, 0)).toISOString().replace('.000Z', 'Z');
const done = (id: string, days: number, hour: number) => ({
  id,
  name: `Task ${id}`,
  type: 'feature',
  status: 'done',
  started_by: 'claude:a1',
  started_at: at(days, hour - 1),
  completed_at: at(days, hour),
});
// Finished this morning at 8 and 9, last Friday, and the Friday before
const project = (name: string, prefix: string) => ({
  meta: { project: name },
  tasks: [
    done(`${prefix}-1`, 0, 8),
    done(`${prefix}-2`, 0, 9),
    done(`${prefix}-3`, 3, 10),
    done(`${prefix}-4`, 10, 10),
  ],
});

async function openActivity(page: Page, tasksServer: TasksServer) {
  await page.clock.setFixedTime(NOW);
  tasksServer.use(project('Alpha', 'A'));
  tasksServer.addProject('beta', project('Beta', 'B'), 'Beta');
  await page.goto('tasks-dashboard.html#overview');
  await page.locator('#activity-card').click();
  await expect(page.locator('#activity-panel')).toBeVisible();
}

/** The charts' time axis (under the lowest, which they share): its first and last labels */
const axis = (page: Page) => page.locator('#dive-cycle .dive-axis span');
/** How many bars the completed chart has room for: its buckets, from the hover's slots */
const completedBars = (page: Page) =>
  page
    .locator('#dive-completed > svg rect[rx="1.5"]')
    .evaluateAll((rects) => new Set(rects.map((r) => r.getAttribute('x'))).size);

test.describe('Activity: the periods and the hover (T-350)', () => {
  test('a period running now spans the whole period: what is to come stays empty', async ({ page, tasksServer }) => {
    await openActivity(page, tasksServer);
    // This week, on a Monday: seven days on the axis, Monday's bar alone, not across the chart
    await page.locator('#activity-panel [data-range="week"]').click();
    await expect(axis(page)).toHaveText(['Apr 13', 'Apr 20']);
    await expect.poll(() => completedBars(page)).toBe(1);
    const bar = await page.locator('#dive-completed > svg rect[rx="1.5"]').first().boundingBox();
    const chart = await page.locator('#dive-completed > svg').boundingBox();
    expect(bar!.width).toBeLessThan(chart!.width / 6);
    // Today: the whole day's hours, the two finished this morning in two bars
    await page.locator('#activity-panel [data-range="day"]').click();
    await expect(axis(page)).toHaveText(['Apr 13 12:00 AM', 'Apr 14 12:00 AM']);
    await expect.poll(() => completedBars(page)).toBe(2);
    // The week before, whole as before: its Friday
    await page.locator('#activity-panel [data-range="week"]').click();
    await page.locator('#activity-prev').click();
    await expect(axis(page)).toHaveText(['Apr 6', 'Apr 13']);
    await expect.poll(() => completedBars(page)).toBe(1);
  });

  test('the hover’s tooltip stays inside the popup, beside the pointer, wherever it is', async ({
    page,
    tasksServer,
  }) => {
    // Tall enough for the whole popup: no scrolling inside it while the pointer moves from chart to chart
    await page.setViewportSize({ width: 1280, height: 1200 });
    await openActivity(page, tasksServer);
    await page.locator('#activity-panel [data-range="week"]').click();
    // Left (Monday's bar), middle and right of the first chart, and the right of the lowest
    // From chart to chart too: leaving one for another keeps the hover
    for (const [chart, fx, fy] of [
      ['#dive-completed', 0.08, 0.5],
      ['#dive-completed', 0.5, 0.5],
      ['#dive-completed', 0.95, 0.5],
      ['#dive-cycle', 0.95, 0.8],
    ] as const) {
      const svg = page.locator(`${chart} > svg`);
      const box = (await svg.boundingBox())!;
      await svg.hover({ position: { x: box.width * fx, y: box.height * fy } });
      const panel = (await page.locator('#activity-panel').boundingBox())!;
      const tip = (await page.locator('#dive-tooltip').boundingBox())!;
      expect(tip.x).toBeGreaterThanOrEqual(panel.x);
      expect(tip.x + tip.width).toBeLessThanOrEqual(panel.x + panel.width);
      expect(tip.y).toBeGreaterThanOrEqual(panel.y);
      expect(tip.y + tip.height).toBeLessThanOrEqual(panel.y + panel.height);
    }
  });
});
