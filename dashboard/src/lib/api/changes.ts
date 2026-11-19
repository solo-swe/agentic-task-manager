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

/** The app's subscriptions `commands` (each a command and its arguments), ended together by the function returned;
 * called before the app has answered, it ends them as soon as it has */
function subscribeAll(commands: [string, Record<string, unknown>][], onChange: (changed: Changed) => void): () => void {
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
  for (const [command, args] of commands) void subscribe(command, args).catch(failed);
  return () => {
    ended = true;
    for (const id of ids.splice(0)) void unsubscribe(id);
  };
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
  return subscribeAll(
    [
      ['subscribe_workspace', { workspace }],
      ...(project ? [['subscribe_project', { workspace, project }] as [string, Record<string, unknown>]] : []),
    ],
    onChange,
  );
}

/**
 * Be told when project `project`'s working copy changes as a live review shows it (ATM-607): a file of the work
 * written, not one in `.git/`, atm's task folder or one git ignores. A live review's page asks again on each, instead
 * of every few seconds. Returns the function that ends the subscription.
 */
export function watchWorkingCopy(workspace: string, project: string, onChange: () => void): () => void {
  return subscribeAll([['subscribe_working_copy', { workspace, project }]], () => onChange());
}

function unsubscribe(id: number): Promise<void> {
  return invoke('unsubscribe', { id }).then(
    () => undefined,
    (e: unknown) => console.error('Unsubscribing failed:', e),
  );
}
