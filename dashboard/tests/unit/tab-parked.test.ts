import { describe, expect, it } from 'vitest';
import { forgetTab, park, unpark } from '../../src/lib/tab-parked';

// What a tab keeps of its questions left open across a switch (ATM-700)
describe('a tab’s parked questions', () => {
  it('are taken back once, by the tab and key they were kept under', () => {
    park('t1', 'form', { summary: 'Half' });
    expect(unpark('t2', 'form')).toBeUndefined();
    expect(unpark('t1', 'other')).toBeUndefined();
    expect(unpark('t1', 'form')).toEqual({ summary: 'Half' });
    expect(unpark('t1', 'form')).toBeUndefined();
  });

  it('none are kept without a tab (the browser)', () => {
    park(undefined, 'form', 1);
    expect(unpark(undefined, 'form')).toBeUndefined();
  });

  it('go with their tab when it closes', () => {
    park('t3', 'a', 1);
    park('t3', 'b', 2);
    forgetTab('t3');
    expect([unpark('t3', 'a'), unpark('t3', 'b')]).toEqual([undefined, undefined]);
  });
});
