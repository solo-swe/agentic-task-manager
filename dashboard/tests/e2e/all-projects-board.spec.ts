// The All projects page's Board tab (T-259, docs/all-projects-kanban-proposal.md §3, §4): every project's tasks in
// one board, from the projects' board summaries.
import { board, boardCard, expect, openDashboard, summary, test, type TasksServer } from './helpers';
import type { Page } from '@playwright/test';

const NOW = new Date('2026-03-01T12:00:00Z');

/** Two projects: Shop (v4 fixture), and Blog, the more recently active one */
function addProjects(tasksServer: TasksServer) {
  tasksServer
    .addProject(
      'shop',
      'v4',
      'Shop',
      summary({
        last_activity: '2026-03-01T11:00:00Z',
        board: board(
          {
            todo: [boardCard('T-008', 'Password reset'), boardCard('T-009', 'Email templates', { labels: ['api'] })],
            active: [boardCard('T-007', 'Checkout flow', { by: 'claude:aaa', since: '2026-03-01T10:00:00Z' })],
            blocked: [
              boardCard('T-005', 'Choose the email provider', {
                since: '2026-03-01T06:00:00Z',
                waiting_on: 'agent',
                questions: 1,
              }),
            ],
            done: [boardCard('T-003', 'Product page', { completed_at: '2026-03-01T11:30:00Z' })],
          },
          { todo: 12 },
        ),
      }),
    )
    .addProject(
      'blog',
      'minimal',
      'Blog',
      summary({
        last_activity: '2026-03-01T11:50:00Z',
        board: board({
          todo: [boardCard('B-1', 'RSS feed', { priority: 'P1', labels: ['ux', 'api', 'seo'] })],
          active: [boardCard('B-3', 'Comments', { by: 'alice', since: '2026-03-01T09:00:00Z' })],
          blocked: [
            boardCard('B-2', 'Pick a theme', { since: '2026-03-01T11:00:00Z', waiting_on: 'user', questions: 2 }),
          ],
          done: [boardCard('B-4', 'Drafts', { completed_at: '2026-03-01T11:55:00Z' })],
        }),
      }),
    );
}

async function openBoard(page: Page, tasksServer: TasksServer) {
  await page.clock.setFixedTime(NOW);
  addProjects(tasksServer);
  await openDashboard(page, tasksServer, 'standard');
  await page.locator('#overview-tab-board').click();
  await expect(page.locator('#all-board')).toBeVisible();
}

/** Three busy projects: ten ready tasks each (the server's cap per project), a long To Do column */
async function openBusyBoard(page: Page, tasksServer: TasksServer) {
  await page.clock.setFixedTime(NOW);
  for (const name of ['Alpha', 'Beta', 'Gamma']) {
    const todo = Array.from({ length: 10 }, (_, i) => boardCard(`${name[0]}-${i + 1}`, `${name} task ${i + 1}`));
    tasksServer.addProject(name.toLowerCase(), 'minimal', name, summary({ board: board({ todo }) }));
  }
  await openDashboard(page, tasksServer, 'standard');
  await page.locator('#overview-tab-board').click();
  await expect(page.locator('#all-board .board-card')).toHaveCount(30);
}

const column = (page: Page, id: string) => page.locator(`#all-board .kanban-column[data-status="${id}"]`);
const ids = (page: Page, id: string) => column(page, id).locator('.board-card .kanban-card-id');

test.describe('All projects board', () => {
  test('the Projects tab is the default; Board shows the columns, each in its own order', async ({
    page,
    tasksServer,
  }) => {
    await page.clock.setFixedTime(NOW);
    addProjects(tasksServer);
    await openDashboard(page, tasksServer, 'standard');
    await expect(page.locator('#overview-tab-projects')).toHaveAttribute('aria-selected', 'true');
    await expect(page.locator('#projects-grid')).toBeVisible();
    await page.locator('#overview-tab-board').click();
    await expect(page).toHaveURL(/view=board/);
    await expect(page.locator('#projects-grid')).toHaveCount(0);

    await expect(column(page, 'todo').locator('.kanban-column-title')).toHaveText('To Do');
    // To Do: the most recently active project first, then each project's (the loop's) order
    await expect(ids(page, 'todo')).toHaveText(['B-1', 'T-008', 'T-009']);
    // In progress: longest running first; blocked: on you first; done: newest first
    await expect(ids(page, 'active')).toHaveText(['B-3', 'T-007']);
    await expect(ids(page, 'blocked')).toHaveText(['B-2', 'T-005']);
    await expect(ids(page, 'done')).toHaveText(['B-4', 'T-003']);
    // The columns count everything, the cards not shown too
    await expect(column(page, 'todo').locator('.kanban-count')).toHaveText('13');
  });

  test('cards: the project chip, the badges, and the column’s line', async ({ page, tasksServer }) => {
    await openBoard(page, tasksServer);
    const b1 = page.locator('#all-board .board-card[data-project="blog"][data-id="B-1"]');
    await expect(b1.locator('.board-project-name')).toHaveText('Blog');
    await expect(b1.locator('.project-logo')).toHaveAttribute('data-initials', 'BL');
    await expect(b1.locator('.priority-badge')).toHaveText('P1');
    await expect(b1.locator('.label-chip')).toHaveText(['ux', 'api', '+1']);
    await expect(b1.locator('.kanban-card-name')).toHaveText('RSS feed');

    const status = (project: string, id: string) =>
      page.locator(`#all-board .board-card[data-project="${project}"][data-id="${id}"] .board-card-status`);
    await expect(status('blog', 'B-3')).toHaveText('alice · 3 h ago');
    await expect(status('blog', 'B-2')).toHaveText('Waiting on you · 1 h · 2 questions');
    await expect(status('blog', 'B-2')).toHaveClass(/on-user/);
    await expect(status('shop', 'T-005')).toHaveText('Waiting on an agent · 6 h · 1 question');
    await expect(status('blog', 'B-4')).toHaveText('Done 5 m ago');
  });

  test('empty columns say why', async ({ page, tasksServer }) => {
    await openBoard(page, tasksServer);
    await expect(column(page, 'review').locator('.kanban-empty')).toHaveText('Nothing waiting for a review');
    await expect(column(page, 'review').locator('.kanban-count')).toHaveText('0');
  });

  test('a card opens its task’s details over the board, which stays; the chip opens the project (T-411)', async ({
    page,
    tasksServer,
  }) => {
    await openBoard(page, tasksServer);
    const url = page.url();
    const panel = page.locator('#detail-overlay.open #detail-panel');
    await page.locator('#all-board .board-card[data-project="shop"][data-id="T-007"]').click();
    await expect(panel.locator('h3')).toHaveText('T-007 Two-factor sign-in');
    // Still the board, at the same address: no jump to the project's Kanban
    expect(page.url()).toBe(url);
    await expect(page.locator('#all-board')).toBeVisible();
    // The details are the project's own: a dependency opens its task
    await panel.locator('.detail-dep-item', { hasText: 'T-001' }).click();
    await expect(panel.locator('h3 > span').first()).toHaveText('T-001');
    // Kept open as the dashboard refreshes, until closed; then the board is as it was
    await page.clock.runFor(6_000);
    await expect(panel).toBeVisible();
    await page.keyboard.press('Escape');
    await expect(page.locator('#detail-overlay.open')).toHaveCount(0);
    await expect(page.locator('#all-board')).toBeVisible();
    // Another card, by the keyboard
    await page.locator('#all-board .board-card[data-project="shop"][data-id="T-005"]').press('Enter');
    await expect(panel.locator('h3 > span').first()).toHaveText('T-005');
    await page.keyboard.press('Escape');
    expect(page.url()).toBe(url);
    // A card whose task its project's data doesn't have (B-1: the Blog fixture's tasks are others) opens the project
    await page.locator('#all-board .board-card[data-project="blog"][data-id="B-1"]').click();
    await expect(page).toHaveURL(/project=blog/);
    await page.goBack();
    await expect(page.locator('#all-board')).toBeVisible();

    await page.locator('#all-board .board-card[data-id="B-1"] .board-project-chip').click();
    await expect(page).toHaveURL(/project=blog/);
    await expect(page.locator('#detail-overlay.open')).toHaveCount(0);
  });

  test('“+ N more” opens the project’s Kanban', async ({ page, tasksServer }) => {
    await openBoard(page, tasksServer);
    const more = column(page, 'todo').locator('.board-more');
    await expect(more).toHaveText(['+ 10 more in Shop']);
    await more.click();
    await expect(page).toHaveURL(/project=shop/);
    await expect(page.locator('#tab-kanban')).toHaveAttribute('aria-selected', 'true');
  });

  test('the tab is remembered, #view= links to it, and Back returns to the other', async ({ page, tasksServer }) => {
    await openBoard(page, tasksServer);
    await page.locator('#overview-tab-projects').click();
    await expect(page.locator('#projects-grid')).toBeVisible();
    await page.goBack();
    await expect(page.locator('#all-board')).toBeVisible();
    // Without a view in the link: the last tab chosen
    await page.goto('tasks-dashboard.html#overview');
    await page.reload();
    await expect(page.locator('#overview-tab-board')).toHaveAttribute('aria-selected', 'true');
    await page.goto('tasks-dashboard.html#overview&view=projects');
    await expect(page.locator('#projects-grid')).toBeVisible();
  });

  test('group by project: a lane per project, the most recently active first, none for an empty one; remembered (T-260)', async ({
    page,
    tasksServer,
  }) => {
    tasksServer.addProject(
      'quiet',
      'minimal',
      'Quiet',
      summary({ last_activity: '2026-03-01T11:59:00Z', board: board({}) }),
    );
    await openBoard(page, tasksServer);
    await page.locator('#board-group-by-project').check();
    const lanes = page.locator('#all-board .board-lane');
    await expect(lanes).toHaveCount(2);
    await expect(lanes.locator('.board-lane-name')).toHaveText(['Blog', 'Shop']);
    // In a lane, the cards don't repeat the project; "+ N more" doesn't either
    await expect(lanes.first().locator('.board-project-chip')).toHaveCount(0);
    await expect(page.locator('.board-lane[data-project="shop"] .board-more')).toHaveText(['+ 10 more']);
    await expect(page.locator('.board-lane[data-project="blog"] [data-status="todo"] .kanban-card-id')).toHaveText([
      'B-1',
    ]);

    await page.reload();
    await expect(page.locator('#board-group-by-project')).toBeChecked();
    await expect(lanes).toHaveCount(2);
    await page.locator('.board-lane[data-project="shop"] .board-lane-name').click();
    await expect(page).toHaveURL(/project=shop/);
  });

  test('search: a match in a description narrows the board, asked once per pause; clearing restores it (T-261)', async ({
    page,
    tasksServer,
  }) => {
    await openBoard(page, tasksServer);
    const search = page.locator('#board-search');
    // T-005's description: "Pick a provider for transactional email." (the card itself has no description)
    await search.pressSequentially('transactional', { delay: 20 });
    await expect(page.locator('#all-board .board-card')).toHaveCount(1);
    await expect(page.locator('#all-board .board-card[data-project="shop"][data-id="T-005"]')).toHaveCount(1);
    await expect(column(page, 'blocked').locator('.kanban-count')).toHaveText('1 of 2');
    await expect(column(page, 'todo').locator('.kanban-count')).toHaveText('0 of 13');
    await expect(column(page, 'todo').locator('.kanban-empty')).toHaveText('No match');
    // No "+ N more" while searching: the search looks at the board's cards
    await expect(page.locator('#all-board .board-more')).toHaveCount(0);
    expect(tasksServer.searches).toEqual(['transactional']);

    // A project's name finds its tasks (Shop's cards are its v4 tasks; Blog's are made up); remembered across a reload
    await search.fill('shop');
    await expect(page.locator('#all-board .board-card')).toHaveCount(5);
    await page.reload();
    await expect(page.locator('#board-search')).toHaveValue('shop');
    await expect(page.locator('#all-board .board-card')).toHaveCount(5);

    await page.locator('.board-search .search-clear').click();
    await expect(page.locator('#all-board .board-card')).toHaveCount(9);
    await expect(column(page, 'todo').locator('.kanban-count')).toHaveText('13');
  });

  test('the page never scrolls on the board: columns reach the status bar and scroll inside (T-265)', async ({
    page,
    tasksServer,
  }) => {
    for (const height of [1600, 720]) {
      await page.setViewportSize({ width: 1280, height });
      await openBusyBoard(page, tasksServer);
      // One scrollbar at most: the page is exactly the window's height
      expect(await page.evaluate(() => document.documentElement.scrollHeight)).toBe(height);
      const todo = column(page, 'todo');
      const footer = (await page.locator('footer').boundingBox())!;
      const box = (await todo.boundingBox())!;
      expect(footer.y - (box.y + box.height)).toBeGreaterThanOrEqual(0);
      // (the overview page's own 24 px bottom padding, and the status bar's margin)
      expect(footer.y - (box.y + box.height)).toBeLessThanOrEqual(48);
      expect(await todo.locator('.kanban-cards').evaluate((el) => el.scrollHeight > el.clientHeight)).toBe(true);
    }
  });

  test('a long column says how many cards are below and above; the wheel scrolls it, not the page (T-265)', async ({
    page,
    tasksServer,
  }) => {
    await page.setViewportSize({ width: 1280, height: 720 });
    await openBusyBoard(page, tasksServer);
    const todo = column(page, 'todo');
    const cards = todo.locator('.kanban-cards');
    // No scrollbar takes room
    expect(await cards.evaluate((el) => (el as HTMLElement).offsetWidth - el.clientWidth)).toBe(0);
    await expect(todo.locator('.kanban-more.top')).toHaveCount(0);
    await expect(todo.locator('.kanban-more.bottom')).toHaveText(/^▼ \d+ more$/);
    const box = (await cards.boundingBox())!;
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await page.mouse.wheel(0, 400);
    await expect.poll(() => cards.evaluate((el) => el.scrollTop)).toBeGreaterThan(0);
    expect(await page.evaluate(() => scrollY)).toBe(0);
    await expect(todo.locator('.kanban-more.top')).toHaveText(/^▲ \d+ more$/);
    // A short column shows neither
    await expect(column(page, 'done').locator('.kanban-more')).toHaveCount(0);
  });

  test('grouped by project, the lanes scroll as one and the page still does not (T-265)', async ({
    page,
    tasksServer,
  }) => {
    await page.setViewportSize({ width: 1280, height: 720 });
    await openBusyBoard(page, tasksServer);
    await page.locator('#board-group-by-project').check();
    await expect(page.locator('#all-board .board-lane')).toHaveCount(3);
    expect(await page.evaluate(() => document.documentElement.scrollHeight)).toBe(720);
    const lanes = page.locator('#all-board');
    expect(await lanes.evaluate((el) => el.scrollHeight > el.clientHeight)).toBe(true);
    // Columns are as long as their cards: nothing is cut off inside a lane
    await expect(page.locator('.kanban-more')).toHaveCount(0);
  });

  test('the Projects tab still scrolls as a page', async ({ page, tasksServer }) => {
    await page.setViewportSize({ width: 1280, height: 720 });
    for (let i = 0; i < 12; i++) tasksServer.addProject(`p${i}`, 'minimal', `Project ${i}`, summary());
    await page.clock.setFixedTime(NOW);
    await openDashboard(page, tasksServer, 'standard');
    // Once the cards are there: measured at once, the page could still be empty
    await expect(page.locator('.project-card')).toHaveCount(13);
    expect(await page.evaluate(() => document.documentElement.scrollHeight)).toBeGreaterThan(720);
  });

  test('the tab is called Omniboard; its ⓘ says what it shows and where the details are (T-266)', async ({
    page,
    tasksServer,
  }) => {
    await openBoard(page, tasksServer);
    await expect(page.locator('#overview-tab-board')).toHaveText('Omniboard');
    // The link keeps its name
    await expect(page).toHaveURL(/view=board/);
    await page.getByRole('button', { name: 'About the Omniboard' }).click();
    const about = page.getByRole('dialog', { name: 'About the Omniboard' });
    await expect(about).toContainText('done in the last 24 hours');
    await expect(about).toContainText("A card opens its task's details here, over the board");
    await expect(about).toContainText("Each project's full board is its own Kanban");
  });

  test('Active only applies to the board too', async ({ page, tasksServer }) => {
    await page.clock.setFixedTime(NOW);
    const idle = summary({
      total: 5,
      done: 5,
      percent: 100,
      last_activity: '2026-01-01T00:00:00Z',
      board: board({ done: [boardCard('I-1', 'Old work', { completed_at: '2026-01-01T00:00:00Z' })] }),
    });
    tasksServer.addProject('idle', 'minimal', 'Idle', idle);
    await openDashboard(page, tasksServer, 'standard');
    await page.locator('#overview-tab-board').click();
    await expect(page.locator('#all-board .board-card')).toHaveCount(0);
    await page.locator('#active-projects-only').uncheck();
    await expect(page.locator('#all-board .board-card[data-id="I-1"]')).toHaveCount(1);
  });

  // T-340: by default the board shows each project's active sprint; All tasks shows everything, as before
  test('Active sprints by default, All tasks as an option; remembered; projects without an active sprint named', async ({
    page,
    tasksServer,
  }) => {
    await page.clock.setFixedTime(NOW);
    const whole = board({
      todo: [boardCard('S-1', 'In the sprint'), boardCard('S-2', 'In the backlog')],
      blocked: [boardCard('S-3', 'Blocked, planned sprint', { since: '2026-03-01T10:00:00Z', waiting_on: 'user' })],
    });
    const sprint = board({ todo: [boardCard('S-1', 'In the sprint')] });
    // Sprints the more recently active, so its cards come first in a column
    const recent = { last_activity: '2026-03-01T11:00:00Z' };
    tasksServer
      .addProject('sprints', 'minimal', 'Sprints', summary({ ...recent, board: whole, board_active_sprint: sprint }))
      .addProject(
        'nosprint',
        'minimal',
        'No Sprint',
        summary({
          last_activity: '2026-03-01T10:00:00Z',
          board: board({ todo: [boardCard('N-1', 'Any')] }),
          board_active_sprint: null,
        }),
      );
    await openDashboard(page, tasksServer, 'standard');
    await page.locator('#overview-tab-board').click();
    const scope = (s: string) => page.locator(`#board-scope [data-scope="${s}"]`);
    await expect(scope('active')).toHaveAttribute('aria-pressed', 'true');
    await expect(ids(page, 'todo')).toHaveText(['S-1']);
    await expect(column(page, 'todo').locator('.kanban-count')).toHaveText('1');
    await expect(ids(page, 'blocked')).toHaveText([]);
    await expect(page.locator('#board-no-active-sprint')).toHaveText('1 project has no active sprint');
    await expect(page.locator('#board-no-active-sprint')).toHaveAttribute('title', 'No Sprint');

    await scope('all').click();
    await expect(ids(page, 'todo')).toHaveText(['S-1', 'S-2', 'N-1']);
    await expect(ids(page, 'blocked')).toHaveText(['S-3']);
    await expect(page.locator('#board-no-active-sprint')).toHaveCount(0);
    await page.reload();
    await page.locator('#overview-tab-board').click();
    await expect(scope('all')).toHaveAttribute('aria-pressed', 'true');
  });
});
