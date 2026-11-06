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
});
