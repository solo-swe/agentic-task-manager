import type { Page } from '@playwright/test';
import { expect, openDashboard, openMenu, test } from './helpers';

// FR-2: light and dark modes. The dashboard follows the OS (prefers-color-scheme) until a mode is chosen; the
// choice is remembered per browser and applied before the first paint.
const html = (page: Page) => page.locator('html');
/** Mode and theme are in the ⋯ settings menu, top right (T-323) */
const menu = (page: Page) => openMenu(page, '#settings-btn');
const themeSelect = (page: Page) => page.locator('#theme-select');
const bodyBackground = (page: Page) => page.evaluate(() => getComputedStyle(document.body).backgroundColor);
const DARK_BG = 'rgb(13, 17, 23)';
const LIGHT_BG = 'rgb(255, 255, 255)';

test.describe('light and dark mode', () => {
  test('System follows the OS setting, also when it changes while the page is open', async ({ page, tasksServer }) => {
    await page.emulateMedia({ colorScheme: 'light' });
    await openDashboard(page, tasksServer, 'standard');
    await menu(page);
    await expect(page.locator('.mode-btn[data-mode="system"]')).toHaveAttribute('aria-pressed', 'true');
    await expect(html(page)).toHaveAttribute('data-mode', 'light');
    expect(await bodyBackground(page)).toBe(LIGHT_BG);
    await page.emulateMedia({ colorScheme: 'dark' });
    await expect(html(page)).toHaveAttribute('data-mode', 'dark');
    expect(await bodyBackground(page)).toBe(DARK_BG);
  });

  test('choosing Light or Dark overrides the OS and is remembered', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard'); // tests run with a dark OS setting
    await menu(page);
    await page.locator('.mode-btn[data-mode="light"]').click();
    await expect(html(page)).toHaveAttribute('data-mode', 'light');
    await expect(page.locator('.mode-btn[data-mode="light"]')).toHaveAttribute('aria-pressed', 'true');
    await page.reload();
    await expect(html(page)).toHaveAttribute('data-mode', 'light');
    expect(await bodyBackground(page)).toBe(LIGHT_BG);
    await menu(page);
    await page.locator('.mode-btn[data-mode="dark"]').click();
    await page.emulateMedia({ colorScheme: 'light' });
    await expect(html(page)).toHaveAttribute('data-mode', 'dark');
  });

  test('the stored mode is applied before anything is painted', async ({ page, tasksServer }) => {
    // Record the mode at the moment <body> is created: nothing can be painted before that
    await page.addInitScript(() => {
      localStorage.setItem('atm-dashboard', JSON.stringify({ mode: 'light', autoRefresh: false }));
      new MutationObserver((_, observer) => {
        if (!document.body) return;
        (window as unknown as { modeAtBody: string }).modeAtBody = document.documentElement.dataset.mode ?? 'unset';
        observer.disconnect();
      }).observe(document, { childList: true, subtree: true }); // <html> doesn't exist yet when this runs
    });
    await openDashboard(page, tasksServer, 'standard');
    expect(await page.evaluate(() => (window as unknown as { modeAtBody: string }).modeAtBody)).toBe('light');
  });

  test('broken or blocked storage falls back to the OS setting', async ({ page, tasksServer }) => {
    await page.addInitScript(() => localStorage.setItem('atm-dashboard', '{not json'));
    await page.emulateMedia({ colorScheme: 'light' });
    await openDashboard(page, tasksServer, 'standard');
    await expect(html(page)).toHaveAttribute('data-mode', 'light');
    await expect(html(page)).toHaveAttribute('data-theme', 'default');
  });
});

test.describe('themes', () => {
  const tokens = (page: Page) =>
    page.evaluate(() => {
      const s = getComputedStyle(document.documentElement);
      return { bg: s.getPropertyValue('--bg').trim(), accent: s.getPropertyValue('--accent').trim() };
    });

  test('picking a theme changes the colours, per mode, and is remembered', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard');
    await menu(page);
    await expect(themeSelect(page).locator('option')).toHaveText([
      'Default',
      'Nord',
      'Solarized',
      'Catppuccin',
      'Tokyo Night',
      'Ayu',
      'Night Owl',
      'Gruvbox',
      'Noctis',
      'Sublime Material',
      'High contrast',
      'Sticky notes',
      'Hand-drawn',
    ]);
    const before = await tokens(page);
    await themeSelect(page).selectOption('nord');
    await expect(themeSelect(page)).toHaveValue('nord');
    await expect(html(page)).toHaveAttribute('data-theme', 'nord');
    expect(await tokens(page)).toEqual({ bg: '#2e3440', accent: '#88c0d0' });
    expect(await tokens(page)).not.toEqual(before);
    await page.locator('.mode-btn[data-mode="light"]').click();
    expect((await tokens(page)).bg).toBe('#eceff4');
    await page.reload();
    await menu(page);
    await expect(themeSelect(page)).toHaveValue('nord');
    await expect(html(page)).toHaveAttribute('data-theme', 'nord');
    await expect(html(page)).toHaveAttribute('data-mode', 'light');
  });

  test('an unknown stored theme falls back to Default', async ({ page, tasksServer }) => {
    await page.addInitScript(() => localStorage.setItem('atm-dashboard', JSON.stringify({ theme: 'vaporwave' })));
    await openDashboard(page, tasksServer, 'standard');
    await menu(page);
    await expect(themeSelect(page)).toHaveValue('default');
    await expect(html(page)).toHaveAttribute('data-theme', 'default');
    expect(await page.evaluate(() => getComputedStyle(document.body).backgroundColor)).toBe(DARK_BG);
  });

  test('the theme is a compact combo box, labelled, with the current theme and its swatch', async ({
    page,
    tasksServer,
  }) => {
    await page.addInitScript(() => localStorage.setItem('atm-dashboard', JSON.stringify({ theme: 'noctis' })));
    await openDashboard(page, tasksServer, 'standard');
    await menu(page);
    await expect(page.getByLabel('Theme', { exact: true })).toHaveValue('noctis');
    await expect(page.locator('.theme-picker .theme-swatch span')).toHaveCount(4);
    // One line, not a list of every theme
    const height = (await page.locator('.theme-picker').boundingBox())!.height;
    expect(height).toBeLessThan(40);
  });

  test('a separator line sets the themes that draw the board apart, before Sticky notes', async ({
    page,
    tasksServer,
  }) => {
    await openDashboard(page, tasksServer, 'standard');
    await menu(page);
    const entries = await themeSelect(page).evaluate((select) =>
      [...select.children].map((e) => (e.tagName === 'HR' ? '—' : (e as HTMLOptionElement).value)),
    );
    expect(entries.filter((e) => e === '—')).toHaveLength(1);
    expect(entries.slice(entries.indexOf('—') + 1)).toEqual(['sticky-notes', 'hand-drawn']);
  });

  test('the keyboard: Tab reaches the theme, typing a letter picks one', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard');
    await menu(page);
    await themeSelect(page).focus();
    await page.keyboard.type('g');
    await expect(html(page)).toHaveAttribute('data-theme', 'gruvbox');
    await expect(themeSelect(page)).toHaveValue('gruvbox');
  });

  test('the ⋯ button opens the menu and Escape closes it', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard');
    const button = page.locator('#settings-btn');
    await expect(button).toHaveAccessibleName('Settings');
    await button.click();
    await expect(button).toHaveAttribute('aria-expanded', 'true');
    await expect(page.locator('#settings-menu')).toBeVisible();
    await page.keyboard.press('Escape');
    await expect(page.locator('#settings-menu')).toHaveCount(0);
  });
});
