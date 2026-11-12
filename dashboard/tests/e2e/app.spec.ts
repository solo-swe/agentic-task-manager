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

  test('the title bar (T-513): on macOS the system’s buttons and menu bar; elsewhere the page draws them', async ({
    page,
    tasksServer,
  }) => {
    await tasksServer.asApp();
    tasksServer.use('standard');
    await page.goto('/');
    // One row: the tab, labelled with the project's name, and a +; the page's header, its subtitle, Filters and the
    // charts are inside the tab, under it, without a second title
    await expect(page.locator('#app-titlebar .title-bar-tab.selected #dashboard-title')).toHaveText('Standard Project');
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
    expect(await page.locator('#filters-toggle .filters-label').evaluate((e) => e.getBoundingClientRect().width)).toBe(
      1,
    );
    await page.locator('#filters-toggle').click();
    await expect(page.locator('.filter-panel')).toBeVisible();
    expect(await page.locator('.filter-panel').evaluate((e) => e.getBoundingClientRect().left)).toBeGreaterThanOrEqual(
      0,
    );
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
    const free = await page.evaluate(() => document.querySelector('#app-tab-new')!.getBoundingClientRect().right + 16);
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
    await expect(bar).toContainText('atm 9.9.9 is available (you have 0.34.0).');
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

  test('a Mermaid diagram draws in the app: the frames from the plugin’s base, the pack’s module given as code (T-503)', async ({
    page,
    tasksServer,
  }) => {
    await tasksServer.asApp();
    tasksServer.use('standard');
    const modules: string[] = [];
    page.on('request', (r) => {
      if (r.url().includes('/compmd/packs/')) modules.push(`${r.frame() === page.mainFrame() ? 'page' : 'frame'}`);
    });
    await page.goto('/');
    // Help ▸ atm Help (the app's footer is a status bar, T-516)
    await expect(page.locator('#dashboard-title')).not.toBeEmpty();
    await tasksServer.chooseMenu({ kind: 'help' });
    const svg = page.frameLocator('#help-panel iframe').locator('svg').first();
    await expect(svg).toBeVisible({ timeout: 15_000 });
    await expect(svg).toContainText('In progress');
    // The page fetched the module once; no frame asked for it by URL
    expect(modules).toEqual(['page']);
    expect(tasksServer.httpRequests).toEqual([]);
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
