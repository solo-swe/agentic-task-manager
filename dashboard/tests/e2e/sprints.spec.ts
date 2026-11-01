import type { Page } from '@playwright/test';
import { expect, filterButton, loadFixture, openDashboard, openFilters, test, type TasksData } from './helpers';

const sprintsOf = (data: TasksData) => data.meta!.sprints as { id: string; state: string }[];
const cardIds = (page: Page) =>
  page.locator('#kanban .kanban-card').evaluateAll((cards) => cards.map((c) => (c as HTMLElement).dataset.id));

// fixtures/sprints.json: "Ada Lovelace" is active (due 2026-04-12), "Foundations" closed, "Alan Turing" planned
const NOW = new Date('2026-04-09T12:00:00Z');

test.describe('scope bar (T-323)', () => {
  test.beforeEach(async ({ page }) => {
    await page.clock.setFixedTime(NOW);
  });

  test('the board opens on the active sprint, marked ▶, above the Kanban, List and Graph, not on the other tabs', async ({
    page,
    tasksServer,
  }) => {
    await openDashboard(page, tasksServer, 'sprints');
    const bar = page.locator('#scope-bar');
    await expect(bar).toHaveAttribute('data-state', 'active');
    await expect(bar.locator('#scope-trigger')).toHaveText('Ada Lovelace');
    await expect(bar.locator('#scope-trigger .sprint-marker.active')).toBeVisible();
    await expect(bar.locator('.sprint-goal')).toHaveText('Sign-in, end to end');
    await expect(bar.locator('.sprint-due')).toHaveText('due in 3 days');
    await expect(bar.locator('.sprint-release')).toHaveText('release 1.0');
    await expect(bar.locator('.sprint-progress')).toHaveText('0 of 3 done');
    await expect(bar.locator('.sprint-added')).toHaveText('1 added mid-sprint');
    await expect(bar.locator('.sprint-carried')).toHaveText('1 carried in');
    expect(await cardIds(page)).toEqual(['T-003', 'T-004', 'T-002']);
    for (const tab of ['list', 'graph']) {
      await page.locator(`#tab-${tab}`).click();
      await expect(bar).toBeVisible();
    }
    for (const tab of ['progress', 'agents', 'project']) {
      await page.locator(`#tab-${tab}`).click();
      await expect(bar).toHaveCount(0);
    }
    // The Progress tab shows every sprint whatever the board's scope
    await page.locator('#tab-progress').click();
    await expect(page.locator('#sprint-table tbody tr')).toHaveCount(3);
  });

  test('the menu picks one scope; the Sprint filter follows, and the other way round', async ({
    page,
    tasksServer,
  }) => {
    await openDashboard(page, tasksServer, 'sprints', { tab: 'list' });
    const bar = page.locator('#scope-bar');
    await page.locator('#scope-trigger').click();
    await expect(page.locator('.scope-choice')).toHaveText([
      /Ada Lovelace/,
      /Alan Turing/,
      /Backlog/,
      /All tasks/,
      /Foundations/,
    ]);
    await page.locator('.scope-choice[data-scope="Foundations"]').click();
    await expect(bar).toHaveAttribute('data-state', 'closed');
    await expect(bar.locator('.sprint-state')).toHaveText('closed');
    await expect(bar.locator('#scope-trigger .sprint-marker.closed')).toBeVisible();
    await expect(bar.locator('.sprint-due')).toHaveCount(0);
    await expect(page.locator('.filter-chip[data-filter="sprint"]')).toContainText('Foundations');

    // Two in the filters: "Filtered", with their names
    await openFilters(page);
    await filterButton(page, 'sprint', ':active').click();
    await expect(bar).toHaveAttribute('data-scope', 'several');
    await expect(bar.locator('#scope-trigger')).toHaveText('Filtered: 2 selected');
    await expect(bar.locator('.sprint-goal')).toHaveText('Foundations, Ada Lovelace');
    await expect(filterButton(page, 'sprint', ':active').locator('.sprint-marker.active')).toBeVisible();
    await expect(filterButton(page, 'sprint', 'Foundations').locator('.sprint-marker.closed')).toBeVisible();

    await page.locator('#scope-trigger').click();
    await page.locator('.scope-choice[data-scope="all"]').click();
    await expect(bar.locator('#scope-trigger')).toHaveText('All tasks');
    await expect(page.locator('#scope-hint')).toHaveText(/Ada Lovelace\s+is active/);
    await page.locator('#scope-show-active').click();
    await expect(bar.locator('#scope-trigger')).toHaveText('Ada Lovelace');
  });

  test('other filters narrow the board within the sprint: "showing N, filtered", and ✕ keeps the sprint', async ({
    page,
    tasksServer,
  }) => {
    await openDashboard(page, tasksServer, 'sprints');
    await page.locator('#task-search').fill('Sessions');
    await expect(page.locator('#scope-filtered')).toHaveText('showing 1, filtered');
    await expect(page.locator('#scope-bar .sprint-progress')).toHaveText('0 of 3 done');
    await page.locator('#scope-filtered button').click();
    await expect(page.locator('#task-search')).toHaveValue('');
    await expect(page.locator('#scope-trigger')).toHaveText('Ada Lovelace');
  });

  test('the sprint all done: close it, ship the release, or pull more from the backlog', async ({
    page,
    tasksServer,
  }) => {
    const data = loadFixture('sprints');
    for (const task of data.tasks) if (String(task.sprint).toLowerCase() === 'ada lovelace') task.status = 'done';
    await openDashboard(page, tasksServer, data);
    await expect(page.locator('#scope-done')).toHaveText('All 3 tasks done');
    await expect(page.locator('#scope-hint')).toHaveText(
      'Close it with /atm:close-sprint · release 1.0 is ready to ship',
    );
    await expect(page.locator('#scope-bar')).toContainText(
      'More work? Pull from the backlog (1 task) with /atm:plan-sprint',
    );
  });

  test('no active sprint: all tasks, the last closed and the next planned; once one starts, the board follows it', async ({
    page,
    tasksServer,
  }) => {
    const data = loadFixture('sprints');
    const ada = sprintsOf(data).find((s) => s.id === 'Ada Lovelace')!;
    Object.assign(ada, { state: 'closed', end: '2026-04-08T18:00:00Z' });
    await openDashboard(page, tasksServer, data);
    await expect(page.locator('#scope-trigger')).toHaveText('All tasks');
    await expect(page.locator('#scope-hint')).toHaveText(
      /^No active sprint\s·\slast: Ada Lovelace, closed .+\s·\snext planned: Alan Turing \(0 tasks\)\s·\sstart\s+it with \/atm:plan-sprint$/,
    );
    await expect(page.locator('.filter-chip[data-filter="sprint"]')).toHaveCount(0);

    // Alan Turing starts: the saved "active sprint" applies again
    const next = loadFixture('sprints');
    for (const s of sprintsOf(next)) s.state = s.id === 'Alan Turing' ? 'active' : 'closed';
    for (const task of next.tasks) if (task.id === 'T-005') task.sprint = 'Alan Turing';
    tasksServer.use(next);
    await page.locator('#reload-btn').click();
    await expect(page.locator('#scope-trigger')).toHaveText('Alan Turing');
    expect(await cardIds(page)).toEqual(['T-005']);
  });

  test('an overdue sprint says so, in red', async ({ page, tasksServer }) => {
    await page.clock.setFixedTime(new Date('2026-04-14T12:00:00Z'));
    await openDashboard(page, tasksServer, 'sprints');
    const due = page.locator('#scope-bar .sprint-due');
    await expect(due).toHaveText('2 days overdue');
    await expect(due).toHaveClass(/overdue/);
  });

  test('a project without sprints has no scope bar; Show skipped stays above the board', async ({
    page,
    tasksServer,
  }) => {
    const data = loadFixture('standard');
    for (const task of data.tasks) delete task.phase;
    delete data.meta?.phases;
    await openDashboard(page, tasksServer, data);
    await expect(page.locator('#kanban .kanban-card').first()).toBeVisible();
    await expect(page.locator('#scope-bar')).toHaveCount(0);
    await expect(page.locator('#kanban-show-skipped')).toBeVisible();
  });

  test('work together and review states on the cards; with: user needs you (SR-6.6)', async ({ page, tasksServer }) => {
    const data = loadFixture('sprints');
    for (const task of data.tasks) {
      if (task.id === 'T-003') task.with = 'user';
      if (task.id === 'T-004') Object.assign(task, { status: 'in_review', review_state: 'changes_requested' });
    }
    await openDashboard(page, tasksServer, data);
    const card = (id: string) => page.locator(`.kanban-card[data-id="${id}"]`);
    await expect(card('T-003').locator('.with-user-chip')).toHaveText('with you');
    await expect(card('T-004').locator('.review-state-chip')).toHaveText('Changes requested');
    // Back from review: an agent's work again, so it doesn't need you; T-003 does (one task: the chip opens it)
    await expect(page.locator('#attention-count')).toHaveText('1 needs you');
    await page.locator('#attention-count').click();
    await expect(page.locator('#detail-overlay.open #detail-with-user')).toHaveText('with you');
  });
  // Its screenshots are in visual.spec.ts, where CI generates the Linux baselines
});
