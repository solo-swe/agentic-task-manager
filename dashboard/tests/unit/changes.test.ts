import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { watchChanges, type Changed } from '../../src/lib/api/changes';

describe('the app’s changes (T-506)', () => {
  afterEach(() => clearMocks());

  /** The app's subscriptions, as the mock answers them: what was subscribed, ended, and each one's channel */
  function app() {
    const subscribed: string[] = [];
    const ended: number[] = [];
    const channels = new Map<number, number>();
    let next = 0;
    let answer: () => Promise<void> = () => Promise.resolve();
    mockIPC(async (command, args) => {
      const a = (args ?? {}) as Record<string, unknown>;
      if (command === 'unsubscribe') {
        ended.push(a.id as number);
        return true;
      }
      await answer();
      const id = ++next;
      subscribed.push(`${command} ${String(a.workspace)}${a.project ? `/${String(a.project)}` : ''}`);
      channels.set(id, (a.channel as { id: number }).id);
      return id;
    });
    return { subscribed, ended, channels, slowly: (wait: () => Promise<void>) => (answer = wait) };
  }

  /** A message on a channel, as Tauri delivers it (the mocks' runCallback) */
  function send(channel: number, message: Changed, index = 0) {
    const internals = (window as unknown as { __TAURI_INTERNALS__: { runCallback: (id: number, m: unknown) => void } })
      .__TAURI_INTERNALS__;
    internals.runCallback(channel, { index, message });
  }

  it('watches the workspace, and the project when one is open; each message reaches the page', async () => {
    const mock = app();
    const seen: Changed[] = [];
    const stop = watchChanges('default', 'abcd1234', (c) => seen.push(c));
    await vi.waitFor(() => expect(mock.subscribed).toHaveLength(2));
    expect(mock.subscribed.sort()).toEqual(['subscribe_project default/abcd1234', 'subscribe_workspace default']);

    for (const [, channel] of mock.channels) send(channel, { workspace: 'default', project: 'abcd1234' });
    expect(seen).toHaveLength(2);

    stop();
    await vi.waitFor(() => expect(mock.ended.sort()).toEqual([1, 2]));
  });

  it('with no project open, only the workspace', async () => {
    const mock = app();
    watchChanges('work', null, () => {});
    await vi.waitFor(() => expect(mock.subscribed).toEqual(['subscribe_workspace work']));
  });

  it('ended before the app answered: unsubscribed once it does, and no message reaches the page', async () => {
    const mock = app();
    let answer = () => {};
    mock.slowly(() => new Promise<void>((resolve) => (answer = resolve)));
    const seen: Changed[] = [];
    const stop = watchChanges('default', null, (c) => seen.push(c));
    stop();
    answer();
    await vi.waitFor(() => expect(mock.ended).toEqual([1]));
    for (const [, channel] of mock.channels) send(channel, { workspace: 'default', project: null });
    expect(seen).toEqual([]);
  });
});
