import type { Page } from '@playwright/test';
import { addressOf, expect, openDashboard, projectDocument, test } from './helpers';

// The Documents page (ATM-766, docs/documents-design.md §4): the project's documents from atm's listing, as a tree or
// by last change, searched in the header line, each opening in the viewer over the page.
const PROJECT = {
  meta: { project: 'Docs', repository: 'https://github.com/o/r' },
  tasks: [{ id: 'T-1', name: 'Write the design', type: 'docs', status: 'todo' }],
};

const list = (page: Page) => page.locator('#documents-list');
const rows = (page: Page) =>
  list(page)
    .locator('.documents-link, .documents-folder')
    .evaluateAll((els) => els.map((e) => (e as HTMLElement).dataset.path ?? `[${(e as HTMLElement).dataset.folder}]`));

test.describe('the Documents page', () => {
  test.beforeEach(async ({ page, tasksServer }) => {
    tasksServer.setDocument(projectDocument('README.md', '# Agentic Task Manager\n'));
    tasksServer.setDocument(projectDocument('docs/design.md', '# The design\n\nIt has **two** parts.\n'));
    tasksServer.setDocument(projectDocument('docs/adr/ADR-003.md', '# Layered crates\n'));
    await openDashboard(page, tasksServer, PROJECT, { tab: 'documents' });
    await expect(list(page)).toBeVisible();
  });

  test('the tree: folders, then files; a click opens the document over the page, and Back returns to the list', async ({
    page,
    tasksServer,
  }) => {
    expect(await rows(page)).toEqual(['[docs]', 'README.md']);
    await page.locator('[data-folder="docs"]').click();
    expect(await rows(page)).toEqual(['[docs]', '[docs/adr]', 'docs/design.md', 'README.md']);
    await expect(page.locator('[data-path="docs/design.md"] .documents-title')).toHaveText('The design');
    // Opened in the viewer, over the page, a step in the history
    await page.locator('[data-path="docs/design.md"]').click();
    await expect.poll(() => addressOf(page)).toMatch(/^\/projects\/fixture\/documents\?doc=docs(\/|%2F)design\.md$/);
    await expect(page.locator('#doc-title')).toHaveText('The design');
    await expect(page.locator('#doc-viewer .compmd-page strong')).toHaveText('two');
    await page.goBack();
    await expect(page.locator('#doc-viewer')).toHaveCount(0);
    await expect.poll(() => addressOf(page)).toBe('/projects/fixture/documents');
    // The folder is still open: kept for the project, a reload too
    await page.reload();
    await expect(page.locator('[data-folder="docs"]')).toHaveAttribute('aria-expanded', 'true');
    expect(tasksServer.documentListRequests.every((q) => q === '')).toBe(true);
  });

  test('the header line’s search filters titles and paths in the page; Recent lists by last change; both in the address', async ({
    page,
  }) => {
    const search = page.getByRole('searchbox', { name: 'Search documents' });
    await expect(search).toBeVisible();
    await search.fill('crates');
    await expect.poll(() => rows(page)).toEqual(['[docs]', '[docs/adr]', 'docs/adr/ADR-003.md']);
    await expect.poll(() => addressOf(page)).toBe('/projects/fixture/documents?q=crates');
    await page.locator('[data-view="recent"]').click();
    await expect.poll(() => addressOf(page)).toBe('/projects/fixture/documents?list=recent&q=crates');
    expect(await rows(page)).toEqual(['docs/adr/ADR-003.md']);
    await search.fill('');
    await expect.poll(() => rows(page).then((r) => r.length)).toBe(3);
    // A reload keeps the view
    await page.reload();
    await expect(page.locator('[data-view="recent"]')).toHaveAttribute('aria-pressed', 'true');
  });
});
