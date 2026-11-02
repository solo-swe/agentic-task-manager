import { afterEach, describe, expect, it, vi } from 'vitest';
import { cutOff, scrollBehavior, scrollToTask } from '../../src/lib/scroll';

describe('scrollBehavior (T-345)', () => {
  afterEach(() => vi.unstubAllGlobals());

  it('smooth, but at once when the system asks to reduce motion, or nothing can tell', () => {
    const media = (reduce: boolean) => vi.fn((q: string) => ({ matches: reduce && q.includes('reduce') }));
    vi.stubGlobal('matchMedia', media(false));
    expect(scrollBehavior()).toBe('smooth');
    vi.stubGlobal('matchMedia', media(true));
    expect(scrollBehavior()).toBe('auto');
    vi.stubGlobal('matchMedia', undefined);
    expect(scrollBehavior()).toBe('smooth');
  });
});

describe('cutOff', () => {
  // Items 100px tall, 10px apart: 0–100, 110–210, 220–320, 330–430
  const items = [0, 110, 220, 330].map((offsetTop) => ({ offsetTop, offsetHeight: 100 }));

  it('counts nothing when everything is in view', () => {
    expect(cutOff(0, 500, items)).toEqual({ above: 0, below: 0 });
  });

  it('counts the items partly or fully below the bottom edge', () => {
    expect(cutOff(0, 250, items)).toEqual({ above: 0, below: 2 });
  });

  it('counts the items whose top is above the top edge', () => {
    expect(cutOff(150, 300, items)).toEqual({ above: 2, below: 0 });
  });

  it('counts both ways in the middle', () => {
    // 115–315: the first two start above, the last two end below
    expect(cutOff(115, 200, items)).toEqual({ above: 2, below: 2 });
  });

  it('allows a pixel of rounding at either edge', () => {
    // 111–431: the second item starts 1px above and the last ends 1px below, both counted as in view
    expect(cutOff(111, 320, items)).toEqual({ above: 1, below: 0 });
    expect(cutOff(108, 320, items)).toEqual({ above: 1, below: 1 });
  });
});

describe('scrollToTask', () => {
  afterEach(() => {
    vi.useRealTimers();
    document.body.innerHTML = '';
  });

  it('scrolls the card into view and highlights it for a moment', () => {
    vi.useFakeTimers();
    document.body.innerHTML = '<div id="task-T-001"></div>';
    const card = document.getElementById('task-T-001')!;
    card.scrollIntoView = vi.fn();
    scrollToTask('T-001');
    expect(card.scrollIntoView).toHaveBeenCalledWith({ behavior: 'smooth', block: 'center' });
    expect(card.style.borderColor).toBe('var(--yellow)');
    vi.advanceTimersByTime(2000);
    expect(card.style.borderColor).toBe('');
    expect(card.style.boxShadow).toBe('');
  });

  it('does nothing for a task without a card', () => {
    expect(() => scrollToTask('T-404')).not.toThrow();
  });
});
