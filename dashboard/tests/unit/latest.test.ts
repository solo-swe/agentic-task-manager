import { describe, expect, it } from 'vitest';
import { latest } from '../../src/lib/latest';

describe('latest (ATM-675)', () => {
  it('an ask is current until a newer one is made; the newer stays current', () => {
    const loads = latest();
    const first = loads.ask();
    expect(first.current()).toBe(true);
    const second = loads.ask();
    expect([first.current(), second.current()]).toEqual([false, true]);
    // Ending one changes neither
    first.done();
    second.done();
    expect([first.current(), second.current()]).toEqual([false, true]);
  });

  it('with a key, current only while what is shown is still that key', () => {
    let shown = 'default/p1';
    const loads = latest(() => shown);
    const ask = loads.ask();
    expect(ask.current('default/p1')).toBe(true);
    expect(ask.current('work/p1')).toBe(false); // the same project ID in another workspace
    shown = 'default/p2';
    expect(ask.current('default/p1')).toBe(false);
    expect(ask.current()).toBe(true); // no key: the sequence alone
    shown = 'default/p1';
    expect(ask.current('default/p1')).toBe(true);
  });

  it('is busy while an ask is out; done more than once counts once', () => {
    const loads = latest();
    expect(loads.busy).toBe(false);
    const [a, b] = [loads.ask(), loads.ask()];
    expect(loads.busy).toBe(true);
    a.done();
    a.done();
    expect(loads.busy).toBe(true); // b is still out
    b.done();
    expect(loads.busy).toBe(false);
  });
});
