import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { render, waitFor } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { resetAppShell } from '../../src/lib/api/shell';
import AppTitleBar from '../../src/lib/shell/AppTitleBar.svelte';
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
  });
  afterEach(() => clearMocks());

  it('on macOS leaves room for the system’s buttons and draws no menus', async () => {
    app({ system_menu_bar: true, system_window_buttons: true });
    const { container } = render(AppTitleBar, { title: 'Standard Project' });
    await waitFor(() => expect(container.querySelector('.app-titlebar.system-buttons')).not.toBeNull());
    expect(container.querySelector('#window-buttons')).toBeNull();
    expect(container.querySelector('#app-menus')).toBeNull();
    expect(container.querySelector('#dashboard-title')).toHaveTextContent('Standard Project');
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
