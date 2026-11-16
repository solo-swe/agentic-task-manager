import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  RECONNECT_FIRST_MS,
  RECONNECT_MAX_MS,
  SLOW_POLL_MS,
  TRIES_BEFORE_POLLING,
  streamChanges,
  subscribeInApp,
  type ChangeSource,
  type Changed,
} from '../../src/lib/api/changes';
import { changesUrl } from '../../src/lib/api/http';

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
    const stop = subscribeInApp('default', 'abcd1234', (c) => seen.push(c));
    await vi.waitFor(() => expect(mock.subscribed).toHaveLength(2));
    expect(mock.subscribed.sort()).toEqual(['subscribe_project default/abcd1234', 'subscribe_workspace default']);

    for (const [, channel] of mock.channels) send(channel, { workspace: 'default', project: 'abcd1234' });
    expect(seen).toHaveLength(2);

    stop();
    await vi.waitFor(() => expect(mock.ended.sort()).toEqual([1, 2]));
  });

  it('with no project open, only the workspace', async () => {
    const mock = app();
    subscribeInApp('work', null, () => {});
    await vi.waitFor(() => expect(mock.subscribed).toEqual(['subscribe_workspace work']));
  });

  it('ended before the app answered: unsubscribed once it does, and no message reaches the page', async () => {
    const mock = app();
    let answer = () => {};
    mock.slowly(() => new Promise<void>((resolve) => (answer = resolve)));
    const seen: Changed[] = [];
    const stop = subscribeInApp('default', null, (c) => seen.push(c));
    stop();
    answer();
    await vi.waitFor(() => expect(mock.ended).toEqual([1]));
    for (const [, channel] of mock.channels) send(channel, { workspace: 'default', project: null });
    expect(seen).toEqual([]);
  });
});

describe('atm serve’s change stream (T-507)', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  /** A stand-in for EventSource: each one opened, which a test opens, fails or sends on */
  class FakeSource implements ChangeSource {
    onopen: ((event: Event) => void) | null = null;
    onerror: ((event: Event) => void) | null = null;
    closed = false;
    private listeners: ((event: MessageEvent<string>) => void)[] = [];
    constructor(readonly url: string) {}
    addEventListener(_type: 'changed', listener: (event: MessageEvent<string>) => void) {
      this.listeners.push(listener);
    }
    close() {
      this.closed = true;
    }
    open() {
      this.onopen?.(new Event('open'));
    }
    fail() {
      this.onerror?.(new Event('error'));
    }
    send(data: string) {
      for (const listener of this.listeners) listener(new MessageEvent('changed', { data }));
    }
  }

  const WORKSPACE: Changed = { workspace: 'work', project: null };

  function stream() {
    const sources: FakeSource[] = [];
    const seen: Changed[] = [];
    const stop = streamChanges(
      changesUrl('work'),
      WORKSPACE,
      (c) => seen.push(c),
      (url) => {
        const source = new FakeSource(url);
        sources.push(source);
        return source;
      },
    );
    const last = () => sources[sources.length - 1];
    return { sources, seen, stop, last };
  }

  it('opens the workspace’s stream, and each event reaches the page with what it names', () => {
    const s = stream();
    expect(s.sources.map((x) => x.url)).toEqual(['/api/v2/workspaces/work/changes']);
    s.last().open();
    expect(s.seen).toEqual([]); // the page has just loaded: nothing to reload for
    s.last().send('{"workspace":"work","project":"abcd1234"}');
    s.last().send('not json'); // still a change: what the stream is about
    expect(s.seen).toEqual([{ workspace: 'work', project: 'abcd1234' }, WORKSPACE]);
  });

  it('a lost stream comes back after a wait that doubles, to a limit; back, the page reloads once', () => {
    const s = stream();
    s.last().open();
    const waits: number[] = [];
    for (let i = 0; i < 7; i++) {
      const failed = s.last();
      failed.fail();
      expect(failed.closed).toBe(true);
      const before = s.sources.length;
      let waited = 0;
      while (s.sources.length === before) {
        vi.advanceTimersByTime(100);
        waited += 100;
      }
      waits.push(waited);
    }
    expect(waits).toEqual([1, 2, 4, 8, 16, 30, 30].map((n) => n * 1000));
    expect(RECONNECT_FIRST_MS).toBe(1000);
    expect(RECONNECT_MAX_MS).toBe(30_000);
    // It couldn't connect for a while: the slow poll reloaded meanwhile
    const polled = s.seen.length;
    expect(polled).toBeGreaterThan(0);
    s.last().open();
    expect(s.seen).toHaveLength(polled + 1);
    // Open again: the slow poll stops
    vi.advanceTimersByTime(SLOW_POLL_MS * 3);
    expect(s.seen).toHaveLength(polled + 1);
  });

  it('polls slowly only while it can’t connect, after a few tries', () => {
    const s = stream();
    for (let i = 1; i < TRIES_BEFORE_POLLING; i++) {
      s.last().fail();
      vi.advanceTimersByTime(RECONNECT_MAX_MS);
    }
    vi.advanceTimersByTime(SLOW_POLL_MS);
    expect(s.seen).toEqual([]); // not yet
    s.last().fail();
    vi.advanceTimersByTime(SLOW_POLL_MS - 1);
    expect(s.seen).toEqual([]);
    vi.advanceTimersByTime(1);
    expect(s.seen).toEqual([WORKSPACE]);
  });

  it('ended: the stream closes, nothing comes back, nothing reaches the page', () => {
    const s = stream();
    s.last().open();
    s.last().fail();
    const waiting = s.sources.length;
    s.stop();
    vi.advanceTimersByTime(SLOW_POLL_MS * 2);
    expect(s.sources).toHaveLength(waiting);
    s.last().send('{"workspace":"work","project":null}');
    expect(s.seen).toEqual([]);

    const t = stream();
    t.stop();
    expect(t.last().closed).toBe(true);
  });
});
