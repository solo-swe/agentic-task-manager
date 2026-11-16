import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { getPref, PREFS_KEY, setPref } from '../../src/lib/prefs';

describe('prefs', () => {
  beforeEach(() => localStorage.clear());
  afterEach(() => vi.restoreAllMocks());

  it('returns defaults when nothing is stored', () => {
    expect(getPref('autoRefresh', true)).toBe(true);
    expect(getPref('autoRefreshSeconds', 5)).toBe(5);
  });

  it('round-trips values and keeps other keys', () => {
    setPref('autoRefresh', false);
    setPref('autoRefreshSeconds', 30);
    expect(getPref('autoRefresh', true)).toBe(false);
    expect(getPref('autoRefreshSeconds', 5)).toBe(30);
    expect(JSON.parse(localStorage.getItem(PREFS_KEY)!)).toEqual({ autoRefresh: false, autoRefreshSeconds: 30 });
  });

  it('ignores values of the wrong type', () => {
    localStorage.setItem(PREFS_KEY, JSON.stringify({ autoRefresh: 'no', autoRefreshSeconds: '30' }));
    expect(getPref('autoRefresh', true)).toBe(true);
    expect(getPref('autoRefreshSeconds', 5)).toBe(5);
  });

  it('survives corrupt or non-object JSON', () => {
    localStorage.setItem(PREFS_KEY, '{ not json');
    expect(getPref('autoRefresh', true)).toBe(true);
    localStorage.setItem(PREFS_KEY, '[1,2]');
    expect(getPref('lastSeenVersion', '')).toBe('');
    setPref('autoRefresh', false);
    expect(getPref('autoRefresh', true)).toBe(false);
  });

  it('never throws when storage itself throws', () => {
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new Error('blocked');
    });
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('quota');
    });
    expect(() => setPref('autoRefresh', false)).not.toThrow();
    expect(getPref('autoRefresh', true)).toBe(true);
  });

  it('never throws when localStorage is inaccessible', () => {
    vi.spyOn(window, 'localStorage', 'get').mockImplementation(() => {
      throw new DOMException('denied', 'SecurityError');
    });
    expect(getPref('autoRefreshSeconds', 5)).toBe(5);
    expect(() => setPref('autoRefreshSeconds', 10)).not.toThrow();
  });
});

describe('storageAvailable', () => {
  afterEach(() => vi.restoreAllMocks());

  it('is true when values round-trip and leaves no probe behind', async () => {
    const { storageAvailable } = await import('../../src/lib/prefs');
    localStorage.clear();
    expect(storageAvailable()).toBe(true);
    expect(localStorage.length).toBe(0);
  });

  it('is false when writing throws', async () => {
    const { storageAvailable } = await import('../../src/lib/prefs');
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('quota');
    });
    expect(storageAvailable()).toBe(false);
  });

  it('is false when localStorage is inaccessible', async () => {
    const { storageAvailable } = await import('../../src/lib/prefs');
    vi.spyOn(window, 'localStorage', 'get').mockImplementation(() => {
      throw new DOMException('denied', 'SecurityError');
    });
    expect(storageAvailable()).toBe(false);
  });
});
