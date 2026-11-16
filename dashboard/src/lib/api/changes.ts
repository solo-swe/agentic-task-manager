// The page reacts to changes instead of polling. In the atm app (T-506) the app watches the folders the store reports
// (T-456: `subscribe_workspace`, `subscribe_project`, `Store::watch_folders`) and says on a channel when something in
// them changed. In the browser (T-507) atm serve says it on a change stream, server-sent events
// (docs/multi-project-api.md §2.9). Either way the page then reloads, with what it already has as validators.
import { Channel, invoke } from '@tauri-apps/api/core';
import { changesUrl, inApp } from './http';

/** What changed: the workspace, and the project for a project's subscription */
export interface Changed {
  workspace: string;
  project: string | null;
}

/** The first wait before a stream that failed is opened again; it doubles with each failure in a row, up to
 * RECONNECT_MAX_MS */
export const RECONNECT_FIRST_MS = 1_000;
export const RECONNECT_MAX_MS = 30_000;
/** After this many tries in a row that didn't connect, the page also reloads every SLOW_POLL_MS until one does */
export const TRIES_BEFORE_POLLING = 3;
export const SLOW_POLL_MS = 60_000;

/** What the stream needs of an `EventSource` (tests give their own) */
export interface ChangeSource {
  onopen: ((event: Event) => void) | null;
  onerror: ((event: Event) => void) | null;
  addEventListener(type: 'changed', listener: (event: MessageEvent<string>) => void): void;
  close(): void;
}

/** Opens a change stream at a URL: the browser's `EventSource` */
export type OpenSource = (url: string) => ChangeSource;

const openEventSource: OpenSource = (url) => new EventSource(url) as unknown as ChangeSource;

/**
 * Be told when the workspace `workspace` (its list of projects, and its projects) or, with `project`, that project's
 * data changes: `onChange` for each message. Returns the function that ends the watch. In the app, a subscription
 * for each; in the browser, the workspace's stream alone, which covers its projects: a browser holds only a few
 * connections to a server across all its tabs (six over HTTP/1), so each tab holds one.
 */
export function watchChanges(
  workspace: string,
  project: string | null,
  onChange: (changed: Changed) => void,
): () => void {
  return inApp
    ? subscribeInApp(workspace, project, onChange)
    : streamChanges(changesUrl(workspace), { workspace, project: null }, onChange);
}

/**
 * Follow the change stream at `url`: `onChange` for each `changed` event. A stream that fails is opened again after
 * a wait that doubles with each failure in a row (from RECONNECT_FIRST_MS to RECONNECT_MAX_MS), and once open again
 * it tells `onChange` (`changed`), since what changed meanwhile wasn't told. While it can't connect at all
 * (TRIES_BEFORE_POLLING tries in a row), the page reloads every SLOW_POLL_MS too, until it connects. Returns the
 * function that ends it all.
 */
export function streamChanges(
  url: string,
  changed: Changed,
  onChange: (changed: Changed) => void,
  open: OpenSource = openEventSource,
): () => void {
  let ended = false;
  let failures = 0;
  let source: ChangeSource | null = null;
  let retry: ReturnType<typeof setTimeout> | undefined;
  let poll: ReturnType<typeof setInterval> | undefined;
  const stopPolling = () => {
    clearInterval(poll);
    poll = undefined;
  };
  const connect = () => {
    const current = open(url);
    source = current;
    current.onopen = () => {
      if (ended) return;
      if (failures > 0) onChange(changed);
      failures = 0;
      stopPolling();
    };
    current.addEventListener('changed', (event) => {
      if (!ended) onChange(parseChanged(event.data) ?? changed);
    });
    current.onerror = () => {
      // Closed here, not left to the browser's own reconnecting, which has no backoff and gives up on an HTTP error
      current.close();
      if (ended) return;
      failures += 1;
      if (failures >= TRIES_BEFORE_POLLING && poll === undefined)
        poll = setInterval(() => onChange(changed), SLOW_POLL_MS);
      retry = setTimeout(connect, Math.min(RECONNECT_MAX_MS, RECONNECT_FIRST_MS * 2 ** (failures - 1)));
    };
  };
  connect();
  return () => {
    ended = true;
    clearTimeout(retry);
    stopPolling();
    source?.close();
  };
}

/** An event's data, `{workspace, project}`, or null when it isn't that */
function parseChanged(data: string): Changed | null {
  try {
    const value = JSON.parse(data) as Partial<Changed> | null;
    return typeof value?.workspace === 'string' && (typeof value.project === 'string' || value.project === null)
      ? { workspace: value.workspace, project: value.project }
      : null;
  } catch {
    return null;
  }
}

/** The app's subscriptions to `workspace` and `project`; called before the app has answered, the function returned
 * ends them as soon as it has */
export function subscribeInApp(
  workspace: string,
  project: string | null,
  onChange: (changed: Changed) => void,
): () => void {
  let ended = false;
  const ids: number[] = [];
  const subscribe = async (command: string, args: Record<string, unknown>) => {
    const channel = new Channel<Changed>();
    channel.onmessage = (changed) => {
      if (!ended) onChange(changed);
    };
    const id = await invoke<number>(command, { ...args, channel });
    if (ended) void unsubscribe(id);
    else ids.push(id);
  };
  const failed = (e: unknown) => console.error('Watching for changes failed:', e);
  void subscribe('subscribe_workspace', { workspace }).catch(failed);
  if (project) void subscribe('subscribe_project', { workspace, project }).catch(failed);
  return () => {
    ended = true;
    for (const id of ids.splice(0)) void unsubscribe(id);
  };
}

function unsubscribe(id: number): Promise<void> {
  return invoke('unsubscribe', { id }).then(
    () => undefined,
    (e: unknown) => console.error('Unsubscribing failed:', e),
  );
}
