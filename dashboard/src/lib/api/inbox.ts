// A project's inbox (docs/inbox-design.md §6–§7, ATM-762): its items, read as the other routes are (conditionally, by
// `ETag`), and the page's changes, as the user: add, edit, discard with a reason, restore. Injecting is an agent's
// (the triage skill's), never the page's.
import { browserFetch, fetchFile, inboxUrl, type Fetch, type InboxState, type Validators } from './http';
import { send, type ChangeResult } from './review-changes';

/** An item, as atm keeps it (`inbox.json`) */
export interface InboxItem {
  id: string;
  text: string;
  state: 'created' | 'discarded' | 'injected';
  reason?: string;
  tasks?: string[];
  created_at?: string;
  created_by?: string;
  updated_at?: string;
  decided_at?: string;
  decided_by?: string;
}

/** A listing: its items, newest first, and how many there are in each state */
export interface InboxListing {
  items: InboxItem[];
  created: number;
  discarded: number;
  injected: number;
}

export type InboxResult = { unchanged: true } | { unchanged: false; listing: InboxListing; validators: Validators };

const count = (n: unknown) => (typeof n === 'number' && n >= 0 ? n : 0);

/** The items of `state`, of the archive with `archived`; anything unexpected in the answer left out */
export async function loadInbox(
  project: string,
  state: InboxState = 'created',
  archived = false,
  fetchFn: Fetch = browserFetch,
  since?: Validators,
): Promise<InboxResult> {
  const file = await fetchFile(inboxUrl(project, state, archived), fetchFn, since);
  if (file.unchanged) return file;
  const data = (file.data ?? {}) as Partial<Record<keyof InboxListing, unknown>>;
  const items = (Array.isArray(data.items) ? data.items : []).filter(
    (i): i is InboxItem => !!i && typeof i === 'object' && typeof i.id === 'string' && typeof i.text === 'string',
  );
  return {
    unchanged: false,
    listing: {
      items,
      created: count(data.created),
      discarded: count(data.discarded),
      injected: count(data.injected),
    },
    validators: file.validators,
  };
}

const item = (id: string) => `/${encodeURIComponent(id)}`;

/** A new item, as the user */
export function addItem(project: string, text: string): Promise<ChangeResult> {
  return send(project, {
    method: 'POST',
    area: 'inbox',
    resource: '',
    body: { text },
    command: 'add_inbox_item',
    args: { item: { text } },
  });
}

/** An open item's new text */
export function editItem(project: string, id: string, text: string): Promise<ChangeResult> {
  return send(project, {
    method: 'PATCH',
    area: 'inbox',
    resource: item(id),
    body: { text },
    command: 'edit_inbox_item',
    args: { id, edit: { text } },
  });
}

/** An item discarded, with why */
export function discardItem(project: string, id: string, reason: string): Promise<ChangeResult> {
  const decision = { decision: 'discarded', reason };
  return send(project, {
    method: 'POST',
    area: 'inbox',
    resource: `${item(id)}/decisions`,
    body: decision,
    command: 'decide_inbox_item',
    args: { id, decision },
  });
}

/** A discarded item brought back */
export function restoreItem(project: string, id: string): Promise<ChangeResult> {
  const decision = { decision: 'restored' };
  return send(project, {
    method: 'POST',
    area: 'inbox',
    resource: `${item(id)}/decisions`,
    body: decision,
    command: 'decide_inbox_item',
    args: { id, decision },
  });
}
