import { addressOf, expect, follow, loadFixture, openDashboard, test } from './helpers';

// Task keys (ADR-007, T-546; plain from creation, ADR-009): a project in the keys format stores its tasks by UUID and
// shows them by key (ATM-10), every key the same, no draft mark; a task a merge moved says the key it had ("was
// ATM-9"); ?task= takes a key, an earlier prefix's key, an alias or the UUID, and the address then shows the key. A
// project from before task keys shows its IDs as it always has.

const SETUP = '0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8001';
const LOGIN = '0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8002';
const ONBOARDING = '0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8003';
const FOREIGN = '3f2a9c1e:0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8f99';

test.describe('task keys', () => {
  test('the dependency graph shows every key plain: no draft mark, no DRAFT label', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'keys', { tab: 'graph' });
    await expect(page.locator('#dep-graph .graph-node')).toHaveCount(3);
    await expect(page.locator('#dep-graph [font-style="italic"], #dep-graph title')).toHaveCount(0);
    await expect(page.locator(`#dep-graph .graph-node[data-id="${ONBOARDING}"]`)).toContainText('ATM-11');
    await expect(page.locator('#dep-graph')).not.toContainText('DRAFT');
  });

  test('cards and list rows show keys, all plain (ADR-009)', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'keys', { tab: 'kanban' });
    await expect(page.locator(`.kanban-card[data-id="${LOGIN}"] .kanban-card-id`)).toHaveText('ATM-10');
    await expect(page.locator(`.kanban-card[data-id="${ONBOARDING}"] .kanban-card-id`)).toHaveText('ATM-11');
    await expect(page.locator('.provisional, .draft-info')).toHaveCount(0);

    await follow(page, `/projects/${tasksServer.projectId}/list`);
    await expect(page.locator(`.task-card[data-id="${SETUP}"] .task-id`)).toHaveText('ATM-2');
    await expect(page.locator('.task-card .task-id')).toHaveText(['ATM-2', 'ATM-10', 'ATM-11']);
    // A dependency by the other task's key, another project's in short
    const deps = page.locator(`.task-card[data-id="${ONBOARDING}"] .dep-link`);
    await expect(deps.nth(0)).toHaveText('ATM-10');
    await expect(deps.nth(1)).toHaveText('3f2a9c1e:0193a5b2');
    await expect(deps.nth(1)).toHaveAttribute(
      'title',
      "A task of 3f2a9c1e: its project isn't in your workspaces (or is gone)",
    );
  });

  test('the details show the key, and dependencies and related tasks by theirs', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'keys', { tab: 'list' });
    await page.locator(`.task-card[data-id="${LOGIN}"] .task-name`).click();
    await expect(page.locator('#detail-overlay.open')).toBeVisible();
    await expect(page.locator('#detail-key')).toHaveText('ATM-10');
    expect(addressOf(page)).toBe(`/projects/${tasksServer.projectId}/list?task=ATM-10`);
    await expect(page.locator('#detail-panel .detail-dep-item').first()).toContainText('ATM-2');
    await expect(page.locator('#detail-related .detail-dep-item')).toContainText('ATM-11');
    // LOGIN's number never moved: nothing said
    await expect(page.locator('#detail-moved')).toHaveCount(0);
    // The task a merge moved (ATM-9 became ATM-11): its heading plain, the key it had said (not its old ID, T-P1-03),
    // the other project's dependency said to be not loaded here
    await page.locator('#detail-related .detail-dep-item').click();
    await expect(page.locator('#detail-key')).toHaveText('ATM-11');
    await expect(page.locator('#detail-key .provisional')).toHaveCount(0);
    await expect(page.locator('#detail-moved')).toHaveText('was ATM-9');
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
    // Followed links: the UUID, an alias (the key a merge moved it off), a key in lower case, one with an old ~
    for (const [ref, key] of [
      [SETUP, 'ATM-2'],
      ['ATM-9', 'ATM-11'],
      ['atm-10', 'ATM-10'],
      ['~ATM-10', 'ATM-10'],
    ]) {
      await follow(page, `/projects/${tasksServer.projectId}/list?task=${ref}`);
      await expect(page.locator('#detail-key')).toHaveText(key === 'ATM-11' ? 'ATM-11' : key);
      await expect.poll(() => addressOf(page)).toBe(`/projects/${tasksServer.projectId}/list?task=${key}`);
    }
  });

  test('another project’s task: its key, name and status from atm, with only this project open; it opens there (T-549)', async ({
    page,
    tasksServer,
  }) => {
    // atm finds the other project registered; the page lists it (so its page opens) but doesn't load its tasks
    tasksServer.registerForAtm('keys-other');
    tasksServer.addProject('3f2a9c1e', 'keys-other', 'Learn Forever Blog');
    await openDashboard(page, tasksServer, 'keys', { tab: 'list' });
    await page.locator(`.task-card[data-id="${ONBOARDING}"] .task-name`).click();
    const other = page.locator(`#detail-panel a.detail-dep-item.foreign[data-foreign="${FOREIGN}"]`);
    await expect(other).toContainText('LFB-5');
    await expect(other.locator('.provisional')).toHaveCount(0);
    await expect(other.locator('.detail-dep-name')).toHaveText('Write the post about the onboarding');
    await expect(other.locator('.badge')).toHaveText('in progress');
    await expect(other).toHaveAttribute('href', '/projects/3f2a9c1e?task=LFB-5');
    await expect(other).toHaveAttribute('title', /\(a task of Learn Forever Blog\)/);
    // The list's dependency link by its key too
    await expect(page.locator(`.task-card[data-id="${ONBOARDING}"] .dep-link`).nth(1)).toHaveText('LFB-5');
    // A click opens it in its project, its details open
    await other.click();
    await expect(page.locator('#dashboard-title')).toHaveText('Learn Forever Blog');
    await expect(page.locator('#detail-key')).toHaveText('LFB-5');
    // On its project's own tab
    await expect.poll(() => addressOf(page)).toMatch(/^\/projects\/3f2a9c1e(\/[a-z]+)?\?task=LFB-5$/);
  });

  test('another project’s task atm can’t find says why, without a link: no such task there, or no such project', async ({
    page,
    tasksServer,
  }) => {
    tasksServer.registerForAtm('keys-other');
    const data = loadFixture('keys');
    const gone = '3f2a9c1e:0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8f00';
    const stranger = 'dddd4444:0193a5b2-7c1d-7e2f-8a3b-4c5d6e7f8f01';
    data.tasks[1].related = [gone, stranger];
    await openDashboard(page, tasksServer, data, { tab: 'list' });
    await page.locator(`.task-card[data-id="${LOGIN}"] .task-name`).click();
    const missing = page.locator(`#detail-related .detail-dep-item.foreign[data-foreign="${gone}"]`);
    await expect(missing).toContainText('Learn Forever Blog:0193a5b2');
    await expect(missing.locator('.detail-foreign-missing')).toHaveText('· no such task in Learn Forever Blog');
    const unknown = page.locator(`#detail-related .detail-dep-item.foreign[data-foreign="${stranger}"]`);
    await expect(unknown).toContainText('dddd4444:0193a5b2');
    await expect(unknown.locator('.detail-foreign-missing')).toHaveText(
      "· its project isn't in your workspaces (or is gone)",
    );
    await expect(page.locator('#detail-related a')).toHaveCount(0);
  });

  test('in the app, another project’s task opens in a new tab (T-549)', async ({ page, tasksServer }) => {
    await tasksServer.asApp();
    tasksServer.registerForAtm('keys-other');
    tasksServer.addProject('3f2a9c1e', 'keys-other', 'Learn Forever Blog');
    tasksServer.use('keys');
    await page.goto(`/projects/${tasksServer.projectId}/list?task=ATM-11`);
    const tabs = page.locator('#app-titlebar [role="tab"]');
    await expect(tabs).toHaveCount(1);
    await page.locator(`#detail-panel a.detail-dep-item.foreign[data-foreign="${FOREIGN}"]`).click();
    await expect(tabs).toHaveCount(2);
    // Its tab is its project's (ATM-754): listed here without a summary, so by its name, cut to 16 characters
    await expect(page.locator('#dashboard-title')).toHaveText(/^Learn Forever B… - /);
    await expect.poll(() => addressOf(page)).toMatch(/^\/projects\/3f2a9c1e(\/[a-z]+)?\?task=LFB-5$/);
    await expect(page.locator('#detail-key')).toHaveText('LFB-5');
  });

  test('the overview’s cards and the All projects board show keys, all plain (T-549, ADR-009)', async ({
    page,
    tasksServer,
  }) => {
    tasksServer.use('keys');
    tasksServer.withAtmSummary();
    await page.goto('/projects');
    const card = page.locator(`.project-card[data-id="${tasksServer.projectId}"]`);
    await expect(card.locator('.project-card-work-line .project-card-work-id')).toHaveText('ATM-10');
    await expect(card.locator('.project-card-last .project-card-work-id')).toHaveText('ATM-2');
    await expect(card.locator('.draft-info')).toHaveCount(0);

    await follow(page, '/board');
    // No sprint is active: the board of all tasks
    await page.getByRole('button', { name: 'All tasks' }).click();
    const board = page.locator('#all-board');
    await expect(board.locator(`.board-card[data-id="${LOGIN}"] .kanban-card-id`)).toHaveText('ATM-10');
    const moved = board.locator(`.board-card[data-id="${ONBOARDING}"]`);
    await expect(moved.locator('.kanban-card-id')).toHaveText('ATM-11');
    await expect(board.locator('.provisional, .draft-info')).toHaveCount(0);
    await expect(moved.locator('.board-card-status')).toHaveText('Waits on ATM-10, 3f2a9c1e:0193a5b2');
  });

  test('a project from before task keys still shows its IDs', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard', { tab: 'list' });
    await expect(page.locator('.task-card[data-id="T-004"] .task-id')).toHaveText('T-004');
    await expect(page.locator('.draft-info')).toHaveCount(0);
    await page.locator('.task-card[data-id="T-004"] .task-name').click();
    await expect(page.locator('#detail-key')).toHaveText('T-004');
    expect(new URL(page.url()).searchParams.get('task')).toBe('T-004');
  });
});
