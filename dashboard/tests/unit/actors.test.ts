// @vitest-environment node
import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { actorsLine, AGENT_PREFIXES, approvedBy, earlierAttempts, isAgent } from '../../src/lib/actors';
import type { Task } from '../../src/lib/types';

const task = (fields: Partial<Task> = {}): Task => ({
  id: 'T-1',
  name: 'x',
  phase: 0,
  type: 'feature',
  description: '',
  status: 'done',
  ...fields,
});

describe('actors', () => {
  it('"claude:" actors are agents; anyone else is a person', () => {
    expect(isAgent('claude:e5ea9db0')).toBe(true);
    // Every harness atm recognizes (ATM-660)
    for (const id of ['codex:abc12345', 'gemini:session', 'opencode:session', 'agent:cursor'])
      expect(isAgent(id)).toBe(true);
    expect(isAgent('Hakan Serce')).toBe(false);
    expect(isAgent('ci-bot')).toBe(false);
    expect(isAgent('codex')).toBe(false);
  });

  it('the prefixes are atm’s own (atm-common’s AGENT_PREFIXES)', () => {
    const rust = readFileSync(new URL('../../../crates/atm-common/src/edits.rs', import.meta.url), 'utf8');
    const listed = /pub const AGENT_PREFIXES: \[&str; \d+\] = \[([^\]]*)\]/.exec(rust)![1];
    expect([...listed.matchAll(/"([^"]+)"/g)].map((m) => m[1])).toEqual([...AGENT_PREFIXES]);
  });

  it('approvedBy: who completed it, when that is not who started it (an older agent field counts)', () => {
    expect(approvedBy(task({ started_by: 'claude:a', completed_by: 'Hakan' }))).toBe('Hakan');
    expect(approvedBy(task({ agent: 'claude:a', completed_by: 'Hakan' }))).toBe('Hakan');
    expect(approvedBy(task({ started_by: 'claude:a', completed_by: 'claude:a' }))).toBeUndefined();
    expect(approvedBy(task({ completed_by: 'Hakan' }))).toBeUndefined();
  });

  it('actorsLine says what the task records', () => {
    expect(actorsLine(task({ created_by: 'Hakan', started_by: 'claude:a', completed_by: 'claude:b' }))).toBe(
      'Created by Hakan · started by claude:a · completed by claude:b',
    );
    expect(actorsLine(task({ agent: 'claude:a' }))).toBe('Started by claude:a');
    expect(actorsLine(task())).toBe('');
  });

  it('earlierAttempts: authors of handoff and failure notes, oldest first, once each, not the current one', () => {
    const t = task({
      started_by: 'claude:c',
      notes: [
        { at: '2026-04-01T10:00:00Z', by: 'claude:a', kind: 'handoff', text: 'half' },
        { at: '2026-04-01T11:00:00Z', by: 'claude:b', kind: 'failure', text: 'tests' },
        { at: '2026-04-01T12:00:00Z', by: 'claude:a', kind: 'failure', text: 'again' },
        { at: '2026-04-01T13:00:00Z', by: 'claude:c', kind: 'handoff', text: 'mine' },
        { at: '2026-04-01T14:00:00Z', by: 'Hakan', kind: 'rejection', text: 'no' },
      ],
    });
    expect(earlierAttempts(t)).toEqual(['claude:a', 'claude:b']);
  });
});
