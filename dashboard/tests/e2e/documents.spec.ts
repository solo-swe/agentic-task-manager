import type { Page } from '@playwright/test';
import { expect, openDashboard, openMenu, projectDocument, test, type TasksServer } from './helpers';

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
    // × closes it too, the task staying open
    await page.evaluate(() => (location.hash = location.hash + '&doc=docs/design.md'));
    await expect(page.locator('#doc-title')).toHaveText('The design');
    await page.locator('#doc-close').click();
    await expect(viewer(page)).toHaveCount(0);
    expect(new URL(page.url()).hash).not.toContain('doc=');
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

  test('a page on a desk; scrolled, the title bar compacts, as the dialogs’ do, and comes back at the top', async ({
    page,
    tasksServer,
  }) => {
    const long = '# A long design\n\n' + Array.from({ length: 60 }, (_, i) => `Paragraph ${i + 1}.`).join('\n\n');
    tasksServer.setDocument(projectDocument('docs/long.md', long));
    await openDashboard(page, tasksServer, PROJECT, { tab: 'list' });
    await page.evaluate(() => (location.hash = 'view=list&doc=docs/long.md'));
    await expect(viewer(page).locator('.doc-page .compmd')).toHaveAttribute('data-compmd', 'ready');
    // The page stands apart from the desk under it
    const colours = await viewer(page).evaluate((el) => ({
      desk: getComputedStyle(el.querySelector('.doc-body')!).backgroundColor,
      page: getComputedStyle(el.querySelector('.doc-page')!).backgroundColor,
      border: getComputedStyle(el.querySelector('.doc-page')!).borderTopStyle,
    }));
    expect(colours.page).not.toBe(colours.desk);
    expect(colours.border).toBe('solid');

    const titleSize = () => page.locator('#doc-title').evaluate((h) => parseFloat(getComputedStyle(h).fontSize));
    expect(await titleSize()).toBe(15);
    await expect(page.locator('#doc-path')).toBeVisible();
    await page.locator('#doc-body').evaluate((el) => el.scrollTo({ top: 400 }));
    await expect(viewer(page)).toHaveClass(/scrolled/);
    await expect.poll(titleSize).toBe(13); // after its transition
    await expect.poll(() => page.locator('#doc-path').evaluate((el) => getComputedStyle(el).opacity)).toBe('0');
    await page.locator('#doc-body').evaluate((el) => el.scrollTo({ top: 0 }));
    await expect(viewer(page)).not.toHaveClass(/scrolled/);
    await expect.poll(titleSize).toBe(15);
  });

  test('the Hand-drawn theme writes the document on a ruled notebook page', async ({ page, tasksServer }) => {
    await page.addInitScript(() =>
      localStorage.setItem('atm-dashboard', JSON.stringify({ theme: 'hand-drawn', mode: 'light', autoRefresh: false })),
    );
    await openWithDocument(page, tasksServer, 'view=list&doc=docs/design.md');
    const page_ = viewer(page).locator('.doc-page');
    expect(await page_.evaluate((el) => getComputedStyle(el).backgroundImage)).toContain('repeating-linear-gradient');
    expect(await page_.locator('.compmd').evaluate((el) => getComputedStyle(el).lineHeight)).toBe('26px');
  });

  test('a document style from Settings gives the page its own type and column, over the theme’s page (T-404)', async ({
    page,
    tasksServer,
  }) => {
    await page.addInitScript(() => {
      if (!sessionStorage.getItem('seeded')) {
        sessionStorage.setItem('seeded', '1');
        localStorage.setItem('atm-dashboard', JSON.stringify({ theme: 'hand-drawn', autoRefresh: false }));
      }
    });
    await openWithDocument(page, tasksServer, 'view=list&doc=docs/design.md');
    const page_ = viewer(page).locator('.doc-page');
    const style = (prop: 'fontFamily' | 'fontSize' | 'maxWidth' | 'backgroundImage', sel = '.compmd') =>
      (sel ? page_.locator(sel) : page_).evaluate((el, p) => getComputedStyle(el)[p], prop);
    await expect(viewer(page)).not.toHaveAttribute('data-doc-style');
    await openMenu(page, '#settings-btn');
    await expect(page.locator('#doc-style-select')).toHaveValue('auto');
    await expect(page.locator('#doc-style-select option')).toHaveText([
      'Auto (the theme’s page)',
      'GitHub',
      'Docs',
      'Magazine',
    ]);
    for (const [id, font, size, width] of [
      ['github', 'Segoe UI', '16px', '980px'],
      ['docs', 'Charter', '18px', '816px'],
      ['magazine', 'Bodoni', '17px', '760px'],
    ]) {
      await page.locator('#doc-style-select').selectOption(id);
      await expect(viewer(page)).toHaveAttribute('data-doc-style', id);
      expect(await style('fontSize')).toBe(size);
      expect(await style('maxWidth', '')).toBe(width);
      // The theme's notebook page gives way to the style's own
      expect(await style('backgroundImage', '')).toBe('none');
      const heading = await page_.locator('h1').evaluate((el) => getComputedStyle(el).fontFamily);
      expect(id === 'magazine' ? heading : await style('fontFamily')).toContain(font);
    }
    await page.reload();
    await expect(viewer(page).locator('.compmd')).toHaveAttribute('data-compmd', 'ready');
    await expect(viewer(page)).toHaveAttribute('data-doc-style', 'magazine');
    await openMenu(page, '#settings-btn');
    await page.locator('#doc-style-select').selectOption('auto');
    await expect(viewer(page)).not.toHaveAttribute('data-doc-style');
    expect(await style('backgroundImage', '')).toContain('repeating-linear-gradient');
  });
});

// T-394: a code or text file's line numbers, #L links, images, refreshing in place, links to the dashboard
test.describe('the document viewer: lines, images, refreshing', () => {
  const CODE = Array.from({ length: 120 }, (_, i) => `const line${i + 1} = ${i + 1};`).join('\n') + '\n';
  const codeDoc = (text = CODE) =>
    projectDocument('src/app.ts', text, {
      kind: 'code',
      language: 'ts',
      document: '```ts\n' + text + '```\n',
      line_offset: 1,
      lines: text.split('\n').length - 1,
    });

  test('a code file: a number per line in a gutter as tall as the code, each beside its line', async ({
    page,
    tasksServer,
  }) => {
    tasksServer.setDocument(codeDoc());
    await openDashboard(page, tasksServer, PROJECT, { tab: 'list' });
    await page.evaluate(() => (location.hash = 'view=list&doc=src/app.ts'));
    const gutter = page.locator('#doc-gutter');
    await expect(gutter.locator('div')).toHaveCount(120);
    const sizes = await page.locator('#doc-viewer').evaluate((el) => {
      const g = el.querySelector('#doc-gutter')!.getBoundingClientRect();
      const code = el.querySelector('.compmd-page pre code')!.getBoundingClientRect();
      return { gutterTop: g.top, gutterHeight: g.height, codeTop: code.top, codeHeight: code.height };
    });
    expect(Math.abs(sizes.gutterTop - sizes.codeTop)).toBeLessThan(2);
    expect(Math.abs(sizes.gutterHeight - sizes.codeHeight)).toBeLessThan(2);
  });

  test('#L40 opens at line 40, marked; a range marks its lines', async ({ page, tasksServer }) => {
    tasksServer.setDocument(codeDoc());
    await openDashboard(page, tasksServer, PROJECT, { tab: 'list' });
    await page.evaluate(() => (location.hash = 'view=list&doc=src/app.ts:L40-L42'));
    const mark = page.locator('#doc-line-mark');
    await expect(mark).toBeInViewport();
    await expect.poll(() => page.locator('#doc-body').evaluate((el) => el.scrollTop)).toBeGreaterThan(200);
    await expect(page.locator('#doc-gutter .marked')).toHaveText(['40', '41', '42']);
    // The mark covers lines 40 to 42 of the code
    const covered = await page.locator('#doc-viewer').evaluate((el) => {
      const m = el.querySelector('#doc-line-mark')!.getBoundingClientRect();
      const rows = [...el.querySelectorAll('#doc-gutter div')].map((d) => d.getBoundingClientRect());
      return { top: m.top - rows[39].top, bottom: m.bottom - rows[41].bottom };
    });
    expect(Math.abs(covered.top)).toBeLessThan(2);
    expect(Math.abs(covered.bottom)).toBeLessThan(2);
  });

  test('a Markdown document has no gutter yet (T-400: its lines come from CompMD)', async ({ page, tasksServer }) => {
    tasksServer.setDocument(projectDocument('docs/design.md', DESIGN));
    await openDashboard(page, tasksServer, PROJECT, { tab: 'list' });
    await page.evaluate(() => (location.hash = 'view=list&doc=docs/design.md:L3'));
    await expect(viewer(page).locator('.compmd')).toHaveAttribute('data-compmd', 'ready');
    await expect(page.locator('#doc-gutter')).toHaveCount(0);
  });

  test('a changed file re-renders where the reader is, not at the top', async ({ page, tasksServer }) => {
    await page.clock.install();
    tasksServer.setDocument(codeDoc());
    await openDashboard(page, tasksServer, PROJECT, { tab: 'list', autoRefresh: true });
    await page.evaluate(() => (location.hash = 'view=list&doc=src/app.ts'));
    await expect(page.locator('#doc-gutter div')).toHaveCount(120);
    await page.locator('#doc-body').evaluate((el) => el.scrollTo({ top: 600 }));
    const before = await page.locator('#doc-body').evaluate((el) => el.scrollTop);
    tasksServer.setDocument(codeDoc(CODE.replace('const line50 = 50;', 'const line50 = 5000;')));
    await page.clock.runFor(5_000);
    await expect(page.locator('#doc-viewer .compmd-page code')).toContainText('5000');
    expect(await page.locator('#doc-body').evaluate((el) => el.scrollTop)).toBe(before);
  });

  test('a relative image loads from this server’s files route; one from elsewhere shows its alt text', async ({
    page,
    tasksServer,
  }) => {
    const png = Buffer.from(
      'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=',
      'base64',
    );
    tasksServer.setImage('docs/img/flow.png', 'image/png', png);
    tasksServer.setDocument(
      projectDocument(
        'docs/design.md',
        '# Design\n\n![the flow](img/flow.png)\n\n![a badge](https://example.com/b.svg)\n',
      ),
    );
    await openDashboard(page, tasksServer, PROJECT, { tab: 'list' });
    await page.evaluate(() => (location.hash = 'view=list&doc=docs/design.md'));
    const img = viewer(page).locator('.compmd-page img');
    await expect(img).toHaveCount(1);
    await expect(img).toHaveAttribute('src', /api\/v1\/projects\/[^/]+\/files\?path=docs%2Fimg%2Fflow\.png$/);
    await expect.poll(() => img.evaluate((el) => (el as HTMLImageElement).naturalWidth)).toBe(1);
    expect(tasksServer.fileRequests).toEqual(['docs/img/flow.png']);
    await expect(viewer(page).locator('.compmd-page')).toContainText('a badge');
  });

  test('a link to the dashboard itself opens here, not in a new tab', async ({ page, tasksServer }) => {
    tasksServer.setDocument(projectDocument('docs/design.md', DESIGN));
    await openDashboard(page, tasksServer, PROJECT, { tab: 'list' });
    const here = page.url().split('#')[0];
    tasksServer.setDocument(
      projectDocument('docs/index.md', `# Index\n\n[the design](${here}#view=list&doc=docs/design.md)\n`),
    );
    await page.evaluate(() => (location.hash = 'view=list&doc=docs/index.md'));
    const link = viewer(page).locator('.compmd-page a');
    await expect(link).toHaveText('the design');
    await expect(link).not.toHaveAttribute('target', '_blank');
    await link.click();
    await expect(page.locator('#doc-title')).toHaveText('The design');
    expect(page.context().pages()).toHaveLength(1);
    await page.goBack();
    await expect(page.locator('#doc-title')).toHaveText('Index');
  });
});

// T-395: the references that open the viewer (docs/compmd-viewer-design.md V1a–V1c)
test.describe('the document viewer: links that open it', () => {
  const withDescription = (description: string, files: string[] = []) => ({
    meta: { project: 'Docs' },
    tasks: [{ id: 'T-1', name: 'Write the design', phase: 0, type: 'docs', status: 'todo', description, files }],
  });

  async function openTask(page: Page, tasksServer: TasksServer, data: ReturnType<typeof withDescription>) {
    await openDashboard(page, tasksServer, data, { tab: 'list' });
    await page.locator('.task-card[data-id="T-1"] .task-name').click();
    await expect(page.locator('#detail-description .compmd')).toHaveAttribute('data-compmd', 'ready');
  }

  test('a relative link in task text opens the file, at its heading or lines; Back returns to the task', async ({
    page,
    tasksServer,
  }) => {
    tasksServer.setDocument(projectDocument('docs/design.md', DESIGN));
    await openTask(page, tasksServer, withDescription('See [the routes](docs/design.md#routes).'));
    const link = page.locator('#detail-description a', { hasText: 'the routes' });
    await expect(link).not.toHaveAttribute('target', '_blank');
    await link.click();
    await expect(page.locator('#doc-title')).toHaveText('The design');
    await expect(viewer(page).locator('.compmd-page h2', { hasText: 'Routes' })).toBeInViewport();
    expect(new URL(page.url()).hash).toContain('doc=docs/design.md%23routes');
    await page.locator('#doc-back').click();
    await expect(viewer(page)).toHaveCount(0);
    await expect(page.locator('#detail-overlay')).toHaveClass(/open/);
  });

  test('a link inside a document is resolved from its folder', async ({ page, tasksServer }) => {
    tasksServer.setDocument(projectDocument('docs/a/index.md', '# Index\n\nSee [the model](../model.md).\n'));
    tasksServer.setDocument(projectDocument('docs/model.md', '# The model\n'));
    await openTask(page, tasksServer, withDescription('Start at [the index](docs/a/index.md).'));
    await page.locator('#detail-description a', { hasText: 'the index' }).click();
    await expect(page.locator('#doc-title')).toHaveText('Index');
    await viewer(page).locator('.compmd-page a', { hasText: 'the model' }).click();
    await expect(page.locator('#doc-title')).toHaveText('The model');
    expect(tasksServer.documentRequests).toEqual(['docs/a/index.md', 'docs/model.md']);
    // Back goes to the document before; × closes them both
    await page.locator('#doc-back').click();
    await expect(page.locator('#doc-title')).toHaveText('Index');
  });

  test('every entry of the Files list opens the viewer; a folder stays text', async ({ page, tasksServer }) => {
    tasksServer.setDocument(projectDocument('src/app.ts', 'x\n', { kind: 'code', language: 'ts' }));
    await openTask(page, tasksServer, withDescription('Plain.', ['src/app.ts', 'plugin/assets/']));
    const files = page.locator('.detail-files');
    await expect(files.locator('.file-link')).toHaveText(['src/app.ts']);
    await expect(files).toContainText('plugin/assets/');
    await files.locator('.file-link').click();
    await expect(page.locator('#doc-title')).toHaveText('app.ts');
  });

  test('a link out of the project stays text; one to another site opens in a new tab', async ({
    page,
    tasksServer,
  }) => {
    await openTask(
      page,
      tasksServer,
      withDescription('[secrets](../../etc/passwd) and [the site](https://example.com).'),
    );
    const description = page.locator('#detail-description');
    await expect(description.locator('a')).toHaveCount(1);
    await expect(description.locator('a')).toHaveAttribute('target', '_blank');
    await expect(description).toContainText('secrets');
    expect(tasksServer.documentRequests).toEqual([]);
  });

  // T-391: a reference by its text (https://web.dev/articles/text-fragments)
  const SPEC =
    '# Requirements\n\n| ID | Rule |\n|---|---|\n| SR-2.1 | Sprint states |\n| SR-2.2 | Due dates |\n\n' +
    Array.from({ length: 40 }, (_, i) => `Filler paragraph ${i + 1}.`).join('\n\n') +
    '\n\n## Later\n\nThe last rule is here.\n';
  const highlighted = (page: Page) =>
    page.evaluate(() => {
      const h = (CSS as unknown as { highlights: Map<string, Iterable<Range>> }).highlights.get('atm-doc-fragment');
      return h ? [...h].map((r) => r.toString()) : [];
    });

  test('a text-fragment link opens the document at the text, marked; a click clears the mark', async ({
    page,
    tasksServer,
  }) => {
    tasksServer.setDocument(projectDocument('docs/spec.md', SPEC));
    await openTask(page, tasksServer, withDescription('Per [the last rule](docs/spec.md#:~:text=last%20rule).'));
    await page.locator('#detail-description a', { hasText: 'the last rule' }).click();
    await expect(page.locator('#doc-title')).toHaveText('Requirements');
    await expect.poll(() => highlighted(page)).toEqual(['last rule']);
    await expect(viewer(page).locator('.compmd-page p', { hasText: 'The last rule is here.' })).toBeInViewport();
    await page.locator('#doc-body').click({ position: { x: 5, y: 5 } });
    await expect.poll(() => highlighted(page)).toEqual([]);
  });

  test('a requirement written as a link opens its definition; a bare ID stays text', async ({ page, tasksServer }) => {
    tasksServer.setDocument(projectDocument('docs/spec.md', SPEC));
    const data = withDescription('Plain.');
    Object.assign(data.tasks[0], { requirements: ['[SR-2.2](docs/spec.md#:~:text=SR%2D2.2)', 'SR-9'] });
    await openTask(page, tasksServer, data);
    // The card shows the link's text; the details panel the link, one entry a line (T-403)
    await expect(page.locator('.task-card[data-id="T-1"] .req-badge')).toHaveText(['SR-2.2', 'SR-9']);
    const entries = page.locator('#detail-panel .detail-ref');
    await expect(entries).toHaveText(['SR-2.2', 'SR-9'], { useInnerText: true });
    await expect(entries.nth(1).locator('a')).toHaveCount(0);
    const [first, second] = [await entries.nth(0).boundingBox(), await entries.nth(1).boundingBox()];
    expect(second!.y).toBeGreaterThanOrEqual(first!.y + first!.height);
    await entries.nth(0).locator('a').click();
    await expect(page.locator('#doc-title')).toHaveText('Requirements');
    await expect.poll(() => highlighted(page)).toEqual(['SR-2.2']);
  });

  test('text the document doesn’t have: it opens at the top, saying so', async ({ page, tasksServer }) => {
    tasksServer.setDocument(projectDocument('docs/spec.md', SPEC));
    await openTask(page, tasksServer, withDescription('[gone](docs/spec.md#:~:text=no%20such%20rule)'));
    await page.locator('#detail-description a', { hasText: 'gone' }).click();
    await expect(page.locator('#doc-fragment-missing')).toContainText("isn't in this document");
    expect(await page.locator('#doc-body').evaluate((el) => el.scrollTop)).toBe(0);
  });
});

// T-402: the contents, out of the way until wanted
test.describe('the document viewer: contents', () => {
  const LONG =
    '# Guide\n\n' +
    ['Setup', 'Usage', 'Reference']
      .map(
        (s) =>
          `## ${s}\n\n` +
          Array.from({ length: 12 }, (_, i) => `${s} paragraph ${i + 1}.`).join('\n\n') +
          `\n\n### ${s} details\n\nMore.\n`,
      )
      .join('\n');

  async function open(page: Page, tasksServer: TasksServer, text = LONG) {
    tasksServer.setDocument(projectDocument('docs/guide.md', text));
    await openDashboard(page, tasksServer, PROJECT, { tab: 'list' });
    await page.evaluate(() => (location.hash = 'view=list&doc=docs/guide.md'));
    await expect(viewer(page).locator('.compmd')).toHaveAttribute('data-compmd', 'ready');
  }

  test('a faint button; hovering it shows the headings by level; a pick scrolls there and hides the list', async ({
    page,
    tasksServer,
  }) => {
    await open(page, tasksServer);
    const button = page.locator('#doc-toc-button');
    await expect(button).toBeVisible();
    await expect(page.locator('.doc-toc-list')).toBeHidden();
    // At the page's top left
    const place = await page.locator('#doc-viewer').evaluate((el) => ({
      box: el.querySelector('.doc-toc-box')!.getBoundingClientRect().left,
      page: el.querySelector('.doc-page')!.getBoundingClientRect(),
    }));
    expect(place.box).toBeLessThan(place.page.left + place.page.width / 4);
    await button.hover();
    const entries = page.locator('.doc-toc-list button');
    await expect(entries).toHaveText([
      'Guide',
      'Setup',
      'Setup details',
      'Usage',
      'Usage details',
      'Reference',
      'Reference details',
    ]);
    await expect(entries.nth(2)).toHaveClass(/doc-toc-level-3/);
    await entries.filter({ hasText: /^Reference$/ }).click();
    await expect(page.locator('.doc-toc-list')).toBeHidden();
    await expect(viewer(page).locator('.compmd-page h2', { hasText: 'Reference' })).toBeInViewport();
    // The button stays in view as the document scrolls
    await expect(button).toBeInViewport();
  });

  test('the section being read is marked; the keyboard opens and walks the list; Escape closes it first', async ({
    page,
    tasksServer,
  }) => {
    await open(page, tasksServer);
    await viewer(page)
      .locator('.compmd-page h2', { hasText: 'Usage' })
      .evaluate((h) => h.scrollIntoView({ block: 'start' }));
    await page.locator('#doc-toc-button').focus();
    await expect(page.locator('.doc-toc-list .current')).toHaveText('Usage');
    await page.keyboard.press('ArrowDown');
    await expect(page.locator('.doc-toc-list button').first()).toBeFocused();
    await page.keyboard.press('ArrowUp');
    await expect(page.locator('.doc-toc-list button').last()).toBeFocused();
    await page.keyboard.press('Escape');
    await expect(page.locator('.doc-toc-list')).toBeHidden();
    await expect(viewer(page)).toHaveCount(1); // the viewer stays: Escape closed the list
  });

  test('the button grows into the list: moving from it to an entry never leaves it, so it stays open', async ({
    page,
    tasksServer,
  }) => {
    await open(page, tasksServer);
    const b = (await page.locator('#doc-toc-button').boundingBox())!;
    await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2);
    await expect(page.locator('#doc-toc')).toHaveClass(/open/);
    const last = page.locator('.doc-toc-list button').last();
    await expect(last).toBeVisible();
    const target = (await last.boundingBox())!;
    // A slow move, in many steps, straight from the button to the last entry
    await page.mouse.move(target.x + 20, target.y + target.height / 2, { steps: 25 });
    await expect(page.locator('#doc-toc')).toHaveClass(/open/);
    await page.mouse.down();
    await page.mouse.up();
    await expect(viewer(page).locator('.compmd-page h3', { hasText: 'Reference details' })).toBeInViewport();
  });

  test('no contents for a document with one heading, or for code', async ({ page, tasksServer }) => {
    await open(page, tasksServer, '# Only one\n\nText.\n');
    await expect(page.locator('#doc-toc-button')).toHaveCount(0);
  });
});
