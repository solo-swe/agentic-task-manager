import { afterEach, describe, expect, it } from 'vitest';
import { externalUrl, sendExternalLink } from '../../src/lib/external-links';

/** Click (or `type`) what `selector` finds in `html`, with the page's listener on the document as the layout has it;
 * what it sent to the app, and whether the page's own handling was left the click */
function click(html: string, selector: string, init: MouseEventInit = {}, type = 'click') {
  const host = document.createElement('div');
  host.innerHTML = html;
  document.body.append(host);
  const sent: string[] = [];
  const listener = (e: Event) => void sendExternalLink(e as MouseEvent, (url) => sent.push(url));
  // What reaches the page after: whether the link was left to it (and then no navigation in jsdom)
  let kept = false;
  const after = (e: Event) => {
    kept = !e.defaultPrevented;
    e.preventDefault();
  };
  document.addEventListener(type, listener, { capture: true });
  document.addEventListener(type, after);
  host.querySelector(selector)!.dispatchEvent(new MouseEvent(type, { bubbles: true, cancelable: true, ...init }));
  document.removeEventListener(type, listener, { capture: true });
  document.removeEventListener(type, after);
  return { sent, kept };
}

describe('external links open in the user’s browser in the app (T-551)', () => {
  afterEach(() => document.body.replaceChildren());

  it('web and mail links are external; the page’s own, relative and other links aren’t', () => {
    const page = 'tauri://localhost/projects/abc/kanban';
    for (const href of ['https://github.com/solo-swe/x/commit/abc', 'http://example.com/', 'mailto:me@example.com'])
      expect(externalUrl(href, page), href).toBe(href);
    for (const href of [
      '#top',
      '?task=T-1',
      '?doc=docs%2Fa.md',
      '/projects/abc/list',
      'docs/a.md',
      'javascript:alert(1)',
      'file:///etc/passwd',
      'atm://projects/abc',
      'vscode://file/x',
      '',
    ])
      expect(externalUrl(href, page), href).toBeNull();
    // Under atm serve, a link to one of the dashboard's own pages stays in the page
    expect(externalUrl('http://127.0.0.1:7777/projects/abc/list', 'http://127.0.0.1:7777/')).toBeNull();
  });

  it('a click on an external link sends it to the app and keeps it from the page', () => {
    const about = click(
      '<a href="https://github.com/solo-swe/agentic-task-manager" target="_blank"><span>source</span></a>',
      'span',
    );
    expect(about).toEqual({ sent: ['https://github.com/solo-swe/agentic-task-manager'], kept: false });
    // ⌘-click and a middle click too: there is no tab to open it in
    expect(click('<a href="https://example.com/">x</a>', 'a', { metaKey: true }).sent).toEqual([
      'https://example.com/',
    ]);
    expect(click('<a href="https://example.com/">x</a>', 'a', { button: 1 }, 'auxclick').sent).toEqual([
      'https://example.com/',
    ]);
  });

  it('internal links, other clicks and links the page already handled stay in the page', () => {
    for (const href of ['?task=T-1', '#heading', '/projects/abc/list', '?doc=docs%2Fa.md'])
      expect(click(`<a href="${href}">x</a>`, 'a'), href).toEqual({ sent: [], kept: true });
    expect(click('<button>x</button>', 'button')).toEqual({ sent: [], kept: true });
    // A right click is the context menu's
    expect(click('<a href="https://example.com/">x</a>', 'a', { button: 2 }, 'auxclick')).toEqual({
      sent: [],
      kept: true,
    });
    const handled = new MouseEvent('click', { bubbles: true, cancelable: true });
    handled.preventDefault();
    const a = document.createElement('a');
    a.href = 'https://example.com/';
    document.body.append(a);
    Object.defineProperty(handled, 'target', { value: a });
    expect(sendExternalLink(handled, () => expect.fail('sent'))).toBe(false);
  });
});
