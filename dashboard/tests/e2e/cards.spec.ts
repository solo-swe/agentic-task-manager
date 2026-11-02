import { expect, loadFixture, matchCount, openDashboard, test, visibleCardIds } from './helpers';

const card = (page: import('@playwright/test').Page, id: string) => page.locator(`.task-card[data-id="${id}"]`);

// The cards are the List tab
test.describe('task cards', () => {
  test('one card per task, in file order', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard', { tab: 'list' });
    // One list, not a group per sprint (T-323)
    await page.locator('#list-group').selectOption('none');
    expect(await visibleCardIds(page)).toEqual([
      'T-001',
      'T-002',
      'T-003',
      'T-004',
      'T-005',
      'T-006',
      'T-007',
      'T-008',
      'T-009',
      'T-010',
      'T-011',
      'T-012',
    ]);
    await expect(matchCount(page)).toHaveText('12 of 12 tasks');
  });

  test('card content: id, name, badges, dependencies and tags', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard', { tab: 'list' });
    const c = card(page, 'T-004');
    await expect(c).toHaveAttribute('id', 'task-T-004');
    await expect(c).toHaveClass(/status-done/);
    await expect(c.locator('.task-id')).toHaveText('T-004');
    await expect(c.locator('.task-name')).toHaveText('User login');
    // No phase badge (T-323): grouped by sprint, the group names it
    await expect(c.locator('.task-meta .badge')).toHaveText(['feature', 'Done', 'P2']);
    await expect(c.locator('.deps-label')).toHaveText('needs:');
    await expect(c.locator('.dep-link')).toHaveText(['T-002', 'T-003']);
    await expect(c.locator('.dep-link').first()).toHaveAttribute('title', 'Write contributing guide');
    await expect(c.locator('.uc-badge')).toHaveText(['UC-001']);
    await expect(c.locator('.req-badge')).toHaveText(['FR-1.1', 'FR-1.2']);
  });

  test('status classes (status and kind) and labels', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard', { tab: 'list' });
    const expected = {
      'T-005': ['in_progress', 'active', 'In Progress'],
      'T-006': ['todo', 'todo', 'To Do'],
      'T-007': ['blocked', 'blocked', 'Blocked'],
      'T-010': ['skipped', 'skipped', 'Skipped'],
    };
    for (const [id, [status, kind, label]] of Object.entries(expected)) {
      await expect(card(page, id)).toHaveClass(new RegExp(`status-${status} kind-${kind}`));
      await expect(card(page, id).locator('.task-meta .badge').nth(1)).toHaveText(label);
    }
  });

  test("a project's own statuses: its labels, styled by kind", async ({ page, tasksServer }) => {
    const data = loadFixture('standard');
    data.meta = {
      ...data.meta,
      statuses: [
        { id: 'todo', label: 'Backlog', kind: 'todo' },
        { id: 'in_progress', label: 'Doing', kind: 'active' },
        { id: 'awaiting_user', label: 'Awaiting you', kind: 'review' },
        { id: 'done', label: 'Shipped', kind: 'done' },
      ],
    };
    data.tasks.find((t) => t.id === 'T-005')!.status = 'awaiting_user';
    await openDashboard(page, tasksServer, data, { tab: 'list' });
    await expect(card(page, 'T-005')).toHaveClass(/kind-review/);
    await expect(card(page, 'T-005').locator('.task-meta .badge').nth(1)).toHaveText('Awaiting you');
    await expect(card(page, 'T-004').locator('.task-meta .badge').nth(1)).toHaveText('Shipped');
  });

  test('tasks without dependencies have no "needs:" label', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard', { tab: 'list' });
    await expect(card(page, 'T-001').locator('.deps-label')).toHaveCount(0);
    await expect(card(page, 'T-001').locator('.task-tags')).toBeEmpty();
  });

  test('dependency link scrolls to and briefly highlights the target', async ({ page, tasksServer }) => {
    // Make the last card depend on the first so the link has to scroll the page
    const data = loadFixture('large-90');
    data.tasks[89].dependencies = ['T-001'];
    // The page's clock, to skip the highlight's 2 s (T-345): the tests reduce motion, so the scroll is instant
    await page.clock.install();
    await openDashboard(page, tasksServer, data, { tab: 'list' });

    const target = card(page, 'T-001');
    await card(page, 'T-090').scrollIntoViewIfNeeded();
    await expect(target).not.toBeInViewport();

    await card(page, 'T-090').locator('.dep-link', { hasText: 'T-001' }).click();
    await expect(target).toBeInViewport();
    await expect(target).toHaveAttribute('style', /var\(--yellow\)/);
    await page.clock.fastForward(2000);
    await expect(target).not.toHaveAttribute('style', /var\(--yellow\)/);
  });
});
