// The dashboard in the atm app (T-457): the same page, its data from the app's commands over Tauri's IPC (mocked here
// as `@tauri-apps/api/mocks` does, backed by the fixture's data), never from HTTP.
import { addressOf, expect, loadFixture, test } from './helpers';

test.describe('the dashboard in the atm app', () => {
  test('loads its projects and their tasks through the app’s commands, with no request to a server', async ({
    page,
    tasksServer,
  }) => {
    await tasksServer.asApp();
    tasksServer.use('standard');
    await page.goto('/');
    await expect(page.locator('#dashboard-title')).toHaveText('Standard Project');
    await expect(page.locator('#kanban .kanban-card').first()).toBeVisible();
    expect(tasksServer.httpRequests).toEqual([]);
  });

  test('the window takes the page’s title, and follows it to another project (T-511)', async ({
    page,
    tasksServer,
  }) => {
    await tasksServer.asApp();
    tasksServer.addProject('second', 'minimal', 'Second Project');
    tasksServer.use('standard');
    await page.goto('/');
    // Two projects: the page opens on the overview
    await expect(page.locator('#dashboard-title')).toHaveText('Projects Overview');
    await expect.poll(() => tasksServer.windowTitles.at(-1)).toBe(await page.title());

    await tasksServer.pushLink('/projects/second/list');
    await expect(page.locator('#dashboard-title')).toHaveText('Minimal Project');
    await expect.poll(() => tasksServer.windowTitles.at(-1)).toContain('Minimal Project');
  });

  test('the menu’s items do what their controls do, and the menu shows what the page shows (T-512)', async ({
    page,
    tasksServer,
  }) => {
    await tasksServer.asApp();
    tasksServer.use('standard');
    await page.goto('/');
    await expect(page.locator('#kanban .kanban-card').first()).toBeVisible();
    await expect.poll(() => tasksServer.menuState?.view).toBe('kanban');

    await tasksServer.chooseMenu({ kind: 'show_list' });
    await expect.poll(() => addressOf(page)).toMatch(/\/list(\?|$)/);
    await expect.poll(() => tasksServer.menuState?.view).toBe('list');

    await tasksServer.chooseMenu({ kind: 'toggle_filters' });
    await expect(page.locator('.filter-panel')).toBeVisible();
    await expect.poll(() => tasksServer.menuState?.filters).toBe(true);
    await page.keyboard.press('Escape');
    await expect.poll(() => tasksServer.menuState?.filters).toBe(false);

    await tasksServer.chooseMenu({ kind: 'about' });
    await expect(page.locator('#about-overlay')).toHaveClass(/open/);
  });

  test('Settings is a sheet (T-517): from the menu’s Settings… and the ⋯, with the badge’s switch', async ({
    page,
    tasksServer,
  }) => {
    await tasksServer.asApp();
    tasksServer.use('standard');
    await page.goto(`/projects/${tasksServer.projectId}/kanban`);
    await expect(page.locator('#kanban .kanban-card').first()).toBeVisible();
    // No popup in the app: the sheet only
    await expect(page.locator('#settings-menu')).toHaveCount(0);

    await tasksServer.chooseMenu({ kind: 'settings' });
    await expect(page.locator('#settings-overlay')).toHaveClass(/open/);
    const sheet = page.locator('#settings-sheet');
    await expect(sheet.locator('#mode-control')).toBeVisible();
    await expect(sheet.locator('#theme-select')).toBeVisible();
    await expect(sheet.locator('#doc-style-select')).toBeVisible();
    await expect(sheet.locator('#reset-view')).toBeVisible();
    // The badge's switch, remembered
    const badge = sheet.locator('#needs-you-badge-check');
    await expect(badge).toBeChecked();
    await sheet.locator('label:has(#needs-you-badge-check)').click();
    await expect(badge).not.toBeChecked();
    expect(await page.evaluate(() => JSON.parse(localStorage.getItem('atm-dashboard') ?? '{}').needsYouBadge)).toBe(
      false,
    );
    // A setting applies at once
    await sheet.locator('[data-mode="dark"]').click();
    await expect(page.locator('html')).toHaveAttribute('data-mode', 'dark');
    await page.keyboard.press('Escape');
    await expect(page.locator('#settings-overlay')).not.toHaveClass(/open/);

    // The title bar's ⋯ opens it too
    await page.locator('#settings-btn').click();
    await expect(page.locator('#settings-overlay')).toHaveClass(/open/);
    await page.keyboard.press('Escape');
    await expect(page.locator('#settings-overlay')).not.toHaveClass(/open/);
  });

  // In WebKit too, the app's engine on macOS: the tab chooser's list was clipped by the tabs' strip (ATM-684)
  test(
    'the title bar (T-513): on macOS the system’s buttons and menu bar; elsewhere the page draws them',
    { tag: '@webkit' },
    async ({ page, tasksServer }) => {
      await tasksServer.asApp();
      tasksServer.use('standard');
      await page.goto('/');
      // One row: the tab, labelled with the project's name, and a +; the page's header, its subtitle, Filters and the
      // charts are inside the tab, under it, without a second title
      await expect(page.locator('#app-titlebar .title-bar-tab.selected #dashboard-title')).toHaveText(
        'Standard Project',
      );
      await expect(page.locator('#dashboard-title')).toHaveCount(1);
      await expect(page.locator('#app-tab-new')).toBeVisible();
      await expect(page.locator('#app-titlebar [role="tab"]')).toHaveCount(1);
      await expect(page.locator('#app-titlebar .view-tab')).toHaveCount(0);
      // The tab's label has the project picker; the bar's right end search and Filters, the charts and the ⋯; inside
      // the tab only the subtitle line; no reload button or version chip
      await expect(page.locator('.title-bar-tab.selected #project-selector')).toBeVisible();
      await expect(page.locator('.title-bar-tab.selected #other-projects-need')).toHaveCount(0);
      // The chooser in the tab opens its menu (its click isn't the tab's)
      await page.locator('.title-bar-tab.selected #project-selector').click();
      await expect(page.locator('#project-menu')).toBeVisible();
      // Drawn where it can be seen and used, not clipped by the tabs' scrolling strip (ATM-684): what is at its
      // middle is the menu itself, once it is placed beside its button, and its filter takes a real click
      await expect
        .poll(() =>
          page.evaluate(() => {
            const menu = document.querySelector('#project-menu')!;
            const box = menu.getBoundingClientRect();
            const at = document.elementFromPoint(box.left + box.width / 2, box.top + box.height / 2);
            return box.height > 40 && !!at && menu.contains(at);
          }),
        )
        .toBe(true);
      await page.locator('#project-filter').click();
      await expect(page.locator('#project-filter')).toBeFocused();
      await page.keyboard.press('Escape');
      // Search and Filters inside the tab, on the subtitle's line, before it; not in the title bar
      await expect(page.locator('.topbar.in-tab .in-tab-row #filters')).toBeVisible();
      await expect(page.locator('#app-titlebar #filters')).toHaveCount(0);
      expect(
        await page.evaluate(() => {
          const box = (s: string) => document.querySelector(s)!.getBoundingClientRect();
          const filters = box('.in-tab-row #filters');
          const subtitle = box('.in-tab-row #subtitle');
          return (
            filters.right <= subtitle.left &&
            Math.abs(filters.top + filters.height / 2 - (subtitle.top + subtitle.height / 2)) < 4
          );
        }),
      ).toBe(true);
      // The Filters button is its icon (its name stays for screen readers), and its popup opens inside the window
      await expect(page.locator('#filters-toggle')).toHaveAccessibleName(/Filters/);
      expect(
        await page.locator('#filters-toggle .filters-label').evaluate((e) => e.getBoundingClientRect().width),
      ).toBe(1);
      await page.locator('#filters-toggle').click();
      await expect(page.locator('.filter-panel')).toBeVisible();
      expect(
        await page.locator('.filter-panel').evaluate((e) => e.getBoundingClientRect().left),
      ).toBeGreaterThanOrEqual(0);
      await page.keyboard.press('Escape');
      await expect(page.locator('.topbar.in-tab #subtitle')).toContainText('tasks');
      await expect(page.locator('#reload-btn')).toHaveCount(0);
      await expect(page.locator('#version-chip')).toHaveCount(0);
      await expect(page.locator('#window-buttons')).toHaveCount(0);
      await expect(page.locator('#app-menus')).toHaveCount(0);
      await page.locator('#tab-list').click();
      await expect.poll(() => addressOf(page)).toMatch(/\/list(\?|$)/);
      // A drag on the bar's free space moves the window; a double click maximizes it
      // The bar's free space: just past the +
      const free = await page.evaluate(
        () => document.querySelector('#app-tab-new')!.getBoundingClientRect().right + 16,
      );
      await page.locator('#app-titlebar').click({ position: { x: free, y: 10 } });
      await page.locator('#app-titlebar').dblclick({ position: { x: free, y: 10 } });
      expect(tasksServer.windowCalls).toContain('start_dragging');
      expect(tasksServer.windowCalls).toContain('toggle_maximize');

      // Windows and Linux: the page's window buttons and its menus
      tasksServer.windowShell = { system_menu_bar: false, system_window_buttons: false };
      tasksServer.windowCalls = [];
      await page.reload();
      await expect(page.locator('#window-buttons')).toBeVisible();
      await page.locator('#window-minimize').click();
      await page.locator('#window-close').click();
      expect(tasksServer.windowCalls).toEqual(['minimize', 'close_window']);
      await page.locator('#app-menus').getByRole('menuitem', { name: 'View' }).click();
      await page.getByRole('menuitemcheckbox', { name: /List/ }).click();
      await expect.poll(() => addressOf(page)).toMatch(/\/list(\?|$)/);
    },
  );

  test('a switch of tab shows the project at once, its name steady; one shown before needs no load (T-525)', async ({
    page,
    tasksServer,
  }) => {
    await tasksServer.asApp();
    tasksServer.addProject('second', 'minimal', 'Minimal Project');
    tasksServer.use('standard');
    tasksServer.restoredTabs = {
      addresses: [`/projects/${tasksServer.projectId}/kanban`, '/projects/second/kanban'],
      active: 0,
    };
    await page.goto('/');
    const tabs = page.locator('.title-bar-tab [role="tab"]');
    await expect(tabs).toHaveCount(2);
    await expect(page.locator('#dashboard-title')).toHaveText('Standard Project');
    // What the switches show: the tabs' labels whenever they change, and whether the board was ever empty
    await page.evaluate(() => {
      const w = window as unknown as { __labels: string[]; __empty: boolean };
      const strip = document.querySelector('.title-bar-tabs')!;
      const labels = () =>
        [...strip.querySelectorAll('.title-bar-tab')]
          .map((t) => (t.textContent ?? '').replace(/[×\s]+/g, ' ').trim())
          .join(' | ');
      w.__labels = [labels()];
      w.__empty = false;
      new MutationObserver(() => {
        if (labels() !== w.__labels.at(-1)) w.__labels.push(labels());
        // The first project's board (the minimal one's is empty in its active sprint, as it should be)
        const first = document.querySelector('#dashboard-title')?.textContent === 'Standard Project';
        if (first && !document.querySelector('#kanban .kanban-card')) w.__empty = true;
      }).observe(document.body, { subtree: true, childList: true, characterData: true });
    });
    const seen = () =>
      page.evaluate(() => {
        const w = window as unknown as { __labels: string[]; __empty: boolean };
        return { __labels: w.__labels, __empty: w.__empty };
      });
    const titles = tasksServer.windowTitles.length;

    // A project not shown yet: loaded; its tab keeps its name (the workspace's, then its own: the same here)
    const asked = tasksServer.projectTaskRequests;
    await tabs.nth(1).click();
    await expect(page.locator('#dashboard-title')).toHaveText('Minimal Project');
    await expect.poll(() => tasksServer.projectTaskRequests).toBeGreaterThan(asked);
    await expect(page.locator('#kanban')).toContainText('To Do');
    // Back to the first: shown from what was kept, before any answer, so its board is never empty
    const loads = tasksServer.projectTaskRequests;
    await tabs.nth(0).click();
    await expect(page.locator('#dashboard-title')).toHaveText('Standard Project');
    expect((await seen()).__empty).toBe(false);
    // Its load still asks (a change since would show), and finds nothing new
    await expect.poll(() => tasksServer.projectTaskRequests).toBeGreaterThan(loads);
    // The labels never changed, and the window's title went only to each project's name: never "Task Dashboard"
    expect((await seen()).__labels).toEqual(['Standard Project | Minimal Project']);
    expect(tasksServer.windowTitles.slice(titles)).not.toContain('Task Dashboard');
  });

  test('more tabs than fit scroll in their own strip, with ‹ ›; the window buttons and the page stay put (ATM-605)', async ({
    page,
    tasksServer,
  }) => {
    await tasksServer.asApp();
    tasksServer.windowShell = { system_menu_bar: true, system_window_buttons: false };
    tasksServer.use('standard');
    const board = `/projects/${tasksServer.projectId}/kanban`;
    tasksServer.restoredTabs = { addresses: Array.from({ length: 12 }, () => board), active: 11 };
    await page.setViewportSize({ width: 1000, height: 700 });
    await page.goto('/');
    await expect(page.locator('.title-bar-tab [role="tab"]')).toHaveCount(12);
    const scroller = page.locator('.title-bar-tabs-scroller');
    const left = page.getByRole('button', { name: 'Tabs to the left' });
    const right = page.getByRole('button', { name: 'Tabs to the right' });
    const state = () =>
      page.evaluate(() => {
        const bar = document.querySelector('#app-titlebar')!;
        const strip = document.querySelector('.title-bar-tabs-scroller')!;
        const buttons = document.querySelector('#window-buttons')!.getBoundingClientRect();
        return {
          barFits: bar.scrollWidth <= bar.clientWidth,
          buttonsIn: buttons.right <= window.innerWidth && buttons.left > 0,
          pageX: document.scrollingElement!.scrollLeft,
          pageY: document.scrollingElement!.scrollTop,
          strip: strip.scrollLeft,
        };
      });
    // The last tab, active, is in view: the strip at its end, only ‹ to press; the bar fits the window
    await expect(left).toBeEnabled();
    await expect(right).toBeDisabled();
    const atEnd = await state();
    expect(atEnd).toMatchObject({ barFits: true, buttonsIn: true, pageX: 0 });
    expect(atEnd.strip).toBeGreaterThan(0);
    // The wheel over the strip scrolls it, and only it
    await scroller.hover();
    await page.mouse.wheel(0, -400);
    await expect.poll(async () => (await state()).strip).toBeLessThan(atEnd.strip);
    expect(await state()).toMatchObject({ pageX: 0, pageY: atEnd.pageY, buttonsIn: true });
    // ‹ to the start, then › away from it
    await page.mouse.wheel(0, -10_000);
    await expect(left).toBeDisabled();
    await expect.poll(async () => (await state()).strip).toBe(0);
    await right.click();
    await expect.poll(async () => (await state()).strip).toBeGreaterThan(0);
    await expect(left).toBeEnabled();
    // Scrolled to the end, then the last tabs closed one by one (ATM-639): while the rest overflow, the strip keeps its
    // room, the last tab at its end; once they fit (two, here), all of them shown, unscrolled, and no arrows
    await scroller.hover();
    await page.mouse.wheel(0, 10_000);
    await expect(right).toBeDisabled();
    const room = await scroller.evaluate((el) => el.clientWidth);
    const geometry = () =>
      scroller.evaluate((el) => ({
        width: el.clientWidth,
        scrolled: el.scrollLeft,
        fits: el.scrollWidth <= el.clientWidth + 1,
        gap: Math.round(el.getBoundingClientRect().right - el.lastElementChild!.getBoundingClientRect().right),
      }));
    for (let open = 11; open >= 2; open--) {
      const tab = page.locator('.title-bar-tab').last();
      await tab.hover();
      await tab.locator('.title-bar-tab-close').click();
      await expect(page.locator('.title-bar-tab')).toHaveCount(open);
      if (open > 2) expect(await geometry(), `${open} tabs`).toMatchObject({ width: room, fits: false, gap: 0 });
    }
    await expect(left).toHaveCount(0);
    await expect(right).toHaveCount(0);
    expect(await geometry()).toMatchObject({ fits: true, scrolled: 0 });
    // Fewer tabs, all fitting: no arrows
    tasksServer.restoredTabs = { addresses: [board, board], active: 0 };
    await page.reload();
    await expect(page.locator('.title-bar-tab [role="tab"]')).toHaveCount(2);
    await expect(left).toHaveCount(0);
    await expect(right).toHaveCount(0);
    await expect(page.locator('#app-tab-more')).toHaveCount(0);
  });

  // ATM-688 (Hakan): the bar's line runs under every tab but the selected one, which joins the page under it. The tab
  // covers the line's pixel with the page's background; the tabs' scroller (ATM-605) clipped that pixel away
  test('the selected tab has no line under it; the others do', { tag: '@webkit' }, async ({ page, tasksServer }) => {
    await tasksServer.asApp();
    tasksServer.use('standard');
    const board = `/projects/${tasksServer.projectId}/kanban`;
    tasksServer.restoredTabs = { addresses: [board, board, board], active: 1 };
    await page.goto('/');
    await expect(page.locator('.title-bar-tab [role="tab"]')).toHaveCount(3);
    const seen = await page.evaluate(() => {
      const bar = document.querySelector('#app-titlebar')!.getBoundingClientRect();
      const strip = document.querySelector('.title-bar-tabs-scroller')!.getBoundingClientRect();
      const tabs = [...document.querySelectorAll<HTMLElement>('.title-bar-tab')];
      const page = getComputedStyle(document.body).getPropertyValue('--bg');
      return {
        // The strip holds the line's row (it is the bar's last pixel)
        stripReachesLine: strip.bottom >= bar.bottom - 0.5,
        tabs: tabs.map((t) => ({
          selected: t.classList.contains('selected'),
          coversLine: t.getBoundingClientRect().bottom >= bar.bottom - 0.5,
          background: getComputedStyle(t).backgroundColor,
        })),
        page,
      };
    });
    expect(seen.stripReachesLine).toBe(true);
    const selected = seen.tabs.find((t) => t.selected)!;
    const others = seen.tabs.filter((t) => !t.selected);
    // The selected tab reaches over the line, opaque; the others leave it showing through
    expect(selected.coversLine).toBe(true);
    expect(selected.background).not.toBe('rgba(0, 0, 0, 0)');
    for (const t of others) expect(t.background).toBe('rgba(0, 0, 0, 0)');
    // What is drawn there, under the selected tab's middle: the tab (its page colour), not the bar
    const at = await page.evaluate(() => {
      const bar = document.querySelector('#app-titlebar')!.getBoundingClientRect();
      const tab = document.querySelector('.title-bar-tab.selected')!.getBoundingClientRect();
      const hit = document.elementFromPoint(tab.left + 8, bar.bottom - 0.5);
      return hit?.closest('.title-bar-tab.selected') !== null;
    });
    expect(at).toBe(true);
  });

  // ATM-685 (Hakan): when the tabs don't all fit, More ⌄ lists those out of view; one chosen there is shown, scrolled
  // into view. In WebKit too, the app's engine on macOS
  test(
    'More ⌄ lists the tabs out of view; one chosen is shown, in view',
    { tag: '@webkit' },
    async ({ page, tasksServer }) => {
      await tasksServer.asApp();
      tasksServer.windowShell = { system_menu_bar: true, system_window_buttons: false };
      tasksServer.use('standard');
      tasksServer.addProject('second', 'minimal', 'Second Project');
      const board = `/projects/${tasksServer.projectId}/kanban`;
      // The first tab is the other project's, the last (active, in view) one of eleven boards
      tasksServer.restoredTabs = {
        addresses: ['/projects/second', ...Array.from({ length: 11 }, () => board)],
        active: 11,
      };
      await page.setViewportSize({ width: 1000, height: 700 });
      await page.goto('/');
      await expect(page.locator('.title-bar-tab [role="tab"]')).toHaveCount(12);
      const more = page.locator('#app-tab-more');
      await expect(more).toBeVisible();
      // It sits in the bar, which still fits the window
      expect(
        await page.evaluate(() => {
          const bar = document.querySelector('#app-titlebar')!;
          return bar.scrollWidth <= bar.clientWidth;
        }),
      ).toBe(true);
      await more.click();
      const items = page.locator('#app-tab-more-menu .title-bar-tabs-menu-item');
      await expect(items.first()).toHaveText('Second Project');
      // Only those out of view: the active one is in view, so it isn't listed
      const listed = await items.evaluateAll((els) => els.map((e) => e.getAttribute('data-tab')));
      const active = await page.locator('.title-bar-tab.selected [role="tab"]').getAttribute('id');
      expect(listed).not.toContain(active!.replace('app-tab-', ''));
      // Over the page, so a real click reaches it (it was under the page's subtitle at first)
      await items.first().click({ timeout: 5000 });
      await expect(page.locator('#dashboard-title')).toHaveText('Minimal Project');
      // Shown, and scrolled into view: wholly inside the strip
      const first = page.locator('.title-bar-tab').first();
      await expect(first).toHaveClass(/selected/);
      await expect
        .poll(() =>
          page.evaluate(() => {
            const strip = document.querySelector('.title-bar-tabs-scroller')!.getBoundingClientRect();
            const tab = document.querySelector('.title-bar-tab')!.getBoundingClientRect();
            return tab.left >= strip.left - 1 && tab.right <= strip.right + 1;
          }),
        )
        .toBe(true);
      // Now the last tabs are out of view: the menu lists them, not the first
      await more.click();
      await expect(items.first()).not.toHaveText('Minimal Project');
      await page.keyboard.press('Escape');
    },
  );

  test('a review opens in a tab of its own (ATM-574), labelled by it, under the window’s own title bar', async ({
    page,
    tasksServer,
  }) => {
    await tasksServer.asApp();
    tasksServer.use('standard');
    const header = {
      id: 'r2',
      reference: 'CR-2',
      number: 2,
      title: 'Trim the names',
      status: 'waiting',
      tasks: [],
      author: 'claude:a',
      reviewers: [],
      created: '2026-11-17T10:00:00Z',
      updated: '2026-11-17T10:00:00Z',
    };
    tasksServer
      .setReviews({ reviews: [{ review: header, branch: null, chains: 0, open: 0, needs_you: false }], you: 'Ana' })
      .setReview('CR-2', {
        review: header,
        summary: '',
        branch: null,
        rounds: [{ number: 1, kind: { kind: 'files' }, files: [], by: 'claude:a', at: '' }],
        chains: [],
        view: { round: 1, against: null, files: [] },
        needs_you: false,
        you: 'Ana',
      });
    tasksServer.addProject('second', 'minimal', 'Second Project');
    tasksServer.link(`/projects/${tasksServer.projectId}/reviews`);
    await page.goto('/');
    const tabs = page.locator('#app-titlebar [role="tab"]');
    await expect(page.locator('#review-list')).toBeVisible();
    await expect(tabs).toHaveCount(1);

    // A plain click opens it in place, as every link (ATM-686), and its back button comes back to the list
    await page.locator('.review-row[data-review="CR-2"] .review-link').click();
    await expect(page.locator('#review-page')).toHaveAttribute('data-review', 'CR-2');
    await expect(tabs).toHaveCount(1);
    await expect(page.locator('.review-back')).toHaveText('Reviews');
    await page.locator('.review-back').click();
    await expect(page.locator('#review-list')).toBeVisible();

    // ⌘-click: a tab of its own, in the background, this tab still the list (ATM-698, as browsers do)
    await page.locator('.review-row[data-review="CR-2"] .review-link').click({ modifiers: ['ControlOrMeta'] });
    await expect(tabs).toHaveCount(2);
    await expect(tabs.nth(0)).toHaveAttribute('aria-selected', 'true');
    await expect(page.locator('#review-list')).toBeVisible();
    await tabs.nth(1).click();
    await expect(tabs.nth(1)).toHaveAttribute('aria-selected', 'true');
    await expect(page.locator('#review-page')).toHaveAttribute('data-review', 'CR-2');
    // Opened on its own, its back button goes up to its reviews list
    await expect(page.locator('.review-back')).toHaveText('Reviews');
    // The tab is the review's: its reference and title, no project chooser
    await expect(page.locator('#dashboard-title')).toHaveText('CR-2: Trim the names');
    await expect(tabs.nth(1).locator('.app-tab-chooser')).toHaveCount(0);
    // The window's own bar and sidebar toggle are there, as on every tab
    await expect(page.locator('#app-titlebar')).toBeVisible();
    // ATM-691 (Hakan): and the ⋯ at its right end, which opens Settings, the document's text size with them
    await page.locator('#app-titlebar-end #settings-btn').click();
    await expect(page.locator('#settings-overlay')).toHaveClass(/open/);
    await page.locator('#settings-sheet #doc-size-select').selectOption('large');
    await expect(page.locator('html')).toHaveAttribute('data-doc-size', 'large');
    await page.keyboard.press('Escape');
    await expect(page.locator('#settings-overlay')).not.toHaveClass(/open/);
    // The selected tab joins the review's top bar under it, in one colour, as a project's tab joins its page (ATM-608)
    const colours = await page.evaluate(() => {
      const of = (selector: string) => getComputedStyle(document.querySelector(selector)!).backgroundColor;
      return { tab: of('.title-bar-tab.selected'), top: of('.review-top'), bar: of('#app-titlebar') };
    });
    expect(colours.top).toBe(colours.tab);
    expect(colours.bar).not.toBe(colours.tab);
    // The first tab still shows the reviews
    await expect(page.locator('#app-tab-t1-label')).toContainText('Standard Project');
    await tabs.nth(0).click();
    await expect(page.locator('#review-list')).toBeVisible();
    // And the review's tab keeps its name while another is shown
    await expect(page.locator('#app-tab-t2-label')).toContainText('CR-2: Trim the names');
    expect(tasksServer.httpRequests).toEqual([]);

    // ATM-581 (Hakan): from the review's tab, a project in the sidebar: the tab becomes that project's page, labelled
    // by it; no new tab
    await tabs.nth(1).click();
    await expect(page.locator('#review-page')).toBeVisible();
    await page.locator('#app-sidebar [data-id="second"]').click();
    await expect(tabs).toHaveCount(2);
    await expect(tabs.nth(1)).toHaveAttribute('aria-selected', 'true');
    await expect(page.locator('#dashboard-title')).toHaveText('Minimal Project');
    await expect(page.locator('#review-page')).toHaveCount(0);
    await expect.poll(() => addressOf(page)).toMatch(/^\/projects\/second(\/[a-z]+)?(\?|$)/);
    await expect(page.locator('#app-tab-t2-label')).toContainText('Minimal Project');
    // The first tab is as it was
    await expect(page.locator('#app-tab-t1-label')).toContainText('Standard Project');
  });

  test('tabs are addresses (T-514): + opens the overview, a click goes back, × closes, the menu steps', async ({
    page,
    tasksServer,
  }) => {
    await tasksServer.asApp();
    tasksServer.addProject('second', 'minimal', 'Second Project');
    tasksServer.use('standard');
    tasksServer.link(`/projects/${tasksServer.projectId}/list`);
    await page.goto('/');
    const tabs = page.locator('#app-titlebar [role="tab"]');
    await expect(page.locator('#dashboard-title')).toHaveText('Standard Project');
    await expect(tabs).toHaveCount(1);

    // + : a second tab, on the overview
    await page.locator('#app-tab-new').click();
    await expect(tabs).toHaveCount(2);
    await expect(page.locator('#dashboard-title')).toHaveText('Projects Overview');
    await expect(tabs.nth(1)).toHaveAttribute('aria-selected', 'true');
    // The other tab keeps its project's name, and a click goes back to where it was
    await expect(page.locator('#app-tab-t1-label')).toContainText('Standard Project');
    await tabs.nth(0).click();
    await expect.poll(() => addressOf(page)).toMatch(new RegExp(`^/projects/${tasksServer.projectId}/list(\\?|$)`));

    // A link while the app runs opens in a new tab
    await tasksServer.pushLink('/projects/second/list');
    await expect(tabs).toHaveCount(3);
    await expect(page.locator('#dashboard-title')).toHaveText('Minimal Project');

    // The menu's Previous Tab, then Close Tab: the next tab takes its place
    await tasksServer.chooseMenu({ kind: 'previous_tab' });
    await expect(page.locator('#dashboard-title')).toHaveText('Standard Project');
    await tasksServer.chooseMenu({ kind: 'close_tab' });
    await expect(tabs).toHaveCount(2);
    await expect(page.locator('#dashboard-title')).toHaveText('Minimal Project');

    // ×, on the last tab left: the window closes
    await page.locator('.title-bar-tab').nth(0).hover();
    await page.locator('.title-bar-tab-close').nth(0).click();
    await expect(tabs).toHaveCount(1);
    await page.locator('.title-bar-tab-close').nth(0).click();
    expect(tasksServer.windowCalls).toContain('close_window');
  });

  // ATM-698 (docs/tabs-and-navigation-design.md §4.5): clicks as in a browser, on every card and row
  test('⌘-click and a middle click on a card or row open a tab in the background; ⇧⌘-click brings it forward', async ({
    page,
    tasksServer,
  }) => {
    await tasksServer.asApp();
    tasksServer.use('standard');
    tasksServer.link(`/projects/${tasksServer.projectId}/kanban`);
    await page.goto('/');
    const tabs = page.locator('#app-titlebar [role="tab"]');
    await expect(page.locator('#panel-kanban')).toBeVisible();
    const card = page.locator('.kanban-card[data-id="T-004"] .card-link');
    await card.click({ button: 'middle' });
    await expect(tabs).toHaveCount(2);
    await expect(tabs.nth(0)).toHaveAttribute('aria-selected', 'true');
    await card.click({ modifiers: ['ControlOrMeta', 'Shift'] });
    await expect(tabs).toHaveCount(3);
    await expect(tabs.nth(1)).toHaveAttribute('aria-selected', 'true');
    await expect.poll(() => addressOf(page)).toContain('task=T-004');
    await expect(page.locator('#detail-overlay')).toHaveClass(/open/);
    await page.keyboard.press('Escape');
    await expect(page.locator('#detail-overlay')).not.toHaveClass(/open/);
    // A list row's name, the same: a real link to its details
    await tabs.nth(0).click({ timeout: 5000 });
    await page.locator('#tab-list').click();
    const row = page.locator('.task-card[data-id="T-004"] a.task-name');
    await expect(row).toHaveAttribute('href', /\/list\?(.*&)?task=T-004$/);
    await row.click({ modifiers: ['ControlOrMeta'] });
    await expect(tabs).toHaveCount(4);
    await expect(tabs.nth(0)).toHaveAttribute('aria-selected', 'true');
    // A plain click opens it here
    await row.click();
    await expect(page.locator('#detail-overlay')).toHaveClass(/open/);
    await expect(tabs).toHaveCount(4);
  });

  // ATM-699 (docs/tabs-and-navigation-design.md §4.5): atm's own right-click menus, WebKit's turned off
  test('right-click menus: on a card, on a tab, elsewhere; each item does what it says; text keeps the system’s', async ({
    page,
    context,
    tasksServer,
  }) => {
    await context.grantPermissions(['clipboard-read', 'clipboard-write']);
    await tasksServer.asApp();
    tasksServer.use('standard');
    tasksServer.link(`/projects/${tasksServer.projectId}/kanban`);
    await page.goto('/');
    const tabs = page.locator('#app-titlebar [role="tab"]');
    const menu = page.locator('#app-context-menu');
    const items = menu.locator('.app-context-item');
    const item = (id: string) => menu.locator(`[data-item="${id}"]`);
    await expect(page.locator('#panel-kanban')).toBeVisible();
    const card = page.locator('.kanban-card[data-id="T-004"]');

    // A card: Open, Open in New Tab, Open in New Window, Copy Link
    await card.click({ button: 'right' });
    await expect(items).toHaveText(['Open', 'Open in New Tab', 'Open in New Window', 'Copy Link']);
    await item('copy-link').click();
    await expect(menu).toBeHidden();
    await expect
      .poll(() => page.evaluate(() => navigator.clipboard.readText()))
      .toBe(`atm://projects/${tasksServer.projectId}/kanban?task=T-004`);
    await card.click({ button: 'right' });
    await item('open-tab').click();
    await expect(tabs).toHaveCount(2);
    await expect(tabs.nth(0)).toHaveAttribute('aria-selected', 'true');
    await card.click({ button: 'right' });
    await item('open-window').click();
    await expect.poll(() => tasksServer.openedWindows.length).toBe(1);
    expect(tasksServer.openedWindows[0]).toContain('task=T-004');
    await card.click({ button: 'right' });
    await item('open').click();
    await expect(page.locator('#detail-overlay')).toHaveClass(/open/);
    await page.keyboard.press('Escape');

    // A tab: its six items; Close Other Tabs leaves it alone
    await tabs.nth(1).click({ button: 'right' });
    await expect(items).toHaveText([
      'New Tab',
      'Duplicate',
      'Move to New Window',
      'Copy Link',
      'Close',
      'Close Other Tabs',
    ]);
    await item('duplicate').click();
    await expect(tabs).toHaveCount(3);
    // The tab duplicated was the card's, its details over its page: the copy shows them too
    await expect(page.locator('#detail-overlay')).toHaveClass(/open/);
    await page.keyboard.press('Escape');
    await tabs.nth(0).click({ button: 'right' });
    await item('close-others').click();
    await expect(tabs).toHaveCount(1);

    // Elsewhere: Back, Forward, Reload; Back is there after a step
    await page.locator('#tab-list').click();
    await page.locator('#app-titlebar-end').click({ button: 'right' });
    await expect(items).toHaveText(['Back', 'Forward', 'Reload']);
    await expect(item('forward')).toHaveAttribute('data-disabled', '');
    await item('back').click();
    await expect(page.locator('#panel-kanban')).toBeVisible();

    // Over text that can be edited: none of atm's (the system's shows)
    await page.locator('#task-search').click({ button: 'right' });
    await expect(menu).toBeHidden();
  });

  // ATM-694 (docs/tabs-and-navigation-design.md §4.1, §4.3): each tab its own history; a switch adds nothing
  test('Back and Forward walk the active tab’s own history; a switch of tab adds to neither; the page’s own stays put', async ({
    page,
    tasksServer,
  }) => {
    await tasksServer.asApp();
    tasksServer.use('standard');
    tasksServer.link(`/projects/${tasksServer.projectId}/list`);
    await page.goto('/');
    await expect(page.locator('#panel-list')).toBeVisible();
    const depth = await page.evaluate(() => history.length);
    const at = () => addressOf(page).split('?')[0];
    const base = `/projects/${tasksServer.projectId}`;

    // Steps in this tab: the Kanban view, then a task's details over it
    await page.locator('#tab-kanban').click();
    await expect(page.locator('#panel-kanban')).toBeVisible();
    await page.locator('.kanban-card[data-id="T-004"] .card-title, .kanban-card[data-id="T-004"]').first().click();
    await expect(page.locator('#detail-overlay')).toHaveClass(/open/);
    // The page's own history never grew
    expect(await page.evaluate(() => history.length)).toBe(depth);

    // Back: the details close, the board stays; Back again: the list; Forward: the board
    await tasksServer.chooseMenu({ kind: 'back' });
    await expect(page.locator('#detail-overlay')).not.toHaveClass(/open/);
    await expect.poll(at).toBe(`${base}/kanban`);
    await tasksServer.chooseMenu({ kind: 'back' });
    await expect.poll(at).toBe(`${base}/list`);
    await tasksServer.chooseMenu({ kind: 'back' }); // nothing before the tab's first entry
    await expect.poll(at).toBe(`${base}/list`);
    await tasksServer.chooseMenu({ kind: 'forward' });
    await expect.poll(at).toBe(`${base}/kanban`);

    // A new tab: its own history, Back going nowhere
    await page.locator('#app-tab-new').click();
    await expect(page.locator('#dashboard-title')).toHaveText('Projects Overview');
    await tasksServer.chooseMenu({ kind: 'back' });
    await expect(page.locator('#dashboard-title')).toHaveText('Projects Overview');
    // Back to the first tab: as it was left, and its Back still the list (the switch added nothing)
    await page.locator('#app-titlebar [role="tab"]').nth(0).click();
    await expect.poll(at).toBe(`${base}/kanban`);
    await tasksServer.chooseMenu({ kind: 'back' });
    await expect.poll(at).toBe(`${base}/list`);
    // The mouse's back and forward buttons too
    await page.mouse.move(400, 300);
    await page.evaluate(() => window.dispatchEvent(new MouseEvent('mouseup', { button: 4, bubbles: true })));
    await expect.poll(at).toBe(`${base}/kanban`);
    await page.evaluate(() => window.dispatchEvent(new MouseEvent('mouseup', { button: 3, bubbles: true })));
    await expect.poll(at).toBe(`${base}/list`);
    expect(await page.evaluate(() => history.length)).toBe(depth);
  });

  // ATM-695 (docs/tabs-and-navigation-design.md §4.5): ‹ › left of the sidebar toggle, on the active tab's history
  test('the ‹ › arrows step through the tab’s history, disabled at its ends, follow the tab switched to, and list its entries', async ({
    page,
    tasksServer,
  }) => {
    await tasksServer.asApp();
    tasksServer.use('standard');
    tasksServer.link(`/projects/${tasksServer.projectId}/list`);
    await page.goto('/');
    await expect(page.locator('#panel-list')).toBeVisible();
    const at = () => addressOf(page).split('?')[0];
    const base = `/projects/${tasksServer.projectId}`;
    // The sidebar is open: they are in its top strip, before its toggle
    const strip = page.locator('#app-sidebar-top');
    const backArrow = strip.locator('#app-back');
    const forwardArrow = strip.locator('#app-forward');
    expect(await strip.locator('button').evaluateAll((all) => all.map((b) => b.id))).toEqual([
      'app-back',
      'app-forward',
      'app-sidebar-toggle',
    ]);
    await expect(backArrow).toBeDisabled();
    await expect(forwardArrow).toBeDisabled();

    // Two steps: the board, the graph
    await page.locator('#tab-kanban').click();
    await expect(page.locator('#panel-kanban')).toBeVisible();
    await page.locator('#tab-graph').click();
    await expect.poll(at).toBe(`${base}/graph`);
    await expect(backArrow).toBeEnabled();
    await expect(forwardArrow).toBeDisabled();

    // ‹ steps back, › forward
    await backArrow.click();
    await expect.poll(at).toBe(`${base}/kanban`);
    await expect(forwardArrow).toBeEnabled();
    await backArrow.click();
    await expect.poll(at).toBe(`${base}/list`);
    await expect(backArrow).toBeDisabled();
    await forwardArrow.click();
    await expect.poll(at).toBe(`${base}/kanban`);

    // A right-click on › lists what is ahead, nearest first; choosing one jumps there
    await forwardArrow.click({ button: 'right' });
    const ahead = page.locator('#app-forward-menu .app-history-menu-item');
    await expect(ahead).toHaveText(['Graph — Standard Project']);
    await ahead.first().click();
    await expect.poll(at).toBe(`${base}/graph`);
    // On ‹, what is behind, nearest first
    await backArrow.click({ button: 'right' });
    const behind = page.locator('#app-back-menu .app-history-menu-item');
    await expect(behind).toHaveText(['Kanban — Standard Project', 'List — Standard Project']);
    await behind.nth(1).click();
    await expect.poll(at).toBe(`${base}/list`);
    await expect(backArrow).toBeDisabled();
    await expect(forwardArrow).toBeEnabled();

    // A long press lists them too, and steps nowhere
    const box = (await forwardArrow.boundingBox())!;
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await page.mouse.down();
    await expect(page.locator('#app-forward-menu')).toBeVisible();
    await page.mouse.up();
    await expect(page.locator('#app-forward-menu .app-history-menu-item')).toHaveCount(2);
    expect(at()).toBe(`${base}/list`);
    await page.keyboard.press('Escape');

    // A new tab: its own history, both disabled; back to the first, the arrows follow it
    await page.locator('#app-tab-new').click();
    await expect(page.locator('#dashboard-title')).toHaveText('Projects Overview');
    await expect(backArrow).toBeDisabled();
    await expect(forwardArrow).toBeDisabled();
    await page.locator('#app-titlebar [role="tab"]').nth(0).click();
    await expect.poll(at).toBe(`${base}/list`);
    await expect(forwardArrow).toBeEnabled();

    // The sidebar closed: they move to the title bar, before its toggle
    await page.locator('#app-sidebar-toggle').click();
    const bar = page.locator('#app-titlebar');
    await expect(bar.locator('#app-back + #app-forward')).toHaveCount(1);
    await expect(bar.locator('.app-history + #app-sidebar-toggle')).toHaveCount(1);
    await bar.locator('#app-forward').click();
    await expect.poll(at).toBe(`${base}/kanban`);
  });

  test('the sidebar (T-515): projects open here or, ⌘-clicked, in a new tab; a task too; its button and menu hide it', async ({
    page,
    tasksServer,
  }) => {
    await tasksServer.asApp();
    tasksServer.addProject('second', 'minimal', 'Second Project');
    tasksServer.use('standard');
    await page.goto('/');
    const sidebar = page.locator('#app-sidebar');
    const tabs = page.locator('#app-titlebar [role="tab"]');
    await expect(sidebar.locator('.app-sidebar-item')).toHaveCount(3);
    await expect.poll(() => tasksServer.menuState?.sidebar).toBe(true);
    // Every project's bar is visible, its empty track too: not the sidebar's own colour (T-533)
    await expect(sidebar.locator('.app-sidebar-bar')).toHaveCount(2);
    const colours = await page.evaluate(() => ({
      sidebar: getComputedStyle(document.querySelector('#app-sidebar')!).backgroundColor,
      tracks: [...document.querySelectorAll('.app-sidebar-bar')].map((b) => getComputedStyle(b).backgroundColor),
    }));
    for (const track of colours.tracks) expect(track).not.toBe(colours.sidebar);

    // A click opens the project here
    await sidebar.locator(`[data-id="${tasksServer.projectId}"]`).click();
    await expect(page.locator('#dashboard-title')).toHaveText('Standard Project');
    await expect(tabs).toHaveCount(1);
    // ⌘-click (Ctrl-click) opens one in a new tab
    await sidebar.locator('[data-id="second"]').click({ modifiers: ['ControlOrMeta'] });
    await expect(tabs).toHaveCount(2);
    await expect(page.locator('#dashboard-title')).toHaveText('Minimal Project');

    // ⌘-click on a task's card opens its details in a new tab
    await tabs.nth(0).click();
    await expect(page.locator('#dashboard-title')).toHaveText('Standard Project');
    const card = page.locator('#kanban .kanban-card').first();
    const id = await card.getAttribute('data-id');
    await card.click({ modifiers: ['ControlOrMeta'] });
    await expect(tabs).toHaveCount(3);
    // In the background, right of this tab, which stays as it was (ATM-698)
    await expect(tabs.nth(0)).toHaveAttribute('aria-selected', 'true');
    await expect(page.locator('#detail-overlay')).not.toHaveClass(/open/);
    await tabs.nth(1).click();
    await expect.poll(() => addressOf(page)).toContain(`task=${id}`);
    await expect(page.locator('#detail-overlay')).toHaveClass(/open/);
    await page.keyboard.press('Escape');
    await expect(page.locator('#detail-overlay')).not.toHaveClass(/open/);

    // The title bar's button, then the menu's Show Sidebar
    await page.locator('#app-sidebar-toggle').click();
    await expect(sidebar).toHaveCount(0);
    await expect.poll(() => tasksServer.menuState?.sidebar).toBe(false);
    await tasksServer.chooseMenu({ kind: 'toggle_sidebar' });
    await expect(sidebar).toBeVisible();
  });

  test('the sidebar is the window’s full height and the title bar only right of it, its tabs from its edge; closed, the bar the whole width (T-535)', async ({
    page,
    tasksServer,
  }) => {
    await tasksServer.asApp();
    tasksServer.use('standard');
    await page.goto('/');
    const sidebar = page.locator('#app-sidebar');
    const bar = page.locator('#app-titlebar');
    const box = async (l: ReturnType<typeof page.locator>) => (await l.boundingBox())!;
    await expect(sidebar).toBeVisible();
    // Open: the sidebar from the window's top, the bar starting at its edge, the first tab right after the bar's start
    const side = await box(sidebar);
    expect(side.y).toBe(0);
    const edge = Math.round(side.x + side.width);
    await expect.poll(async () => Math.round((await box(bar)).x)).toBe(edge);
    // The first tab starts right at the sidebar's edge, its border the sidebar's: one line there, not two
    const firstTab = page.locator('.title-bar-tab').first();
    expect(Math.round((await box(firstTab)).x)).toBe(edge);
    await expect(firstTab).toHaveCSS('border-left-style', 'none');
    // The button is in the sidebar's top strip, not the bar; the strip drags the window
    await expect(page.locator('#app-sidebar-top #app-sidebar-toggle')).toBeVisible();
    await expect(bar.locator('#app-sidebar-toggle')).toHaveCount(0);
    const strip = await box(page.locator('#app-sidebar-top'));
    expect(strip.height).toBe((await box(bar)).height);
    await page.mouse.click(side.x + side.width - 10, strip.y + 10);
    await expect.poll(() => tasksServer.windowCalls).toContain('start_dragging');
    // Closed: the bar is the window's width from its left edge, with the button
    await page.locator('#app-sidebar-toggle').click();
    await expect(sidebar).toHaveCount(0);
    await expect.poll(async () => Math.round((await box(bar)).x)).toBe(0);
    await expect(bar.locator('#app-sidebar-toggle')).toBeVisible();
    // And back
    await page.locator('#app-sidebar-toggle').click();
    await expect(sidebar).toBeVisible();
    await expect.poll(async () => Math.round((await box(bar)).x)).toBe(edge);
  });

  test('Progress and Activity are the tab’s: at the right of its subtitle line, not in the title bar; they give way on a narrow tab (T-530)', async ({
    page,
    tasksServer,
  }) => {
    await tasksServer.asApp();
    tasksServer.use('standard');
    await page.setViewportSize({ width: 1400, height: 900 });
    await page.goto(`/projects/${tasksServer.projectId}/kanban`);
    const trail = page.locator('.topbar.in-tab #in-tab-trail');
    await expect(trail.locator('#progress-card')).toBeVisible();
    await expect(trail.locator('#activity-card')).toBeVisible();
    await expect(page.locator('#app-titlebar #progress-card, #app-titlebar #activity-card')).toHaveCount(0);
    // On the subtitle's line, at its right end
    const row = (await page.locator('.topbar.in-tab .in-tab-row').boundingBox())!;
    const pills = (await trail.boundingBox())!;
    const subtitle = (await page.locator('#subtitle').boundingBox())!;
    expect(Math.abs(pills.y + pills.height / 2 - (subtitle.y + subtitle.height / 2))).toBeLessThan(6);
    expect(Math.round(row.x + row.width - (pills.x + pills.width))).toBeLessThan(2);
    // They still open what they open
    await trail.locator('#progress-card').click();
    await expect(page.locator('#progress-panel')).toBeVisible();
    await page.keyboard.press('Escape');
    // A narrow tab: they give way, the subtitle stays
    await page.setViewportSize({ width: 1100, height: 900 });
    await expect(trail).toBeVisible();
    await page.setViewportSize({ width: 900, height: 900 });
    await expect(trail).toBeHidden();
    await expect(page.locator('#subtitle')).toBeVisible();
    // The overview has none in its tab
    await page.locator('#app-sidebar .app-sidebar-item').first().click();
    await expect(page.locator('#dashboard-title')).toHaveText('Projects Overview');
    await expect(page.locator('#in-tab-trail')).toHaveCount(0);
  });

  // Hakan: in the app, the regular themes' board scrolled as a whole, with a scrollbar, where each column should
  // scroll inside without one (the sticky-notes and hand-drawn themes, whose columns stretch, didn't)
  for (const theme of ['default', 'sticky-notes']) {
    test(`in the app, the board fits its tab and each column scrolls inside it, without a scrollbar (${theme}) @webkit`, async ({
      page,
      tasksServer,
    }) => {
      await tasksServer.asApp();
      tasksServer.use('kanban');
      // A large window, and a small one (Hakan's: about 1000 × 615)
      for (const [width, height] of [
        [1280, 720],
        [1000, 615],
      ]) {
        await page.setViewportSize({ width, height });
        await page.goto(`/projects/${tasksServer.projectId}/kanban`);
        if (theme !== 'default') await page.evaluate((t) => (document.documentElement.dataset.theme = t), theme);
        const done = page.locator('.kanban-column[data-status="done"]');
        const showAll = done.getByRole('button', { name: /Show all/ });
        if (await showAll.count()) await showAll.click();
        // The board doesn't scroll as a whole, nor does the tab or the page
        const overflow = (selector: string) =>
          page.locator(selector).evaluate((el) => el.scrollHeight - el.clientHeight);
        await expect.poll(() => overflow('.kanban-board'), { message: `${width} × ${height}` }).toBeLessThanOrEqual(0);
        // The tab and the page neither (in a small window the board keeps its 320px minimum, and the page gives way)
        if (height >= 720) {
          expect(await overflow('#app-main')).toBeLessThanOrEqual(0);
          expect(await page.evaluate(() => document.documentElement.scrollHeight - innerHeight)).toBeLessThanOrEqual(0);
        }
        // The column does, inside, with no scrollbar taking room
        const cards = done.locator('.kanban-cards');
        expect(await cards.evaluate((el) => el.scrollHeight > el.clientHeight)).toBe(true);
        expect(await cards.evaluate((el) => (el as HTMLElement).offsetWidth - el.clientWidth)).toBe(0);
      }
    });
  }

  test('the status bar (T-516): up to date, the agents at work, the copyright, the version and the links', async ({
    page,
    tasksServer,
  }) => {
    await tasksServer.asApp();
    tasksServer.use('standard');
    await page.goto('/');
    const bar = page.locator('#status-bar');
    await expect(bar.locator('#status-live')).toHaveText('Up to date');
    for (const link of ['#footer-version', '#footer-github', '#footer-help', '#footer-about', '.footer-copyright'])
      await expect(bar.locator(link), link).toBeVisible();
    await expect(page.locator('#site-footer')).toHaveCount(0);
    await bar.locator('#footer-version').click();
    await expect(page.locator('#whats-new-overlay')).toHaveClass(/open/);
    await page.keyboard.press('Escape');
    await bar.locator('#footer-about').click();
    await expect(page.locator('#about-overlay')).toHaveClass(/open/);
  });

  test('Connect your agents (T-458): opening it only reads the status; a click connects, saying what it wrote where', async ({
    page,
    tasksServer,
  }) => {
    await tasksServer.asApp();
    tasksServer.use('standard');
    await page.goto('/');
    // From the menu: atm ▸ Connect Your Agents… (the app's footer is a status bar, T-516)
    await expect(page.locator('#dashboard-title')).toHaveText('Standard Project');
    await tasksServer.chooseMenu({ kind: 'connect_agents' });
    const harness = page.locator('.agents-harness[data-harness="claude"]');
    await expect(harness.locator('.agents-harness-state')).toHaveText('Not connected');
    expect(tasksServer.agentCalls).toEqual(['agents_status']);

    await harness.locator('.agents-connect').click();
    await expect(page.locator('#agents-result li')).toHaveText([
      '/Users/me/.claude/skills/atm-do-task (installed, linked)',
      '/Users/me/.claude/skills/atm-serve (installed, linked)',
    ]);
    await expect(harness.locator('.agents-harness-state')).toHaveText('Connected');
    await expect(harness.locator('.agents-connect')).toHaveText('Connect Claude Code again');
    expect(tasksServer.agentCalls).toEqual(['agents_status', 'connect_agents claude', 'agents_status']);
  });

  test('Connect your agents (T-413): the shared folder names the tools found here, and a tool that lists the skills twice', async ({
    page,
    tasksServer,
  }) => {
    await tasksServer.asApp();
    tasksServer.use('standard');
    await page.goto('/');
    await expect(page.locator('#dashboard-title')).toHaveText('Standard Project');
    await tasksServer.chooseMenu({ kind: 'connect_agents' });
    const shared = page.locator('.agents-harness[data-harness="agents"]');
    await expect(shared.locator('.agents-harness-tools')).toHaveText('Found here: Codex, Cursor');
    // Claude Code's folder is its own: no list of tools
    await expect(page.locator('.agents-harness[data-harness="claude"] .agents-harness-tools')).toHaveCount(0);
    await expect(shared.locator('.agents-harness-twice')).toHaveCount(0);

    await shared.locator('.agents-connect').click();
    await expect(shared.locator('.agents-harness-state')).toHaveText('Connected');
    await expect(shared.locator('.agents-harness-twice')).toHaveCount(0);
    await page.locator('.agents-harness[data-harness="claude"] .agents-connect').click();
    await expect(shared.locator('.agents-harness-twice')).toHaveText(
      "Cursor also reads another folder atm is in, so it lists atm's skills twice.",
    );
  });

  test('windows come back with their tabs (T-518): the active one shown, a start link in a new tab; changes are kept', async ({
    page,
    tasksServer,
  }) => {
    await tasksServer.asApp();
    tasksServer.addProject('second', 'minimal', 'Second Project');
    tasksServer.use('standard');
    const first = `/projects/${tasksServer.projectId}/kanban`;
    tasksServer.restoredTabs = { addresses: [first, '/projects/second/list'], active: 1 };
    tasksServer.link('/board');
    await page.goto('/');
    // The two tabs, the second active, then the start link's tab after it
    await expect(page.locator('.title-bar-tab')).toHaveCount(3);
    await expect.poll(() => addressOf(page)).toMatch(/^\/board(\?|$)/);
    await expect(page.locator('.title-bar-tab').nth(0)).toContainText('Standard Project');
    await expect(page.locator('.title-bar-tab').nth(1)).toContainText('Second Project');
    // What the window shows is kept for the next start
    await expect
      .poll(() => tasksServer.savedTabs)
      .toEqual({ addresses: [first, '/projects/second/list', '/board'], active: 2 });
    await page.locator('.title-bar-tab').nth(0).locator('[role="tab"]').click();
    await expect.poll(() => tasksServer.savedTabs?.active).toBe(0);
  });

  test('Reload (ATM-584) loads the window again through the app, back at its tabs and the one it showed', async ({
    page,
    tasksServer,
  }) => {
    await tasksServer.asApp();
    tasksServer.addProject('second', 'minimal', 'Second Project');
    tasksServer.use('standard');
    const list = `/projects/${tasksServer.projectId}/list`;
    tasksServer.restoredTabs = { addresses: ['/board', list, '/projects/second/kanban'], active: 1 };
    // The list's address, with the scope the page adds to it
    const atList = new RegExp(`^${list}(\\?|$)`);
    await page.goto('/');
    await expect(page.locator('.title-bar-tab')).toHaveCount(3);
    await expect.poll(() => addressOf(page)).toMatch(atList);
    await expect.poll(() => tasksServer.savedTabs?.active).toBe(1);

    // A mark on this page, which a page loaded again doesn't have
    await page.evaluate(() => ((window as unknown as { beforeReload: boolean }).beforeReload = true));
    await tasksServer.chooseMenu({ kind: 'reload' });
    await expect.poll(() => tasksServer.windowCalls).toContain('reload_window');
    // A look that lands while the page is going away (its context destroyed) is no answer yet: look again
    await expect
      .poll(() =>
        page.evaluate(() => (window as unknown as { beforeReload?: boolean }).beforeReload ?? false).catch(() => true),
      )
      .toBe(false);
    // Loaded again: the same tabs, the same one shown, its page drawn
    await page.waitForLoadState('load');
    await expect(page.locator('.title-bar-tab')).toHaveCount(3);
    await expect.poll(() => addressOf(page)).toMatch(atList);
    await expect(page.locator('.task-card').first()).toBeVisible();
    await expect(page.locator('.title-bar-tab').nth(2)).toContainText('Second Project');
  });

  test('a tab dragged along the strip moves there and is kept; a plain click still selects; the bar still drags the window (T-532)', async ({
    page,
    tasksServer,
  }) => {
    await tasksServer.asApp();
    tasksServer.addProject('second', 'minimal', 'Second Project');
    tasksServer.use('standard');
    const first = `/projects/${tasksServer.projectId}/kanban`;
    tasksServer.restoredTabs = { addresses: [first, '/projects/second/list', '/board'], active: 0 };
    await page.goto('/');
    const tabs = page.locator('.title-bar-tab');
    await expect(tabs).toHaveCount(3);
    await expect(tabs.nth(1)).toContainText('Second Project');
    // Drag the third tab to the first place, with the mouse
    const third = (await tabs.nth(2).boundingBox())!;
    const start = (await tabs.nth(0).boundingBox())!;
    await page.mouse.move(third.x + third.width / 2, third.y + third.height / 2);
    await page.mouse.down();
    await page.mouse.move(third.x + third.width / 2 - 40, third.y + third.height / 2, { steps: 4 });
    await page.mouse.move(start.x + 20, third.y + third.height / 2, { steps: 8 });
    await page.mouse.up();
    await expect(tabs.nth(1)).toContainText('Standard Project');
    await expect(tabs.nth(2)).toContainText('Second Project');
    // Kept in that order (the tab shown has its scope in its address by now)
    await expect
      .poll(() => tasksServer.savedTabs?.addresses.map((a) => a.split('?')[0]))
      .toEqual(['/board', first, '/projects/second/list']);
    // The drag didn't select it: the first project's tab is still shown
    await expect(page.locator('#dashboard-title')).toHaveText('Standard Project');
    // A plain click selects: the tab is chosen and the page goes to its project (not its heading's text, which turns
    // from the listed name to the project's own once its page loads)
    await tabs.nth(2).locator('[role="tab"]').click();
    await expect(tabs.nth(2).locator('[role="tab"]')).toHaveAttribute('aria-selected', 'true');
    await expect.poll(() => new URL(page.url()).pathname).toBe('/projects/second/list');
    // The bar's free space (just past the +) still moves the window
    const free = await page.evaluate(() => document.querySelector('#app-tab-new')!.getBoundingClientRect().right + 16);
    await page.locator('#app-titlebar').click({ position: { x: free, y: 10 } });
    await expect.poll(() => tasksServer.windowCalls).toContain('start_dragging');
  });

  test('what needs you (T-518, T-520): the page tells the app its workspace; the badge and notifications, off in Settings', async ({
    page,
    tasksServer,
  }) => {
    await tasksServer.asApp();
    tasksServer.use('standard');
    await page.goto(`/projects/${tasksServer.projectId}/kanban`);
    // Only the workspace and the two switches: the app reads the counts and writes the notifications itself
    await expect
      .poll(() => tasksServer.needsYouCalls.at(-1))
      .toEqual({ workspace: 'default', badge: true, notify: true });
    await tasksServer.chooseMenu({ kind: 'settings' });
    await page.locator('label:has(#needs-you-badge-check)').click();
    await expect
      .poll(() => tasksServer.needsYouCalls.at(-1))
      .toEqual({ workspace: 'default', badge: false, notify: true });
    await page.locator('label:has(#needs-you-notify-check)').click();
    await expect
      .poll(() => tasksServer.needsYouCalls.at(-1))
      .toEqual({ workspace: 'default', badge: false, notify: false });
  });

  test('deep links (T-459): the one the app was started with opens its page; a later one too; anything else is ignored', async ({
    page,
    tasksServer,
  }) => {
    await tasksServer.asApp();
    tasksServer.addProject('second', 'minimal', 'Second Project');
    tasksServer.use('standard');
    tasksServer.link('/projects/second/list');
    await page.goto('/');
    await expect(page.locator('#dashboard-title')).toHaveText('Minimal Project');
    await expect.poll(() => addressOf(page)).toMatch(/^\/projects\/second\/list(\?|$)/);

    await tasksServer.pushLink(`/projects/${tasksServer.projectId}/kanban`);
    await expect(page.locator('#dashboard-title')).toHaveText('Standard Project');
    // Not a page of the dashboard: the page stays where it is
    await tasksServer.pushLink('/api/projects');
    await expect.poll(() => addressOf(page)).toMatch(new RegExp(`^/projects/${tasksServer.projectId}/kanban(\\?|$)`));
    expect(tasksServer.httpRequests).toEqual([]);
  });

  test('it reacts to the app’s changes instead of polling (T-506): no refresh settings, a change reloads the board', async ({
    page,
    tasksServer,
  }) => {
    await page.clock.install();
    await tasksServer.asApp();
    tasksServer.addProject('second', 'minimal', 'Second Project');
    const data = loadFixture('standard');
    const task = data.tasks[0] as { id: string; name: string };
    tasksServer.use(data);
    await page.goto(`/projects/${tasksServer.projectId}/kanban`);
    const card = page.locator(`#kanban .kanban-card[data-id="${task.id}"]`);
    await expect(card).toContainText(task.name);

    // No interval on the reload button, no auto-refresh in the settings
    await expect(page.locator('#reload-btn .refresh-state')).toHaveCount(0);
    await page.locator('#settings-btn').click();
    await expect(page.locator('#auto-refresh-check')).toHaveCount(0);
    await expect(page.locator('#last-loaded')).toContainText('Updates as tasks change');
    await page.keyboard.press('Escape');

    // It watches its workspace and the open project
    await expect
      .poll(() => [...tasksServer.subscriptions.values()].map((s) => `${s.workspace}/${s.project}`).sort())
      .toEqual([`default/${tasksServer.projectId}`, 'default/null']);

    // The tasks change: no poll picks it up, the app's message does
    tasksServer.use({ ...data, tasks: [{ ...task, name: 'Renamed by an agent' }, ...data.tasks.slice(1)] });
    await page.clock.runFor(30_000);
    await expect(card).toContainText(task.name);
    await tasksServer.pushChange(tasksServer.projectId);
    await expect(card).toContainText('Renamed by an agent');

    // A change the watcher missed shows when the window regains focus
    tasksServer.use({ ...data, tasks: [{ ...task, name: 'Renamed while away' }, ...data.tasks.slice(1)] });
    await page.evaluate(() => window.dispatchEvent(new Event('focus')));
    await expect(card).toContainText('Renamed while away');

    // Another project (a deep link: in-page, as the project menu goes): the first one's subscription ends, the second's
    // starts
    await tasksServer.pushLink('/projects/second/kanban');
    await expect(page.locator('#dashboard-title')).toHaveText('Minimal Project');
    await expect
      .poll(() => [...tasksServer.subscriptions.values()].map((s) => `${s.workspace}/${s.project}`).sort())
      .toEqual(['default/null', 'default/second']);
    expect(tasksServer.httpRequests).toEqual([]);
  });

  test('a newer version of the app is offered, installed only on the click; a failure says why; Not now lasts (T-483)', async ({
    page,
    tasksServer,
  }) => {
    await tasksServer.asApp();
    tasksServer.use('standard');
    tasksServer.update = { version: '9.9.9', current_version: '0.34.0', notes: '- One thing.\n- Another.' };
    tasksServer.updateFails = 'the signature does not verify';
    await page.goto('/');
    const bar = page.locator('#app-update');
    await expect(bar).toContainText('Agentic Task Manager 9.9.9 is available (you have 0.34.0).');
    await bar.locator('summary').click();
    await expect(bar.locator('.app-update-notes p')).toHaveText('- One thing.\n- Another.');
    // Nothing is installed until the click
    expect(tasksServer.updateCalls).toEqual(['check_update']);

    await page.locator('#app-update-install').click();
    await expect(bar.locator('.app-update-error')).toHaveText('Couldn’t install it: the signature does not verify');
    expect(tasksServer.updateCalls).toEqual(['check_update', 'install_update']);

    // Not now: gone, and not offered again for this version, after a reload too
    await bar.getByRole('button', { name: 'Not now' }).click();
    await expect(bar).toHaveCount(0);
    await page.reload();
    await expect(page.locator('#dashboard-title')).toHaveText('Standard Project');
    await expect.poll(() => tasksServer.updateCalls.filter((c) => c === 'check_update').length).toBe(2);
    await expect(bar).toHaveCount(0);
  });

  test('the install shows its download’s progress, then restarts; an app still there offers Restart now (T-526)', async ({
    page,
    tasksServer,
  }) => {
    await page.clock.install();
    await tasksServer.asApp();
    tasksServer.use('standard');
    tasksServer.update = { version: '9.9.9', current_version: '0.36.0', notes: null };
    const MB = 1024 * 1024;
    tasksServer.updateProgress = [
      { step: 'downloading', downloaded: MB / 4, total: 4 * MB },
      { step: 'downloading', downloaded: MB, total: 4 * MB },
    ];
    let finish = () => {};
    tasksServer.updateInstallHeld = new Promise<void>((resolve) => (finish = resolve));
    await page.goto('/');
    const bar = page.locator('#app-update');
    await expect(bar).toContainText('Agentic Task Manager 9.9.9 is available (you have 0.36.0).');

    // Downloading: how much of how much, on a bar; the buttons wait
    await page.locator('#app-update-install').click();
    await expect(bar.locator('#app-update-step')).toHaveText('Downloading… 1.0 of 4.0 MB');
    await expect(bar.locator('.app-update-progress')).toHaveAttribute('value', String(MB));
    await expect(bar.locator('.app-update-progress')).toHaveAttribute('max', String(4 * MB));
    await expect(page.locator('#app-update-install')).toBeDisabled();
    await expect(bar.getByRole('button', { name: 'Not now' })).toBeDisabled();

    // Installed: the page asks the app to restart
    finish();
    await expect(bar.locator('#app-update-step')).toHaveText('Restarting…');
    await expect.poll(() => tasksServer.updateCalls).toEqual(['check_update', 'install_update', 'restart_app']);
    await expect(bar.locator('.app-update-progress')).toHaveCount(0);

    // Still here after the wait: installed, Restart now tries again
    await page.clock.runFor(10_000);
    await expect(bar).toContainText('Agentic Task Manager 9.9.9 is installed: restart to use it.');
    await page.locator('#app-update-restart').click();
    await expect(bar.locator('#app-update-step')).toHaveText('Restarting…');
    await expect
      .poll(() => tasksServer.updateCalls)
      .toEqual(['check_update', 'install_update', 'restart_app', 'restart_app']);
  });

  test('a Mermaid diagram draws in the app: the frames and the pack’s module from the bundle’s origin CompMD’s plugin gives (ADR-006)', async ({
    page,
    tasksServer,
  }) => {
    await tasksServer.asApp();
    tasksServer.use('standard');
    const modules: string[] = [];
    const frames: string[] = [];
    page.on('request', (r) => {
      const url = r.url();
      if (url.includes('/compmd/packs/')) modules.push(r.frame() === page.mainFrame() ? 'page' : 'frame');
      if (/\/compmd\/frames\/[0-9a-f]{16}\/strict\.html$/.test(url)) frames.push(url);
    });
    await page.goto('/');
    // Help ▸ Help (the app's footer is a status bar, T-516)
    await expect(page.locator('#dashboard-title')).not.toBeEmpty();
    await tasksServer.chooseMenu({ kind: 'help' });
    const svg = page.frameLocator('#help-panel iframe').locator('svg').first();
    await expect(svg).toBeVisible({ timeout: 15_000 });
    await expect(svg).toContainText('In progress');
    // The frame came from the origin the plugin gave the page, under the bundle's hash; the pack's module was loaded
    // by the frame, by its place, never by the page (no code handed over)
    expect(frames.length).toBeGreaterThan(0);
    expect(frames.every((url) => url.startsWith(page.url().split('/').slice(0, 3).join('/') + '/assets/compmd/'))).toBe(
      true,
    );
    expect(modules.length).toBeGreaterThan(0);
    expect(modules.every((by) => by === 'frame')).toBe(true);
    expect(tasksServer.httpRequests).toEqual([]);
  });

  test('external links open in the user’s browser through the app’s open_url; links within the page stay (T-551)', async ({
    page,
    tasksServer,
  }) => {
    await tasksServer.asApp();
    const data = loadFixture('v4');
    data.meta = { ...data.meta, repository: 'https://github.com/acme/demo' };
    const task = data.tasks.find((t) => t.id === 'T-003')!;
    task.description = 'See [the reset docs](https://docs.example/reset) and [the endpoint](#heading).';
    tasksServer.use(data);
    await page.goto(`/projects/${tasksServer.projectId}/list?task=T-003`);
    // A commit link (RepoRef) and a link in the description (CompMD's)
    const commit = page.locator('#detail-commit a.repo-ref');
    const href = (await commit.getAttribute('href'))!;
    expect(href).toMatch(/^https:\/\/github\.com\/acme\/demo\/commit\/0f1e2d3/);
    await commit.click();
    await expect.poll(() => tasksServer.openedUrls).toEqual([href]);
    const description = page.locator('#detail-overlay');
    await description.getByRole('link', { name: 'the reset docs' }).click();
    await expect.poll(() => tasksServer.openedUrls.at(-1)).toBe('https://docs.example/reset');
    // A link within the text stays in the page, and the page stayed where it was
    await description.getByRole('link', { name: 'the endpoint' }).click();
    expect(addressOf(page)).toContain('task=T-003');
    expect(tasksServer.openedUrls).toHaveLength(2);
    await page.keyboard.press('Escape');

    // About's source link, and Help ▸ CompMD
    await tasksServer.chooseMenu({ kind: 'about' });
    const source = page.locator('#about-overlay .about-facts a').first();
    const repository = (await source.getAttribute('href'))!;
    expect(repository).toMatch(/^https:\/\//);
    await source.click();
    await expect.poll(() => tasksServer.openedUrls.at(-1)).toBe(repository);
    await page.keyboard.press('Escape');
    await tasksServer.chooseMenu({ kind: 'compmd' });
    await expect.poll(() => tasksServer.openedUrls.at(-1)).toBe('https://compmd.dev/');
    expect(tasksServer.openedUrls).toHaveLength(4);
  });

  test('in the browser an external link stays a new tab’s, and no app command is called (T-551)', async ({
    page,
    tasksServer,
  }) => {
    const data = loadFixture('v4');
    data.meta = { ...data.meta, repository: 'https://github.com/acme/demo' };
    tasksServer.use(data);
    await page.goto(`/projects/${tasksServer.projectId}/list?task=T-003`);
    const commit = page.locator('#detail-commit a.repo-ref');
    await expect(commit).toHaveAttribute('target', '_blank');
    await expect(commit).toHaveAttribute('rel', 'noopener noreferrer');
    await page.keyboard.press('Escape');
    await page.locator('#footer-about').click();
    const source = page.locator('#about-overlay .about-facts a').first();
    await expect(source).toHaveAttribute('target', '_blank');
    await expect(source).toHaveAttribute('rel', 'noopener noreferrer');
    expect(tasksServer.openedUrls).toEqual([]);
  });

  test('in the browser there is no button: the dialog gives the command instead', async ({ page, tasksServer }) => {
    tasksServer.use('standard');
    await page.goto('/?agents');
    await expect(page.locator('#footer-agents')).toHaveCount(0);
    await expect(page.locator('#agents-panel .agents-intro')).toContainText('atm agents install');
    await expect(page.locator('#agents-panel .agents-connect')).toHaveCount(0);
  });

  test('a project’s logo comes through the app too, as a data: URL the page’s CSP allows', async ({
    page,
    tasksServer,
  }) => {
    await tasksServer.asApp();
    tasksServer.addProject('second', 'minimal', 'Second Project');
    tasksServer.logo(
      'second',
      '<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><rect width="8" height="8"/></svg>',
    );
    tasksServer.use('standard');
    await page.goto('/projects');
    const logo = page.locator('.project-card[data-id="second"] .project-logo img');
    await expect(logo).toHaveAttribute('src', /^data:image\/svg\+xml;base64,/);
    expect(tasksServer.httpRequests).toEqual([]);
  });
});
