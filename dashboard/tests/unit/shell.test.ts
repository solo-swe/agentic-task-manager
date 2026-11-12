import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  appShell,
  closeWindow,
  minimizeWindow,
  NO_SHELL,
  onBarMouseDown,
  openWindow,
  resetAppShell,
  restoredTabs,
  saveTabs,
  setBadge,
  setWindowTitle,
} from '../../src/lib/api/shell';

describe('the app’s window (T-511)', () => {
  afterEach(() => {
    clearMocks();
    resetAppShell();
    vi.restoreAllMocks();
  });

  it('asks for the shell once', async () => {
    const asked: string[] = [];
    mockIPC((command) => {
      asked.push(command);
      return { system_menu_bar: true, system_window_buttons: true };
    });
    expect(await appShell()).toEqual({ system_menu_bar: true, system_window_buttons: true });
    expect(await appShell()).toEqual({ system_menu_bar: true, system_window_buttons: true });
    expect(asked).toEqual(['shell']);
  });

  it('the tabs it comes back with are the dashboard’s pages only; keeping them and the badge (T-518)', async () => {
    const calls: [string, unknown][] = [];
    let answer: unknown = { addresses: ['/projects/a', 'https://example.com', '/board'], active: 2 };
    mockIPC((command, args) => {
      calls.push([command, args]);
      return command === 'restored_tabs' ? answer : null;
    });
    expect(await restoredTabs()).toEqual({ addresses: ['/projects/a', '/board'], active: 1 });
    answer = { addresses: ['//example.com'], active: 0 };
    expect(await restoredTabs()).toBeNull();
    answer = null;
    expect(await restoredTabs()).toBeNull();
    await saveTabs({ addresses: ['/board'], active: 0 });
    await setBadge(3);
    await setBadge(0);
    await setBadge(null);
    expect(calls.slice(3)).toEqual([
      ['save_tabs', { tabs: { addresses: ['/board'], active: 0 } }],
      ['set_badge', { count: 3 }],
      ['set_badge', { count: null }],
      ['set_badge', { count: null }],
    ]);
    // A failure is only logged
    const logged = vi.spyOn(console, 'error').mockImplementation(() => {});
    mockIPC(() => {
      throw new Error('no');
    });
    expect(await restoredTabs()).toBeNull();
    await saveTabs({ addresses: ['/'], active: 0 });
    await setBadge(1);
    expect(logged).toHaveBeenCalledTimes(3);
  });

  it('draws its own shell when the app can’t say', async () => {
    vi.spyOn(console, 'error').mockImplementation(() => {});
    mockIPC(() => {
      throw new Error('no such command');
    });
    expect(await appShell()).toEqual(NO_SHELL);
  });

  it('sets the window’s title, and a failure is only logged', async () => {
    const titles: unknown[] = [];
    mockIPC((command, args) => {
      if (command === 'set_title') titles.push((args as { title: string }).title);
    });
    await setWindowTitle('atm (2)');
    expect(titles).toEqual(['atm (2)']);

    clearMocks();
    const logged = vi.spyOn(console, 'error').mockImplementation(() => {});
    mockIPC(() => {
      throw new Error('no window');
    });
    await setWindowTitle('atm');
    expect(logged).toHaveBeenCalled();
  });

  it('drags the window from a bar’s free space, maximizes it on a double click, and leaves its controls alone', async () => {
    const asked: string[] = [];
    mockIPC((command) => void asked.push(command));
    const bar = document.createElement('div');
    const button = document.createElement('button');
    bar.append(button);
    const down = (target: Element, detail: number, button = 0) => {
      const e = new MouseEvent('mousedown', { button, detail, bubbles: true, cancelable: true });
      Object.defineProperty(e, 'target', { value: target });
      onBarMouseDown(e);
    };
    down(bar, 1);
    down(bar, 2);
    down(button, 1);
    down(bar, 1, 2);
    await Promise.resolve();
    await Promise.resolve();
    expect(asked).toEqual(['start_dragging', 'toggle_maximize']);

    asked.length = 0;
    await minimizeWindow();
    await closeWindow();
    expect(asked).toEqual(['minimize', 'close_window']);

    clearMocks();
    const logged = vi.spyOn(console, 'error').mockImplementation(() => {});
    mockIPC(() => {
      throw new Error('no window');
    });
    await minimizeWindow();
    expect(logged).toHaveBeenCalled();
  });

  it('opens a window only at one of the dashboard’s pages', async () => {
    const opened: unknown[] = [];
    mockIPC((command, args) => {
      if (command === 'open_window') opened.push((args as { address: string }).address);
    });
    expect(await openWindow('/projects/abc/list?task=T-1')).toBe(true);
    for (const address of ['https://example.com', '/api/projects', '//example.com/x', '/projects/a#x'])
      expect(await openWindow(address), address).toBe(false);
    expect(opened).toEqual(['/projects/abc/list?task=T-1']);

    clearMocks();
    vi.spyOn(console, 'error').mockImplementation(() => {});
    mockIPC(() => {
      throw new Error('refused');
    });
    expect(await openWindow('/board')).toBe(false);
  });
});
