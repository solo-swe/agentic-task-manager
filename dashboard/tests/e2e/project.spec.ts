import type { Page } from '@playwright/test';
import { expect, openDashboard, test, type TasksData } from './helpers';

// The Project tab (T-252): the project as a whole, Information on the left. Times are UTC on 1 May; now is 10:00.
const NOW = new Date('2026-05-01T10:00:00Z');
const at = (time: string) => `2026-05-01T${time}:00Z`;
const task = (id: string, fields: Record<string, unknown>) => ({
  id,
  name: `Task ${id}`,
  phase: 0,
  type: 'feature',
  description: '',
  status: 'todo',
  ...fields,
});

function shop(): TasksData {
  return {
    meta: {
      project: 'Shop',
      id: 'shop',
      description: 'An online shop',
      repository: 'https://github.com/acme/shop',
      workflow: {
        type: 'pull-request',
        base: 'main',
        commit_template: 'feat({id}): {name}',
        trailers: { require: ['Signed-off-by: Ana'] },
      },
      phases: [
        { id: 0, name: 'Foundations' },
        { id: 1, name: 'Checkout' },
      ],
      labels: [{ id: 'ux', label: 'UX' }],
    },
    tasks: [
      // Ana created three; two sessions did phase 0 (s2's until it asked for a review, which Ana approved)
      task('T-1', {
        created_by: 'Ana',
        created_at: '2026-04-28T09:00:00Z',
        started_by: 'claude:s1',
        started_at: at('08:00'),
        completed_by: 'claude:s1',
        completed_at: at('08:30'),
        status: 'done',
        labels: ['ux'],
        commit: 'abc1234',
        pr: { number: 7 },
      }),
      task('T-2', {
        created_by: 'Ana',
        type: 'bugfix',
        started_by: 'claude:s2',
        started_at: at('08:10'),
        review_requested_at: at('08:40'),
        completed_by: 'Ana',
        completed_at: at('09:10'),
        status: 'done',
      }),
      task('T-3', { created_by: 'Ana', phase: 1, priority: 'P0', labels: ['extra'] }),
      // A session created one that Bo works on himself
      task('T-4', {
        created_by: 'claude:s1',
        phase: 1,
        started_by: 'Bo',
        started_at: at('09:00'),
        status: 'in_progress',
      }),
    ],
  } as TasksData;
}

const items = (page: Page, block: string) => page.locator(`#project-${block} .vocab-item`);

test.describe('Project tab', () => {
  test.beforeEach(async ({ page }) => {
    await page.clock.setFixedTime(NOW);
  });

  test('the name, no description line, and Information: repository, workflow, people, agents (T-252, T-267)', async ({
    page,
    tasksServer,
  }) => {
    await openDashboard(page, tasksServer, shop(), { tab: 'project' });
    await expect(page.locator('#project-name')).toHaveText('Shop');
    // meta.description ("An online shop") isn't shown under the name
    await expect(page.locator('.project-head')).not.toContainText('An online shop');
    await expect(page.locator('#project-repository a')).toHaveAttribute('href', 'https://github.com/acme/shop');
    await expect(page.locator('#project-repository')).toHaveText('github.com/acme/shop');
    await expect(page.locator('#project-workflow > div')).toHaveText([
      'pull-request → main',
      'Verifies before pushing',
      'feat({id}): {name}',
      'Required trailers: Signed-off-by: Ana',
    ]);
    await expect(page.locator('.project-person')).toHaveText([
      'Ana created 3 · started 0 · completed 1',
      'Bo created 0 · started 1 · completed 0',
    ]);
    // s1 and s2; T-1 30 m and T-2 30 m (to its review); Bo's work isn't agent time
    await expect(page.locator('#project-agents')).toHaveText('2 sessions · started 2 · completed 1 · 1 h agent time');
  });

  test('the vocabulary with counts; closed sprints folded (T-252, T-323)', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, shop(), { tab: 'project' });
    // Not migrated yet: its phases read as sprints, the finished one closed
    await expect(items(page, 'sprints')).toHaveText(['Checkout 2', 'Backlog 0']);
    await page.locator('#project-closed-toggle').click();
    await expect(items(page, 'sprints')).toHaveText(['Checkout 2', 'Backlog 0', 'Foundations 2']);
    await expect(page.locator('#project-sprints [data-sprint="Foundations"] .sprint-marker.closed')).toBeVisible();
    await expect(page.locator('#project-components')).toHaveText('None: atm components add');
    await expect(items(page, 'labels')).toHaveText(['UX 1', 'extra 1']);
    await expect(page.locator('#project-types [data-type="feature"]')).toHaveText('Feature 3');
    await expect(page.locator('#project-types [data-type="bugfix"]')).toHaveText('Bugfix 1');
    await expect(items(page, 'statuses')).toHaveText([
      'To Do 1',
      'In Progress (active) 1',
      'In Review (review) 0',
      'Blocked 0',
      'Done 2',
      'Skipped 0',
    ]);
    await expect(items(page, 'priorities')).toHaveText([
      'P0 Critical 1',
      'P1 High 0',
      'P2 Medium (default) 3',
      'P3 Low 0',
    ]);
  });

  test('the stats: when, how much, and agent time (T-253)', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, shop(), { tab: 'project' });
    const stat = (key: string) => page.locator(`#project-stats [data-stat="${key}"]`);
    await expect(stat('started')).toHaveText('Apr 28, 2026 started');
    // From the first start (08:00) to now, tasks still open
    await expect(stat('duration')).toHaveText('2 h so far');
    await expect(stat('tasks')).toHaveText('4 tasks');
    await expect(stat('tasks')).toHaveAttribute('title', /^2 done, 2 open, 0 skipped/);
    await expect(stat('sprints')).toHaveText('2 sprints · 1 closed');
    await expect(stat('commits')).toHaveText('1 task commits');
    await expect(stat('prs')).toHaveText('1 pull requests');
    // s1 08:00–08:30 and s2 08:10–08:40 (to its review); Bo's own work isn't agent time
    await expect(stat('agent-time')).toHaveText('1 h agent time');
    await expect(stat('agent-time')).toHaveAttribute('title', /1 h on tasks agents did alone, 0 m on interactive/);
    await expect(stat('most')).toHaveText('2 most at once');
    // 60 m of agent time over 40 m with an agent at work
    await expect(page.locator('#project-parallelism')).toHaveText('1.5 agents working on average while any works');
  });

  test('time per task: stacked bars, a tooltip per bin, the slowest tasks open (T-254)', async ({
    page,
    tasksServer,
  }) => {
    // T-1 30 m alone and T-2 30 m with you (until its review): both in the 30–45 m bin
    const data = shop();
    data.tasks[1].labels = ['interactive'];
    await openDashboard(page, tasksServer, data, { tab: 'project' });
    const chart = page.locator('#time-histogram');
    await expect(chart.locator('.hist-bar.alone')).toHaveCount(1);
    await expect(chart.locator('.hist-bar.with-you')).toHaveCount(1);
    await expect(chart.locator('.hist-bin[data-from="30"] title')).toHaveText(
      '30 m–45 m: 1 alone, 1 with you\nT-1 Task T-1 (30 m)\nT-2 Task T-2 (30 m)',
    );
    await expect(chart.locator('.hist-median')).toHaveCount(2);
    await expect(chart.locator('.time-histogram-legend')).toContainText('agents alone · median 30 m');

    await expect(page.locator('#time-histogram-slowest button')).toHaveText(['T-1 30 m', 'T-2 30 m']);
    await page.locator('#time-histogram-slowest button[data-id="T-2"]').click();
    await expect(page.locator('#detail-overlay')).toHaveClass(/open/);
    await expect(page.locator('#detail-overlay')).toContainText('T-2');
  });

  test('says how to set a repository and a workflow when there are none; ignores the filters', async ({
    page,
    tasksServer,
  }) => {
    const data = shop();
    delete data.meta!.repository;
    delete data.meta!.workflow;
    delete data.meta!.description;
    await openDashboard(page, tasksServer, data, { tab: 'project' });
    await expect(page.locator('#project-repository')).toHaveText('Not set: atm repository set <url>');
    await expect(page.locator('#project-workflow')).toHaveText('Not set: /atm-push asks the first time');

    await page.locator('#task-search').fill('T-1');
    await expect(page.locator('#project-types [data-type="feature"]')).toHaveText('Feature 3');
    await page.locator('#project-info-tip').click();
    await expect(page.locator('.infotip-popover')).toContainText("The search and filters don't apply here");
  });

  test('Information and Stats end at the same height side by side; stacked, each keeps its own (T-268)', async ({
    page,
    tasksServer,
  }) => {
    await page.setViewportSize({ width: 1280, height: 900 });
    await openDashboard(page, tasksServer, shop(), { tab: 'project' });
    const info = page.locator('#project-info');
    const stats = page.locator('#project-stats');
    const box = async (l: typeof info) => (await l.boundingBox())!;
    let [a, b] = [await box(info), await box(stats)];
    expect(Math.abs(a.y - b.y)).toBeLessThan(1);
    expect(Math.abs(a.height - b.height)).toBeLessThan(1);

    // Narrow: one above the other, each as tall as its content
    await page.setViewportSize({ width: 640, height: 900 });
    [a, b] = [await box(info), await box(stats)];
    expect(b.y).toBeGreaterThan(a.y + a.height - 1);
    expect(Math.abs(a.height - b.height)).toBeGreaterThan(1);
  });
});
