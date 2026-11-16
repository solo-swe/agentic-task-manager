import { addressOf, expect, follow, openDashboard, test } from './helpers';

// Task keys (ADR-007, T-546): a project in the keys format stores its tasks by UUID and shows them by key (ATM-10),
// a provisional key lighter with a "draft" mark; ?task= takes a key, an earlier prefix's key, an alias or the UUID,
// and the address then shows the key. A project from before task keys shows its IDs as it always has.

const SETUP = '0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8001';
const LOGIN = '0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8002';
const ONBOARDING = '0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8003';
const TIP = 'Provisional until merged into main: the number may change if another branch took it';

test.describe('task keys', () => {
  test('cards and list rows show keys, a provisional one with its draft mark', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'keys', { tab: 'kanban' });
    await expect(page.locator(`.kanban-card[data-id="${LOGIN}"] .kanban-card-id`)).toHaveText('ATM-10');
    const draft = page.locator(`.kanban-card[data-id="${ONBOARDING}"] .kanban-card-id .provisional`);
    await expect(draft).toHaveText('ATM-11draft');
    await expect(draft).toHaveAttribute('title', TIP);
    await expect(draft.locator('.draft-mark')).toHaveText('draft');
    await expect(page.locator(`.kanban-card[data-id="${LOGIN}"] .draft-mark`)).toHaveCount(0);

    await follow(page, `/projects/${tasksServer.projectId}/list`);
    await expect(page.locator(`.task-card[data-id="${SETUP}"] .task-id`)).toHaveText('ATM-2');
    await expect(page.locator('.task-card .task-id')).toHaveText(['ATM-2', 'ATM-10', /^ATM-11/]);
    // A dependency by the other task's key, another project's in short
    const deps = page.locator(`.task-card[data-id="${ONBOARDING}"] .dep-link`);
    await expect(deps.nth(0)).toHaveText('ATM-10');
    await expect(deps.nth(1)).toHaveText('3f2a9c1e:0193a5b2');
    await expect(deps.nth(1)).toHaveAttribute('title', "A task of another project (3f2a9c1e) that isn't loaded here");
  });

  test('the details show the key, and dependencies and related tasks by theirs', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'keys', { tab: 'list' });
    await page.locator(`.task-card[data-id="${LOGIN}"] .task-name`).click();
    await expect(page.locator('#detail-overlay.open')).toBeVisible();
    await expect(page.locator('#detail-key')).toHaveText('ATM-10');
    expect(addressOf(page)).toBe(`/projects/${tasksServer.projectId}/list?task=ATM-10`);
    await expect(page.locator('#detail-panel .detail-dep-item').first()).toContainText('ATM-2');
    await expect(page.locator('#detail-related .detail-dep-item')).toContainText('ATM-11draft');
    // The provisional task: its heading marked, the other project's dependency said to be not loaded here
    await page.locator('#detail-related .detail-dep-item').click();
    await expect(page.locator('#detail-key .provisional')).toHaveAttribute('title', TIP);
    expect(addressOf(page)).toBe(`/projects/${tasksServer.projectId}/list?task=ATM-11`);
    await expect(page.locator('#detail-panel .detail-dep-item.foreign')).toContainText('3f2a9c1e:0193a5b2');
  });

  test('?task= takes a key in any case, an earlier prefix, an alias or the UUID, and shows the key', async ({
    page,
    tasksServer,
  }) => {
    tasksServer.use('keys');
    // On load: an earlier prefix
    await page.goto(`/projects/${tasksServer.projectId}/list?task=T-10`);
    await expect(page.locator('#detail-overlay.open')).toBeVisible();
    await expect(page.locator('#detail-key')).toHaveText('ATM-10');
    await expect.poll(() => addressOf(page)).toBe(`/projects/${tasksServer.projectId}/list?task=ATM-10`);
    // Followed links: the UUID, an alias, a key in lower case
    for (const [ref, key] of [
      [SETUP, 'ATM-2'],
      ['ATM-9', 'ATM-11'],
      ['atm-10', 'ATM-10'],
    ]) {
      await follow(page, `/projects/${tasksServer.projectId}/list?task=${ref}`);
      await expect(page.locator('#detail-key')).toHaveText(key === 'ATM-11' ? 'ATM-11draft' : key);
      await expect.poll(() => addressOf(page)).toBe(`/projects/${tasksServer.projectId}/list?task=${key}`);
    }
  });

  test('a project from before task keys still shows its IDs', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard', { tab: 'list' });
    await expect(page.locator('.task-card[data-id="T-004"] .task-id')).toHaveText('T-004');
    await expect(page.locator('.draft-mark')).toHaveCount(0);
    await page.locator('.task-card[data-id="T-004"] .task-name').click();
    await expect(page.locator('#detail-key')).toHaveText('T-004');
    expect(new URL(page.url()).searchParams.get('task')).toBe('T-004');
  });
});
