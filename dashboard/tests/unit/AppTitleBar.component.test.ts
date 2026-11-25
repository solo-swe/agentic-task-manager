import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { render, waitFor } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { resetAppShell } from '../../src/lib/api/shell';
import { appTabs } from '../../src/lib/app-tabs.svelte';
import AppTitleBar from '../../src/lib/shell/AppTitleBar.svelte';
import { dashboard } from '../../src/lib/state.svelte';
import standard from '../fixtures/standard.json';
import { loadIntoDashboard } from './components.helpers';

/** The app answers `shell` with `shell`, a short menu for `menu`, and nothing else */
function app(shell: { system_menu_bar: boolean; system_window_buttons: boolean }) {
  mockIPC((command) => {
    if (command === 'shell') return shell;
    if (command === 'menu') return [{ label: 'View', windows: false, entries: [] }];
    return null;
  });
}

describe('AppTitleBar (T-513)', () => {
  beforeEach(() => {
    resetAppShell();
    loadIntoDashboard(standard);
    appTabs.start('/projects/fixture/kanban');
  });
  afterEach(() => clearMocks());

  it('on macOS leaves room for the system’s buttons while the sidebar is closed, and draws no menus (T-535)', async () => {
    app({ system_menu_bar: true, system_window_buttons: true });
    dashboard.sidebarOpen = false;
    const { container } = render(AppTitleBar, { title: 'Standard Project' });
    await waitFor(() => expect(container.querySelector('.app-titlebar.system-buttons')).not.toBeNull());
    // Closed, the sidebar button is in the bar
    expect(container.querySelector('#app-sidebar-toggle')).not.toBeNull();
    expect(container.querySelector('#window-buttons')).toBeNull();
    expect(container.querySelector('#app-menus')).toBeNull();
    // Titled by its scope and item (ATM-754): a project not listed by its ID
    expect(container.querySelector('#dashboard-title')).toHaveTextContent('fixture - Tasks');
    // Open, the traffic lights and the button are over the sidebar's strip instead
    dashboard.sidebarOpen = true;
    await waitFor(() => expect(container.querySelector('.app-titlebar.system-buttons')).toBeNull());
    expect(container.querySelector('#app-sidebar-toggle')).toBeNull();
  });

  it('names each tab by the page in its own address, chosen or not (T-525, ATM-754): its scope, its item and its icon', async () => {
    app({ system_menu_bar: true, system_window_buttons: true });
    dashboard.projects = [
      { id: 'fixture', name: 'Standard Project', path: '', missing: false, summary: { key: 'STD' } },
    ] as unknown as typeof dashboard.projects;
    appTabs.open('/');
    const { container } = render(AppTitleBar, { title: 'Standard Project' });
    // The page still shows the project (its title), but the chosen tab is All's Overview: named so at once
    expect(container.querySelector('#dashboard-title')).toHaveTextContent('All - Overview');
    // The other, the project's board: by its key, with the item's icon
    expect(container.querySelector('.app-tab-name')).toHaveTextContent('STD - Tasks');
    const icons = [...container.querySelectorAll('.app-tab-icon svg')].map((s) => s.getAttribute('class'));
    expect(icons).toEqual([expect.stringContaining('square-kanban'), expect.stringContaining('layout-dashboard')]);
    dashboard.projects = [];
  });

  it('on Windows and Linux draws its own buttons and the app’s menus', async () => {
    app({ system_menu_bar: false, system_window_buttons: false });
    const { container } = render(AppTitleBar, { title: 'Standard Project' });
    await waitFor(() => expect(container.querySelector('#window-buttons')).not.toBeNull());
    expect(container.querySelector('#app-menus')).not.toBeNull();
    expect(container.querySelector('.app-titlebar.system-buttons')).toBeNull();
    for (const id of ['#window-minimize', '#window-maximize', '#window-close'])
      expect(container.querySelector(id), id).not.toBeNull();
  });
});
