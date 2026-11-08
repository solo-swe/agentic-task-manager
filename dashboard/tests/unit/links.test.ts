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
});
