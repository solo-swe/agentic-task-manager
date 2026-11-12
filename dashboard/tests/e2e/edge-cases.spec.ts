import type { Page } from '@playwright/test';
import { expect, filterButton, openDashboard, openFilters, test } from './helpers';

const card = (page: Page, id: string) => page.locator(`.task-card[data-id="${id}"]`);
const graphNode = (page: Page, id: string) => page.locator('#dep-graph g', { hasText: id });
const MUTED = 'rgb(139, 148, 158)';
const RED = 'rgb(248, 81, 73)';

test.describe('edge cases', () => {
  test.beforeEach(async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'edge-cases', { tab: 'list' });
  });

  test('unknown type uses the fallback style', async ({ page }) => {
    const typeBadge = card(page, 'T-001').locator('.task-meta .badge').nth(0);
    await expect(typeBadge).toHaveText('spike');
    await expect(typeBadge).toHaveCSS('color', MUTED);
    await openFilters(page);
    await expect(filterButton(page, 'type', 'spike').locator('.filter-count')).toHaveText('1');
  });

  test('"bug" is styled like "bugfix"', async ({ page }) => {
    await expect(card(page, 'T-006').locator('.task-meta .badge').nth(0)).toHaveCSS('color', RED);
  });

  test('unknown status renders with the todo style but is not counted as todo', async ({ page }) => {
    await expect(card(page, 'T-002')).toHaveClass(/status-review kind-none/);
    const statusBadge = card(page, 'T-002').locator('.task-meta .badge').nth(1);
    await expect(statusBadge).toHaveText('Review');
    await expect(statusBadge).toHaveCSS('color', MUTED);
    await openFilters(page);
    await expect(filterButton(page, 'status', 'todo').locator('.filter-count')).toHaveText('5');
  });

  test('a dependency on a missing task is shown but inert', async ({ page }) => {
    const errors: string[] = [];
    page.on('pageerror', (e) => errors.push(e.message));
    const link = card(page, 'T-003').locator('.dep-link');
    await expect(link).toHaveText('T-999');
    await expect(link).toHaveAttribute('title', 'T-999');
    await link.click();

    await card(page, 'T-003').locator('.task-name').click();
    const dep = page.locator('#detail-panel .detail-dep-item');
    await expect(dep).toHaveText(/T-999/);
    await expect(dep.locator('.badge')).toHaveCount(0);
    await dep.click();
    await expect(page.locator('#detail-panel h3')).toHaveText('T-003 Depends on a missing task');
    expect(errors).toEqual([]);
  });

  // The List's card, then the graph's node (each view's built only while it is shown, T-525)
  test('long names are truncated to 22 characters in the graph only', async ({ page }) => {
    await expect(card(page, 'T-004').locator('.task-name')).toContainText('and then some');
    await page.locator('#tab-graph').click();
    await expect(graphNode(page, 'T-004')).toContainText('A very long task name…');
    await expect(graphNode(page, 'T-004')).not.toContainText('keeps going');
  });

  test('unicode survives in cards and graph', async ({ page }) => {
    await expect(card(page, 'T-005').locator('.task-name')).toHaveText('Ünïcödé naïve café 🚀 任务');
    await page.locator('#tab-graph').click();
    await expect(graphNode(page, 'T-005')).toContainText('Ünïcödé naïve café 🚀…');
  });

  test('task text is rendered as text, not HTML', async ({ page }) => {
    // Regression guard: the pre-Svelte dashboard (v0.4.0) injected task fields with innerHTML (XSS).
    await expect(card(page, 'T-006').locator('.task-name')).toHaveText('<b>Bold</b> name');
    await card(page, 'T-006').locator('.task-name').click();
    await expect(page.locator('#detail-panel')).toContainText('Has <i>markup</i> & ampersand');
  });
});
