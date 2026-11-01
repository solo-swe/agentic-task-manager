import type { Page } from '@playwright/test';
import { expect, filterButton, loadFixture, openDashboard, test, type TasksData, openFilters } from './helpers';

// Expected numbers: tests/fixtures/progress.md (now = 2026-04-06 12:00 UTC; the tests run in UTC, en-US)
const NOW = new Date('2026-04-06T12:00:00Z');
// Its phases read as sprints ("Phase n"): the fixtures are not migrated
const row = (page: Page, phase: number | string) =>
  page.locator(`#sprint-table tr[data-sprint="${typeof phase === 'number' ? `Phase ${phase}` : phase}"]`);
const cells = (page: Page, phase: number | string) => row(page, phase).locator('td');

async function openProgress(page: Page, tasksServer: Parameters<typeof openDashboard>[1], data: string | TasksData) {
  await page.clock.setFixedTime(NOW);
  await openDashboard(page, tasksServer, data);
  await page.locator('#tab-progress').click();
  await expect(page.locator('#progress')).toBeVisible();
}

test.describe('Progress tab: sprint table', () => {
  test('says what the estimates are based on', async ({ page, tasksServer }) => {
    await openProgress(page, tasksServer, 'progress');
    await expect(page.locator('#progress-basis')).toContainText(
      'Pace: ~9 m per task (last 8 tasks, one agent at a time)',
    );
    await expect(page.locator('#progress-basis span[title]')).toHaveAttribute('title', /8 m – 11 m/);
  });

  test('one row per sprint with progress and when it finishes, the calendar time first (T-241)', async ({
    page,
    tasksServer,
  }) => {
    await openProgress(page, tasksServer, 'progress');
    await expect(page.locator('#sprint-table tbody tr')).toHaveCount(3);
    await expect(page.locator('#sprint-table th')).toHaveText([
      'Sprint',
      'Done',
      'Progress',
      'Remaining',
      'Finished by',
    ]);

    // All its tasks done: a closed sprint
    await expect(cells(page, 0)).toHaveText(['✓ Phase 0', '4/4', '', '0', 'Closed Apr 1, 09:30 AM']);
    await expect(row(page, 0)).toHaveAttribute('data-state', 'done');
    await expect(row(page, 0)).toHaveAttribute('data-sprint-state', 'closed');
    await expect(row(page, 1).locator('.sprint-state-name')).toHaveText('planned');

    await expect(cells(page, 1).nth(1)).toHaveText('4/6');
    // 1 task left to work on (T-009), plus a warning for the blocked T-010, which the estimate doesn't cover
    await expect(cells(page, 1).nth(3)).toHaveText(/^1/);
    await expect(cells(page, 1).nth(3).locator('.phase-blocked')).toHaveText('1 blocked');
    await expect(cells(page, 1).nth(4).locator('.estimate-main')).toHaveText('≈ 12:08 PM');
    await expect(cells(page, 1).nth(4).locator('.estimate-calendar')).toHaveText('9 m of agent work');
    await expect(cells(page, 1).nth(4)).toHaveAttribute('title', /9 m of agent work \(8 m – 11 m\)/);

    // Not started: muted; its estimate includes Phase 1's work, and says so
    await expect(row(page, 2)).toHaveClass(/not-started/);
    await expect(cells(page, 2).nth(4).locator('.estimate-main')).toHaveText('≈ 12:51 PM');
    await expect(cells(page, 2).nth(4).locator('.estimate-calendar')).toHaveText(
      '51 m of agent work, including earlier sprints',
    );
  });

  test("a named phase's sprint has its name; the ⓘ explains the estimates (T-241)", async ({ page, tasksServer }) => {
    const data = loadFixture('progress');
    data.meta = { ...data.meta, phases: [{ id: 1, name: 'Core features' }] };
    await openProgress(page, tasksServer, data);
    await expect(row(page, 'Core features').locator('.phase-badge')).toHaveText('Core features');
    await expect(row(page, 1)).toHaveCount(0);
    await page.locator('#sprint-table-info').click();
    await expect(page.locator('.infotip-popover')).toContainText('includes the work left in the ones before it');
  });

  test('a migrated project: the active sprint, its due date, the backlog last, a closed sprint with its report', async ({
    page,
    tasksServer,
  }) => {
    await openProgress(page, tasksServer, 'sprints');
    await expect(page.locator('#sprint-table tbody tr')).toHaveCount(3);
    expect(
      await page
        .locator('#sprint-table tbody tr')
        .evaluateAll((rows) => rows.map((r) => r.getAttribute('data-sprint'))),
    ).toEqual(['Foundations', 'Ada Lovelace', ':backlog']);
    await expect(row(page, 'Ada Lovelace').locator('.sprint-state-name')).toHaveText('active · due 2026-04-12');
    await expect(row(page, ':backlog').locator('.phase-badge')).toHaveText('Backlog');
    await expect(cells(page, 'Foundations').nth(4)).toContainText('Closed Apr 5');
    await expect(cells(page, 'Foundations').nth(4).locator('.sprint-report')).toHaveText('1 carried to Ada Lovelace');
  });

  // The progress fixture plus phase 3, finished on Mar 1, before phase 0 (Apr 1): two finished phases. No start
  // time, so the pace doesn't change.
  function twoFinishedPhases(): TasksData {
    const data = loadFixture('progress');
    data.tasks.push({
      id: 'T-099',
      name: 'Early wrap-up',
      phase: 3,
      type: 'docs',
      description: '',
      status: 'done',
      completed_at: '2026-03-01T10:00:00Z',
    });
    return data;
  }

  test('closed sprints fold into one row that opens to show them (T-241)', async ({ page, tasksServer }) => {
    await openProgress(page, tasksServer, twoFinishedPhases());
    await expect(page.locator('#active-phases-only')).toHaveCount(0);
    // The folded row, then Phase 1 and Phase 2
    await expect(page.locator('#sprint-table tbody tr')).toHaveCount(3);
    await expect(page.locator('#sprint-closed')).toContainText('✓ 2 closed sprints · 5 tasks · last closed Apr 1');
    await expect(row(page, 0)).toHaveCount(0);
    await expect(row(page, 3)).toHaveCount(0);
    // The burndown still covers every sprint
    await expect(page.locator('#burndown-focus')).toHaveText('All sprints');

    await page.locator('#sprint-closed-toggle').click();
    await expect(page.locator('#sprint-table tbody tr')).toHaveCount(5);
    await expect(row(page, 0)).toHaveCount(1);
    await expect(row(page, 3)).toHaveCount(1);
    // A closed sprint's row still focuses the burndown
    await row(page, 3).click();
    await expect(page.locator('#burndown-focus')).toHaveText('Phase 3');
  });

  test('the Progress panel: rings for open sprints, one chip for the closed ones that opens the tab on the last (T-243)', async ({
    page,
    tasksServer,
  }) => {
    const data = twoFinishedPhases();
    // Phase 4 finished too: three closed sprints
    data.tasks.push({ ...data.tasks.at(-1)!, id: 'T-100', phase: 4 });
    await openProgress(page, tasksServer, data);
    await page.locator('#tab-kanban').click();

    await page.locator('#progress-card').click();
    const panel = page.locator('#progress-panel');
    await expect(panel.locator('#panel-active-phases-only')).toHaveCount(0);
    await expect(panel.locator('#progress-by-sprint .ring-label')).toHaveText(['Phase 1', 'Phase 2']);
    const chips = panel.locator('#progress-closed-chips .finished-chip');
    await expect(chips).toHaveText(['✓ 3 closed']);
    await expect(chips).toHaveAttribute('title', /^3 sprints: closed, the last on Apr 1/);

    await chips.click();
    await expect(panel).toHaveCount(0);
    await expect(page.locator('#progress')).toBeVisible();
    await expect(page.locator('#burndown-focus')).toHaveText('Phase 4');
  });

  test('progress bars show each phase', async ({ page, tasksServer }) => {
    await openProgress(page, tasksServer, 'progress');
    const width = (phase: number) =>
      row(page, phase)
        .locator('.progress-fill')
        .evaluate((el) => (el as HTMLElement).style.width);
    expect([await width(0), await width(1), await width(2)]).toEqual(['100%', '67%', '0%']);
  });

  test('skipped tasks count as neither done nor left: done and skipped is 100%, the skipped shown apart', async ({
    page,
    tasksServer,
  }) => {
    const task = (id: string, status: string) => ({ id, name: id, phase: 1, type: 'feature', description: '', status });
    await openProgress(page, tasksServer, {
      meta: { project: 'Skips' },
      tasks: [task('T-1', 'done'), task('T-2', 'done'), task('T-3', 'skipped')],
    });
    await expect(cells(page, 1).nth(1)).toHaveText('2/2 · 1 skipped');
    await expect(row(page, 1)).toHaveAttribute('data-state', 'done');
    await expect(row(page, 1).locator('.progress-fill')).toHaveAttribute('style', /width: 100%/);

    await page.locator('#progress-card').click();
    await expect(page.locator('#progress-card')).toHaveAccessibleName('Progress 100%, 2 of 2 tasks done');
    await expect(page.locator('#overall-skipped')).toHaveText('· 1 skipped');
    // A thin outer arc on the overall ring; screen readers hear the count. Phase 1 is closed: a chip, not a ring
    await expect(page.locator('#progress-panel .ring-skip-arc')).toHaveCount(1);
    await expect(page.getByRole('img', { name: 'All tasks 100% done, 1 skipped' })).toBeVisible();
    await expect(page.locator('#progress-closed-chips')).toHaveText('✓ Phase 1');
  });

  test('says when there is not enough history', async ({ page, tasksServer }) => {
    await openProgress(page, tasksServer, 'minimal');
    await expect(page.locator('#progress-basis')).toContainText('Not enough history to estimate yet');
    await expect(cells(page, 0).nth(4)).toHaveText('Not enough history to estimate');
  });

  test('idle gaps are capped when tasks have no start times', async ({ page, tasksServer }) => {
    // standard.json only has updated_at, with hours between completions: every measured duration is the 30 min cap
    await openProgress(page, tasksServer, 'standard');
    await expect(page.locator('#progress-basis')).toContainText('Pace: ~30 m per task (last 4 tasks');
  });

  test('filters choose the rows; the pace and each sprint’s finish stay the whole project’s (T-239)', async ({
    page,
    tasksServer,
  }) => {
    // Phase 2's tasks, by a label: the Sprint filter scopes the board views, not this tab (T-323)
    const data = loadFixture('progress');
    for (const task of data.tasks) if (task.phase === 2) task.labels = ['ui'];
    await openProgress(page, tasksServer, data);
    await openFilters(page);
    await filterButton(page, 'label', 'ui').click();
    await expect(page.locator('#sprint-table tbody tr')).toHaveCount(1);
    await expect(page.locator('#progress-basis')).toContainText('last 8 tasks');
    // Not phase 2's 5 tasks alone (43 m): the loop finishes phase 1 first, so the whole project's 51 m
    await expect(cells(page, 2).nth(4)).toContainText('51 m of agent work');
    await expect(page.locator('#progress-filter-banner')).toContainText('Showing 6 of 16 tasks: label ui');
    await expect(page.locator('#burndown-focus')).toHaveText('All sprints · matching tasks');
    await page.locator('#progress-clear-filters').click();
    await expect(page.locator('#progress-filter-banner')).toHaveCount(0);
    await expect(page.locator('#sprint-table tbody tr')).toHaveCount(3);
  });

  test('the status and stage filters don’t apply on this tab, and the banner says so (T-239)', async ({
    page,
    tasksServer,
  }) => {
    await openProgress(page, tasksServer, 'progress');
    const summary = await page.locator('#burndown svg').getAttribute('aria-label');
    await openFilters(page);
    await filterButton(page, 'status', 'done').click();
    await expect(page.locator('#progress-filter-ignored')).toHaveText(/^Status filter not applied here/);
    await expect(page.locator('#burndown svg')).toHaveAttribute('aria-label', summary!);
    await expect(page.locator('#sprint-table tbody tr')).toHaveCount(3);
  });

  test('empty filter result', async ({ page, tasksServer }) => {
    await openProgress(page, tasksServer, 'progress');
    await page.locator('#task-search').fill('no task is called this');
    await expect(page.locator('#progress')).toContainText('No tasks match current filters');
  });
});

test.describe('Progress tab: burndown chart', () => {
  test('summarises the burndown for screen readers', async ({ page, tasksServer }) => {
    await openProgress(page, tasksServer, 'progress');
    await expect(page.locator('#burndown svg')).toHaveAttribute(
      'aria-label',
      'Burndown: 7 of 15 tasks remaining, forecast done around 12:51 PM',
    );
  });

  test('draws remaining, scope and the forecast with direct labels and a legend', async ({ page, tasksServer }) => {
    await openProgress(page, tasksServer, 'progress');
    for (const cls of ['burndown-remaining', 'burndown-scope', 'burndown-forecast', 'burndown-fan']) {
      await expect(page.locator(`#burndown .${cls}`)).toHaveCount(1);
    }
    await expect(page.locator('.burndown-label-remaining')).toHaveText('Remaining 7');
    await expect(page.locator('.burndown-label-scope')).toHaveText('Scope 15');
    await expect(page.locator('.burndown-forecast-label')).toHaveText('done ≈ 12:51 PM');
    await expect(page.locator('.burndown-legend')).toHaveText(/Remaining\s*Scope\s*Forecast/);
  });

  test('hovering shows the values at that time, or the forecast after now', async ({ page, tasksServer }) => {
    await openProgress(page, tasksServer, 'progress');
    const hit = (await page.locator('.burndown-hit').boundingBox())!;
    const tooltip = page.locator('.burndown-tooltip');
    await expect(tooltip).toHaveCount(0);

    // Halfway through the history (Apr 3–4): phase 2 has been added, T-008 is done
    await page.mouse.move(hit.x + hit.width * 0.5, hit.y + 20);
    await expect(tooltip).toContainText('Remaining 7');
    await expect(tooltip).toContainText('Scope 15');
    await expect(page.locator('.burndown-crosshair')).toHaveCount(1);

    // Just after "now": the forecast
    await page.mouse.move(hit.x + hit.width - 1, hit.y + 20);
    await expect(tooltip).toContainText('Forecast');
    await expect(tooltip).toContainText('Done ≈ 12:51 PM');
    await expect(tooltip).toContainText('Range 12:45 PM – 01:03 PM');

    await page.mouse.move(hit.x - 30, hit.y - 30);
    await expect(tooltip).toHaveCount(0);
  });

  test('no forecast without enough history', async ({ page, tasksServer }) => {
    const data = loadFixture('minimal');
    for (const task of data.tasks) task.created_at = '2026-04-01T09:00:00Z';
    await openProgress(page, tasksServer, data);
    await expect(page.locator('#burndown .burndown-remaining')).toHaveCount(1);
    await expect(page.locator('#burndown .burndown-forecast')).toHaveCount(0);
    await expect(page.locator('.burndown-legend')).not.toContainText('Forecast');
  });

  test('without any times on the tasks, it says there is no timeline yet (T-240)', async ({ page, tasksServer }) => {
    await openProgress(page, tasksServer, 'minimal');
    await expect(page.locator('#burndown-empty')).toHaveText(
      'No timeline yet: tasks get times when agents start and finish them.',
    );
    await expect(page.locator('#burndown')).toHaveCount(0);
  });

  test('the forecast ends at what waits on people, and says so (T-240)', async ({ page, tasksServer }) => {
    // progress.json: T-010 is blocked, so the forecast ends at 1, not 0
    await openProgress(page, tasksServer, 'progress');
    await expect(page.locator('#burndown-waiting')).toHaveText('1 waiting on review or a decision');
  });
});

test.describe('Progress tab: a sprint on the burndown (SR-7.4)', () => {
  test("a closed sprint's burndown ends where it closed; the active sprint's due date is marked", async ({
    page,
    tasksServer,
  }) => {
    // fixtures/sprints.json, with times: a timeline to draw
    const data = loadFixture('sprints');
    for (const task of data.tasks) task.created_at = '2026-04-01T09:00:00Z';
    data.tasks[0].completed_at = '2026-04-04T09:00:00Z';
    await openProgress(page, tasksServer, data);
    await expect(page.locator('#burndown-due')).toHaveCount(1);
    await expect(page.locator('#burndown .burndown-now')).toHaveCount(1);
    await row(page, 'Foundations').click();
    await expect(page.locator('#burndown-focus')).toHaveText('Foundations');
    await expect(page.locator('#burndown .burndown-now')).toHaveCount(0);
    await expect(page.locator('#burndown-due')).toHaveCount(0);
  });
});

test.describe('Progress tab: releases (SR-7.5)', () => {
  test('each open release with its sprints, its tasks by stage, and a notes draft to copy', async ({
    page,
    tasksServer,
  }) => {
    await page.context().grantPermissions(['clipboard-read', 'clipboard-write']);
    await openProgress(page, tasksServer, 'sprints');
    const rows = page.locator('#release-view .release-row');
    await expect(rows).toHaveCount(2);
    const first = page.locator('.release-row[data-release="1.0"]');
    await expect(first.locator('.release-sprints')).toHaveText('Foundations, Ada Lovelace');
    await expect(first.locator('.release-counts')).toHaveText('1 of 3 done · 0 in PR · 0 merged');
    await first.locator('summary').click();
    await expect(first.locator('pre')).toContainText('## Not done yet');
    await first.getByRole('button', { name: 'Copy' }).click();
    await expect(first.getByRole('button', { name: 'Copied' })).toBeVisible();
    expect(await page.evaluate(() => navigator.clipboard.readText())).toMatch(/^# 1\.0 “First”/);
  });

  test('a project without releases shows none', async ({ page, tasksServer }) => {
    await openProgress(page, tasksServer, 'progress');
    await expect(page.locator('#release-view')).toHaveCount(0);
  });
});

test.describe('Progress tab: burndown title, info and range (T-240)', () => {
  test('the title names the scope; the ⓘ explains the chart and closes on Escape', async ({ page, tasksServer }) => {
    await openProgress(page, tasksServer, 'progress');
    await expect(page.locator('.burndown-title')).toContainText('Burndown · All sprints');
    await page.locator('#burndown-info').click();
    const popover = page.locator('.infotip-popover');
    await expect(popover).toContainText('one agent, one task at a time');
    await expect(popover).toContainText('interactive');
    await page.keyboard.press('Escape');
    await expect(popover).toHaveCount(0);
  });

  test('the range: since the sprint the loop works on started by default, 14 days or all time; remembered', async ({
    page,
    tasksServer,
  }) => {
    await openProgress(page, tasksServer, 'progress');
    const range = page.locator('#burndown-range button');
    await expect(range).toHaveText(['Since Phase 1 started', 'Last 14 days', 'All time']);
    await expect(page.locator('#burndown-range [aria-pressed="true"]')).toHaveText('Since Phase 1 started');
    const sinceLabels = await page.locator('#burndown text.burndown-axis').allTextContents();
    await page.locator('#burndown-range [data-range="all"]').click();
    await expect(page.locator('#burndown-range [aria-pressed="true"]')).toHaveText('All time');
    await expect
      .poll(async () => (await page.locator('#burndown text.burndown-axis').allTextContents()).join())
      .not.toBe(sinceLabels.join());
    await page.reload();
    await page.locator('#tab-progress').click();
    await expect(page.locator('#burndown-range [aria-pressed="true"]')).toHaveText('All time');
  });
});

test.describe('Progress tab: focus a sprint', () => {
  const label = (page: Page) => page.locator('#burndown svg');

  test('clicking a sprint row limits the burndown to that sprint; clicking again resets', async ({
    page,
    tasksServer,
  }) => {
    await openProgress(page, tasksServer, 'progress');
    await expect(page.locator('#burndown-focus')).toHaveText('All sprints');
    await row(page, 1).click();
    await expect(row(page, 1)).toHaveClass(/selected/);
    await expect(page.locator('#burndown-focus')).toHaveText('Phase 1');
    // Phase 1: 6 tasks, 2 not done (T-009 in progress, T-010 blocked); its estimate covers T-009 only
    await expect(label(page)).toHaveAttribute(
      'aria-label',
      'Burndown: 2 of 6 tasks remaining, forecast done around 12:08 PM',
    );
    await row(page, 1).click();
    await expect(page.locator('#burndown-focus')).toHaveText('All sprints');
    await expect(row(page, 1)).not.toHaveClass(/selected/);
  });

  test('"Show all sprints", the keyboard, and filters that hide the sprint', async ({ page, tasksServer }) => {
    await openProgress(page, tasksServer, 'progress');
    await row(page, 0).focus();
    await page.keyboard.press('Enter');
    await expect(page.locator('#burndown-focus')).toHaveText('Phase 0');
    await expect(label(page)).toHaveAttribute('aria-label', 'Burndown: 0 of 4 tasks remaining');
    await page.getByRole('button', { name: 'Show all sprints' }).click();
    await expect(page.locator('#burndown-focus')).toHaveText('All sprints');

    await row(page, 2).click();
    await openFilters(page);
    // The Sprint filter scopes the board views only (T-323): the focus stays
    await filterButton(page, 'sprint', 'Phase 1').click();
    await expect(page.locator('#burndown-focus')).toHaveText('Phase 2');
    // A search that leaves only phase 0's T-001 hides the focused sprint
    await page.locator('#task-search').fill('Project skeleton');
    await expect(page.locator('#burndown-focus')).toHaveText('All sprints · matching tasks');
  });
});

test.describe('Progress tab: where the time went (T-242)', () => {
  // fixtures/agents.md: work from 08:00 to now (12:00, T-007 running), nobody working 11:00–11:30; 5 h 30 m of agent
  // work; alice, bob and the unattributed T-004 all at work 09:10–09:20
  test('agents working, waiting on you and idle over the burndown range, with the agent time', async ({
    page,
    tasksServer,
  }) => {
    await page.clock.setFixedTime(new Date('2026-04-01T12:00:00Z'));
    await openDashboard(page, tasksServer, 'agents', { tab: 'progress' });
    await page.locator('#burndown-range [data-range="all"]').click();
    const section = page.locator('#progress-time-use');
    await expect(section.locator('.status-legend li')).toHaveText([
      'Agents working3 h 30 m',
      'Waiting on you0 m',
      'Idle30 m',
    ]);
    await expect(section.locator('.time-split-agents')).toHaveText('5 h 30 m of agent time, up to 3 agents at once');
    await expect(page.locator('#utilization')).toHaveCount(0);

    // The last 14 days cover all of it too; a range is measured from its start or the first work
    await page.locator('#burndown-range [data-range="14d"]').click();
    await expect(section.locator('[data-part="idle"]')).toHaveText('Idle30 m');
  });

  test('is left out before any task has started', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard', { tab: 'progress' });
    await expect(page.locator('#progress-time-use')).toHaveCount(0);
  });
});
