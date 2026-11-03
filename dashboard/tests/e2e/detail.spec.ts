import type { Page } from '@playwright/test';
import { expect, loadFixture, openDashboard, test } from './helpers';

const overlay = (page: Page) => page.locator('#detail-overlay');
const panel = (page: Page) => page.locator('#detail-panel');
const section = (page: Page, label: string) =>
  panel(page)
    .locator('.detail-section')
    .filter({ has: page.locator('.label', { hasText: new RegExp(`^${label}$`) }) });
/** Opens a task's details, once its description is drawn: CompMD renders it a moment after the panel opens (T-284),
 * and these tests measure and scroll the panel */
async function openFromCard(page: Page, id: string) {
  await page.locator(`.task-card[data-id="${id}"] .task-name`).click();
  await expect(page.locator('#detail-description .compmd')).toHaveAttribute('data-compmd', 'ready');
}

test.describe('detail panel', () => {
  test.beforeEach(async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard', { tab: 'list' });
  });

  test('is closed initially and opens from a card name', async ({ page }) => {
    await expect(overlay(page)).toBeHidden();
    await openFromCard(page, 'T-004');
    await expect(overlay(page)).toHaveClass(/open/);
    await expect(overlay(page)).toBeVisible();
    await expect(panel(page).locator('h3')).toHaveText('T-004 User login');
  });

  test('shows all task fields', async ({ page }) => {
    await openFromCard(page, 'T-004');
    await expect(panel(page).locator('.task-meta > *')).toHaveText(['feature', 'Done', 'Phase 1', 'P2']);
    await expect(section(page, 'Description').locator('.value')).toHaveText('Description of T-004: User login.');
    await expect(section(page, 'Use Cases').locator('.uc-badge')).toHaveText(['UC-001']);
    await expect(section(page, 'Requirements').locator('.req-badge')).toHaveText(['FR-1.1', 'FR-1.2']);
    await expect(section(page, 'Timestamps')).toContainText('Created: 3/31/2026, 8:00:00 AM');
    await expect(section(page, 'Timestamps')).toContainText('Updated: 4/3/2026, 11:00:00 AM');
    await expect(section(page, 'Files').locator('.detail-files div')).toHaveText(['src/auth/login.ts']);
  });

  test('lists dependencies both ways with their statuses', async ({ page }) => {
    await openFromCard(page, 'T-004');
    const dependsOn = section(page, 'Depends On').locator('.detail-dep-item');
    await expect(dependsOn).toHaveCount(2);
    await expect(dependsOn.first()).toContainText('T-002 Write contributing guide');
    await expect(dependsOn.first().locator('.badge')).toHaveText('Done');
    const requiredBy = section(page, 'Required By').locator('.detail-dep-item');
    await expect(requiredBy).toHaveCount(2);
    await expect(requiredBy.nth(0)).toContainText('T-005 Post editor');
    await expect(requiredBy.nth(0).locator('.badge')).toHaveText('In Progress');
    await expect(requiredBy.nth(1)).toContainText('T-006 Fix session timeout');
  });

  test('dependency entries navigate to that task', async ({ page }) => {
    await openFromCard(page, 'T-004');
    await section(page, 'Required By').locator('.detail-dep-item', { hasText: 'T-005' }).click();
    await expect(panel(page).locator('h3')).toHaveText('T-005 Post editor');
    await section(page, 'Depends On').locator('.detail-dep-item', { hasText: 'T-004' }).click();
    await expect(panel(page).locator('h3')).toHaveText('T-004 User login');
  });

  test('how it ended, duplicates and related tasks, both ways and linked (SR-7.6)', async ({ page, tasksServer }) => {
    const data = loadFixture('standard');
    const set = (id: string, fields: Record<string, unknown>) =>
      Object.assign(
        data.tasks.find((t) => t.id === id)!,
        fields,
      );
    set('T-001', { related: ['T-004'] });
    set('T-009', { status: 'skipped', resolution: 'duplicate', duplicate_of: 'T-004' });
    tasksServer.use(data);
    await page.locator('#reload-btn').click();
    await openFromCard(page, 'T-004');
    await expect(section(page, 'Related').locator('.detail-dep-item')).toContainText(['T-001 Set up repository']);
    await expect(section(page, 'Duplicated By').locator('.detail-dep-item')).toContainText(['T-009']);
    await section(page, 'Duplicated By').locator('.detail-dep-item').click();
    await expect(panel(page).locator('h3')).toHaveText(/^T-009 /);
    await expect(panel(page).locator('#detail-resolution')).toHaveText('duplicate');
    await section(page, 'Duplicate Of').locator('.detail-dep-item').click();
    await expect(panel(page).locator('h3')).toHaveText('T-004 User login');
  });

  test('placeholders for missing optional fields', async ({ page }) => {
    await openFromCard(page, 'T-009');
    await expect(section(page, 'Use Cases').locator('.value')).toHaveText('None');
    await expect(section(page, 'Requirements').locator('.value')).toHaveText('None');
    await expect(section(page, 'Files').locator('.detail-files')).toHaveText('None specified');
    await expect(section(page, 'Timestamps')).toHaveCount(0);
  });

  test('a task with no relations has no dependency sections', async ({ page }) => {
    await openFromCard(page, 'T-010');
    await expect(section(page, 'Depends On')).toHaveCount(0);
    await expect(section(page, 'Required By')).toHaveCount(0);
  });

  test('a short task that fits opens with Info active, not the last group', async ({ page }) => {
    await openFromCard(page, 'T-004');
    await expect(page.locator('#detail-rail .detail-rail-item.active')).toHaveAttribute('data-group', 'info');
    await expect(page.locator('#detail-panel .kanban-more')).toHaveCount(0);
  });

  test('closes with the × button', async ({ page }) => {
    await openFromCard(page, 'T-004');
    await page.locator('#detail-panel .detail-close').click();
    await expect(overlay(page)).toBeHidden();
  });

  test('closes when clicking the backdrop, not the panel', async ({ page }) => {
    await openFromCard(page, 'T-004');
    await panel(page).locator('h3').click();
    await expect(overlay(page)).toBeVisible();
    await overlay(page).click({ position: { x: 5, y: 5 } });
    await expect(overlay(page)).toBeHidden();
  });

  test('closes with Escape', async ({ page }) => {
    await openFromCard(page, 'T-004');
    await page.keyboard.press('Escape');
    await expect(overlay(page)).toBeHidden();
  });

  test('opens from a dependency graph node', async ({ page }) => {
    await page.locator('#tab-graph').click();
    await page.locator('#dep-graph g', { hasText: 'T-007' }).click();
    await expect(overlay(page)).toBeVisible();
    await expect(panel(page).locator('h3')).toHaveText('T-007 Extract auth service');
  });
});

test.describe('detail panel scroll', () => {
  const long = Array.from({ length: 60 }, (_, i) => `Paragraph ${i + 1} of a long description.`).join('\n\n');
  const task = (id: string, name: string, extra: Record<string, unknown> = {}) => ({
    id,
    name,
    phase: 0,
    type: 'feature',
    description: long,
    status: 'todo',
    ...extra,
  });
  const data = (name = 'Long one') => ({
    meta: { project: 'Scroll' },
    tasks: [task('T-1', name), task('T-2', 'Another long one', { dependencies: ['T-1'] })],
  });
  // The sections scroll under the title, in #detail-body (T-280)
  const body = (page: Page) => page.locator('#detail-body');
  const scrollTop = (page: Page) => body(page).evaluate((el) => el.scrollTop);
  async function openScrolled(page: Page) {
    await openFromCard(page, 'T-1');
    await body(page).evaluate((el) => (el.scrollTop = el.scrollHeight));
    expect(await scrollTop(page)).toBeGreaterThan(200);
  }

  test('a rail entry brings its group to the top, even a short last group', async ({ page, tasksServer }) => {
    const activity = { created_at: '2026-01-02T10:00:00Z', notes: [{ kind: 'note', text: 'A note.' }] };
    await openDashboard(
      page,
      tasksServer,
      { ...data(), tasks: [task('T-1', 'Long one', activity), task('T-2', 'Another', { dependencies: ['T-1'] })] },
      { tab: 'list' },
    );
    await openFromCard(page, 'T-1');
    for (const group of ['activity', 'ref', 'notes', 'dependencies', 'info']) {
      await page.locator(`#detail-rail [data-group="${group}"]`).click();
      await expect(page.locator(`#detail-rail [data-group="${group}"]`)).toHaveClass(/active/);
      // Where the group's top is, relative to the top of the scroller, once the smooth scroll settles: at the top, or
      // just below the "▲ N more" hint
      const hint = group === 'info' ? 0 : 28;
      await expect
        .poll(
          () =>
            body(page).evaluate(
              (el, g) =>
                Math.round(
                  el.querySelector(`[data-group="${g}"]`)!.getBoundingClientRect().top - el.getBoundingClientRect().top,
                ),
              group,
            ),
          { message: `the ${group} group's top` },
        )
        .toBe(hint);
      if (hint)
        expect(
          await page.locator('#detail-panel .kanban-more.top').evaluate((el) => (el as HTMLElement).offsetHeight),
        ).toBe(hint);
    }
  });

  test('another task opens at its top', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, data(), { tab: 'list' });
    await openScrolled(page);
    await section(page, 'Required By').locator('.detail-dep-item', { hasText: 'T-2' }).click();
    await expect(panel(page).locator('h3')).toHaveText('T-2 Another long one');
    expect(await scrollTop(page)).toBe(0);
  });

  test('the same task re-rendered by auto-refresh keeps its position', async ({ page, tasksServer }) => {
    await page.clock.install();
    await openDashboard(page, tasksServer, data(), { tab: 'list', autoRefresh: true });
    await openScrolled(page);
    const before = await scrollTop(page);
    tasksServer.use(data('Long one, renamed'));
    await page.clock.runFor(5_000);
    await expect(panel(page).locator('h3')).toHaveText('T-1 Long one, renamed');
    expect(await scrollTop(page)).toBe(before);
  });
});

// Task text rendered with CompMD, loaded from the asset folder under the page's real CSP (T-284,
// docs/compmd-rendering-design.md). The guards fail any test that makes a request outside localhost or breaks the CSP.
test.describe('task text with CompMD', () => {
  const withText = (description: string, extra: Record<string, unknown> = {}) => ({
    meta: { project: 'CompMD' },
    tasks: [{ id: 'T-1', name: 'Rich text', phase: 0, type: 'feature', status: 'todo', description, ...extra }],
  });
  const description = (page: Page) => page.locator('#detail-description');

  test('GFM renders: a table, a task list, strikethrough; and the task’s own data', async ({ page, tasksServer }) => {
    await openDashboard(
      page,
      tasksServer,
      withText(
        '## Plan\n\n| Step | Who |\n|---|---|\n| Draft | agent |\n| Review | Hakan |\n\n' +
          '- [x] drafted\n- [ ] reviewed\n\n~~old idea~~\n\n' +
          'This is {{md.data.task.id}}, ready: :value[unknown]{of=(string(md.data.task.ready))}.',
      ),
      { tab: 'list' },
    );
    await openFromCard(page, 'T-1');
    await expect(description(page).locator('h2')).toHaveText('Plan');
    await expect(description(page).locator('table tbody tr')).toHaveCount(2);
    await expect(description(page).locator('.compmd-task input[type=checkbox]')).toHaveCount(2);
    await expect(description(page).locator('del')).toHaveText('old idea');
    // md.data.task carries atm's derived values (from the v1 route): T-1 has no dependencies, so it is ready
    await expect(description(page)).toContainText('This is T-1, ready: true.');
  });

  test('raw HTML, scripts and javascript: links run nothing', async ({ page, tasksServer }) => {
    await openDashboard(
      page,
      tasksServer,
      withText(
        '<img src=x onerror="window.ranOnerror=1"> <script>window.ranScript=1</script> ' +
          '<b onclick="window.ranClick=1">bold?</b> [click](javascript:window.ranLink=1)',
      ),
      { tab: 'list' },
    );
    await openFromCard(page, 'T-1');
    await expect(description(page).locator('img, script, b')).toHaveCount(0);
    await expect(description(page).locator('a')).toHaveCount(0);
    // The link is text now, in the paragraph with the rest: a click on it does nothing
    await expect(description(page)).toContainText('<b onclick="window.ranClick=1">bold?</b> click');
    await description(page).locator('p').click();
    const ran = await page.evaluate(() => {
      const w = window as unknown as Record<string, unknown>;
      return [w.ranOnerror, w.ranScript, w.ranClick, w.ranLink];
    });
    expect(ran).toEqual([undefined, undefined, undefined, undefined]);
  });

  test('an image from elsewhere shows its text and is never requested; web links open in a new tab', async ({
    page,
    tasksServer,
  }) => {
    const requested: string[] = [];
    page.on('request', (r) => requested.push(r.url()));
    await openDashboard(
      page,
      tasksServer,
      withText(
        '![a chart](https://example.com/chart.png) <img src="https://example.com/x.png"> [docs](https://example.com)',
      ),
      { tab: 'list' },
    );
    await openFromCard(page, 'T-1');
    await expect(description(page)).toContainText('a chart');
    await expect(description(page).locator('img')).toHaveCount(0);
    await expect(description(page).locator('a')).toHaveAttribute('target', '_blank');
    await expect(description(page).locator('a')).toHaveAttribute('rel', 'noopener noreferrer');
    expect(requested.filter((url) => url.includes('example.com'))).toEqual([]);
  });

  test('notes, the review note and questions render with CompMD too', async ({ page, tasksServer }) => {
    await openDashboard(
      page,
      tasksServer,
      withText('Plain.', {
        status: 'blocked',
        waiting_on: 'user',
        questions: ['Keep `--force`?'],
        notes: [{ at: '2026-04-01T08:00:00Z', by: 'Hakan', kind: 'note', text: '| a |\n|---|\n| 1 |' }],
      }),
      { tab: 'list' },
    );
    await openFromCard(page, 'T-1');
    await expect(panel(page).locator('.note-text table td')).toHaveText('1');
    await expect(panel(page).locator('.block-questions li code')).toHaveText('--force');
  });

  test('when CompMD can’t load, the text shows as written', async ({ page, tasksServer }) => {
    await page.route('**/assets/compmd/**', (route) => route.fulfill({ status: 404, body: 'not found' }));
    await openDashboard(page, tasksServer, withText('## Plan\n\nStep **one**.'), { tab: 'list' });
    await page.locator('.task-card[data-id="T-1"] .task-name').click();
    await expect(description(page).locator('.compmd')).toHaveAttribute('data-compmd', 'failed');
    await expect(description(page).locator('.compmd-plain')).toHaveText('## Plan\n\nStep **one**.');
  });
});
