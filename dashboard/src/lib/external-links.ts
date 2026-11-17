// In the atm app, an external link opens in the user's browser (T-551): the window's webview opens none itself (a
// `target="_blank"` link does nothing there). One listener on the page decides (routes/+layout.svelte), before the
// click reaches the link, and hands the URL to the app (`openUrl`, which the app checks again); links within the page
// (its addresses, `?task=`, `#…`, a document's relative links) stay its own. In the browser links open as they always
// have, in a new tab.
import { dashboardAddress } from './compmd';

const EXTERNAL = /^(https?|mailto):$/;

/** `href` when it is a link the app opens outside: an absolute `https:` or `http:` URL with a host, or a `mailto:`
 * one, that isn't one of the dashboard's own pages; null for anything else (`#…`, a relative link, a page's address,
 * any other scheme) */
export function externalUrl(href: string, page: string = location.href): string | null {
  let url: URL;
  try {
    url = new URL(href.trim());
  } catch {
    return null;
  }
  if (!EXTERNAL.test(url.protocol)) return null;
  if (url.protocol !== 'mailto:' && !url.host) return null;
  if (dashboardAddress(href, page) !== null) return null;
  return url.href;
}

/** A click (a plain one, ⌘ or Ctrl with it, or a middle one) on an external link: its URL given to `open`, and the
 * link kept from opening it; whether it was one. Anything else is left to the page */
export function sendExternalLink(e: MouseEvent, open: (url: string) => unknown): boolean {
  if (e.defaultPrevented || e.button > 1 || !(e.target instanceof Element)) return false;
  const href = e.target.closest('a[href]')?.getAttribute('href');
  const url = href ? externalUrl(href) : null;
  if (!url) return false;
  e.preventDefault();
  void open(url);
  return true;
}
