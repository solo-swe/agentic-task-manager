import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { listenForMenu, menuDefinition, reportMenuState, shortcutLabel, type MenuAction } from '../../src/lib/api/menu';
import {
  COMPMD_URL,
  menuStateOf,
  pageLink,
  runDrawnAction,
  runMenuAction,
  runSystemItem,
  type MenuTarget,
  type PageActions,
  type WindowActions,
} from '../../src/lib/menu-actions';
import { pluginInfo } from '../../src/lib/plugin-info';
import type { ViewId } from '../../src/lib/tabs';

/** A stand-in for the dashboard, recording what each action did */
function target(onProject = true): MenuTarget & { did: string[] } {
  const did: string[] = [];
  return {
    did,
    onProject,
    tab: 'kanban' as ViewId,
    includeArchived: false,
    workspace: 'work',
    settingsOpen: false,
    filtersOpen: false,
    openDialog: (id) => did.push(`dialog ${id}`),
    openWhatsNew: () => did.push('whats new'),
    selectTab: (tab) => did.push(`tab ${tab}`),
    setIncludeArchived: (on) => did.push(`archived ${on}`),
    openProject: (id) => did.push(`project ${id}`),
    openWorkspace: (id) => did.push(`workspace ${id}`),
    checkAppUpdate: async (_now, force) => void did.push(`update ${force}`),
  };
}

function page(): PageActions & { did: string[] } {
  const did: string[] = [];
  return {
    did,
    back: () => did.push('back'),
    forward: () => did.push('forward'),
    reload: () => did.push('reload'),
    open: (url) => did.push(`open ${url}`),
    copy: async (text) => void did.push(`copy ${text}`),
    focusSearch: () => did.push('search'),
  };
}

describe('the app’s menu on the page (T-512)', () => {
  afterEach(() => clearMocks());

  it('does what the same control does', () => {
    const t = target();
    const p = page();
    const run = (action: MenuAction) => runMenuAction(action, t, p);
    run({ kind: 'about' });
    run({ kind: 'help' });
    run({ kind: 'connect_agents' });
    run({ kind: 'whats_new' });
    run({ kind: 'check_for_updates' });
    run({ kind: 'show_list' });
    run({ kind: 'show_graph' });
    run({ kind: 'toggle_archived' });
    run({ kind: 'overview' });
    run({ kind: 'project', id: 'abc' });
    run({ kind: 'workspace', id: 'home' });
    expect(t.did).toEqual([
      'dialog about',
      'dialog help',
      'dialog agents',
      'whats new',
      'update true',
      'tab list',
      'tab graph',
      'archived true',
      'project null',
      'project abc',
      'workspace home',
    ]);
    run({ kind: 'find' });
    run({ kind: 'reload' });
    run({ kind: 'back' });
    run({ kind: 'forward' });
    run({ kind: 'github' });
    run({ kind: 'compmd' });
    expect(p.did).toEqual([
      'search',
      'reload',
      'back',
      'forward',
      `open ${pluginInfo.repository}`,
      `open ${COMPMD_URL}`,
    ]);
  });

  it('opens and closes the popups, and copies the page’s link', async () => {
    const t = target();
    const p = page();
    runMenuAction({ kind: 'settings' }, t, p);
    runMenuAction({ kind: 'toggle_filters' }, t, p);
    expect([t.settingsOpen, t.filtersOpen]).toEqual([true, true]);
    runMenuAction({ kind: 'toggle_filters' }, t, p);
    expect(t.filtersOpen).toBe(false);
    runMenuAction({ kind: 'copy_link' }, t, p);
    await Promise.resolve();
    expect(p.did).toEqual([`copy ${pageLink()}`]);
    expect(pageLink('/projects/abc/list?task=T-1')).toBe('atm://projects/abc/list?task=T-1');
  });

  it('switches views only on a project’s page, and leaves tabs and the sidebar for later', () => {
    const t = target(false);
    const p = page();
    for (const kind of ['show_kanban', 'new_tab', 'close_tab', 'next_tab', 'toggle_sidebar', 'quit'] as const)
      runMenuAction({ kind }, t, p);
    expect([...t.did, ...p.did]).toEqual([]);
  });

  it('in the menus the page draws, Quit and New Window are the window’s, the system’s items the webview’s', () => {
    const did: string[] = [];
    const window: WindowActions = {
      openWindow: async (address) => void did.push(`open ${address}`),
      closeWindow: async () => void did.push('close'),
      minimize: async () => void did.push('minimize'),
      exec: (command) => void did.push(`exec ${command}`),
    };
    const t = target();
    runDrawnAction({ kind: 'quit' }, t, window, page());
    runDrawnAction({ kind: 'new_window' }, t, window, page());
    runDrawnAction({ kind: 'show_list' }, t, window, page());
    for (const item of ['copy', 'select_all', 'minimize', 'services'] as const) runSystemItem(item, window);
    expect(did).toEqual(['close', 'open /', 'exec copy', 'exec selectAll', 'minimize']);
    expect(t.did).toEqual(['tab list']);
  });

  it('shows shortcuts as Windows and Linux write them', () => {
    expect(shortcutLabel('CmdOrCtrl+Digit3')).toBe('Ctrl+3');
    expect(shortcutLabel('Alt+CmdOrCtrl+Comma')).toBe('Alt+Ctrl+,');
    expect(shortcutLabel('Alt+ArrowLeft')).toBe('Alt+←');
    expect(shortcutLabel(null)).toBe('');
  });

  it('asks the app for its menus, none when it can’t say', async () => {
    mockIPC((command) => (command === 'menu' ? [{ label: 'View', windows: false, entries: [] }] : null));
    expect(await menuDefinition()).toEqual([{ label: 'View', windows: false, entries: [] }]);
    clearMocks();
    const logged = vi.spyOn(console, 'error').mockImplementation(() => {});
    mockIPC(() => {
      throw new Error('no app');
    });
    expect(await menuDefinition()).toEqual([]);
    logged.mockRestore();
  });

  it('reports what the menu shows checked', () => {
    expect(menuStateOf(target())).toEqual({
      view: 'kanban',
      sidebar: false,
      filters: false,
      archived: false,
      workspace: 'work',
    });
    expect(menuStateOf(target(false)).view).toBeNull();
  });

  it('listens on a channel and reports the state through the app’s commands', async () => {
    const asked: [string, unknown][] = [];
    mockIPC((command, args) => {
      asked.push([command, command === 'menu_state' ? (args as { shown: unknown }).shown : null]);
    });
    await listenForMenu(() => {});
    const shown = menuStateOf(target());
    await reportMenuState(shown);
    expect(asked).toEqual([
      ['subscribe_menu', null],
      ['menu_state', shown],
    ]);

    clearMocks();
    const logged = vi.spyOn(console, 'error').mockImplementation(() => {});
    mockIPC(() => {
      throw new Error('no app');
    });
    await reportMenuState(shown);
    expect(logged).toHaveBeenCalled();
    logged.mockRestore();
  });
});
