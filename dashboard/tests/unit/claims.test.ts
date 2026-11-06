import { describe, expect, it, vi } from 'vitest';
import { claimSummary, formatAgo, loadClaims, parseClaims, shortAgent, type Claim } from '../../src/lib/claims';

const NOW = Date.parse('2026-05-01T10:30:00Z');
const claim = (task: string, overrides: Partial<Claim> = {}): Claim => ({
  task,
  agent: 'claude:e5ea9db0',
  branch: 'agent-2',
  worktree: '/work/proj-2',
  claimed_at: '2026-05-01T10:18:00Z',
  lease_expires_at: '2026-05-01T10:48:00Z',
  ...overrides,
});

describe('parseClaims', () => {
  it('maps live claims by task', () => {
    const claims = parseClaims({ claims: [claim('T-1'), claim('T-2')] }, NOW);
    expect([...claims.keys()]).toEqual(['T-1', 'T-2']);
    expect(claims.get('T-1')?.branch).toBe('agent-2');
  });

  it('leaves out expired leases and malformed entries', () => {
    const data = {
      claims: [
        claim('T-1', { lease_expires_at: '2026-05-01T10:29:59Z' }),
        claim('T-2', { claimed_at: 'not a date' }),
        { task: 'T-3' },
        null,
        'T-4',
        claim('T-5'),
      ],
    };
    expect([...parseClaims(data, NOW).keys()]).toEqual(['T-5']);
  });

  it('anything but a list of claims means no claims', () => {
    for (const data of [null, undefined, {}, { claims: {} }, [], 'x']) expect(parseClaims(data, NOW).size).toBe(0);
  });
});

describe('loadClaims', () => {
  const json = (body: unknown) => new Response(JSON.stringify(body), { status: 200 });

  it("fetches the project's claims.json with a cache-busting timestamp", async () => {
    const fetchFn = vi.fn().mockResolvedValue(json({ claims: [claim('T-1')] }));
    const claims = await loadClaims('abcd1234', fetchFn, () => NOW);
    expect(fetchFn).toHaveBeenCalledWith('/p/abcd1234/claims.json?' + NOW);
    expect(claims.has('T-1')).toBe(true);
  });

  it('a missing file, invalid JSON or a network error means no claims', async () => {
    const responses = [
      () => Promise.resolve(new Response('not found', { status: 404 })),
      () => Promise.resolve(new Response('{', { status: 200 })),
      () => Promise.reject(new TypeError('offline')),
    ];
    for (const respond of responses) expect((await loadClaims('p1', vi.fn(respond), () => NOW)).size).toBe(0);
  });
});

describe('labels', () => {
  it('shortAgent drops the worktree path of a user@host identity', () => {
    expect(shortAgent('claude:e5ea9db0')).toBe('claude:e5ea9db0');
    // A session's agent in a linked worktree (T-202) keeps its worktree folder: that's what tells it apart
    expect(shortAgent('claude:e5ea9db0@agent-a74a')).toBe('claude:e5ea9db0@agent-a74a');
    expect(shortAgent('hakan@mac.local:/Users/hakan/proj')).toBe('hakan@mac.local');
    expect(shortAgent('ci-runner')).toBe('ci-runner');
  });

  it('formatAgo', () => {
    expect(formatAgo('2026-05-01T10:29:30Z', NOW)).toBe('just now');
    expect(formatAgo('2026-05-01T10:18:00Z', NOW)).toBe('12 m ago');
    expect(formatAgo('2026-05-01T09:10:00Z', NOW)).toBe('1 h 20 m ago');
  });

  it('claimSummary: agent · branch · when', () => {
    expect(claimSummary(claim('T-1'), NOW)).toBe('claude:e5ea9db0 · agent-2 · 12 m ago');
    expect(claimSummary(claim('T-1', { branch: null }), NOW)).toBe('claude:e5ea9db0 · 12 m ago');
  });
});
