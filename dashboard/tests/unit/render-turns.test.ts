import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { resetTurns, takeTurn, turnsWaiting } from '../../src/lib/render-turns';

// Renders that take turns (lib/render-turns.ts, ATM-587): one at a time, the one nearest the view first, each when
// the page is idle after the one before has drawn

/** An element at `top`..`bottom` in a window 800 high */
function at(top: number, bottom = top + 100): Element {
  const el = document.createElement('div');
  document.body.append(el);
  el.getBoundingClientRect = () => ({ top, bottom }) as DOMRect;
  return el;
}

/** A render that is done when told */
function render(name: string, log: string[]) {
  let done: () => void = () => {};
  const fn = vi.fn(
    () =>
      new Promise<void>((resolve) => {
        log.push(name);
        done = resolve;
      }),
  );
  return { fn, finish: () => done() };
}

let idle: (() => void)[] = [];
const runIdle = async () => {
  const waiting = idle;
  idle = [];
  waiting.forEach((act) => act());
  await Promise.resolve();
};

beforeEach(() => {
  resetTurns();
  idle = [];
  vi.stubGlobal('innerHeight', 800);
  vi.stubGlobal('requestIdleCallback', (act: () => void) => idle.push(act));
});

afterEach(() => {
  vi.unstubAllGlobals();
  document.body.innerHTML = '';
});

describe('takeTurn', () => {
  it('one at a time, each when the page is idle after the one before has drawn', async () => {
    const log: string[] = [];
    const a = render('a', log);
    const b = render('b', log);
    takeTurn(() => at(0), a.fn);
    takeTurn(() => at(100), b.fn);
    expect(log).toEqual([]);
    await runIdle();
    expect(log).toEqual(['a']);
    expect(turnsWaiting()).toEqual({ waiting: 1, running: true });
    // b waits for a to have drawn
    await runIdle();
    expect(log).toEqual(['a']);
    a.finish();
    await vi.waitFor(() => expect(idle).toHaveLength(1));
    await runIdle();
    expect(log).toEqual(['a', 'b']);
    b.finish();
    await vi.waitFor(() => expect(turnsWaiting()).toEqual({ waiting: 0, running: false }));
  });

  it('the one nearest the view goes first: in it, then by distance, above or below; one not on the page last', async () => {
    const log: string[] = [];
    const first = render('first', log);
    takeTurn(() => at(0), first.fn);
    await runIdle();
    // While the first runs, more come: far below, gone, just above, in view
    const turns = [
      ['far below', at(5000)],
      ['gone', undefined],
      ['just above', at(-300, -200)],
      ['in view', at(400)],
    ] as const;
    const made = turns.map(([name, el]) => {
      const r = render(name, log);
      takeTurn(() => el, r.fn);
      return r;
    });
    // Each one finished lets the nearest of those left go
    for (const r of [first, made[3], made[2], made[0]]) {
      r.finish();
      await vi.waitFor(() => expect(idle).toHaveLength(1));
      await runIdle();
    }
    expect(log).toEqual(['first', 'in view', 'just above', 'far below', 'gone']);
  });

  it('a turn cancelled never runs; one whose render fails lets the next go', async () => {
    const log: string[] = [];
    const failing = vi.fn(() => {
      log.push('failing');
      return Promise.reject(new Error('no'));
    });
    const cancelled = render('cancelled', log);
    const next = render('next', log);
    takeTurn(() => at(0), failing);
    const cancel = takeTurn(() => at(0), cancelled.fn);
    takeTurn(() => at(50), next.fn);
    cancel();
    await runIdle();
    await vi.waitFor(() => expect(idle).toHaveLength(1));
    await runIdle();
    expect(log).toEqual(['failing', 'next']);
  });

  // ATM-620: a turn picked to go next waits for the page to be idle; cancelled meanwhile (its CompMd destroyed), it
  // never renders, and the next one goes. Rendered, it made a view nothing would destroy, holding its card in memory
  it('a turn cancelled after it was picked, before the page was idle, never runs; the next one goes', async () => {
    const log: string[] = [];
    const picked = render('picked', log);
    const next = render('next', log);
    const cancel = takeTurn(() => at(0), picked.fn);
    takeTurn(() => at(50), next.fn);
    // `picked` was picked at once and waits for the page to be idle
    expect(turnsWaiting()).toEqual({ waiting: 1, running: true });
    cancel();
    await runIdle();
    await vi.waitFor(() => expect(idle).toHaveLength(1));
    await runIdle();
    expect(log).toEqual(['next']);
    expect(picked.fn).not.toHaveBeenCalled();
  });

  it('while a mouse button is held (a drag picking lines), no turn starts; let go, they go on', async () => {
    const log: string[] = [];
    window.dispatchEvent(new MouseEvent('mousedown'));
    takeTurn(() => at(0), render('a', log).fn);
    expect(idle).toHaveLength(0);
    window.dispatchEvent(new MouseEvent('mouseup'));
    await runIdle();
    expect(log).toEqual(['a']);
    // Pressed after its turn was given, before the page was idle: back in line until let go
    const b = render('b', log);
    resetTurns();
    takeTurn(() => at(0), b.fn);
    window.dispatchEvent(new MouseEvent('mousedown'));
    await runIdle();
    expect(log).toEqual(['a']);
    expect(turnsWaiting()).toEqual({ waiting: 1, running: false });
    window.dispatchEvent(new MouseEvent('mouseup'));
    await runIdle();
    expect(log).toEqual(['a', 'b']);
  });

  it('without an idle callback, a turn goes after a moment', async () => {
    vi.stubGlobal('requestIdleCallback', undefined);
    vi.useFakeTimers();
    try {
      const log: string[] = [];
      takeTurn(() => at(0), render('a', log).fn);
      expect(log).toEqual([]);
      vi.advanceTimersByTime(1);
      expect(log).toEqual(['a']);
    } finally {
      vi.useRealTimers();
    }
  });
});
