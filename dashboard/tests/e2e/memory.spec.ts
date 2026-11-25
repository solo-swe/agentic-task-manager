import type { Page } from '@playwright/test';
import {
  addressOf,
  expect,
  follow,
  loadFixture,
  openDashboard,
  projectDocument,
  test,
  type TasksServer,
} from './helpers';

// ATM-734: the app's page was seen growing from about 70 MB to 4.5 GB over a few hours. The page refreshes every
// 5 s; each refresh that brings new data must leave no more behind than the one before. Measured in Chromium (its
// DevTools protocol: a garbage collection forced, then the JS heap and the DOM nodes alive, detached ones too).

test.skip(({ browserName }) => browserName !== 'chromium', 'measured through Chromium’s DevTools protocol');

async function alive(page: Page) {
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('Performance.enable');
  for (let i = 0; i < 3; i++) await cdp.send('HeapProfiler.collectGarbage');
  const { metrics } = await cdp.send('Performance.getMetrics');
  const of = (name: string) => metrics.find((m) => m.name === name)!.value;
  const result = {
    heapMB: Math.round((of('JSHeapUsedSize') / 1e6) * 10) / 10,
    nodes: of('Nodes'),
    listeners: of('JSEventListeners'),
  };
  await cdp.detach();
  return result;
}

/** One refresh with new data: a task's name changed, so nothing is answered 304, and the page draws it */
async function refreshWithNewData(
  page: Page,
  tasksServer: TasksServer,
  data: ReturnType<typeof loadFixture>,
  n: number,
) {
  const name = `First task, version ${n}`;
  data.tasks[0].name = name;
  tasksServer.use(data);
  const before = tasksServer.requests;
  await page.clock.runFor(5_000);
  await expect.poll(() => tasksServer.requests).toBeGreaterThan(before);
  await expect(page.getByText(name).first()).toBeAttached();
}

// A review's page (its Diffs: three files, a chain on each), read again with a change each time
const lines = (n: number, change: (i: number) => string | null = () => null) =>
  Array.from({ length: n }, (_, i) => change(i + 1) ?? `line ${i + 1}`).join('\n') + '\n';
function reviewOf(version: number) {
  const files = ['src/a.rs', 'src/b.rs', 'docs/c.md'].map((path) => ({
    path,
    old_path: path,
    status: 'modified',
    binary: false,
    patch: `diff --git a/${path} b/${path}\n--- a/${path}\n+++ b/${path}\n@@ -9,3 +9,3 @@\n line 9\n-line 10\n+LINE 10\n line 11\n`,
    before: 'base1',
    after: 'head3',
  }));
  const at = '2026-11-17T10:00:00Z';
  return {
    review: {
      id: 'r2',
      reference: 'CR-2',
      number: 2,
      title: `Review 2, version ${version}`,
      status: 'waiting',
      tasks: ['T-001'],
      author: 'claude:a',
      reviewers: [],
      created: at,
      updated: at,
    },
    summary: 'Trims the names.',
    branch: 'dev/1',
    rounds: [{ number: 1, kind: { kind: 'files' }, files: [], by: 'claude:a', at }],
    chains: files.map((f, i) => ({
      id: `t${i + 1}`,
      anchor: { round: 1, path: f.path, side: 'new', lines: [10, 10], quote: 'LINE 10' },
      comments: [
        {
          id: `c${i + 1}`,
          author: 'claude:a',
          at,
          body: `Comment ${i + 1}, version ${version}`,
          resolved: false,
          replies: [],
        },
      ],
      placed: [10, 10],
      drifted: null,
    })),
    view: { round: 1, against: null, files },
    needs_you: true,
    you: 'Ana',
  };
}

test('a review’s Diffs: read again 100 times with a change each time, the page’s memory stays where it was', async ({
  page,
  tasksServer,
}) => {
  test.setTimeout(120_000);
  const header = reviewOf(0).review;
  tasksServer.setReviews({
    reviews: [{ review: header, branch: 'dev/1', chains: 3, open: 3, needs_you: true }],
    you: 'Ana',
  });
  for (const path of ['src/a.rs', 'src/b.rs', 'docs/c.md']) {
    const fields = path.endsWith('.md') ? {} : { kind: 'code', language: 'rust', line_offset: 1 };
    tasksServer
      .setRevisionDocument('base1', { path, text: lines(40), ...fields })
      .setRevisionDocument('head3', { path, text: lines(40, (i) => (i === 10 ? 'LINE 10' : null)), ...fields });
  }
  tasksServer.setReview('CR-2', reviewOf(0));
  await openDashboard(page, tasksServer, 'standard', { tab: 'reviews' });
  await page.goto(`/projects/${tasksServer.projectId}/reviews/CR-2/diffs`);
  await expect(page.locator('.diff-view').first()).toBeVisible();
  const readAgain = async (version: number) => {
    tasksServer.setReview('CR-2', reviewOf(version));
    await page.evaluate(() => window.dispatchEvent(new Event('focus')));
    await expect(page.getByText(`Comment 3, version ${version}`).first()).toBeVisible();
  };
  // The first reads settle what the page keeps once (measured: 24 listeners more by the 60th, none after)
  for (let v = 1; v <= 60; v++) await readAgain(v);
  const start = await alive(page);
  for (let v = 61; v <= 160; v++) await readAgain(v);
  const end = await alive(page);
  console.log('review diffs: after 60 reads', JSON.stringify(start), 'after 160', JSON.stringify(end));
  expect(end.nodes).toBeLessThan(start.nodes * 1.2 + 200);
  expect(end.listeners).toBeLessThan(start.listeners + 20);
  expect(end.heapMB).toBeLessThan(start.heapMB * 1.2 + 2);
});

test('the document viewer: documents opened and closed 40 times leave nothing behind', async ({
  page,
  tasksServer,
}) => {
  test.setTimeout(120_000);
  const text = (n: number) =>
    `# Plan ${n}\n\n${Array.from({ length: 40 }, (_, i) => `Paragraph ${i + 1} with **bold** and \`code\`.`).join('\n\n')}\n\n\`\`\`rust\nfn main() {}\n\`\`\`\n`;
  tasksServer.setDocument(projectDocument('docs/one.md', text(1)));
  tasksServer.setDocument(projectDocument('docs/two.md', text(2)));
  await openDashboard(page, tasksServer, 'standard', { tab: 'list' });
  const viewer = page.locator('#doc-viewer');
  const base = addressOf(page);
  // Opened from a link, in place (no load of the page), then closed: one document, then the other
  const cycle = async () => {
    for (const n of [1, 2]) {
      await follow(page, `${base}${base.includes('?') ? '&' : '?'}doc=docs/${n === 1 ? 'one' : 'two'}.md`);
      await expect(viewer.locator('.compmd')).toHaveAttribute('data-compmd', 'ready');
      await expect(page.locator('#doc-title')).toHaveText(`Plan ${n}`);
      await page.keyboard.press('Escape');
      await expect(viewer).toHaveCount(0);
    }
  };
  for (let i = 0; i < 5; i++) await cycle();
  const start = await alive(page);
  for (let i = 0; i < 20; i++) await cycle();
  const end = await alive(page);
  console.log('document viewer: after 10 opened', JSON.stringify(start), 'after 50', JSON.stringify(end));
  expect(end.nodes).toBeLessThan(start.nodes * 1.2 + 200);
  expect(end.heapMB).toBeLessThan(start.heapMB * 1.2 + 2);
});

for (const tab of ['kanban', 'list', 'graph', 'progress', 'agents', 'project'] as const) {
  test(`the ${tab} tab: 60 refreshes with new data keep the page’s memory where it was`, async ({
    page,
    tasksServer,
  }) => {
    test.setTimeout(120_000);
    await page.clock.install();
    const data = loadFixture('standard');
    await openDashboard(page, tasksServer, data, { autoRefresh: true, tab });
    for (let n = 1; n <= 10; n++) await refreshWithNewData(page, tasksServer, data, n);
    const start = await alive(page);
    for (let n = 11; n <= 60; n++) await refreshWithNewData(page, tasksServer, data, n);
    const end = await alive(page);
    console.log(`${tab}: after 10 refreshes`, JSON.stringify(start), 'after 60', JSON.stringify(end));
    expect(end.nodes).toBeLessThan(start.nodes * 1.2 + 200);
    expect(end.listeners).toBeLessThan(start.listeners + 20);
    expect(end.heapMB).toBeLessThan(start.heapMB * 1.2 + 2);
  });
}
