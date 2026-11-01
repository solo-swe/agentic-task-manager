import type { Page } from '@playwright/test';
import { expect, filterButton, loadFixture, openDashboard, openFilters, test } from './helpers';

const column = (page: Page, status: string) => page.locator(`.kanban-column[data-status="${status}"]`);
const cardIds = (page: Page, status: string) =>
  column(page, status)
    .locator('.kanban-card')
    .evaluateAll((cards) => cards.map((c) => (c as HTMLElement).dataset.id));
const card = (page: Page, id: string) => page.locator(`.kanban-card[data-id="${id}"]`);

async function openKanban(
  page: Page,
  tasksServer: Parameters<typeof openDashboard>[1],
  data: Parameters<typeof openDashboard>[2],
) {
  // Next up is atm's answer, from the project's summary in api/projects (T-321)
  await openDashboard(page, tasksServer.withAtmSummary(), data);
  await page.locator('#tab-kanban').click();
  await expect(page.locator('#kanban')).toBeVisible();
}

test.describe('Kanban board', () => {
  test('status columns with counts; Other only for unknown statuses', async ({ page, tasksServer }) => {
    await openKanban(page, tasksServer, 'kanban');
    const columns = await page
      .locator('.kanban-column')
      .evaluateAll((cols) => cols.map((c) => (c as HTMLElement).dataset.status));
    expect(columns).toEqual(['todo', 'in_progress', 'in_review', 'blocked', 'done', 'other']);
    await expect(page.locator('.kanban-column-title')).toHaveText([
      'To Do',
      'In Progress',
      'In Review',
      'Blocked',
      'Done',
      'Other',
    ]);
    await expect(page.locator('.kanban-count')).toHaveText(['6', '3', '0', '2', '24', '1']);
  });

  test('cards are ordered like the loop works', async ({ page, tasksServer }) => {
    await openKanban(page, tasksServer, 'kanban');
    // Todo: ready first (by priority, then ID; phases no longer count), then waiting
    expect(await cardIds(page, 'todo')).toEqual(['T-028', 'T-029', 'T-032', 'T-030', 'T-031', 'T-033']);
    // In progress: most recently started first
    expect(await cardIds(page, 'in_progress')).toEqual(['T-027', 'T-025', 'T-026']);
    // Done: most recently completed first
    expect((await cardIds(page, 'done')).slice(0, 3)).toEqual(['T-024', 'T-023', 'T-022']);
    expect(await cardIds(page, 'blocked')).toEqual(['T-035', 'T-034']);
  });

  test("a project's git worktrees on one board: each task where it got furthest, with its branch", async ({
    page,
    tasksServer,
  }) => {
    const tasks = loadFixture('kanban').tasks;
    const copy = (id: string, fields: Record<string, unknown>) => ({ ...tasks.find((t) => t.id === id)!, ...fields });
    tasksServer.worktrees([
      { path: '/projects/fixture', branch: 'main', main: true },
      {
        path: '/projects/fixture-2',
        branch: 'agent-2',
        main: false,
        tasks: [
          copy('T-029', { status: 'in_progress', started_at: '2026-01-02T00:00:00Z' }),
          copy('T-027', { status: 'done', completed_at: '2026-04-05T00:00:00Z' }), // the newest, so the first shown
          copy('T-024', { status: 'todo' }), // behind the main worktree: not shown
          { id: 'T-099', name: 'Only on the branch', phase: 1, type: 'feature', description: '', status: 'todo' },
        ],
      },
    ]);
    await openKanban(page, tasksServer, 'kanban');
    await expect(column(page, 'in_progress').locator('.kanban-card[data-id="T-029"]')).toBeVisible();
    await expect(column(page, 'done').locator('.kanban-card[data-id="T-027"]')).toBeVisible();
    await expect(column(page, 'todo').locator('.kanban-card[data-id="T-099"]')).toBeVisible();
    for (const id of ['T-029', 'T-027', 'T-099']) {
      await expect(card(page, id).locator('.branch-chip')).toHaveText('agent-2');
    }
    await expect(card(page, 'T-024').locator('.branch-chip')).toHaveCount(0);
    await expect(column(page, 'done').locator('.kanban-card[data-id="T-024"]')).toBeVisible();
    await card(page, 'T-027').click();
    await expect(page.locator('#detail-overlay.open .branch-chip')).toHaveText('agent-2');
    await page.keyboard.press('Escape');

    // The branch merged and its worktree was removed: the board shows the main worktree's tasks again
    tasksServer.worktrees([{ path: '/projects/fixture', branch: 'main', main: true }]);
    await page.locator('#reload-btn').click();
    await expect(page.locator('.branch-chip')).toHaveCount(0);
    await expect(card(page, 'T-099')).toHaveCount(0);
    await expect(column(page, 'todo').locator('.kanban-card[data-id="T-029"]')).toBeVisible();
  });

  test('todo cards show readiness', async ({ page, tasksServer }) => {
    await openKanban(page, tasksServer, 'kanban');
    await expect(card(page, 'T-029').locator('.kanban-readiness')).toHaveText('Ready');
    await expect(card(page, 'T-030').locator('.kanban-readiness')).toHaveText('Waiting on T-025, T-026');
    await expect(card(page, 'T-031').locator('.kanban-readiness')).toHaveText('Waiting on T-999');
    await expect(card(page, 'T-025').locator('.kanban-readiness')).toHaveCount(0);
  });

  test('Next up marks the task the loop would pick, and moves when it is done', async ({ page, tasksServer }) => {
    await openKanban(page, tasksServer, 'kanban');
    await expect(page.locator('.kanban-card.next-up')).toHaveCount(1);
    await expect(card(page, 'T-028')).toHaveClass(/next-up/);
    await expect(card(page, 'T-028').locator('.kanban-next-up')).toHaveText('Next up');

    const data = loadFixture('kanban');
    data.tasks.find((t) => t.id === 'T-028')!.status = 'done';
    tasksServer.use(data);
    await page.locator('#reload-btn').click();
    await expect(card(page, 'T-029')).toHaveClass(/next-up/);
    await expect(page.locator('.kanban-card.next-up')).toHaveCount(1);
  });

  test('no Next up without one from atm (a summary without next)', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'kanban');
    await page.locator('#tab-kanban').click();
    await expect(card(page, 'T-028')).toBeVisible();
    await expect(page.locator('.kanban-card.next-up')).toHaveCount(0);
  });

  test("a project's own statuses make the columns, by kind", async ({ page, tasksServer }) => {
    const data = loadFixture('kanban');
    data.meta = {
      ...data.meta,
      statuses: [
        { id: 'todo', label: 'Backlog', kind: 'todo' },
        { id: 'in_progress', label: 'Doing', kind: 'active' },
        { id: 'awaiting_user', label: 'Awaiting you', kind: 'review' },
        { id: 'done', label: 'Shipped', kind: 'done' },
      ],
    };
    data.tasks.find((t) => t.id === 'T-025')!.status = 'awaiting_user';
    await openKanban(page, tasksServer, data);
    await expect(page.locator('.kanban-column-title')).toHaveText([
      'Backlog',
      'Doing',
      'Awaiting you',
      'Shipped',
      'Other',
    ]);
    expect(await cardIds(page, 'awaiting_user')).toEqual(['T-025']);
    // blocked is no longer one of its statuses: those tasks show under Other
    expect(await cardIds(page, 'other')).toContain('T-035');
  });

  test('In Review: its own column; cards and the detail panel say how long it has waited', async ({
    page,
    tasksServer,
  }) => {
    await page.clock.setFixedTime(new Date('2026-04-03T13:30:00Z'));
    const data = loadFixture('kanban');
    Object.assign(
      data.tasks.find((t) => t.id === 'T-025')!,
      {
        status: 'in_review',
        review_requested_at: '2026-04-03T11:00:00Z',
        notes: [{ at: '2026-04-03T11:00:00Z', kind: 'review', text: 'Check the column order' }],
      },
    );
    await openKanban(page, tasksServer, data);
    expect(await cardIds(page, 'in_review')).toEqual(['T-025']);
    await expect(card(page, 'T-025').locator('.review-wait')).toHaveText('Waiting for review · 2 h 30 m');
    await card(page, 'T-025').click();
    await expect(page.locator('#detail-review')).toContainText('Waiting for review · 2 h 30 m');
    await expect(page.locator('#detail-review .review-note')).toHaveText('Check the column order');
  });

  test('Next up for the standard fixture', async ({ page, tasksServer }) => {
    await openKanban(page, tasksServer, 'standard');
    await expect(page.locator('.kanban-card.next-up')).toHaveAttribute('data-id', 'T-006');
  });

  test('clicking a card or pressing Enter on it opens its details', async ({ page, tasksServer }) => {
    await openKanban(page, tasksServer, 'kanban');
    await card(page, 'T-030').click();
    await expect(page.locator('#detail-panel h3')).toHaveText('T-030 Waiting on work in progress');
    await page.keyboard.press('Escape');
    await card(page, 'T-025').focus();
    await page.keyboard.press('Enter');
    await expect(page.locator('#detail-panel h3')).toHaveText('T-025 Build the board');
  });

  test('skipped tasks are hidden behind a toggle', async ({ page, tasksServer }) => {
    await openKanban(page, tasksServer, 'kanban');
    await expect(column(page, 'skipped')).toHaveCount(0);
    await page.locator('#kanban-show-skipped').check();
    await expect(column(page, 'skipped').locator('.kanban-card')).toHaveCount(2);
  });

  test('Done shows the 20 most recent tasks until "Show all"', async ({ page, tasksServer }) => {
    await openKanban(page, tasksServer, 'kanban');
    await expect(column(page, 'done').locator('.kanban-card')).toHaveCount(20);
    await column(page, 'done').getByRole('button', { name: 'Show all (24)' }).click();
    await expect(column(page, 'done').locator('.kanban-card')).toHaveCount(24);
    await expect(column(page, 'done').getByRole('button', { name: /Show all/ })).toHaveCount(0);
    await column(page, 'done').getByRole('button', { name: 'Show the latest 20' }).click();
    await expect(column(page, 'done').locator('.kanban-card')).toHaveCount(20);
  });

  test('columns reach down to the status bar and scroll inside; the page itself never scrolls (T-237, T-262)', async ({
    page,
    tasksServer,
  }) => {
    for (const height of [1600, 720]) {
      await page.setViewportSize({ width: 1280, height });
      await openKanban(page, tasksServer, 'kanban');
      const done = column(page, 'done');
      // "Show all" is remembered: only the first round has the button
      const showAll = done.getByRole('button', { name: /Show all/ });
      if (await showAll.count()) await showAll.click();
      // One scrollbar at most: the page is exactly the window's height
      expect(await page.evaluate(() => document.documentElement.scrollHeight)).toBe(height);
      const footer = (await page.locator('footer').boundingBox())!;
      const box = (await done.boundingBox())!;
      expect(footer.y - (box.y + box.height)).toBeGreaterThanOrEqual(0);
      expect(footer.y - (box.y + box.height)).toBeLessThanOrEqual(40);
      const cards = done.locator('.kanban-cards');
      expect(await cards.evaluate((el) => el.scrollHeight > el.clientHeight)).toBe(true);
    }
  });

  test('a long column hides its scrollbar and says how many cards are above and below (T-262)', async ({
    page,
    tasksServer,
  }) => {
    await page.setViewportSize({ width: 1280, height: 720 });
    await openKanban(page, tasksServer, 'kanban');
    const done = column(page, 'done');
    await done.getByRole('button', { name: /Show all/ }).click();
    const cards = done.locator('.kanban-cards');
    // Reaching the button scrolled the column down: back to the top
    await cards.evaluate((el) => (el.scrollTop = 0));
    // No scrollbar takes room: the scroller is as wide inside as it is outside
    expect(await cards.evaluate((el) => (el as HTMLElement).offsetWidth - el.clientWidth)).toBe(0);
    await expect(done.locator('.kanban-more.top')).toHaveCount(0);
    await expect(done.locator('.kanban-more.bottom')).toHaveText(/^▼ \d+ more$/);

    // The wheel scrolls the column, not the page
    const box = (await cards.boundingBox())!;
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await page.mouse.wheel(0, 400);
    await expect.poll(() => cards.evaluate((el) => el.scrollTop)).toBeGreaterThan(0);
    expect(await page.evaluate(() => scrollY)).toBe(0);
    await expect(done.locator('.kanban-more.top')).toHaveText(/^▲ \d+ more$/);

    // At the end, nothing below; a hint scrolls a page back up
    await cards.evaluate((el) => (el.scrollTop = el.scrollHeight));
    await expect(done.locator('.kanban-more.bottom')).toHaveCount(0);
    const before = await cards.evaluate((el) => el.scrollTop);
    await done.locator('.kanban-more.top').click();
    await expect.poll(() => cards.evaluate((el) => el.scrollTop)).toBeLessThan(before);

    // A short column shows neither
    await expect(column(page, 'in_progress').locator('.kanban-more')).toHaveCount(0);
  });

  test('grouped by sprint, the lanes scroll as one and the page still does not (T-262)', async ({
    page,
    tasksServer,
  }) => {
    await page.setViewportSize({ width: 1280, height: 720 });
    await openKanban(page, tasksServer, 'kanban');
    await page.locator('#kanban-group-by-sprint').check();
    expect(await page.evaluate(() => document.documentElement.scrollHeight)).toBe(720);
    const kanban = page.locator('#kanban');
    expect(await kanban.evaluate((el) => el.scrollHeight > el.clientHeight)).toBe(true);
    // Columns are as long as their cards: nothing is cut off inside a lane
    await expect(page.locator('.kanban-more')).toHaveCount(0);
  });

  test('many long labels stay on one line; +N shows them all without opening the task (T-256)', async ({
    page,
    tasksServer,
  }) => {
    const data = loadFixture('kanban');
    const labels = ['security', 'dashboard-performance-improvements', 'interactive', 'spike-2', 'ux'];
    const id = 'T-028';
    data.tasks.find((t) => t.id === id)!.labels = labels;
    await openKanban(page, tasksServer, data);

    // One line of chips, not three, and inside the card
    const row = (await card(page, id).locator('.label-chips').boundingBox())!;
    const box = (await card(page, id).boundingBox())!;
    expect(row.height).toBeLessThan(24);
    expect(row.x + row.width).toBeLessThanOrEqual(box.x + box.width);
    const chips = card(page, id).locator('.label-chips > span.label-chip');
    await expect(chips).toHaveCount(2);
    const more = card(page, id).locator('.label-more');
    await expect(more).toHaveText('+3');

    await more.hover();
    await expect(page.locator('.label-popover .label-chip')).toHaveCount(5);
    await more.click();
    await expect(page.locator('#detail-overlay.open')).toHaveCount(0);
    await page.mouse.move(0, 0);
    await expect(page.locator('.label-popover')).toBeVisible(); // pinned
    await page.keyboard.press('Escape');
    await expect(page.locator('.label-popover')).toHaveCount(0);
  });

  test('filters apply; a status filter empties columns but keeps them', async ({ page, tasksServer }) => {
    await openKanban(page, tasksServer, 'kanban');
    await openFilters(page);
    await filterButton(page, 'sprint', 'Phase 2').click();
    await expect(page.locator('.kanban-count')).toHaveText(['3', '1', '0', '1', '0', '1']);
    await expect(column(page, 'done').locator('.kanban-empty')).toHaveText('No tasks');
    await filterButton(page, 'sprint', 'all').click();
    await filterButton(page, 'status', 'done').click();
    await filterButton(page, 'status', 'blocked').click();
    const columns = await page
      .locator('.kanban-column')
      .evaluateAll((cols) => cols.map((c) => (c as HTMLElement).dataset.status));
    expect(columns).toEqual(['todo', 'in_progress', 'in_review', 'blocked', 'done']);
    await expect(column(page, 'in_review').locator('.kanban-empty')).toHaveText('No tasks');
  });

  test("the scope bar has the board's one progress figure, skipped ones apart (T-323)", async ({
    page,
    tasksServer,
  }) => {
    await openKanban(page, tasksServer, 'kanban');
    const progress = page.locator('#scope-bar .sprint-progress');
    // 24 done of 36 that count (6 + 3 + 2 + 24 + 1); the 2 skipped are apart. No ring of its own on the board
    await expect(progress).toHaveText('24 of 36 done');
    await expect(page.locator('#kanban-completion')).toHaveCount(0);
    await openFilters(page);
    await filterButton(page, 'sprint', 'Phase 2').click();
    await expect(page.locator('#scope-trigger')).toHaveText('Phase 2');
    await filterButton(page, 'sprint', 'Phase 1').click();
    await expect(progress).toHaveText('8 of 20 done');
    await filterButton(page, 'status', 'done').click();
    await expect(page.locator('#scope-filtered')).toHaveText('showing 8, filtered: Status Done');
  });
});

test.describe('Kanban: group by sprint', () => {
  const lanes = (page: Page) => page.locator('.kanban-lane');

  test('one swimlane per sprint (here its phases, read as sprints), with progress and aligned columns; remembered', async ({
    page,
    tasksServer,
  }) => {
    await openKanban(page, tasksServer, 'kanban');
    await page.locator('#kanban-group-by-sprint').check();
    await expect(lanes(page)).toHaveCount(3);
    await expect(page.locator('.kanban-lane-header')).toHaveText([
      /Phase 0\s*16\/16 done/,
      /Phase 1\s*8\/15 done/,
      /Phase 2\s*0\/7 done/,
    ]);
    for (const lane of await lanes(page).all()) {
      await expect(lane.locator('.kanban-column-title')).toHaveText([
        'To Do',
        'In Progress',
        'In Review',
        'Blocked',
        'Done',
        'Other',
      ]);
    }
    await expect(
      page.locator('.kanban-lane[data-sprint="Phase 1"] .kanban-column[data-status="todo"] .kanban-card'),
    ).toHaveCount(3);
    // A lane names its sprint, so its cards don't
    await expect(page.locator('.kanban-card-sprint')).toHaveCount(0);

    await page.reload();
    await page.locator('#tab-kanban').click();
    await expect(page.locator('#kanban-group-by-sprint')).toBeChecked();
    await expect(lanes(page)).toHaveCount(3);
  });

  test('a migrated project: the sprints in order, the backlog last; cards name their sprint only when the board shows several', async ({
    page,
    tasksServer,
  }) => {
    // It opens on the active sprint: one sprint, so cards don't name it, and no grouping to offer (T-323)
    await openKanban(page, tasksServer, 'sprints');
    await expect(page.locator('.kanban-card-sprint')).toHaveCount(0);
    await expect(page.locator('#kanban-group-by-sprint')).toHaveCount(0);
    await openFilters(page);
    await filterButton(page, 'sprint', 'all').click();
    await expect(card(page, 'T-002').locator('.kanban-card-sprint')).toHaveText('Ada Lovelace');
    await expect(card(page, 'T-005').locator('.kanban-card-sprint')).toHaveText('Backlog');

    await page.locator('#kanban-group-by-sprint').check();
    expect(await lanes(page).evaluateAll((all) => all.map((lane) => (lane as HTMLElement).dataset.sprint))).toEqual([
      'Foundations',
      'Ada Lovelace',
      ':backlog',
    ]);
    await expect(page.locator('.kanban-lane-header')).toHaveText([
      /Foundations\s*1\/1 done/,
      /Ada Lovelace\s*0\/3 done/,
      /Backlog\s*0\/1 done/,
    ]);
  });

  test('a view saved before sprints keeps its grouping', async ({ page, tasksServer }) => {
    await openKanban(page, tasksServer, 'kanban');
    await page.evaluate(() => {
      for (const key of Object.keys(localStorage).filter((k) => k.startsWith('atm-dashboard:view:'))) {
        const view = JSON.parse(localStorage.getItem(key)!);
        localStorage.setItem(key, JSON.stringify({ ...view, kanban: { groupByPhase: true } }));
      }
    });
    await page.reload();
    await page.locator('#tab-kanban').click();
    await expect(page.locator('#kanban-group-by-sprint')).toBeChecked();
  });
});
