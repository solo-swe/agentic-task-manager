<script lang="ts">
  import { formatMinutes } from '../format';
  import type { TimeUse } from '../timeuse';
  import InfoTip from '../ui/InfoTip.svelte';

  // Where the time went (docs/activity-popup-review.md §3.1): one 100% bar, agents working / waiting on you / idle,
  // in the charts' colours (idle as their hatching), the three durations under it and the agent time. Used by the
  // Activity popup and the Progress tab.
  let { use, id }: { use: TimeUse; id?: string } = $props();

  const total = $derived(use.workingMs + use.waitingMs + use.idleMs);
  const parts = $derived(
    [
      { key: 'working', label: 'Agents working', ms: use.workingMs },
      { key: 'waiting', label: 'Waiting on you', ms: use.waitingMs },
      { key: 'idle', label: 'Idle', ms: use.idleMs },
    ].map((p) => ({ ...p, pct: total ? Math.round((p.ms / total) * 100) : 0 })),
  );
  const duration = (msValue: number) => (msValue === 0 ? '0 m' : formatMinutes(msValue / 60_000));
</script>

<div class="time-split" {id}>
  <div class="time-split-head">
    <span class="menu-label">Where the time went</span>
    <InfoTip label="where the time went">
      <p>
        From the first work on: <strong>agents working</strong> while at least one agent works on a task (short pauses
        between tasks included), <strong>waiting on you</strong> while no agent works and a task is in review or blocked
        on you, <strong>idle</strong> for the rest, in stretches of 10 minutes or more.
      </p>
      <p>Idle includes nights and weekends: it says no agent ran, not that one was stuck.</p>
    </InfoTip>
  </div>
  <div class="status-bar" aria-hidden="true">
    {#each parts as p (p.key)}
      {#if p.ms > 0}
        <span class="time-split-{p.key}" style:flex={p.ms} title="{p.label}: {duration(p.ms)} ({p.pct}%)"></span>
      {/if}
    {/each}
  </div>
  <ul class="status-legend">
    {#each parts as p (p.key)}
      <li data-part={p.key}>
        <span class="legend-dot time-split-{p.key}"></span>{p.label}<strong>{duration(p.ms)}</strong>
      </li>
    {/each}
  </ul>
  <div class="time-split-agents">
    {duration(use.agentMs)} of agent time{use.maxAgents > 1 ? `, up to ${use.maxAgents} agents at once` : ''}
  </div>
</div>
