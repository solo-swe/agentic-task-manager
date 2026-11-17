import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it } from 'vitest';
import { linkTarget, listenForLinks } from '../../src/lib/api/links';

describe('the app’s deep links (T-459)', () => {
  afterEach(() => clearMocks());

  it('go only to the dashboard’s own pages', () => {
    for (const address of ['/projects/abc/list?task=T-1', '/w/work/board', '/', '/projects?about'])
      expect(linkTarget(address), address).toBe(address);
    for (const address of [
      'https://example.com/projects',
      '//example.com/projects',
      'projects/abc',
      '/api/projects',
      '/p/abc/claims.json',
      '/projects/abc#x',
      '/projects/a/b/c',
      null,
      42,
    ])
      expect(linkTarget(address), String(address)).toBeNull();
  });

  it('open the link the app was started with, and leave out one that isn’t a page', async () => {
    const asked: string[] = [];
    mockIPC((command) => {
      asked.push(command);
      return command === 'subscribe_links' ? '/projects/abc/list?task=T-1' : null;
    });
    const went: string[] = [];
    await listenForLinks((address) => went.push(address));
    expect(asked).toEqual(['subscribe_links']);
    expect(went).toEqual(['/projects/abc/list?task=T-1']);

    clearMocks();
    mockIPC(() => '/api/projects');
    const none: string[] = [];
    await listenForLinks((address) => none.push(address));
    expect(none).toEqual([]);
  });

  it('started without a link, goes nowhere', async () => {
    mockIPC(() => null);
    const went: string[] = [];
    await listenForLinks((address) => went.push(address));
    expect(went).toEqual([]);
  });

  /** The channel the page handed `subscribe_links`, and a way to send it a link as Tauri delivers one */
  function app(first: string | null) {
    let channel = -1;
    mockIPC((command, args) => {
      if (command === 'subscribe_links') channel = (args as { channel: { id: number } }).channel.id;
      return first;
    });
    let index = 0;
    const send = (address: unknown) =>
      (
        window as unknown as { __TAURI_INTERNALS__: { runCallback: (id: number, m: unknown) => void } }
      ).__TAURI_INTERNALS__.runCallback(channel, { index: index++, message: address });
    return { send };
  }

  it('a link that comes while it runs goes to `later`, the first one to `go`; one that isn’t a page nowhere', async () => {
    const { send } = app('/board');
    const went: string[] = [];
    const later: string[] = [];
    await listenForLinks(
      (address) => went.push(address),
      (address) => later.push(address),
    );
    send('/projects/abc');
    send('https://example.com');
    send(null);
    send('/w/work/agents?about');
    expect(went).toEqual(['/board']);
    expect(later).toEqual(['/projects/abc', '/w/work/agents?about']);
  });

  it('without `later`, a link that comes while it runs goes to `go` too', async () => {
    const { send } = app(null);
    const went: string[] = [];
    await listenForLinks((address) => went.push(address));
    send('/projects/abc');
    expect(went).toEqual(['/projects/abc']);
  });
});
