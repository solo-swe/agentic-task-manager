import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { getPref, PREFS_KEY, setPref } from '../../src/lib/prefs';

describe('prefs', () => {
  beforeEach(() => localStorage.clear());
  afterEach(() => vi.restoreAllMocks());

  it('returns defaults when nothing is stored', () => {
    expect(getPref('needsYouBadge', true)).toBe(true);
    expect(getPref('sidebarWidth', 240)).toBe(240);
  });

  it('round-trips values and keeps other keys', () => {
    setPref('needsYouBadge', false);
    setPref('sidebarWidth', 300);
    expect(getPref('needsYouBadge', true)).toBe(false);
    expect(getPref('sidebarWidth', 240)).toBe(300);
    expect(JSON.parse(localStorage.getItem(PREFS_KEY)!)).toEqual({ needsYouBadge: false, sidebarWidth: 300 });
  });

  it('ignores values of the wrong type', () => {
    localStorage.setItem(PREFS_KEY, JSON.stringify({ needsYouBadge: 'no', sidebarWidth: '300' }));
    expect(getPref('needsYouBadge', true)).toBe(true);
    expect(getPref('sidebarWidth', 240)).toBe(240);
  });

  it('survives corrupt or non-object JSON', () => {
    localStorage.setItem(PREFS_KEY, '{ not json');
    expect(getPref('needsYouBadge', true)).toBe(true);
    localStorage.setItem(PREFS_KEY, '[1,2]');
    expect(getPref('lastSeenVersion', '')).toBe('');
    setPref('needsYouBadge', false);
    expect(getPref('needsYouBadge', true)).toBe(false);
  });

  it('never throws when storage itself throws', () => {
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new Error('blocked');
    });
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('quota');
    });
    expect(() => setPref('needsYouBadge', false)).not.toThrow();
    expect(getPref('needsYouBadge', true)).toBe(true);
  });

  it('never throws when localStorage is inaccessible', () => {
    vi.spyOn(window, 'localStorage', 'get').mockImplementation(() => {
      throw new DOMException('denied', 'SecurityError');
    });
    expect(getPref('sidebarWidth', 240)).toBe(240);
    expect(() => setPref('sidebarWidth', 300)).not.toThrow();
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
