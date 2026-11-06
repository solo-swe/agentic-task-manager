import { fireEvent, render } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import TimeSplitBar from '../../src/lib/progress/TimeSplitBar.svelte';
import type { TimeUse } from '../../src/lib/timeuse';

const H = 3_600_000;
const use = (fields: Partial<TimeUse>): TimeUse => ({
  from: 0,
  to: 51 * H,
  workingMs: 14 * H,
  waitingMs: 6 * H,
  idleMs: 31 * H,
  agentMs: 14 * H + 20 * 60_000,
  maxAgents: 3,
  waiting: [],
  idle: [],
  ...fields,
});

describe('TimeSplitBar (T-245)', () => {
  it('one bar split three ways, the durations under it, and the agent time', () => {
    const { container } = render(TimeSplitBar, { use: use({}) });
    const segments = [...container.querySelectorAll('.status-bar span')] as HTMLElement[];
    expect(segments.map((s) => [s.className, s.style.flexGrow])).toEqual([
      ['time-split-working', `${14 * H}`],
      ['time-split-waiting', `${6 * H}`],
      ['time-split-idle', `${31 * H}`],
    ]);
    expect(segments[0]).toHaveAttribute('title', 'Agents working: 14 h (27%)');
    expect([...container.querySelectorAll('.status-legend li')].map((li) => li.textContent)).toEqual([
      'Agents working14 h',
      'Waiting on you6 h',
      'Idle31 h',
    ]);
    expect(container.querySelector('.time-split-agents')).toHaveTextContent(
      '14 h 20 m of agent time, up to 3 agents at once',
    );
  });

  it('a part with no time has no segment; one agent at a time says nothing about parallelism', () => {
    const { container } = render(TimeSplitBar, { use: use({ waitingMs: 0, maxAgents: 1 }) });
    expect(container.querySelectorAll('.status-bar span')).toHaveLength(2);
    expect(container.querySelector('[data-part="waiting"]')).toHaveTextContent('Waiting on you0 m');
    expect(container.querySelector('.time-split-agents')).toHaveTextContent(/^14 h 20 m of agent time$/);
  });

  it('the ⓘ explains the three parts', async () => {
    const { container, getByRole } = render(TimeSplitBar, { use: use({}) });
    await fireEvent.click(container.querySelector('.infotip-button')!);
    expect(getByRole('dialog')).toHaveTextContent('Idle includes nights and weekends');
  });
});
