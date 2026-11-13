import { describe, expect, it } from 'vitest';
import { newTabAddress } from '../../src/lib/new-tab-clicks';
import { wantsNewTab } from '../../src/lib/tab-actions';

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
});
