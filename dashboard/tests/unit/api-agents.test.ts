// lib/api/agents.ts (T-458); tests/unit/agents.test.ts is lib/agents.ts's
import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it } from 'vitest';
import { agentsStatus, connectAgents, connected, type HarnessStatus, type SkillState } from '../../src/lib/api/agents';

const harness = (...states: SkillState[]): HarnessStatus => ({
  harness: 'claude',
  label: 'Claude Code',
  detected: true,
  folder: '~/.claude/skills',
  skills: states.map((state, i) => ({ skill: `atm-${i}`, path: `~/.claude/skills/atm-${i}`, state })),
});

describe('connecting the user’s agents (T-458)', () => {
  afterEach(() => clearMocks());

  it.each<[string, SkillState[], boolean]>([
    ['no skills', [], false],
    ['one linked', ['linked'], true],
    ['one copied', ['copied'], true],
    ['linked and copied', ['linked', 'copied', 'linked'], true],
    ['one missing', ['missing'], false],
    ['one stale among linked', ['linked', 'stale'], false],
    ["one not atm's among copied", ['copied', "not atm's"], false],
    ['one missing among many', ['linked', 'copied', 'missing'], false],
  ])('is connected with %s: %s', (_, states, expected) => {
    expect(connected(harness(...states))).toBe(expected);
  });

  it('asks the app for the status, and connects only the harness named', async () => {
    const calls: [string, unknown][] = [];
    mockIPC((command, args) => {
      calls.push([command, args]);
      return command === 'agents_status' ? [harness('linked')] : { written: [], refused: [] };
    });
    expect(await agentsStatus()).toEqual([harness('linked')]);
    expect(await connectAgents('codex')).toEqual({ written: [], refused: [] });
    expect(calls).toEqual([
      ['agents_status', {}],
      ['connect_agents', { harness: 'codex' }],
    ]);
  });

  it('a refusal of the app’s rejects, for the dialog to show', async () => {
    mockIPC(() => {
      throw { kind: 'refused', message: 'not a harness' };
    });
    await expect(connectAgents('x')).rejects.toEqual({ kind: 'refused', message: 'not a harness' });
    await expect(agentsStatus()).rejects.toEqual({ kind: 'refused', message: 'not a harness' });
  });
});
