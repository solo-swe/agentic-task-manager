import { readFileSync } from 'node:fs';
import type { Page } from '@playwright/test';
import { RECENT_RELEASES } from '../../build/changelog';
import { parseChangelog } from '../../src/lib/changelog';
import { expect, openDashboard, test } from './helpers';

const pluginVersion: string = JSON.parse(
  readFileSync(new URL('../../../plugin/.claude-plugin/plugin.json', import.meta.url), 'utf8'),
).version;
// What's new shows the short notes for users, not the changelog
const entries = parseChangelog(readFileSync(new URL('../../../plugin/WHATS-NEW.md', import.meta.url), 'utf8'));

const chip = (page: Page) => page.locator('#version-chip');
const overlay = (page: Page) => page.locator('#whats-new-overlay');
const panel = (page: Page) => page.locator('#whats-new-panel');

test.describe("What's new", () => {
  test.beforeEach(async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard');
  });

  test('the header shows the dashboard version', async ({ page }) => {
    // First visit: the "new" marker sits inside the chip, after the version
    await expect(chip(page)).toHaveText(new RegExp(`^v${pluginVersion.replace(/\./g, '\\.')}`));
    // The title keeps its exact text; the chip sits next to it
    await expect(page.locator('#dashboard-title')).toHaveText('Standard Project');
  });

  test('the chip opens release notes, newest first', async ({ page }) => {
    await expect(overlay(page)).toBeHidden();
    await chip(page).click();
    await expect(overlay(page)).toBeVisible();
    const versions = await panel(page)
      .locator('.release')
      .evaluateAll((els) => els.map((el) => (el as HTMLElement).dataset.version));
    // Only the newest releases (build/changelog.ts); the rest are linked
    expect(versions).toEqual(entries.slice(0, RECENT_RELEASES).map((e) => e.version));

    const v0270 = panel(page).locator('.release[data-version="0.27.0"]');
    await expect(v0270.locator('> h4')).toHaveText(/^v0\.27\.0\s*2026-11-03$/);
    // Rendered with CompMD; plain bullets: no Added / Changed / Fixed headings
    await expect(v0270.locator('.compmd')).toHaveAttribute('data-compmd', 'ready');
    await expect(v0270.locator('.compmd-page :is(h1, h2, h3, h4, h5, h6)')).toHaveCount(0);
    await expect(v0270.locator('.compmd-page li')).toHaveCount(
      entries.find((e) => e.version === '0.27.0')!.sections.flatMap((s) => s.items).length,
    );
    await expect(v0270.locator('li').first()).toContainText('no longer blanks the dashboard');
  });

  test('older releases are a link to every release’s notes, in a new tab', async ({ page }) => {
    await chip(page).click();
    const older = panel(page).locator('.whats-new-older a');
    await expect(older).toHaveText('all release notes');
    await expect(older).toHaveAttribute(
      'href',
      'https://github.com/solo-swe/agentic-task-manager/blob/main/plugin/WHATS-NEW.md',
    );
    await expect(older).toHaveAttribute('target', '_blank');
    await expect(older).toHaveAttribute('rel', 'noopener noreferrer');
    // Markup renders, it never shows as written
    await expect(panel(page)).not.toContainText('`');
  });

  test('the releases scroll under the title without a scrollbar, with hints that count and scroll (T-282)', async ({
    page,
  }) => {
    await chip(page).click();
    // Every release's notes drawn by CompMD: until then only the version headings take room
    await expect(panel(page).locator('.compmd:not([data-compmd="ready"])')).toHaveCount(0);
    const body = panel(page).locator('#whats-new-body');
    const sizes = await body.evaluate((el) => ({
      overflows: el.scrollHeight > el.clientHeight,
      scrollbar: (el as HTMLElement).offsetWidth - el.clientWidth,
      panelScrolls: el.closest('#whats-new-panel')!.scrollHeight > el.closest('#whats-new-panel')!.clientHeight,
    }));
    expect(sizes).toEqual({ overflows: true, scrollbar: 0, panelScrolls: false });

    // At the top: nothing above, the releases cut off below counted
    const cutBelow = () =>
      body.evaluate((el) => {
        const bottom = el.scrollTop + el.clientHeight;
        return [...el.querySelectorAll<HTMLElement>('.release')].filter(
          (r) => r.offsetTop + r.offsetHeight > bottom + 1,
        ).length;
      });
    await expect(panel(page).locator('.kanban-more.top')).toHaveCount(0);
    await expect(panel(page).locator('.kanban-more.bottom')).toHaveText(`▼ ${await cutBelow()} more`);

    // A hairline under the title, at its full size at the top
    const titleStyle = () =>
      panel(page)
        .locator('> h3')
        .evaluate((h) => {
          const s = getComputedStyle(h);
          return { line: s.borderBottomStyle + ' ' + s.borderBottomWidth, size: parseFloat(s.fontSize) };
        });
    expect(await titleStyle()).toEqual({ line: 'solid 1px', size: 20 });

    // ▼ scrolls a page; then ▲ shows, and the title compacts (as the dashboard's title bar does) and stays in view
    await panel(page).locator('.kanban-more.bottom').click();
    await expect.poll(() => body.evaluate((el) => el.scrollTop)).toBeGreaterThan(100);
    await expect(panel(page).locator('.kanban-more.top')).toHaveText(/^▲ \d+ more$/);
    await expect(panel(page)).toHaveClass(/scrolled/);
    await expect.poll(titleStyle).toEqual({ line: 'solid 1px', size: 16 });
    await expect(panel(page).locator('> h3')).toBeInViewport();
  });

  test('closes with ×, the backdrop and Escape', async ({ page }) => {
    await chip(page).click();
    await panel(page).locator('.detail-close').click();
    await expect(overlay(page)).toBeHidden();

    await chip(page).click();
    await panel(page).locator('h3').click();
    await expect(overlay(page)).toBeVisible();
    await overlay(page).click({ position: { x: 5, y: 5 } });
    await expect(overlay(page)).toBeHidden();

    await chip(page).click();
    await page.keyboard.press('Escape');
    await expect(overlay(page)).toBeHidden();
  });
});

test.describe('"new" marker', () => {
  const marker = (page: Page) => page.locator('#version-chip .version-new');

  test('shows until the release notes are opened, and stays gone after a reload', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard');
    await expect(marker(page)).toBeVisible();
    await chip(page).click();
    await expect(marker(page)).toHaveCount(0);
    await page.keyboard.press('Escape');
    await page.reload();
    await expect(chip(page)).toHaveText(`v${pluginVersion}`);
    await expect(marker(page)).toHaveCount(0);
  });

  test('comes back after an upgrade', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard');
    await chip(page).click();
    // Pretend the last version whose notes were read is older than this dashboard
    await page.evaluate(() => {
      const prefs = JSON.parse(localStorage.getItem('atm-dashboard') || '{}');
      localStorage.setItem('atm-dashboard', JSON.stringify({ ...prefs, lastSeenVersion: '0.1.0' }));
    });
    await page.reload();
    await expect(marker(page)).toBeVisible();
  });

  test('is never shown when storage is unavailable, and nothing breaks', async ({ page, tasksServer }) => {
    const errors: string[] = [];
    page.on('pageerror', (e) => errors.push(e.message));
    await page.addInitScript(() => {
      Object.defineProperty(window, 'localStorage', {
        get() {
          throw new DOMException('denied', 'SecurityError');
        },
      });
    });
    await openDashboard(page, tasksServer, 'standard');
    await expect(chip(page)).toHaveText(`v${pluginVersion}`);
    await expect(marker(page)).toHaveCount(0);
    await chip(page).click();
    await expect(overlay(page)).toBeVisible();
    expect(errors).toEqual([]);
  });
});
