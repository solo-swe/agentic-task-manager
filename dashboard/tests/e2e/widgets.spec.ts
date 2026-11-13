import type { Page } from '@playwright/test';
import { expect, loadFixture, openDashboard, test } from './helpers';

// The top bar: compact Progress and Activity pills between the title and the controls; a click opens the details.
// Numbers from fixtures/agents.md: 6 of 8 tasks done, completions between 08:00 and 11:00 UTC.
const NOW = new Date('2026-04-01T12:00:00Z');
const progressPill = (page: Page) => page.locator('#progress-card');
const activityPill = (page: Page) => page.locator('#activity-card');

test.beforeEach(async ({ page, tasksServer }) => {
  await page.clock.setFixedTime(NOW);
  await openDashboard(page, tasksServer, 'agents');
});

test.describe('Progress widget', () => {
  test('the pill is a ring with the share of tasks done', async ({ page }) => {
    await expect(progressPill(page)).toHaveAccessibleName('Progress 75%, 6 of 8 tasks done');
    await expect(progressPill(page).locator('.number')).toHaveText('75%');
    await expect(progressPill(page).locator('svg.mini-ring circle')).toHaveCount(2);
  });

  test('opens the details: overall, by status, by sprint and by type', async ({ page }) => {
    await progressPill(page).click();
    await expect(progressPill(page)).toHaveAttribute('aria-expanded', 'true');
    const panel = page.locator('#progress-panel');
    await expect(panel.locator('.insights-title')).toHaveText('6 of 8 tasks done');
    await expect(panel.locator('.insights-hero .status-legend li')).toHaveText(['Done6', 'In Progress1', 'To Do1']);
    // Its phases read as sprints: Phase 0 is finished, so closed: a chip; Phase 1 a ring (T-243)
    await expect(panel.locator('#progress-by-sprint .ring-label')).toHaveText(['Phase 1']);
    await expect(panel.locator('#progress-by-sprint .ring-count')).toHaveText(['2/4']);
    await expect(panel.locator('#progress-closed-chips')).toHaveText('✓ Phase 0');
    // By type: one stacked bar, a segment per type, and its legend
    await expect(panel.locator('#progress-by-type .type-bar-segment')).toHaveCount(5);
    await expect(panel.locator('#progress-by-type li')).toHaveText([
      'Bugfix 1/1',
      'Docs 1/1',
      'Feature 2/3',
      'Infra 1/1',
      'Test 1/2',
    ]);
  });

  test("finished tasks that weren't done are counted apart from done, by resolution (SR-2.8)", async ({
    page,
    tasksServer,
  }) => {
    const panel = page.locator('#progress-panel');
    await progressPill(page).click();
    await expect(panel.locator('#progress-not-done')).toHaveCount(0);
    await page.keyboard.press('Escape');

    const data = loadFixture('agents');
    const todo = data.tasks.filter((t) => t.status === 'todo');
    todo[0].status = 'skipped';
    data.tasks.push({ ...todo[0], id: 'T-990', status: 'skipped', resolution: 'obsolete' });
    tasksServer.use(data);
    await page.locator('#reload-btn').click();
    await progressPill(page).click();
    await expect(panel.locator('#progress-not-done')).toHaveText("Not done: 1 won't do · 1 obsolete");
  });

  test('Go to: the Progress tab, or the Kanban filtered to a status (T-243)', async ({ page }) => {
    const panel = page.locator('#progress-panel');
    await progressPill(page).click();
    await expect(panel.locator('.go-to-link')).toHaveText([
      'Progress tab',
      'In review (0)',
      'Blocked (0)',
      'In progress (1)',
    ]);
    await expect(panel.locator('.go-to-link[data-go="review"]')).toBeDisabled();

    await panel.locator('.go-to-link[data-go="active"]').click();
    await expect(panel).toHaveCount(0);
    await expect(page.locator('#tab-kanban')).toHaveAttribute('aria-selected', 'true');
    // The Kanban's cards (the List's are built only while it is shown, T-525)
    await expect(page.locator('#kanban .kanban-card')).toHaveCount(1);

    // A status in the legend does the same
    await progressPill(page).click();
    await panel.locator('.status-legend-link[data-status="todo"]').click();
    await expect(page.locator('#kanban .kanban-card')).toHaveCount(1);

    await progressPill(page).click();
    await panel.locator('.go-to-link[data-go="progress"]').click();
    await expect(page.locator('#progress')).toBeVisible();
  });

  test('closes on Escape, on a click outside, and with the pill', async ({ page }) => {
    const panel = page.locator('#progress-panel');
    await progressPill(page).click();
    await page.keyboard.press('Escape');
    await expect(panel).toHaveCount(0);
    await progressPill(page).click();
    await page.locator('h1').click();
    await expect(panel).toHaveCount(0);
    await progressPill(page).click();
    await progressPill(page).click();
    await expect(panel).toHaveCount(0);
    await expect(progressPill(page)).toHaveAttribute('aria-expanded', 'false');
  });
});

test.describe('Activity widget', () => {
  test('the pill is a sparkline of completions per period with their average', async ({ page }) => {
    await expect(activityPill(page)).toHaveAccessibleName('Activity: 6 tasks completed');
    await expect(activityPill(page).locator('svg.sparkline rect')).toHaveCount(4); // 08:00, 09:00, 10:00, 11:00
    await expect(activityPill(page).locator('svg.sparkline path')).toHaveCount(1);
    // At 12:00 the last hour is over: nothing "so far", no lighter bar
    await expect(activityPill(page)).toHaveAttribute(
      'title',
      'Tasks completed per hour, all time: 6 · the line is the average of the last 3 hours · click for details',
    );
    await expect(activityPill(page).locator('svg.sparkline rect.ongoing')).toHaveCount(0);
  });

  test('the sparkline: the current hour, not over yet, lighter and counted "so far" (T-249)', async ({ page }) => {
    await page.clock.setFixedTime(new Date('2026-04-01T11:30:00Z'));
    await page.reload();
    await expect(activityPill(page)).toHaveAttribute('title', /· this hour so far: \d+ · click for details$/);
    await expect(activityPill(page).locator('svg.sparkline rect').last()).toHaveClass(/ongoing/);
  });

  test('the popup and the completed chart explain themselves (T-249)', async ({ page }) => {
    await activityPill(page).click();
    const panel = page.locator('#activity-panel');
    await panel.locator('.activity-heading .infotip-button').click();
    const about = page.getByRole('dialog', { name: 'About activity' });
    await expect(about).toContainText("the search and filters don't apply here");
    await panel.locator('.activity-heading .infotip-button').click();
    await expect(about).toHaveCount(0);
    await panel.locator('#dive-completed .infotip-button').click();
    await expect(page.getByRole('dialog', { name: 'About tasks completed' })).toContainText('stacked by type');
  });

  test('an explanation near the popup’s right edge moves left to fit, nothing cut off', async ({ page }) => {
    await activityPill(page).click();
    const panel = page.locator('#activity-panel');
    await panel.locator('#kpi-where .infotip-button').click();
    const popover = page.getByRole('dialog', { name: 'About where the time went' });
    await expect(popover).toContainText('Idle includes nights and weekends');
    // Placed at its ⓘ it would run past the popup's right edge (the card is the popup's last): it moved
    await expect(popover).toHaveAttribute('style', /translateX\(-\d+(\.\d+)?px\)/);
    // Inside the popup's box (what clips it) and the window
    const fits = await popover.evaluate((el) => {
      const box = el.getBoundingClientRect();
      const panel = el.closest('#activity-panel')!.getBoundingClientRect();
      return box.left >= panel.left && box.right <= panel.right && box.right <= document.documentElement.clientWidth;
    });
    expect(fits).toBe(true);
  });

  test('the deep dive: key numbers, and charts by type, by agent and per task', async ({ page }) => {
    await activityPill(page).click();
    const panel = page.locator('#activity-panel');
    // The heading names no bar size (T-263: "Activity per hour" read as the period); the charts' headings do
    await expect(panel.locator('#activity-title')).toContainText('Activity');
    await expect(panel.locator('#activity-title')).not.toContainText('per');
    await expect(panel.locator('#dive-completed h4')).toHaveText('Tasks completed over all time, per hour, by type');
    // 6 done; cycle times 20–80 m, median 55 m, no interactive task; from 08:00 to now (12:00) nobody works 11:00–11:30
    // (see fixtures/agents.md). Three cards (T-246)
    await expect(panel.locator('#kpi-done .kpi-value')).toHaveText('6');
    await expect(panel.locator('#kpi-time .kpi-side')).toHaveText([
      '55 m agents alone (6 tasks)',
      '– with you (0 tasks)',
    ]);
    await expect(panel.locator('#kpi-where .status-legend li')).toHaveText([
      'Agents working3 h 30 m',
      'Waiting on you0 m',
      'Idle30 m',
    ]);
    await expect(panel.locator('#kpi-where .time-split-agents')).toHaveText(
      '5 h 30 m of agent time, up to 3 agents at once',
    );
    // No claims: nobody is shown working now
    await expect(panel.locator('#kpi-working-now')).toHaveCount(0);
    await expect(panel.locator('#dive-completed .type-chip')).toHaveText([
      'feature2',
      'bugfix1',
      'docs1',
      'infra1',
      'test1',
    ]);
    // By kind, not per session (T-247): all agents here (unattributed work counts as an agent), and idle hatched
    await expect(panel.locator('#dive-agents .type-chip')).toHaveText(['Agents', 'Idle']);
    await expect(panel.locator('#dive-cycle .cycle-mark circle')).toHaveCount(6);
    await expect(panel.locator('#dive-agents .idle-label')).toHaveText(['idle 30 m']);
    await expect(panel.locator('.idle-band')).toHaveCount(3); // the same stretch in all three charts

    // Hovering a period reads it across the charts: 09:00–10:00
    const chart = (await panel.locator('#dive-agents > svg').boundingBox())!;
    const plot = { x: chart.x + (34 / 640) * chart.width, w: ((640 - 42) / 640) * chart.width };
    await page.mouse.move(plot.x + plot.w * 0.375, chart.y + chart.height / 2);
    const tooltip = panel.locator('.deep-dive-tooltip');
    await expect(tooltip.locator('.tooltip-title')).toHaveText('Apr 1 09:00 AM – Apr 1 10:00 AM');
    await expect(tooltip).toContainText('3 completed');
    // alice 09:00–09:40 (40 m), bob 09:10–10:00 (50 m), unattributed 09:00–09:20 (20 m)
    await expect(tooltip.locator('.tooltip-type').filter({ hasText: 'alice' })).toHaveText('claude:alice001 67%');
    await expect(tooltip.locator('.tooltip-type').filter({ hasText: 'bob' })).toHaveText('claude:bob00002 83%');
    await expect(tooltip.locator('.tooltip-task')).toHaveText([
      'T-002 Login form',
      'T-004 Old task',
      'T-003 Login tests',
    ]);
  });

  test('who was working: agents and people, and waiting on you instead of idle (T-247)', async ({
    page,
    tasksServer,
  }) => {
    // T-005 (09:10–10:30) was a person's; T-006 (10:00–11:00) then waited for your review until 11:30, when nobody
    // worked: no idle stretch is left
    const data = loadFixture('agents');
    for (const t of data.tasks) {
      if (t.id === 'T-005') t.started_by = 'Hakan';
      if (t.id === 'T-006') {
        t.review_requested_at = '2026-04-01T11:00:00Z';
        t.completed_at = '2026-04-01T11:30:00Z';
        t.reviews = [{ from: '2026-04-01T11:00:00Z', to: '2026-04-01T11:30:00Z' }];
      }
    }
    tasksServer.use(data);
    await page.reload();
    await activityPill(page).click();
    const panel = page.locator('#activity-panel');
    await expect(panel.locator('#dive-agents .type-chip')).toHaveText(['Agents', 'People', 'Waiting on you']);
    await expect(panel.locator('#dive-agents .waiting-band')).toHaveCount(1);
    await expect(panel.locator('#dive-agents .idle-band')).toHaveCount(0);

    // The 11:00 hour: alice from 11:30, and the half hour waiting on you
    const chart = (await panel.locator('#dive-agents > svg').boundingBox())!;
    const plot = { x: chart.x + (34 / 640) * chart.width, w: ((640 - 42) / 640) * chart.width };
    await page.mouse.move(plot.x + plot.w * 0.875, chart.y + chart.height / 2);
    await expect(panel.locator('#tooltip-waiting')).toHaveText('Waiting on you 30 m');
    await page.locator('#dive-agents h4 .infotip-button').click();
    await expect(panel.locator('#dive-agents .infotip-popover')).toContainText('Claude Code sessions');
  });

  test('time per task: an outlier on the top edge, interactive tasks hollow with their own median, a click opens one (T-248)', async ({
    page,
    tasksServer,
  }) => {
    // Four 10-minute tasks and one of 24 h 40 m join the six (20–80 m): the scale stops at the 90th percentile, 80 m
    const data = loadFixture('agents');
    const quick = (id: string, start: string, end: string) => ({
      id,
      name: `Task ${id}`,
      phase: 1,
      type: 'feature',
      description: '',
      status: 'done',
      started_by: 'claude:alice001',
      started_at: start,
      completed_at: end,
    });
    for (const id of ['T-101', 'T-102', 'T-103', 'T-104'])
      data.tasks.push(quick(id, '2026-04-01T10:00:00Z', '2026-04-01T10:10:00Z'));
    data.tasks.push(quick('T-105', '2026-03-31T10:00:00Z', '2026-04-01T10:40:00Z'));
    for (const t of data.tasks) if (t.id === 'T-003') t.labels = ['interactive'];
    tasksServer.use(data);
    await page.reload();
    await activityPill(page).click();
    const chart = page.locator('#dive-cycle');
    await expect(chart.locator('.cycle-mark.outlier')).toHaveCount(1);
    await expect(chart.locator('.cycle-mark.outlier')).toHaveAttribute('aria-label', 'T-105 Task T-105: 24 h 40 m');
    await expect(chart.locator('.cycle-mark.with-you')).toHaveCount(1);
    await expect(chart.locator('.cycle-mark.with-you')).toHaveAttribute('data-id', 'T-003');
    // T-003 alone with you (60 m); the other ten: 10, 10, 10, 10, 20, 30, 50, 60, 80, 1480 → 25 m
    await expect(chart.locator('.median-label')).toHaveText(['agents alone 25 m', 'with you 1 h']);
    await expect(chart.locator('.chart-median.solid')).toHaveCount(1);

    await chart.locator('.cycle-mark.outlier').click();
    await expect(page.locator('#detail-overlay')).toHaveClass(/open/);
    await expect(page.locator('#detail-overlay')).toContainText('T-105');
  });

  test('the period: a unit and ‹ › to step back and forth, remembered', async ({ page }) => {
    await activityPill(page).click();
    const label = page.locator('#activity-period');
    await expect(label).toHaveText('All time');
    await expect(page.locator('#activity-prev')).toHaveCount(0);

    await page.locator('[data-range="day"]').click();
    await expect(label).toHaveText('Today');
    await expect(page.locator('#activity-next')).toBeDisabled();
    await expect(page.locator('#kpi-done')).toHaveText('6 tasks done');
    await page.locator('#activity-prev').click();
    await expect(label).toHaveText('Yesterday');
    await expect(page.locator('#kpi-done')).toHaveText('0 tasks done');
    await page.locator('#activity-prev').click();
    await expect(label).toHaveText('Mon, Mar 30');
    await page.locator('#activity-next').click();
    await page.locator('#activity-next').click();
    await expect(label).toHaveText('Today');

    await page.locator('[data-range="hour"]').click();
    await expect(page.locator('#dive-completed h4')).toContainText('per 5 minutes');
    await page.locator('[data-range="week"]').click();
    // A week reads per day, and the headings say which week (it used to say "per 6 hours"); the popup's own heading
    // names neither (T-263)
    await expect(page.locator('#activity-title')).not.toContainText('per');
    await expect(page.locator('#dive-completed h4')).toHaveText('Tasks completed this week, per day, by type');
    await page.locator('#activity-prev').click();
    await expect(label).toHaveText('Last week');
    await expect(page.locator('#dive-agents h4')).toContainText('Who was working last week');
    await page.locator('[data-range="month"]').click();
    await page.locator('#activity-prev').click();
    await expect(label).toHaveText('Last month');
    await page.locator('#activity-prev').click();
    await expect(label).toHaveText('February 2026');

    await page.reload();
    await activityPill(page).click();
    await expect(label).toHaveText('February 2026');
  });

  // T-264: every period, its headings. The popup's heading is just "Activity"; the period label names the chosen
  // period; each chart names that period and its own bar size. (An earlier test pinned "Activity per hour", the bar
  // size, as if it were the period, and nothing checked the other presets: that bug came back twice.)
  test('every period: the heading, the period label and the charts name the period and its own bar size', async ({
    page,
  }) => {
    await activityPill(page).click();
    const panel = page.locator('#activity-panel');
    const expectHeadings = async (label: string, phrase: string, bar: string | RegExp) => {
      await expect(panel.locator('#activity-period')).toHaveText(label);
      await expect(panel.locator('#activity-title')).toContainText('Activity');
      await expect(panel.locator('#activity-title')).not.toContainText(/\bper\b/);
      const per = typeof bar === 'string' ? bar : bar.source;
      await expect(panel.locator('#dive-completed h4')).toHaveText(
        // (the ⓘ button leaves trailing space, which a pattern, unlike a string, doesn't trim)
        new RegExp(`^Tasks completed ${phrase}, per ${per}, by type\\s*$`),
      );
      await expect(panel.locator('#dive-agents h4')).toContainText(
        new RegExp(`^Who was working ${phrase} \\(1 = one busy the whole ${per}\\)`),
      );
      await expect(panel.locator('#dive-cycle h4')).toContainText(`Time per task ${phrase} (`);
    };
    const presets = [
      { range: 'hour', label: 'Last hour', phrase: 'in the last hour', bar: '5 minutes' },
      { range: 'day', label: 'Today', phrase: 'today', bar: 'hour' },
      { range: 'week', label: 'This week', phrase: 'this week', bar: 'day' },
      { range: 'month', label: 'This month', phrase: 'this month', bar: 'day' },
      // All time: the automatic size, an hour for this fixture's four hours of work
      { range: 'all', label: 'All time', phrase: 'over all time', bar: 'hour' },
    ];
    for (const p of presets) {
      await panel.locator(`[data-range="${p.range}"]`).click();
      await expectHeadings(p.label, p.phrase, p.bar);
    }
    // One step back keeps the preset's bar size and names the earlier period
    await panel.locator('[data-range="week"]').click();
    await panel.locator('#activity-prev').click();
    await expectHeadings('Last week', 'last week', 'day');
    // Custom: the automatic size for the range chosen
    await panel.locator('[data-range="custom"]').click();
    await expectHeadings('Custom', 'in the chosen period', /(?:\d+ minutes|hour|\d+ hours|day)/);
  });

  test('a custom period', async ({ page }) => {
    await activityPill(page).click();
    await page.locator('[data-range="custom"]').click();
    await page.locator('#activity-from').fill('2026-04-01T09:00');
    await page.locator('#activity-from').dispatchEvent('change');
    await page.locator('#activity-to').fill('2026-04-01T10:00');
    await page.locator('#activity-to').dispatchEvent('change');
    await expect(page.locator('#kpi-done')).toHaveText('3 tasks done');
  });

  test('time per task: agents alone and with you; who works now, for a period up to now (T-246)', async ({
    page,
    tasksServer,
  }) => {
    // T-003 (08:40–09:40) and T-005 (09:10–10:30) worked with you
    const data = loadFixture('agents');
    for (const t of data.tasks) if (t.id === 'T-003' || t.id === 'T-005') t.labels = ['interactive'];
    tasksServer.use(data);
    tasksServer.claims([
      {
        task: 'T-007',
        agent: 'claude:alice001',
        branch: null,
        claimed_at: '2026-04-01T11:30:00Z',
        lease_expires_at: '2026-04-01T12:30:00Z',
      },
    ]);
    await page.reload();
    await activityPill(page).click();
    // Alone: 20, 30, 50, 60 → 40 m; with you: 60, 80 → 70 m
    await expect(page.locator('#kpi-time .kpi-side')).toHaveText([
      '40 m agents alone (4 tasks)',
      '1 h 10 m with you (2 tasks)',
    ]);
    await expect(page.locator('#kpi-working-now')).toHaveText('1 working now');
    await page.locator('[data-range="day"]').click();
    await page.locator('#activity-prev').click();
    await expect(page.locator('#kpi-working-now')).toHaveCount(0);
  });

  test('the key numbers stay on one line, and idle is keyed to the hatching', async ({ page, tasksServer }) => {
    // Nobody working now, three days on: a long time such as "75 h 39 m" used to put its "m" on a line of its own
    const data = loadFixture('agents');
    for (const t of data.tasks) if (t.status === 'in_progress') t.status = 'todo';
    tasksServer.use(data);
    await page.clock.setFixedTime(new Date('2026-04-04T11:09:00Z'));
    for (const width of [1280, 560]) {
      await page.setViewportSize({ width, height: 900 });
      await page.reload();
      await activityPill(page).click();
      const idle = page.locator('#kpi-where [data-part="idle"]');
      await expect(idle.locator('strong')).toHaveText(/^\d+ h \d+ m$/);
      await expect(idle.locator('.time-split-idle')).toHaveCount(1);
      for (const value of await page.locator('.kpi-value, #kpi-where strong').all()) {
        // One line high, and nothing spilling out of its box (it's a flex item, so a block)
        const fits = await value.evaluate(
          (el) =>
            el.scrollWidth <= el.clientWidth &&
            el.getBoundingClientRect().height < 2 * parseFloat(getComputedStyle(el).fontSize),
        );
        expect(fits, `${await value.textContent()} at ${width} px`).toBe(true);
      }
      await page.keyboard.press('Escape');
    }
  });

  test('is left out when nothing has been completed yet', async ({ page, tasksServer }) => {
    const data = loadFixture('agents');
    for (const t of data.tasks) t.status = 'todo';
    tasksServer.use(data);
    await page.locator('#reload-btn').click();
    await expect(activityPill(page)).toHaveCount(0);
    await expect(progressPill(page)).toHaveAccessibleName('Progress 0%, 0 of 8 tasks done');
  });
});

test('the header stays at the top while scrolling', async ({ page }) => {
  await page.addStyleTag({ content: 'body { min-height: 3000px }' });
  const header = page.locator('header.topbar');
  await expect(header).not.toHaveClass(/scrolled/);
  await page.mouse.wheel(0, 800);
  await expect(header).toHaveClass(/scrolled/);
  await expect(header).toBeInViewport();
  expect((await header.boundingBox())!.y).toBe(0);
});
