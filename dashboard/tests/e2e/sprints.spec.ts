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

  test('the sprint is in the URL: a link opens on it, a pick writes it, Back returns to the previous one', async ({
    page,
    tasksServer,
  }) => {
    tasksServer.use('sprints');
    await page.goto('tasks-dashboard.html#view=kanban&sprint=foundations');
    const trigger = page.locator('#scope-trigger');
    await expect(trigger).toHaveText('Foundations');
    // A closed sprint's board: its empty columns fold, Done takes the width
    await expect(page.locator('.kanban-column.folded')).toHaveCount(4);
    await expect(page.locator('.kanban-column[data-status="done"]')).toHaveClass(/wide/);

    await trigger.click();
    await page.locator('.scope-choice[data-scope=":backlog"]').click();
    await expect(page).toHaveURL(/sprint=backlog/);
    await page.goBack();
    await expect(trigger).toHaveText('Foundations');

    // An unknown name (a renamed sprint in an old link) is dropped: all tasks
    await page.goto('tasks-dashboard.html#view=kanban&sprint=Renamed');
    await expect(trigger).toHaveText('All tasks');
  });

  test('other filters narrow the board within the sprint: "showing N, filtered", and ✕ keeps the sprint', async ({
    page,
    tasksServer,
  }) => {
    await openDashboard(page, tasksServer, 'sprints');
    await page.locator('#task-search').fill('Sessions');
    await expect(page.locator('#scope-filtered')).toHaveText('showing 1, filtered: “Sessions”');
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

  test('the List grouped by sprint is the planning view: the active sprint, planned, backlog, closed folded', async ({
    page,
    tasksServer,
  }) => {
    const data = loadFixture('sprints');
    for (const task of data.tasks) if (task.id === 'T-005') task.labels = ['ux'];
    await openDashboard(page, tasksServer, data, { tab: 'list' });
    const groups = page.locator('.list-group');
    // One sprint: listed as it is, with no headers; the options sit in the scope bar
    await expect(page.locator('.list-group-header')).toHaveCount(0);
    await expect(page.locator('#scope-bar #list-group')).toHaveValue('none');

    await page.locator('#scope-trigger').click();
    await page.locator('.scope-choice[data-scope="all"]').click();
    await expect(page.locator('#list-group')).toHaveValue('sprint');
    expect(await groups.evaluateAll((all) => all.map((g) => (g as HTMLElement).dataset.group))).toEqual([
      'Ada Lovelace',
      ':backlog',
    ]);
    await expect(page.locator('.list-group-header').first()).toContainText('Ada Lovelace');
    await expect(page.locator('.list-group-header').first()).toContainText('3 tasks · 0 of 3 done');
    await expect(page.locator('.list-group[data-group=":backlog"] .list-group-hint')).toHaveText(
      'plan them into a sprint with /atm:plan-sprint',
    );
    // A group names the sprint, so its cards don't
    await expect(page.locator('.task-card-sprint')).toHaveCount(0);

    // The closed sprints fold into one row that opens them
    const toggle = page.locator('#list-closed-toggle');
    await expect(toggle).toContainText('Foundations · 1 task');
    await toggle.click();
    await expect(page.locator('.list-group[data-group="Foundations"]')).toBeVisible();

    // By label: the label's group, the rest under "No label"; cards name their sprint again
    await page.locator('#list-group').selectOption('label');
    await expect(page.locator('.list-group-name')).toHaveText(['ux', 'No label']);
    await expect(page.locator('.task-card[data-id="T-002"] .task-card-sprint')).toHaveText('Ada Lovelace');
    await page.reload();
    await expect(page.locator('#list-group')).toHaveValue('label');
  });

  test('Progress: the burndown opens on the active sprint, listed first; velocity per closed sprint; releases folded', async ({
    page,
    tasksServer,
  }) => {
    const data = loadFixture('sprints');
    const meta = data.meta as Record<string, unknown>;
    // Ada Lovelace (active) is listed after Alan Turing (planned), as when a sprint is started after others were planned
    const sprints = sprintsOf(data);
    meta.sprints = [
      { id: 'Zero', state: 'closed', end: '2026-03-30T18:00:00Z', report: { done: ['X-1', 'X-2'] } },
      sprints[0],
      sprints[2],
      sprints[1],
    ];
    data.tasks.push({
      id: 'T-006',
      name: 'Logo',
      phase: 0,
      type: 'polish',
      description: '',
      status: 'todo',
      sprint: 'Alan Turing',
    });
    meta.releases = [
      ...(meta.releases as object[]),
      { id: '0.7', state: 'released', released_at: '2026-03-01T00:00:00Z' },
      { id: '0.8', state: 'released', released_at: '2026-03-20T00:00:00Z' },
    ];
    await openDashboard(page, tasksServer, data, { tab: 'progress' });
    await expect(page.locator('#burndown-focus')).toHaveText('Ada Lovelace');
    const open = page.locator('#sprint-table tbody tr:not(#sprint-closed):not([data-sprint-state="closed"])');
    expect(await open.evaluateAll((rows) => rows.map((r) => (r as HTMLElement).dataset.sprint))).toEqual([
      'Ada Lovelace',
      'Alan Turing',
      ':backlog',
    ]);
    await page.getByRole('button', { name: 'Show all sprints' }).click();
    await expect(page.locator('#burndown-focus')).toHaveText('All sprints');

    // Velocity: Zero's report (2), Foundations' (1), oldest first
    await expect(page.locator('#velocity .velocity-column')).toHaveCount(2);
    await expect(page.locator('#velocity .velocity-value')).toHaveText(['2', '1']);
    await expect(page.locator('#velocity .velocity-sub')).toContainText('average 1.5');

    // The open releases, then the released ones in one row
    await expect(page.locator('.release-row').first()).toHaveAttribute('data-release', '1.0');
    await expect(page.locator('#release-released-toggle')).toContainText('✓ 2 released · last 0.8');
    await expect(page.locator('.release-row[data-state="released"]')).toHaveCount(0);
    await page.locator('#release-released-toggle').click();
    expect(
      await page
        .locator('.release-row[data-state="released"]')
        .evaluateAll((rows) => rows.map((r) => (r as HTMLElement).dataset.release)),
    ).toEqual(['0.8', '0.7']);
    // Recorded without tasks: no counts, no notes draft
    await expect(page.locator('.release-row[data-release="0.8"] .release-counts')).toHaveCount(0);
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
