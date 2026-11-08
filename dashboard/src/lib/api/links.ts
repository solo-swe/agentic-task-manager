// The atm app's deep links (T-459): the app turns an `atm://…` link into one of the dashboard's addresses
// (app/src-tauri/src/links.rs) and sends it on a channel; the page goes there if, and only if, it is one of its own
// pages. An address is only text for the router: nothing is fetched or run because of it.
import { Channel, invoke } from '@tauri-apps/api/core';
import { isPagePath } from '../address';

/** `address` when it is one of the dashboard's pages (a path from the root, with its query), else null */
export function linkTarget(address: unknown): string | null {
  if (typeof address !== 'string' || !address.startsWith('/') || address.startsWith('//')) return null;
  const path = address.split(/[?#]/, 1)[0];
  return isPagePath(path) && !address.includes('#') ? address : null;
}

/** Listen for the app's links, `go` to each (the one it was started with first, if any) */
export async function listenForLinks(go: (address: string) => void): Promise<void> {
  const channel = new Channel<string>();
  channel.onmessage = (address) => {
    const target = linkTarget(address);
    if (target) go(target);
  };
  const first = linkTarget(await invoke<string | null>('subscribe_links', { channel }));
  if (first) go(first);
}
