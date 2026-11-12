import type { Page } from '@playwright/test';
import { expect, loadFixture, openDashboard, test } from './helpers';

// NFR-2: with 500 tasks, switching tabs and refreshing data render within 200 ms. Measured inside the page from the
// action until two animation frames later (so layout and paint are included). Shared CI runners are slower and
// noisier, so the budget is doubled there.
const BUDGET_MS = 200 * (process.env.CI ? 2 : 1);

/** Click `selector`, then wait until `done()` holds and two frames have been painted; returns elapsed ms. */
function timeAction(page: Page, selector: string, done: string) {
  return page.evaluate(
    async ([selector, done]) => {
      const check = new Function(`return (${done})`) as () => boolean;
      const frame = () => new Promise((r) => requestAnimationFrame(r));
      const start = performance.now();
      (document.querySelector(selector) as HTMLElement).click();
      while (!check()) await frame();
      await frame();
      await frame();
      return performance.now() - start;
    },
    [selector, done],
  );
}

// D-6.3: a large archive (5,000 finished tasks) costs nothing until a view needs it, and stays within budget after
const ARCHIVE_SIZE = 5000;
function largeArchive() {
  return {
    meta: { archived_at: '2026-04-01T00:00:00Z' },
    tasks: Array.from({ length: ARCHIVE_SIZE }, (_, i) => {
      const day = String(1 + (i % 28)).padStart(2, '0');
      return {
        id: `A-${String(i + 1).padStart(4, '0')}`,
        name: `Archived task ${i + 1} with a reasonably long name`,
        phase: Math.floor(i / 250),
        type: ['feature', 'test', 'docs', 'infra'][i % 4],
        description: `Archived ${i + 1}.`,
        status: 'done',
        dependencies: [],
        created_at: `2026-03-${day}T08:00:00Z`,
        started_at: `2026-03-${day}T09:00:00Z`,
        started_by: `claude:arch${i % 5}`,
        completed_at: `2026-03-${day}T10:00:00Z`,
      };
    }),
  };
}

/** Wait until `done()` holds and two frames have been painted; the ms since `window.__started` (set before the action) */
function sinceStarted(page: Page, done: string) {
  return page.evaluate(async (done) => {
    const check = new Function(`return (${done})`) as () => boolean;
    const frame = () => new Promise((r) => requestAnimationFrame(r));
    while (!check()) await frame();
    await frame();
    await frame();
    return performance.now() - (window as unknown as { __started: number }).__started;
  }, done);
}

test.describe('performance in the app: its tabs (T-525)', () => {
  // The large project's Kanban, List and Graph, and a small second project, in the window's tabs (T-514), as the app
  // brings them back at its start (T-518)
  const LARGE = 'Performance Project';
  const shown = (title: string, view: string) =>
    `document.querySelector('#dashboard-title')?.textContent === ${JSON.stringify(title)} && ${view}`;
  const KANBAN = "document.querySelector('#kanban .kanban-card')?.offsetParent";
  const LIST = "document.querySelector('#panel-list:not([hidden]) .task-card')?.offsetParent";
  const GRAPH = "document.querySelector('#panel-graph:not([hidden]) #dep-graph .graph-node')";
  const tabs = [
    { name: 'kanban', done: shown(LARGE, KANBAN) },
    { name: 'list', done: shown(LARGE, LIST) },
    { name: 'graph', done: shown(LARGE, GRAPH) },
    // The page is titled by the project's own name (its tasks.json's), the minimal fixture's
    { name: 'second project', done: shown('Minimal Project', KANBAN) },
  ];

  test.beforeEach(async ({ page, tasksServer }) => {
    await page.clock.setFixedTime(new Date('2026-04-20T12:00:00Z'));
    await tasksServer.asApp();
    tasksServer.use('large-500');
    tasksServer.addProject('second', 'minimal', 'Second Project');
    const large = `/projects/${tasksServer.projectId}`;
    tasksServer.restoredTabs = {
      addresses: [`${large}/kanban`, `${large}/list`, `${large}/graph`, '/projects/second/kanban'],
      active: 0,
    };
    await page.goto('/');
    await expect(page.locator('.title-bar-tab')).toHaveCount(4);
    await expect(page.locator('#kanban .kanban-card').first()).toBeVisible();
  });

  test('a click on a tab, and Next Tab round them all', async ({ page, tasksServer }) => {
    const timings: Record<string, number> = {};
    // A click on each tab from the one before it, then on the first (another project, then the large one again: its
    // data loaded again and its Kanban drawn; its graph's layout kept, and the hidden graph not drawn, T-525)
    const round = [1, 2, 3, 0].map((i) => [i, tabs[i]] as const);
    for (const [i, tab] of round) {
      timings[i === 0 ? 'back to the large project' : tab.name] = await page.evaluate(
        async ([i, done]) => {
          const check = new Function(`return (${done})`) as () => boolean;
          const frame = () => new Promise((r) => requestAnimationFrame(r));
          const start = performance.now();
          (document.querySelectorAll('.title-bar-tab [role="tab"]')[i] as HTMLElement).click();
          while (!check()) await frame();
          await frame();
          await frame();
          return performance.now() - start;
        },
        [i, tab.done] as const,
      );
    }
    // Next Tab (⌃Tab, the menu's) from the first tab round them all, back to it
    for (const [i, tab] of round) {
      await page.evaluate(() => ((window as unknown as { __started: number }).__started = performance.now()));
      await tasksServer.chooseMenu({ kind: 'next_tab' });
      timings[`next tab: ${i === 0 ? 'back to the large project' : tab.name}`] = await sinceStarted(page, tab.done);
    }
    console.log('tab switch in the app (ms):', JSON.stringify(timings));
    for (const [tab, ms] of Object.entries(timings)) expect(ms, tab).toBeLessThan(BUDGET_MS);
  });
});

test.describe('performance with 500 tasks and a large archive', () => {
  test.beforeEach(async ({ page, tasksServer }) => {
    await page.clock.setFixedTime(new Date('2026-04-20T12:00:00Z'));
    const data = loadFixture('large-500');
    data.meta = { ...data.meta, archived: ARCHIVE_SIZE };
    tasksServer.archive(largeArchive());
    await openDashboard(page, tasksServer, data);
    await expect(page.locator('#kanban .kanban-card').first()).toBeAttached();
  });

  test('the views that need history load it once, then switch within budget', async ({ page, tasksServer }) => {
    // Graph, Kanban, List and Agents don't fetch it (unless "Include archived" is on)
    await timeAction(page, '#tab-kanban', "document.querySelector('#kanban .kanban-card')?.offsetParent");
    await timeAction(page, '#tab-agents', "document.querySelector('#panel-agents .agents-grid')");
    expect(tasksServer.archiveRequests).toBe(0);
    // The first visit to Progress loads it (once); after that, switches are measured with it loaded
    await page.locator('#tab-progress').click();
    await expect.poll(() => tasksServer.archiveRequests).toBe(1);
    await expect(page.locator('#sprint-table')).toBeVisible();
    await page.locator('#tab-graph').click();
    const timings = {
      progress: await timeAction(page, '#tab-progress', "document.querySelector('#burndown svg')?.isConnected"),
      agents: await timeAction(page, '#tab-agents', "document.querySelector('#panel-agents .agents-grid')"),
      kanban: await timeAction(page, '#tab-kanban', "document.querySelector('#kanban .kanban-card')?.offsetParent"),
    };
    console.log(`tab switch with ${ARCHIVE_SIZE} archived (ms):`, JSON.stringify(timings));
    for (const [tab, ms] of Object.entries(timings)) expect(ms, tab).toBeLessThan(BUDGET_MS);
  });
});

test.describe('performance with 500 tasks', () => {
  test.beforeEach(async ({ page, tasksServer }) => {
    await page.clock.setFixedTime(new Date('2026-04-20T12:00:00Z'));
    await openDashboard(page, tasksServer, 'large-500');
    await expect(page.locator('#kanban .kanban-card').first()).toBeAttached();
  });

  test('switching tabs', async ({ page }) => {
    const timings = {
      kanban: await timeAction(page, '#tab-kanban', "document.querySelector('#kanban .kanban-card')?.offsetParent"),
      progress: await timeAction(page, '#tab-progress', "document.querySelector('#burndown svg')?.isConnected"),
      // Mounted when selected: the timeline is built from scratch
      agents: await timeAction(page, '#tab-agents', "document.querySelector('#panel-agents .agents-grid')"),
      // Drawn when selected (T-525): its nodes, as the List's rows
      graph: await timeAction(
        page,
        '#tab-graph',
        "document.querySelector('#panel-graph:not([hidden]) #dep-graph .graph-node')",
      ),
      list: await timeAction(page, '#tab-list', "document.querySelector('#panel-list .task-card')?.offsetParent"),
    };
    console.log('tab switch (ms):', JSON.stringify(timings));
    for (const [tab, ms] of Object.entries(timings)) expect(ms, tab).toBeLessThan(BUDGET_MS);
  });

  test('a reload that changes the data, on every tab', async ({ page, tasksServer }) => {
    const data = loadFixture('large-500');
    // The change shows in the Kanban, which stays mounted under every view (the List is built only while it is shown,
    // T-525): the task in its new status's column. Two statuses whose columns show every card
    const changed = (status: string) =>
      `document.querySelector('#kanban .kanban-column[data-status="${status}"] .kanban-card[data-id="T-301"]')`;
    const timings: Record<string, number> = {};
    for (const tab of ['graph', 'kanban', 'progress']) {
      await page.locator(`#tab-${tab}`).click();
      // Alternate the data so every reload really changes something
      const status = tab === 'kanban' ? 'in_progress' : 'blocked';
      data.tasks.find((t) => t.id === 'T-301')!.status = status;
      tasksServer.use(data);
      timings[tab] = await timeAction(page, '#reload-btn', changed(status));
    }
    console.log('reload (ms):', JSON.stringify(timings));
    for (const [tab, ms] of Object.entries(timings)) expect(ms, tab).toBeLessThan(BUDGET_MS);
  });
});
