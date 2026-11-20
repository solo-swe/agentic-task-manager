import { describe, expect, it } from 'vitest';
import { followInPlace, newTabAddress, plainClick } from '../../src/lib/new-tab-clicks';
import { inBackground, wantsNewTab } from '../../src/lib/tab-actions';

function element(html: string, selector: string): Element {
  const host = document.createElement('div');
  host.innerHTML = html;
  return host.querySelector(selector)!;
}

describe('open in a new tab (T-515)', () => {
  it('a task card or row opens its task, a document link its document; anything else nothing', () => {
    const here = '/projects/abc/kanban';
    expect(newTabAddress(element('<div class="kanban-card" data-id="T-1"><span>x</span></div>', 'span'), here)).toBe(
      '/projects/abc/kanban?task=T-1',
    );
    expect(newTabAddress(element('<div class="task-card" data-id="T-2"></div>', 'div'), here)).toBe(
      '/projects/abc/kanban?task=T-2',
    );
    const doc = newTabAddress(element('<a href="?doc=docs%2Fdesign.md">d</a>', 'a'), here);
    expect(doc).toMatch(/^\/projects\/abc\/kanban\?doc=docs/);
    expect(newTabAddress(element('<a href="https://example.com">x</a>', 'a'), here)).toBeNull();
    expect(newTabAddress(element('<button>x</button>', 'button'), here)).toBeNull();
    expect(newTabAddress(null, here)).toBeNull();
  });

  it('⌘-click, Ctrl-click and a middle click ask for a new tab; a plain click doesn’t', () => {
    expect(wantsNewTab(new MouseEvent('click', { metaKey: true }))).toBe(true);
    expect(wantsNewTab(new MouseEvent('click', { ctrlKey: true }))).toBe(true);
    expect(wantsNewTab(new MouseEvent('auxclick', { button: 1 }))).toBe(true);
    expect(wantsNewTab(new MouseEvent('click'))).toBe(false);
  });

  // ATM-698: every card and row is a real link; a link to any of the dashboard's pages opens in a new tab
  it('a link to one of the dashboard’s pages (a card’s, a review row’s, a reference) is its address; `#…` and other sites aren’t', () => {
    const here = '/projects/abc/kanban';
    expect(newTabAddress(element('<a href="/projects/abc/reviews/CR-7">r</a>', 'a'), here)).toBe(
      '/projects/abc/reviews/CR-7',
    );
    expect(newTabAddress(element('<a href="/projects/abc/list?task=ATM-1"><b>x</b></a>', 'b'), here)).toBe(
      '/projects/abc/list?task=ATM-1',
    );
    expect(newTabAddress(element('<a href="/api/projects">x</a>', 'a'), here)).toBeNull();
    // A task's key in text is the text's to open (CompMD's link handler)
    expect(newTabAddress(element('<a href="#task=ATM-1">x</a>', 'a'), here)).toBeNull();
  });

  it('a plain click opens in place, any other is left alone; ⇧ keeps the new tab forward, else it is in the background', () => {
    let opened = 0;
    const open = () => opened++;
    const plain = new MouseEvent('click', { button: 0, cancelable: true });
    followInPlace(plain, open);
    expect([opened, plain.defaultPrevented]).toEqual([1, true]);
    for (const other of [{ metaKey: true }, { ctrlKey: true }, { shiftKey: true }, { altKey: true }, { button: 1 }]) {
      const e = new MouseEvent('click', { button: 0, cancelable: true, ...other });
      followInPlace(e, open);
      expect([opened, e.defaultPrevented], JSON.stringify(other)).toEqual([1, false]);
    }
    expect(plainClick(new MouseEvent('click'))).toBe(true);
    expect(inBackground(new MouseEvent('click', { metaKey: true }))).toBe(true);
    expect(inBackground(new MouseEvent('click', { metaKey: true, shiftKey: true }))).toBe(false);
  });
});
