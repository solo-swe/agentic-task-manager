// An image the dashboard shows from its server (a project's logo): in the browser its URL is the image; in the atm app
// there is no server, so the image comes through the app's commands (api/ipc.ts) and shows as a `data:` URL, made
// once per URL. `data:` is what the page's Content-Security-Policy allows for images besides its own origin
// (`img-src 'self' data:`, I13), in atm serve's and the app's alike: no `blob:` is needed.
import { inApp } from './http';
import { ipcFetch } from './ipc';

const made = new Map<string, Promise<string | null>>();

/** A blob as a `data:` URL */
function dataUrl(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(reader.error ?? new Error('unreadable image'));
    reader.readAsDataURL(blob);
  });
}

/** What an <img> loads for `url`: the URL itself, or in the app the image as a `data:` URL (null if it can't be read) */
export function imageSource(url: string): Promise<string | null> {
  if (!inApp) return Promise.resolve(url);
  let source = made.get(url);
  if (!source) {
    source = ipcFetch(url)
      .then(async (resp) => (resp.ok ? dataUrl(await resp.blob()) : null))
      .catch(() => null);
    made.set(url, source);
  }
  return source;
}
