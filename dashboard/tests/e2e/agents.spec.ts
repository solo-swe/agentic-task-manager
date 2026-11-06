import type { Page } from '@playwright/test';
import { expect, loadFixture, openDashboard, openFilters, filterButton, test, type TasksServer } from './helpers';

// The Agents tab: a lane per agent, a bar per task from start to completion. Expected numbers: fixtures/agents.md.
const NOW = new Date('2026-04-01T12:00:00Z');
const lane = (page: Page, agent: string) => page.locator(`.agent-info[data-agent="${agent}"]`);
const bar = (page: Page, id: string) => page.locator(`.agent-bar[data-id="${id}"]`);
const range = (page: Page) => page.locator('#agents-range');
const grid = (page: Page) => page.locator('.agents-grid');

async function openAgents(page: Page, tasksServer: TasksServer, data: Parameters<typeof openDashboard>[2] = 'agents') {
  await page.clock.setFixedTime(NOW);
  await openDashboard(page, tasksServer, data, { tab: 'agents' });
  await expect(page.locator('#panel-agents')).toBeVisible();
}

/** The bars in the track next to an agent's info, by task ID */
const barsOf = (page: Page, agent: string) =>
  lane(page, agent)
    .locator('xpath=following-sibling::div[1]')
    .locator('.agent-bar')
    .evaluateAll((bars) => bars.map((b) => (b as HTMLElement).dataset.id));

test.describe('Agents tab', () => {
  test('a lane per agent, most recently active first, unattributed work last', async ({ page, tasksServer }) => {
    await openAgents(page, tasksServer);
    await expect(page.locator('.agent-name')).toHaveText(['claude:alice001', 'claude:bob00002', 'Unattributed']);
    await expect(lane(page, 'claude:alice001').locator('.agent-stats')).toHaveText('3 done · median 1 h');
    await expect(lane(page, 'claude:alice001').locator('.agent-now')).toHaveText('Now: T-007');
    await expect(lane(page, 'claude:alice001').locator('.agent-dot')).toHaveClass(/active/);
    await expect(lane(page, 'claude:bob00002').locator('.agent-stats')).toHaveText('2 done · median 1 h 5 m');
    await expect(lane(page, 'claude:bob00002').locator('.agent-dot')).not.toHaveClass(/active/);
    await expect(lane(page, '').locator('.agent-stats')).toHaveText('1 done · median 20 m');

    expect(await barsOf(page, 'claude:alice001')).toEqual(['T-001', 'T-003', 'T-006', 'T-007']);
    expect(await barsOf(page, 'claude:bob00002')).toEqual(['T-002', 'T-005']);
    expect(await barsOf(page, '')).toEqual(['T-004']);
    await expect(page.locator('.agent-bar')).toHaveCount(7); // T-008 is todo
  });

  test('bars show their task, live work is marked, and a click opens the details', async ({ page, tasksServer }) => {
    await openAgents(page, tasksServer);
    await expect(bar(page, 'T-007')).toHaveClass(/live/);
    await expect(bar(page, 'T-001')).not.toHaveClass(/live/);
    await expect(bar(page, 'T-001')).toHaveAttribute('title', 'T-001 Set up repository\n08:00 AM → 08:30 AM (30 m)');
    await expect(bar(page, 'T-007')).toHaveAttribute('title', /11:30 AM → now \(30 m\)/);
    await expect(page.locator('.agents-now')).toHaveCount(3); // the "now" line, in every lane
    await bar(page, 'T-003').click();
    await expect(page.locator('#detail-panel h3')).toHaveText('T-003 Login tests');
  });

  test('work in progress belongs to the agent holding its claim', async ({ page, tasksServer }) => {
    tasksServer.claims([
      {
        task: 'T-007',
        agent: 'claude:bob00002',
        branch: null,
        claimed_at: '2026-04-01T11:30:00Z',
        lease_expires_at: '2026-04-01T13:00:00Z',
      },
    ]);
    await openAgents(page, tasksServer);
    expect(await barsOf(page, 'claude:bob00002')).toEqual(['T-002', 'T-005', 'T-007']);
    await expect(lane(page, 'claude:bob00002').locator('.agent-now')).toHaveText('Now: T-007');
  });

  test('tasks outside the search and filters are greyed out, not hidden', async ({ page, tasksServer }) => {
    await openAgents(page, tasksServer);
    await openFilters(page);
    await filterButton(page, 'type', 'feature').click();
    await expect(page.locator('.agent-bar')).toHaveCount(7);
    await expect(page.locator('.agent-bar:not(.graph-dimmed)')).toHaveCount(3);
    await expect(bar(page, 'T-001')).toHaveClass(/graph-dimmed/);
    // Lane stats count only the matching tasks
    await expect(lane(page, 'claude:alice001').locator('.agent-stats')).toHaveText('0 done');
    await page.locator('#task-search').fill('login');
    await expect(lane(page, 'claude:bob00002')).not.toHaveClass(/graph-dimmed/);
    await expect(lane(page, 'claude:alice001')).toHaveClass(/graph-dimmed/);
    await expect(lane(page, '')).toHaveClass(/graph-dimmed/);
  });

  test('explains itself when no task has been started', async ({ page, tasksServer }) => {
    const data = loadFixture('agents');
    for (const t of data.tasks) {
      delete t.started_at;
      t.status = 'todo';
    }
    await openAgents(page, tasksServer, data);
    await expect(page.locator('.agents-empty')).toContainText('No task has a start time yet');
  });
});

test.describe('Agents tab: time range', () => {
  test('shows everything by default; presets zoom in on the latest work', async ({ page, tasksServer }) => {
    await openAgents(page, tasksServer);
    await expect(range(page)).toHaveText('08:00 AM – now');
    await expect(page.locator('[data-preset="all"]')).toHaveClass(/active/);
    await expect(page.getByRole('button', { name: 'Zoom out' })).toBeDisabled();
    await expect(page.locator('[data-preset]')).toHaveText(['1h', 'All']); // 6h and 1d are longer than the timeline

    await page.locator('[data-preset="1h"]').click();
    await expect(range(page)).toHaveText('11:00 AM – now');
    await expect(page.locator('[data-preset="1h"]')).toHaveClass(/active/);
    await expect(bar(page, 'T-001')).toHaveCount(0); // outside the window
    await expect(bar(page, 'T-007')).toBeVisible();

    await page.locator('[data-preset="all"]').click();
    await expect(range(page)).toHaveText('08:00 AM – now');
  });

  test('zoom buttons, keyboard and Ctrl + wheel', async ({ page, tasksServer }) => {
    await openAgents(page, tasksServer);
    await page.getByRole('button', { name: 'Zoom in' }).click();
    await expect(range(page)).toHaveText('08:40 AM – 11:20 AM'); // 4 h / 1.5 around the middle
    await expect(page.getByRole('button', { name: 'Zoom out' })).toBeEnabled();

    await grid(page).focus();
    await page.keyboard.press('ArrowLeft');
    await expect(range(page)).toHaveText('08:08 AM – 10:48 AM');
    await page.keyboard.press('0');
    await expect(range(page)).toHaveText('08:00 AM – now');

    // Ctrl + wheel zooms around the pointer: at the left edge, the start stays put
    const axis = (await page.locator('.agents-axis').boundingBox())!;
    await page.mouse.move(axis.x + 1, axis.y + 60);
    await page.keyboard.down('Control');
    await page.mouse.wheel(0, -200);
    await page.keyboard.up('Control');
    await expect(range(page)).toHaveText(/^08:00 AM – (?!now)/);
  });

  test('dragging the lanes moves through time; a short click still opens a task', async ({ page, tasksServer }) => {
    await openAgents(page, tasksServer);
    await page.locator('[data-preset="1h"]').click();
    const axis = (await page.locator('.agents-axis').boundingBox())!;
    // Drag right by half the track: half an hour back in time
    await page.mouse.move(axis.x + axis.width * 0.25, axis.y + 60);
    await page.mouse.down();
    await page.mouse.move(axis.x + axis.width * 0.75, axis.y + 60, { steps: 5 });
    await page.mouse.up();
    await expect(range(page)).toHaveText('10:30 AM – 11:30 AM');
    await expect(page.locator('#detail-overlay')).toBeHidden();
  });

  test('the overview picks a range, and moves the window', async ({ page, tasksServer }) => {
    await openAgents(page, tasksServer);
    const ov = (await page.locator('#agents-overview').boundingBox())!;
    const at = (fraction: number) => ov.x + ov.width * fraction;
    // Drag across the first quarter: 08:00–09:00
    await page.mouse.move(at(0), ov.y + ov.height / 2);
    await page.mouse.down();
    await page.mouse.move(at(0.25), ov.y + ov.height / 2, { steps: 5 });
    await page.mouse.up();
    await expect(range(page)).toHaveText('08:00 AM – 09:00 AM');
    await expect(page.locator('.agents-brush')).not.toHaveClass(/full/);

    // Drag the window itself one quarter to the right
    await page.mouse.move(at(0.125), ov.y + ov.height / 2);
    await page.mouse.down();
    await page.mouse.move(at(0.375), ov.y + ov.height / 2, { steps: 5 });
    await page.mouse.up();
    await expect(range(page)).toHaveText('09:00 AM – 10:00 AM');
  });

  test('Live keeps the window at now and moves it with time', async ({ page, tasksServer }) => {
    const data = loadFixture('agents');
    for (const t of data.tasks) if (t.status === 'in_progress') t.status = 'todo'; // nobody working
    await openAgents(page, tasksServer, data);
    await expect(range(page)).toHaveText('08:00 AM – 11:00 AM'); // the last work ended at 11:00
    await page.locator('#agents-live').check();
    await expect(range(page)).toHaveText('08:00 AM – now'); // Live runs up to now even with nobody working
    await page.locator('[data-preset="1h"]').click();
    await expect(range(page)).toHaveText('11:00 AM – now');
    // Zooming keeps Live, anchored at now
    await page.getByRole('button', { name: 'Zoom in' }).click();
    await expect(range(page)).toHaveText('11:20 AM – now');
    await expect(page.locator('#agents-live')).toBeChecked();
    // Time passes: the window moves along
    await page.clock.setFixedTime(new Date('2026-04-01T12:10:00Z'));
    await page.clock.runFor(5_000);
    await expect(range(page)).toHaveText('11:30 AM – now');
    // Moving into the past ends Live
    await grid(page).focus();
    await page.keyboard.press('ArrowLeft');
    await expect(page.locator('#agents-live')).not.toBeChecked();
    await page.reload();
    await expect(page.locator('#agents-live')).not.toBeChecked();
  });

  test('the range is remembered and keeps following live work', async ({ page, tasksServer }) => {
    await openAgents(page, tasksServer);
    await page.locator('[data-preset="1h"]').click();
    await page.locator('#tab-graph').click();
    // Half an hour later, the "last hour" is still the last hour
    await page.clock.setFixedTime(new Date('2026-04-01T12:30:00Z'));
    await page.goto('/');
    await page.locator('#tab-agents').click();
    await expect(range(page)).toHaveText('11:30 AM – now');

    // A window in the past stays where it was
    await page.locator('[data-preset="all"]').click();
    await page.getByRole('button', { name: 'Zoom in' }).click();
    const past = await range(page).textContent();
    await page.reload();
    await expect(range(page)).toHaveText(past!);
  });
});
