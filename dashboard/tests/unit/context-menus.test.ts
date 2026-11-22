import { describe, expect, it, vi } from 'vitest';
import type { AppTab } from '../../src/lib/app-tabs.svelte';
import { menuItems, menuTargetAt, TAB_ID_PREFIX, type MenuDeps, type MenuTarget } from '../../src/lib/context-menus';

// The app's right-click menus (ATM-699, docs/tabs-and-navigation-design.md §4.5): what a right-click is on, and the
// items each menu has and what they do, on stand-ins for the tabs and the page

/** A page with `html` in its body; the element `selector` finds */
function on(html: string, selector: string): Element {
  document.body.innerHTML = html;
  return document.querySelector(selector)!;
}

const tab = (id: string, address: string): AppTab => ({ id, address, history: { entries: [], at: 0 } }) as AppTab;

function deps(
  tabs: AppTab[] = [tab('t1', '/projects/a/kanban'), tab('t2', '/')],
  history = { back: true, forward: false },
) {
  const done: string[] = [];
  const said =
    (what: string) =>
    (...args: unknown[]) =>
      void done.push([what, ...args.map(String)].join(' '));
  const fake: MenuDeps = {
    tabs: () => tabs,
    go: said('go'),
    openTab: (address?: string, background?: boolean) =>
      done.push(`open-tab ${address ?? '(overview)'} ${!!background}`),
    closeTab: said('close'),
    openWindow: vi.fn(async (address: string) => (done.push(`window ${address}`), true)),
    copy: vi.fn(async (text: string) => void done.push(`copy ${text}`)),
    canGo: (delta) => (delta < 0 ? history.back : history.forward),
    back: said('back'),
    forward: said('forward'),
    reload: said('reload'),
  };
  return { fake, done };
}

const run = async (target: MenuTarget, id: string, d = deps()) => {
  const item = menuItems(target, d.fake).find((i) => i.id === id)!;
  item.run();
  await Promise.resolve();
  await Promise.resolve();
  return d.done;
};

describe('what a right-click is on', () => {
  it.each([
    ['an input', '<input id="x">', '#x'],
    ['a text area', '<textarea id="x"></textarea>', '#x'],
    ['something editable', '<div contenteditable="true"><span id="x">t</span></div>', '#x'],
  ])('%s is text: the system’s menu', (_, html, selector) => {
    expect(menuTargetAt(on(html, selector), '/')).toEqual({ kind: 'text' });
  });

  it('selected text is text, wherever it is', () => {
    expect(menuTargetAt(on('<p id="x">hello</p>', '#x'), '/', 'hello')).toEqual({ kind: 'text' });
    expect(menuTargetAt(on('<p id="x">hello</p>', '#x'), '/', '  ')).toEqual({ kind: 'page' });
  });

  it('a title-bar tab, from anywhere inside it, by its ID', () => {
    const inside = on(`<div role="tab" id="${TAB_ID_PREFIX}t7"><span id="x">Name</span></div>`, '#x');
    expect(menuTargetAt(inside, '/')).toEqual({ kind: 'tab', id: 't7' });
    // Another strip's tab is not the window's
    expect(menuTargetAt(on('<div role="tab" id="review-tab-diffs"><b id="x">D</b></div>', '#x'), '/').kind).toBe(
      'page',
    );
  });

  it('a link, card or row to one of the dashboard’s pages, by the address it opens', () => {
    const card = on('<div class="kanban-card" data-id="ATM-1"><span id="x">t</span></div>', '#x');
    expect(menuTargetAt(card, '/projects/a/kanban')).toEqual({
      kind: 'link',
      address: '/projects/a/kanban?task=ATM-1',
    });
    const link = on('<a id="x" href="/projects/a/reviews/CR-7">CR-7</a>', '#x');
    expect(menuTargetAt(link, '/projects/a/list')).toEqual({ kind: 'link', address: '/projects/a/reviews/CR-7' });
  });

  it('anything else is the page; nothing at all too', () => {
    expect(menuTargetAt(on('<p id="x">hi</p>', '#x'), '/')).toEqual({ kind: 'page' });
    expect(menuTargetAt(null, '/')).toEqual({ kind: 'page' });
  });
});

describe('the menus', () => {
  const link: MenuTarget = { kind: 'link', address: '/projects/a/list?task=ATM-1' };

  it('text has none: the system’s shows', () => {
    expect(menuItems({ kind: 'text' }, deps().fake)).toEqual([]);
  });

  it('a link: Open, Open in New Tab, Open in New Window, Copy Link (atm://), each doing that', async () => {
    expect(menuItems(link, deps().fake).map((i) => i.label)).toEqual([
      'Open',
      'Open in New Tab',
      'Open in New Window',
      'Copy Link',
    ]);
    expect(await run(link, 'open')).toEqual(['go /projects/a/list?task=ATM-1']);
    expect(await run(link, 'open-tab')).toEqual(['open-tab /projects/a/list?task=ATM-1 true']);
    expect(await run(link, 'open-window')).toEqual(['window /projects/a/list?task=ATM-1']);
    expect(await run(link, 'copy-link')).toEqual(['copy atm://projects/a/list?task=ATM-1']);
  });

  it('a tab: New Tab, Duplicate, Move to New Window, Copy Link, Close, Close Other Tabs', async () => {
    const t1: MenuTarget = { kind: 'tab', id: 't1' };
    expect(menuItems(t1, deps().fake).map((i) => i.label)).toEqual([
      'New Tab',
      'Duplicate',
      'Move to New Window',
      'Copy Link',
      'Close',
      'Close Other Tabs',
    ]);
    expect(await run(t1, 'new-tab')).toEqual(['open-tab (overview) false']);
    expect(await run(t1, 'duplicate')).toEqual(['open-tab /projects/a/kanban false']);
    // Moved: opened in its own window, then closed here
    expect(await run(t1, 'to-window')).toEqual(['window /projects/a/kanban', 'close t1']);
    expect(await run(t1, 'copy-link')).toEqual(['copy atm://projects/a/kanban']);
    expect(await run(t1, 'close')).toEqual(['close t1']);
    expect(await run(t1, 'close-others')).toEqual(['close t2']);
  });

  it('a tab that won’t open in a window stays; the only tab has no others to close; a gone tab has no menu', async () => {
    const d = deps();
    d.fake.openWindow = async () => false;
    expect(await run({ kind: 'tab', id: 't1' }, 'to-window', d)).toEqual([]);
    const alone = deps([tab('t1', '/')]);
    expect(menuItems({ kind: 'tab', id: 't1' }, alone.fake).find((i) => i.id === 'close-others')?.disabled).toBe(true);
    expect(menuItems({ kind: 'tab', id: 'gone' }, deps().fake)).toEqual([]);
  });

  it('elsewhere: Back, Forward, Reload, each disabled at its history’s end', async () => {
    const items = menuItems({ kind: 'page' }, deps().fake);
    expect(items.map((i) => [i.label, !!i.disabled])).toEqual([
      ['Back', false],
      ['Forward', true],
      ['Reload', false],
    ]);
    expect(await run({ kind: 'page' }, 'back')).toEqual(['back']);
    expect(await run({ kind: 'page' }, 'reload')).toEqual(['reload']);
  });
});
