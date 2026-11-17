// A review's changes from the page (docs/code-review-design.md §3.12, ATM-572): in the browser the REST routes that
// change something, from atm's own page with its session token (the `atm-session` meta tag the server puts in it), as
// JSON; in the app the app's commands of the same names. Either acts as the user; only ever on the user's action.
import { invoke } from '@tauri-apps/api/core';
import { inApp } from './http';
import { apiWorkspace } from './workspace';

/** What a change answered, or why it was refused */
export type ChangeResult = { ok: true; body: Record<string, unknown> } | { ok: false; message: string };

/** The session token the server put in the page, or null (a page atm didn't serve) */
export function sessionToken(): string | null {
  return document.querySelector<HTMLMetaElement>('meta[name="atm-session"]')?.content || null;
}

/** The token: the page's; in development (`pnpm run dev`, whose page is Vite's) the dev server's copy of the API's
 * (build/dev-api.ts SESSION_ROUTE). The build has no such branch. */
async function token(): Promise<string | null> {
  const own = sessionToken();
  if (own || !import.meta.env.DEV) return own;
  try {
    const answer = (await (await fetch('/__dev/atm-session', { cache: 'no-store' })).json()) as { token?: string };
    return answer.token ?? null;
  } catch {
    return null;
  }
}

/** One change: its REST method and resource, and its command with the arguments past workspace and project */
interface Change {
  method: 'POST' | 'PATCH' | 'PUT';
  resource: string;
  body: unknown;
  command: string;
  args: Record<string, unknown>;
}

async function send(project: string, change: Change): Promise<ChangeResult> {
  const workspace = apiWorkspace();
  if (inApp) {
    try {
      const body = await invoke<Record<string, unknown>>(change.command, { workspace, project, ...change.args });
      return { ok: true, body };
    } catch (e) {
      return { ok: false, message: (e as { message?: string }).message ?? String(e) };
    }
  }
  const session = await token();
  if (!session) return { ok: false, message: 'This page wasn’t served by atm, so it can’t change a review.' };
  const url = `/api/v2/workspaces/${encodeURIComponent(workspace)}/projects/${encodeURIComponent(project)}/reviews${change.resource}`;
  try {
    const resp = await fetch(url, {
      method: change.method,
      headers: { 'Content-Type': 'application/json', 'X-Atm-Session': session },
      body: JSON.stringify(change.body),
      cache: 'no-store',
    });
    const body = (await resp.json().catch(() => ({}))) as Record<string, unknown>;
    if (!resp.ok) return { ok: false, message: String(body.error ?? `HTTP ${resp.status}`) };
    return { ok: true, body };
  } catch (e) {
    return { ok: false, message: (e as Error).message };
  }
}

const ref = (review: string) => `/${encodeURIComponent(review)}`;

/** A new chain on lines of a file (or a whole file, or the review) */
export interface NewComment {
  path?: string | null;
  lines?: [number, number] | null;
  side?: 'new' | 'old';
  quote?: string | null;
  round?: number | null;
  body: string;
}

export function addComment(project: string, review: string, comment: NewComment): Promise<ChangeResult> {
  return send(project, {
    method: 'POST',
    resource: `${ref(review)}/comments`,
    body: comment,
    command: 'add_review_comment',
    args: { review, comment },
  });
}

export function replyTo(project: string, review: string, comment: string, body: string): Promise<ChangeResult> {
  return send(project, {
    method: 'POST',
    resource: `${ref(review)}/comments/${encodeURIComponent(comment)}/replies`,
    body: { body },
    command: 'reply_to_review_comment',
    args: { review, comment, body },
  });
}

export function resolveComment(
  project: string,
  review: string,
  comment: string,
  resolved: boolean,
): Promise<ChangeResult> {
  return send(project, {
    method: 'PATCH',
    resource: `${ref(review)}/comments/${encodeURIComponent(comment)}`,
    body: { resolved },
    command: 'update_review_comment',
    args: { review, comment, update: { resolved } },
  });
}

/** A reviewer's decision, with a comment on the whole review */
export interface Decision {
  decision: 'approve' | 'request_changes' | 'comment';
  body?: string;
}

export function submitDecision(project: string, review: string, decision: Decision): Promise<ChangeResult> {
  return send(project, {
    method: 'POST',
    resource: `${ref(review)}/decisions`,
    body: decision,
    command: 'submit_review_decision',
    args: { review, decision },
  });
}
