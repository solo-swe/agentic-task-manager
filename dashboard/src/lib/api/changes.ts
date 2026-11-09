// In the atm app the page reacts to changes instead of polling (T-506): the app watches the folders the store reports
// (T-456: `subscribe_workspace`, `subscribe_project`, `Store::watch_folders`) and says on a channel when something in
// them changed; the page then reloads, with what it already has as validators. The browser keeps polling until atm
// serve pushes changes too (T-507).
import { Channel, invoke } from '@tauri-apps/api/core';

/** What the app says changed: the workspace, and the project for a project's subscription */
export interface Changed {
  workspace: string;
  project: string | null;
}

/**
 * Be told when the workspace `workspace` (its list of projects) or, with `project`, that project's data changes:
 * `onChange` for each message. Returns the function that ends both subscriptions; called before the app has answered,
 * it ends them as soon as it has.
 */
export function watchChanges(
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
