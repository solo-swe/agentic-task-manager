import { afterEach, describe, expect, it, vi } from 'vitest';
import { forgetAllUnsent, forgetUnsent, keepsUnsent, unsent, unsentKeys } from '../../src/lib/reviews/unsent';

// What is being written and not sent, kept outside its box (lib/reviews/unsent.ts, ATM-732)

function box(value = '') {
  const el = document.createElement('textarea');
  el.value = value;
  document.body.append(el);
  return el;
}
const settle = () => new Promise((r) => queueMicrotask(() => r(null)));

afterEach(() => {
  forgetAllUnsent();
  document.body.innerHTML = '';
});

describe('keepsUnsent', () => {
  it('keeps the text, the cursor and the focus as they change', () => {
    const el = box();
    keepsUnsent('reply:c1')(el);
    el.focus();
    el.value = 'Half';
    el.dispatchEvent(new Event('input'));
    el.setSelectionRange(2, 3);
    el.dispatchEvent(new Event('select'));
    expect(unsent('reply:c1')).toEqual({ text: 'Half', start: 2, end: 3, focused: true });
  });

  it('nothing is kept for an empty box; emptied, what was kept goes', () => {
    const el = box();
    keepsUnsent('reply:c1')(el);
    el.dispatchEvent(new Event('focus'));
    expect(unsent('reply:c1')).toBeUndefined();
    el.value = 'x';
    el.dispatchEvent(new Event('input'));
    expect(unsent('reply:c1')).toBeDefined();
    el.value = '';
    el.dispatchEvent(new Event('input'));
    expect(unsent('reply:c1')).toBeUndefined();
  });

  it('a box made again with what was kept gets its cursor and, only if it had it, the focus; not focused anew', () => {
    const focus = vi.fn();
    keepsUnsent('reply:c1')(box());
    const first = document.querySelector('textarea')!;
    first.focus();
    first.value = 'Half a reply';
    first.setSelectionRange(4, 4);
    first.dispatchEvent(new Event('input'));
    // Removed while it had the focus: kept as focused
    first.remove();
    const again = box('Half a reply');
    keepsUnsent('reply:c1', focus)(again);
    expect(document.activeElement).toBe(again);
    expect([again.selectionStart, again.selectionEnd]).toEqual([4, 4]);
    expect(focus).not.toHaveBeenCalled();
  });

  it('a blur while the box stays on the page counts: made again, it doesn’t take the focus', async () => {
    const el = box();
    keepsUnsent('comment:a.rs')(el);
    el.focus();
    el.value = 'Later';
    el.dispatchEvent(new Event('input'));
    const other = box();
    other.focus();
    await settle();
    expect(unsent('comment:a.rs')?.focused).toBe(false);
    el.remove();
    const again = box('Later');
    keepsUnsent('comment:a.rs')(again);
    expect(document.activeElement).toBe(other);
  });

  it('a box just opened (nothing kept, or another text) is focused by the focus given, its cleanup called', () => {
    const undo = vi.fn();
    const focus = vi.fn(() => undo);
    const detach = keepsUnsent('reply:c1', focus)(box());
    expect(focus).toHaveBeenCalledTimes(1);
    detach();
    expect(undo).toHaveBeenCalled();
    // Something kept for another text: a box opened anew, not the one remade
    const el = box('kept');
    keepsUnsent('reply:c2')(el);
    el.dispatchEvent(new Event('input'));
    keepsUnsent('reply:c2', focus)(box('other'));
    expect(focus).toHaveBeenCalledTimes(2);
  });

  it('detached, it no longer keeps anything', () => {
    const el = box();
    const detach = keepsUnsent('reply:c1')(el);
    detach();
    el.value = 'after';
    el.dispatchEvent(new Event('input'));
    expect(unsent('reply:c1')).toBeUndefined();
  });
});

describe('forgetting', () => {
  it('one key, or all of them (the review’s page gone)', () => {
    for (const key of ['reply:c1', 'comment:a.rs']) {
      const el = box();
      keepsUnsent(key)(el);
      el.value = key;
      el.dispatchEvent(new Event('input'));
    }
    expect(unsentKeys().sort()).toEqual(['comment:a.rs', 'reply:c1']);
    forgetUnsent('reply:c1');
    expect(unsentKeys()).toEqual(['comment:a.rs']);
    forgetAllUnsent();
    expect(unsentKeys()).toEqual([]);
  });
});
