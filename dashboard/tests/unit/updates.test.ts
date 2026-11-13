import { beforeEach, describe, expect, it, vi } from 'vitest';

// The app's commands, as the app answers them; a Channel that keeps what it was given
const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({
  invoke,
  Channel: class {
    onmessage: (m: unknown) => void = () => {};
  },
}));

const { installAppUpdate, restartApp } = await import('../../src/lib/api/updates');

describe('the app’s update install (T-526)', () => {
  beforeEach(() => invoke.mockReset());

  it('hands the app a channel for its progress and says it installed', async () => {
    const seen: unknown[] = [];
    invoke.mockImplementationOnce(async (...all: unknown[]) => {
      const args = all[1] as { progress: { onmessage: (m: unknown) => void } };
      args.progress.onmessage({ step: 'downloading', downloaded: 1, total: 2 });
      args.progress.onmessage({ step: 'installing' });
      return true;
    });
    expect(await installAppUpdate((p) => seen.push(p))).toEqual({ installed: true });
    expect(invoke.mock.calls[0][0]).toBe('install_update');
    expect(seen).toEqual([{ step: 'downloading', downloaded: 1, total: 2 }, { step: 'installing' }]);
  });

  it('nothing to install, and a failure with its message or as text', async () => {
    invoke.mockResolvedValueOnce(false);
    expect(await installAppUpdate(() => {})).toEqual({ installed: false });
    invoke.mockRejectedValueOnce({ kind: 'failed', message: 'the signature does not verify' });
    expect(await installAppUpdate(() => {})).toEqual({ error: 'the signature does not verify' });
    invoke.mockRejectedValueOnce('offline');
    expect(await installAppUpdate(() => {})).toEqual({ error: 'offline' });
  });

  it('the restart is its own command, and a failure only logged', async () => {
    const log = vi.spyOn(console, 'error').mockImplementation(() => {});
    invoke.mockResolvedValueOnce(null);
    await restartApp();
    expect(invoke).toHaveBeenLastCalledWith('restart_app');
    invoke.mockRejectedValueOnce('no');
    await restartApp();
    expect(log).toHaveBeenCalled();
    log.mockRestore();
  });
});
