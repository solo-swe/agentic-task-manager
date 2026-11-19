// The title bar's "N need(s) you" chip takes you to the task: one opens its details, several open a list of them.
import type { Page } from '@playwright/test';
import { expect, loadFixture, openDashboard, test, type TasksData } from './helpers';

/** fixtures/v4.json (T-004 in review, T-005 blocked on the user, T-006 on an agent), with T-005 and T-006 waiting on
 * whom `on` says. */
function v4(on: { 'T-005': string; 'T-006': string }): TasksData {
  const data = loadFixture('v4');
  data.tasks = data.tasks.map((t) =>
    t.id === 'T-005' || t.id === 'T-006' ? { ...t, waiting_on: on[t.id as 'T-005' | 'T-006'] } : t,
  );
  return data;
}

/** The detail panel is open on task `id`. */
async function expectDetail(page: Page, id: string) {
  await expect(page.locator('#detail-overlay.open #detail-panel h3 > span').first()).toHaveText(id);
}

test.describe('"need you" chip', () => {
  test('one task needs you: the chip opens its details', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, v4({ 'T-005': 'agent', 'T-006': 'agent' }), { tab: 'list' });
    const chip = page.locator('#attention-count');
    await expect(chip).toHaveText('1 needs you');
    await chip.click();
    await expect(page.locator('#needs-you-menu')).toHaveCount(0);
    await expectDetail(page, 'T-004');
  });

  test('several: a list under the chip; a row opens that task', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, v4({ 'T-005': 'user', 'T-006': 'user' }), { tab: 'list' });
    const chip = page.locator('#attention-count');
    await expect(chip).toHaveText('3 need you');
    await chip.click();
    const rows = page.locator('#needs-you-menu .needs-you-row');
    await expect(rows).toHaveCount(3);
    await expect(rows.locator('.needs-you-id')).toHaveText(['T-004', 'T-005', 'T-006']);
    await expect(rows.locator('.needs-you-reason')).toHaveText([
      'in review',
      'blocked on a question',
      'blocked on a question',
    ]);
    await rows.nth(2).click();
    await expect(page.locator('#needs-you-menu')).toHaveCount(0);
    await expectDetail(page, 'T-006');
  });

  test('keyboard: arrows move, Enter opens; Escape and a click outside close', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, v4({ 'T-005': 'user', 'T-006': 'user' }), { tab: 'list' });
    const chip = page.locator('#attention-count');
    await chip.focus();
    await page.keyboard.press('Enter');
    const rows = page.locator('#needs-you-menu .needs-you-row');
    await expect(rows.first()).toBeFocused();
    await page.keyboard.press('Escape');
    await expect(page.locator('#needs-you-menu')).toHaveCount(0);
    await expect(chip).toBeFocused();
    await expect(page.locator('#detail-overlay.open')).toHaveCount(0);

    await chip.click();
    await page.locator('#dashboard-title').click();
    await expect(page.locator('#needs-you-menu')).toHaveCount(0);

    // The first row has the focus once the menu has opened (Bits UI's menu, T-431)
    await chip.click();
    await expect(rows.first()).toBeFocused();
    await page.keyboard.press('ArrowDown');
    await expect(rows.nth(1)).toBeFocused();
    await page.keyboard.press('Enter');
    await expectDetail(page, 'T-005');
  });

  test('a long list scrolls in a small popup, without a scroll bar: "▼ N more", the arrows reach every row (ATM-640)', async ({
    page,
    tasksServer,
  }) => {
    const data = v4({ 'T-005': 'user', 'T-006': 'user' });
    const inReview = data.tasks.find((t) => t.id === 'T-004')!;
    for (let n = 100; n < 120; n++)
      data.tasks.push({ ...inReview, id: `T-${n}`, name: `Review ${n}`, dependencies: [] });
    await openDashboard(page, tasksServer, data, { tab: 'list' });
    const chip = page.locator('#attention-count');
    await expect(chip).toHaveText('23 need you');
    await chip.click();
    const menu = page.locator('#needs-you-menu');
    const list = menu.locator('.needs-you-list');
    const rows = menu.locator('.needs-you-row');
    await expect(rows).toHaveCount(23);
    // Small: well under the window, the list cut off, with no scroll bar of its own
    const box = (await menu.boundingBox())!;
    expect(box.height).toBeLessThan(400);
    expect(box.y + box.height).toBeLessThan(page.viewportSize()!.height);
    const scroller = await list.evaluate((el: HTMLElement) => ({
      cut: el.scrollHeight > el.clientHeight,
      bar: el.offsetWidth - el.clientWidth,
      overflow: getComputedStyle(el).overflowY,
    }));
    expect(scroller).toEqual({ cut: true, bar: 0, overflow: 'auto' });
    await expect(menu.locator('.kanban-more.bottom')).toHaveText(/^▼ \d+ more$/);
    await expect(menu.locator('.kanban-more.top')).toHaveCount(0);

    // The arrows go to the last row, scrolled into view; "▲ N more" then, and no "▼"
    await expect(rows.first()).toBeFocused();
    await page.keyboard.press('End');
    await expect(rows.last()).toBeFocused();
    await expect(rows.last()).toBeInViewport();
    await expect(menu.locator('.kanban-more.top')).toHaveText(/^▲ \d+ more$/);
    await expect(menu.locator('.kanban-more.bottom')).toHaveCount(0);
    await page.keyboard.press('Enter');
    await expectDetail(page, 'T-119');
  });

  test('a short list has no hints', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, v4({ 'T-005': 'user', 'T-006': 'user' }), { tab: 'list' });
    await page.locator('#attention-count').click();
    await expect(page.locator('#needs-you-menu .needs-you-row')).toHaveCount(3);
    await expect(page.locator('#needs-you-menu .kanban-more')).toHaveCount(0);
  });
});
