import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { appShell, NO_SHELL, openWindow, resetAppShell, setWindowTitle } from '../../src/lib/api/shell';

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
