import { expect, openDashboard, test } from './helpers';

// Claims come from the claims.json snapshot `atm` writes next to tasks.json. T-005 is the in-progress task of the
// standard fixture.
const minutesFromNow = (m: number) => new Date(Date.now() + m * 60_000).toISOString().replace(/\.\d+Z$/, 'Z');
const claim = (task: string, overrides: Record<string, unknown> = {}) => ({
  task,
  agent: 'claude:e5ea9db0',
  branch: 'agent-2',
  worktree: '/work/proj-2',
  claimed_at: minutesFromNow(-12),
  lease_expires_at: minutesFromNow(18),
  ...overrides,
});

test.describe('claims', () => {
  test('an in-progress task shows who works on it: card, Kanban card and details', async ({ page, tasksServer }) => {
    tasksServer.claims([claim('T-005')]);
    await openDashboard(page, tasksServer, 'standard', { tab: 'list' });
    await expect(page.locator('.task-card[data-id="T-005"] .task-claim')).toHaveText(
      'claude:e5ea9db0 · agent-2 · 12 m ago',
    );
    await expect(page.locator('.task-claim')).toHaveCount(1);

    await page.locator('#tab-kanban').click();
    await expect(page.locator('.kanban-card[data-id="T-005"] .kanban-claim')).toHaveText(
      'claude:e5ea9db0 · agent-2 · 12 m ago',
    );

    await page.locator('#tab-list').click();
    await page.locator('.task-card[data-id="T-005"] .task-name').click();
    await expect(page.locator('#detail-claim')).toContainText('claude:e5ea9db0 on agent-2');
    await expect(page.locator('#detail-claim')).toContainText('/work/proj-2');
  });

  test('expired claims, and claims on tasks that are not in progress, are not shown', async ({ page, tasksServer }) => {
    tasksServer.claims([claim('T-005', { lease_expires_at: minutesFromNow(-1) }), claim('T-006')]);
    // The List's cards (built only while it is shown, T-525)
    await openDashboard(page, tasksServer, 'standard', { tab: 'list' });
    await expect(page.locator('.task-card')).toHaveCount(12);
    await expect(page.locator('.task-claim')).toHaveCount(0);
  });

  test('without claims.json the dashboard works as before', async ({ page, tasksServer }) => {
    const errors: string[] = [];
    page.on('pageerror', (e) => errors.push(e.message));
    await openDashboard(page, tasksServer, 'standard', { tab: 'list' });
    await expect(page.locator('.task-card')).toHaveCount(12);
    await expect(page.locator('.task-claim')).toHaveCount(0);
    expect(errors).toEqual([]);
  });

  test('a reload picks up new claims', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard', { tab: 'list' });
    await expect(page.locator('.task-claim')).toHaveCount(0);
    tasksServer.claims([claim('T-005', { agent: 'bob@laptop:/src/proj', branch: null })]);
    await page.locator('#reload-btn').click();
    await expect(page.locator('.task-card[data-id="T-005"] .task-claim')).toHaveText('bob@laptop · 12 m ago');
  });
});
