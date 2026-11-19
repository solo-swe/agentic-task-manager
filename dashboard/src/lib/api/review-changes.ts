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
  method: 'POST' | 'PATCH' | 'PUT' | 'DELETE';
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

/** A poke (ATM-611): the user's nudge on the whole review, which its agent hears and answers; an optional message */
export function pokeReview(project: string, review: string, body?: string): Promise<ChangeResult> {
  const message = body?.trim() || undefined;
  return send(project, {
    method: 'POST',
    resource: `${ref(review)}/pokes`,
    body: message ? { body: message } : {},
    command: 'poke_review',
    args: { review, body: message ?? null },
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

/** One's own comment's text changed (ATM-638; atm refuses another's) */
export function editComment(project: string, review: string, comment: string, body: string): Promise<ChangeResult> {
  return send(project, {
    method: 'PATCH',
    resource: `${ref(review)}/comments/${encodeURIComponent(comment)}`,
    body: { body },
    command: 'update_review_comment',
    args: { review, comment, update: { body } },
  });
}

/** A review opened from the page: a live one follows the working copy against `base` (HEAD by default; ATM-576) */
export interface Opening {
  title: string;
  summary?: string;
  tasks?: string[];
  start: 'live';
  change: { base?: string };
}

/** The review opened: its header (`reference`, CR-7, among it) */
export function openReview(project: string, opening: Opening): Promise<ChangeResult> {
  return send(project, { method: 'POST', resource: '', body: opening, command: 'create_review', args: { opening } });
}

/** A review closed, ended without approval, with why */
export function closeReview(project: string, review: string, reason: string): Promise<ChangeResult> {
  const closing = { status: 'closed', reason };
  return send(project, {
    method: 'PATCH',
    resource: ref(review),
    body: closing,
    command: 'close_review',
    args: { review, closing },
  });
}

/** A draft of the user's (ATM-612): a comment (placed as a new comment is) or a reply, only they see until it is sent
 * with their decision (or *Publish comments*) */
export type Draft = { id: string; at: string; body: string } & (
  | {
      kind: 'comment';
      path: string | null;
      lines: [number, number] | null;
      side: 'new' | 'old';
      quote: string | null;
      round: number | null;
    }
  | { kind: 'reply'; to: string }
);

/** A new draft: a comment's place and text, or the comment it answers and its text */
export type NewDraft = NewComment | { reply_to: string; body: string };

export function addDraft(project: string, review: string, draft: NewDraft): Promise<ChangeResult> {
  return send(project, {
    method: 'POST',
    resource: `${ref(review)}/drafts`,
    body: draft,
    command: 'add_review_draft',
    args: { review, draft },
  });
}

export function updateDraft(project: string, review: string, draft: string, body: string): Promise<ChangeResult> {
  return send(project, {
    method: 'PATCH',
    resource: `${ref(review)}/drafts/${encodeURIComponent(draft)}`,
    body: { body },
    command: 'update_review_draft',
    args: { review, draft, body },
  });
}

export function removeDraft(project: string, review: string, draft: string): Promise<ChangeResult> {
  return send(project, {
    method: 'DELETE',
    resource: `${ref(review)}/drafts/${encodeURIComponent(draft)}`,
    body: {},
    command: 'remove_review_draft',
    args: { review, draft },
  });
}

/** The user's drafts on a review, the oldest first: a read, as the user (the app's command, or the REST GET); none
 * when they can't be had (a page atm didn't serve), with why */
export async function listDrafts(
  project: string,
  review: string,
): Promise<{ ok: true; drafts: Draft[] } | { ok: false; message: string }> {
  const workspace = apiWorkspace();
  try {
    const body = inApp
      ? await invoke<{ drafts: Draft[] }>('list_review_drafts', { workspace, project, review })
      : await (async () => {
          const url = `/api/v2/workspaces/${encodeURIComponent(workspace)}/projects/${encodeURIComponent(project)}/reviews${ref(review)}/drafts`;
          const resp = await fetch(url, { cache: 'no-store' });
          const json = (await resp.json().catch(() => ({}))) as { drafts?: Draft[]; error?: string };
          if (!resp.ok) throw new Error(json.error ?? `HTTP ${resp.status}`);
          return { drafts: json.drafts ?? [] };
        })();
    return { ok: true, drafts: body.drafts };
  } catch (e) {
    return { ok: false, message: (e as { message?: string }).message ?? String(e) };
  }
}

/** A reviewer's decision, with a comment on the whole review; the user's drafts go with it (a comment with no words
 * of its own only sends them: *Publish comments*) */
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
