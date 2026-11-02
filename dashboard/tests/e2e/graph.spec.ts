import type { Page } from '@playwright/test';
import { expect, filterButton, openDashboard, openFilters, test } from './helpers';

const nodes = (page: Page) => page.locator('#dep-graph g');
const edges = (page: Page) => page.locator('#dep-graph path[marker-end]');

/** Each task's left edge, by ID. */
async function lefts(page: Page): Promise<Record<string, number>> {
  const placed = await nodes(page).evaluateAll((gs) =>
    gs.map((g) => [g.getAttribute('data-id')!, Number(g.querySelector('rect')?.getAttribute('x'))] as const),
  );
  return Object.fromEntries(placed);
}

test.describe('dependency graph', () => {
  test('one node per task and one edge per dependency', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard', { tab: 'graph' });
    await expect(nodes(page)).toHaveCount(12);
    await expect(edges(page)).toHaveCount(12);
  });

  test('every task is to the right of what it waits for', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard', { tab: 'graph' });
    // Once laid out: read at once, the graph could still be empty
    await expect(nodes(page)).toHaveCount(12);
    const x = await lefts(page);
    expect(x['T-004']).toBeGreaterThan(x['T-002']);
    expect(x['T-004']).toBeGreaterThan(x['T-003']);
    expect(x['T-011']).toBeGreaterThan(x['T-009']);
    expect(x['T-011']).toBeGreaterThan(x['T-007']);
  });

  test('nodes show id and name', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard', { tab: 'graph' });
    const node = nodes(page).filter({ hasText: 'T-004' });
    await expect(node).toContainText('User login');
  });

  test('a dependency cycle still renders every task', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'cycle', { tab: 'graph' });
    await expect(nodes(page)).toHaveCount(5);
    await expect(edges(page)).toHaveCount(5);
  });

  test('hidden dependencies are ignored for layout and edges', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard', { tab: 'graph' });
    await openFilters(page);
    await filterButton(page, 'sprint', 'Phase 2').click();
    await expect(nodes(page)).toHaveCount(4);
    await expect(edges(page)).toHaveCount(2);
    // T-008 waits on phase 1 tasks, filtered out: it starts its chain at the left
    const x = await lefts(page);
    expect(x['T-008']).toBeLessThan(x['T-009']);
    expect(x['T-009']).toBeLessThan(x['T-011']);
  });

  test('hovering a task highlights it and everything it waits for; the rest fades', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard', { tab: 'graph' });
    await nodes(page).filter({ hasText: 'T-008' }).hover();
    const chain = page.locator('#dep-graph g.graph-chain');
    await expect(chain).toHaveCount(7);
    const ids = await chain.evaluateAll((gs) => gs.map((g) => g.getAttribute('data-id')).sort());
    expect(ids).toEqual(['T-001', 'T-002', 'T-003', 'T-004', 'T-005', 'T-006', 'T-008']);
    await expect(page.locator('#dep-graph g.graph-hovered')).toHaveAttribute('data-id', 'T-008');
    await expect(page.locator('#dep-graph g.graph-faded')).toHaveCount(5);
    // The arrows inside the chain: T-001→T-002, T-001→T-003, T-002→T-004, T-003→T-004, T-004→T-005, T-004→T-006,
    // T-005→T-008, T-006→T-008
    await expect(page.locator('#dep-graph path.graph-chain')).toHaveCount(8);

    await page.mouse.move(2, 2);
    await expect(page.locator('#dep-graph .graph-chain, #dep-graph .graph-faded')).toHaveCount(0);
  });

  test('tasks outside the filters can be hidden (the default) or greyed out', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard', { tab: 'graph' });
    const choice = page.locator('#graph-filtered');
    await expect(choice).toHaveCount(0); // nothing is filtered out yet
    await page.locator('#task-search').fill('editor');
    await expect(nodes(page)).toHaveCount(3);
    await expect(choice.locator('[data-mode="hide"]')).toHaveAttribute('aria-pressed', 'true');

    await choice.locator('[data-mode="dim"]').click();
    await expect(nodes(page)).toHaveCount(12);
    await expect(page.locator('#dep-graph g.graph-dimmed')).toHaveCount(9);
    await expect(nodes(page).filter({ hasText: 'T-005' })).not.toHaveClass(/graph-dimmed/);
    await expect(page.locator('#dep-graph path.graph-dimmed').first()).toBeAttached();

    await choice.locator('[data-mode="hide"]').click();
    await expect(nodes(page)).toHaveCount(3);
  });

  test('"Dim done" greys out the done tasks, whatever the filters', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard', { tab: 'graph' });
    const dimDone = page.locator('#graph-dim-done');
    await expect(dimDone).toHaveAttribute('aria-pressed', 'false');
    await expect(page.locator('#dep-graph g.graph-dimmed')).toHaveCount(0);

    await dimDone.click();
    await expect(dimDone).toHaveAttribute('aria-pressed', 'true');
    const dimmed = page.locator('#dep-graph g.graph-dimmed');
    await expect(dimmed).toHaveCount(5); // the fixture's done tasks
    for (const text of await dimmed.allTextContents()) expect(text).toContain('\u{2705}');
    await expect(page.locator('#dep-graph path.graph-dimmed').first()).toBeAttached();

    // A status filter showing only done tasks keeps them dimmed
    await openFilters(page);
    await filterButton(page, 'status', 'done').click();
    await expect(nodes(page)).toHaveCount(5);
    await expect(dimmed).toHaveCount(5);
  });
});
