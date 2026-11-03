import type { Page } from '@playwright/test';
import { expect, openDashboard, projectDocument, test, type TasksServer } from './helpers';

// The document viewer (T-393, docs/compmd-viewer-design.md §2, §6): a window over the details panel that shows a
// file of the project through the documents route, rendered by CompMD. Opened here by the URL hash (#doc=), as a
// link to it does; T-395 adds the links in task text.
const PROJECT = {
  meta: { project: 'Docs', repository: 'https://github.com/o/r' },
  tasks: [
    { id: 'T-1', name: 'Write the design', phase: 0, type: 'docs', status: 'todo', description: 'See the design.' },
  ],
};
const DESIGN = '# The design\n\nIt has **two** parts.\n\n## Routes\n\nOne route.\n';

const viewer = (page: Page) => page.locator('#doc-viewer');

async function openWithDocument(page: Page, tasksServer: TasksServer, hash = 'doc=docs/design.md') {
  tasksServer.setDocument(projectDocument('docs/design.md', DESIGN));
  await openDashboard(page, tasksServer, PROJECT, { tab: 'list' });
  await page.evaluate((h) => (location.hash = h), hash);
  await expect(viewer(page).locator('.compmd')).toHaveAttribute('data-compmd', 'ready');
}

async function box(page: Page) {
  return (await viewer(page).boundingBox())!;
}

test.describe('the document viewer', () => {
  test('opens a file from the URL: its title, its path, the document as CompMD renders it', async ({
    page,
    tasksServer,
  }) => {
    await openWithDocument(page, tasksServer);
    await expect(viewer(page)).toHaveAttribute('role', 'dialog');
    await expect(page.locator('#doc-title')).toHaveText('The design');
    await expect(page.locator('#doc-path')).toHaveText('docs/design.md');
    await expect(viewer(page).locator('.compmd-page h2')).toHaveText('Routes');
    await expect(viewer(page).locator('.compmd-page strong')).toHaveText('two');
    expect(tasksServer.documentRequests).toEqual(['docs/design.md']);
  });

  test('Reload keeps it open; Back from a linked document closes it, leaving the rest of the URL', async ({
    page,
    tasksServer,
  }) => {
    await openWithDocument(page, tasksServer, 'view=list&doc=docs/design.md');
    await page.reload();
    await expect(page.locator('#doc-title')).toHaveText('The design');
    await page.locator('#doc-back').click();
    await expect(viewer(page)).toHaveCount(0);
    expect(new URL(page.url()).hash).not.toContain('doc=');
    expect(new URL(page.url()).hash).toContain('view=list');
  });

  test("the browser's Back goes to the document before", async ({ page, tasksServer }) => {
    tasksServer.setDocument(projectDocument('docs/other.md', '# Other\n'));
    await openWithDocument(page, tasksServer);
    await page.evaluate(() => (location.hash = 'view=list&doc=docs/other.md'));
    await expect(page.locator('#doc-title')).toHaveText('Other');
    await page.goBack();
    await expect(page.locator('#doc-title')).toHaveText('The design');
  });

  test('Escape closes the viewer, not the task under it', async ({ page, tasksServer }) => {
    tasksServer.setDocument(projectDocument('docs/design.md', DESIGN));
    await openDashboard(page, tasksServer, PROJECT, { tab: 'list' });
    await page.locator('.task-card[data-id="T-1"] .task-name').click();
    await expect(page.locator('#detail-overlay')).toHaveClass(/open/);
    await page.evaluate(() => (location.hash = location.hash + '&doc=docs/design.md'));
    await expect(page.locator('#doc-title')).toHaveText('The design');
    await page.keyboard.press('Escape');
    await expect(viewer(page)).toHaveCount(0);
    await expect(page.locator('#detail-overlay')).toHaveClass(/open/);
    await page.keyboard.press('Escape');
    await expect(page.locator('#detail-overlay')).not.toHaveClass(/open/);
  });

  test('moves by its title bar and resizes from its corner, inside the window; kept after a reload', async ({
    page,
    tasksServer,
  }) => {
    await openWithDocument(page, tasksServer);
    const before = await box(page);
    const title = page.locator('.doc-titlebar');
    const t = (await title.boundingBox())!;
    await page.mouse.move(t.x + t.width / 2, t.y + t.height / 2);
    await page.mouse.down();
    await page.mouse.move(t.x + t.width / 2 - 120, t.y + t.height / 2 + 40, { steps: 4 });
    await page.mouse.up();
    const moved = await box(page);
    expect(Math.round(moved.x)).toBe(Math.round(before.x - 120));
    expect(Math.round(moved.y)).toBe(Math.round(before.y + 40));
    expect([Math.round(moved.width), Math.round(moved.height)]).toEqual([
      Math.round(before.width),
      Math.round(before.height),
    ]);

    // The grip: bigger and smaller, never below the minimum size
    const g = (await page.locator('#doc-grip').boundingBox())!;
    await page.mouse.move(g.x + 8, g.y + 8);
    await page.mouse.down();
    await page.mouse.move(g.x + 8 - 2000, g.y + 8 - 2000, { steps: 4 });
    await page.mouse.up();
    const small = await box(page);
    expect([Math.round(small.width), Math.round(small.height)]).toEqual([360, 240]);

    // Dragged far off, it stays inside the window
    const t2 = (await title.boundingBox())!;
    await page.mouse.move(t2.x + 60, t2.y + 10);
    await page.mouse.down();
    await page.mouse.move(t2.x + 60 + 3000, t2.y + 10 + 3000, { steps: 4 });
    await page.mouse.up();
    const viewport = page.viewportSize()!;
    const edge = await box(page);
    expect(Math.round(edge.x + edge.width)).toBeLessThanOrEqual(viewport.width);
    expect(Math.round(edge.y + edge.height)).toBeLessThanOrEqual(viewport.height);

    await page.reload();
    await expect(page.locator('#doc-title')).toHaveText('The design');
    const kept = await box(page);
    expect([Math.round(kept.x), Math.round(kept.width)]).toEqual([Math.round(edge.x), Math.round(edge.width)]);
  });

  test('the keyboard moves it (arrows on the title bar) and resizes it (Shift, or arrows on the grip)', async ({
    page,
    tasksServer,
  }) => {
    await openWithDocument(page, tasksServer);
    const before = await box(page);
    await page.locator('.doc-titlebar').focus();
    await page.keyboard.press('ArrowLeft');
    await page.keyboard.press('ArrowLeft');
    expect(Math.round((await box(page)).x)).toBe(Math.round(before.x - 32));
    await page.keyboard.press('Shift+ArrowUp');
    expect(Math.round((await box(page)).height)).toBe(Math.round(before.height - 16));
    await page.locator('#doc-grip').focus();
    await page.keyboard.press('ArrowRight');
    expect(Math.round((await box(page)).width)).toBe(Math.round(before.width + 16));
  });

  test('a narrow window: the viewer fills it, without moving or resizing', async ({ page, tasksServer }) => {
    await page.setViewportSize({ width: 600, height: 700 });
    await openWithDocument(page, tasksServer);
    await expect(viewer(page)).toHaveClass(/narrow/);
    await expect(page.locator('#doc-grip')).toHaveCount(0);
    const b = await box(page);
    expect([Math.round(b.x), Math.round(b.y), Math.round(b.width), Math.round(b.height)]).toEqual([0, 0, 600, 700]);
  });

  test('Copy path copies the path, with the lines from the link', async ({ page, tasksServer, context }) => {
    await context.grantPermissions(['clipboard-read', 'clipboard-write']);
    await openWithDocument(page, tasksServer, 'doc=docs/design.md:L3-L5');
    await expect(page.locator('#doc-path')).toHaveText('docs/design.md · lines 3–5');
    await page.locator('#doc-copy').click();
    await expect(page.locator('#doc-copy')).toHaveText('Copied');
    expect(await page.evaluate(() => navigator.clipboard.readText())).toBe('docs/design.md#L3-L5');
  });

  test('Open on GitHub: the file at the commit, in a new tab; only for a file git tracks', async ({
    page,
    tasksServer,
  }) => {
    await openWithDocument(page, tasksServer, 'doc=docs/design.md:L3');
    const link = page.locator('#doc-repo');
    await expect(link).toHaveText('Open on GitHub');
    await expect(link).toHaveAttribute(
      'href',
      `https://github.com/o/r/blob/${'c0ffee'.padEnd(40, '0')}/docs/design.md#L3`,
    );
    await expect(link).toHaveAttribute('target', '_blank');
    await expect(link).toHaveAttribute('rel', 'noopener noreferrer');

    tasksServer.setDocument(projectDocument('docs/draft.md', '# Draft\n', { tracked: false }));
    await page.evaluate(() => (location.hash = 'view=list&doc=docs/draft.md'));
    await expect(page.locator('#doc-title')).toHaveText('Draft');
    await expect(page.locator('#doc-repo')).toHaveCount(0);
  });

  test('says when a file is not in the project, or not shown', async ({ page, tasksServer }) => {
    tasksServer.refuseDocument('.env', 403, ".env: hidden files and folders aren't shown");
    await openDashboard(page, tasksServer, PROJECT, { tab: 'list' });
    await page.evaluate(() => (location.hash = 'view=list&doc=docs/gone.md'));
    await expect(page.locator('#doc-fallback')).toHaveAttribute('data-error', 'missing');
    await expect(page.locator('#doc-fallback')).toContainText('Not in the project');
    await expect(page.locator('#doc-title')).toHaveText('gone.md');
    await page.evaluate(() => (location.hash = 'view=list&doc=.env'));
    await expect(page.locator('#doc-fallback')).toHaveAttribute('data-error', 'refused');
    await expect(page.locator('#doc-fallback')).toContainText("Can't be shown here");
    await expect(page.locator('#doc-fallback')).toContainText('hidden files');
  });

  test('a link that would leave the project opens nothing', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, PROJECT, { tab: 'list' });
    await page.evaluate(() => (location.hash = 'view=list&doc=../secrets.md'));
    await expect(viewer(page)).toHaveCount(0);
    expect(tasksServer.documentRequests).toEqual([]);
  });

  test('kept current on the dashboard’s refresh: unchanged is a 304, a change shows', async ({ page, tasksServer }) => {
    await page.clock.install();
    tasksServer.setDocument(projectDocument('docs/design.md', DESIGN));
    await openDashboard(page, tasksServer, PROJECT, { tab: 'list', autoRefresh: true });
    await page.evaluate(() => (location.hash = 'view=list&doc=docs/design.md'));
    await expect(page.locator('#doc-title')).toHaveText('The design');
    await page.clock.runFor(5_000);
    await expect.poll(() => tasksServer.documentsNotModified).toBe(1);
    tasksServer.setDocument(projectDocument('docs/design.md', '# The new design\n'));
    await page.clock.runFor(5_000);
    await expect(page.locator('#doc-title')).toHaveText('The new design');
  });
});
