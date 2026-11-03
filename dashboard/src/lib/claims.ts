// Live claims (which agent works on which task), from the gitignored claims.json snapshot that `atm` writes next to
// tasks.json, served as p/<id>/claims.json. The file is optional: a project without one simply has no claims.
import { formatMinutes } from './format';
import { browserFetch, projectFileUrl, type Fetch } from './api/http';

export interface Claim {
  task: string;
  agent: string;
  branch: string | null;
  worktree?: string;
  claimed_at: string;
  lease_expires_at: string;
}

export type ClaimMap = ReadonlyMap<string, Claim>;

const isClaim = (c: unknown): c is Claim =>
  !!c &&
  typeof c === 'object' &&
  typeof (c as Claim).task === 'string' &&
  typeof (c as Claim).agent === 'string' &&
  !Number.isNaN(Date.parse((c as Claim).claimed_at)) &&
  !Number.isNaN(Date.parse((c as Claim).lease_expires_at));

/** Live claims by task ID. Malformed entries and expired leases are left out. */
export function parseClaims(data: unknown, now: number): Map<string, Claim> {
  const list = (data as { claims?: unknown })?.claims;
  const claims = new Map<string, Claim>();
  if (!Array.isArray(list)) return claims;
  for (const c of list) if (isClaim(c) && Date.parse(c.lease_expires_at) > now) claims.set(c.task, c);
  return claims;
}

/** Fetch a project's claims.json; any failure (missing file, bad JSON) means no claims rather than an error. */
export async function loadClaims(
  project: string,
  fetchFn: Fetch = browserFetch,
  now: () => number = Date.now,
): Promise<Map<string, Claim>> {
  try {
    const resp = await fetchFn(projectFileUrl(project, 'claims.json') + '?' + now());
    if (!resp.ok) return new Map();
    return parseClaims(await resp.json(), now());
  } catch {
    return new Map();
  }
}

/** "claude:e5ea9db0" stays; "user@host:/path/to/worktree" becomes "user@host". */
export function shortAgent(agent: string): string {
  const at = agent.indexOf('@');
  const colon = agent.indexOf(':', at + 1);
  return at > 0 && colon > at ? agent.slice(0, colon) : agent;
}

export function formatAgo(iso: string, now: number): string {
  const minutes = (now - Date.parse(iso)) / 60_000;
  return minutes < 1 ? 'just now' : `${formatMinutes(minutes)} ago`;
}

/** One line for a card: "claude:e5ea9db0 · dev/x · 12 m ago". */
export function claimSummary(claim: Claim, now: number): string {
  return [shortAgent(claim.agent), claim.branch, formatAgo(claim.claimed_at, now)].filter(Boolean).join(' · ');
}
