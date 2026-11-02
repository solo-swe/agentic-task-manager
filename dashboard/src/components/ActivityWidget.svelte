<script lang="ts">
  import {
    ACTIVITY_PRESET_LABELS,
    ACTIVITY_PRESETS,
    activitySeries,
    ongoing,
    periodPhrase,
    PRESET_BUCKET_MS,
    rangeLabel,
    rangeWindow,
    sparklineTitle,
    STEPPED_PRESETS,
    type ActivityPreset,
    type ActivityRange,
  } from '../lib/insights';
  import type { ClaimMap } from '../lib/claims';
  import { dashboard } from '../lib/state.svelte';
  import { timeUse } from '../lib/timeuse';
  import type { Task } from '../lib/types';
  import { DEFAULT_VOCABULARY } from '../lib/vocab';
  import ActivityDeepDive from './ActivityDeepDive.svelte';
  import Icon from './Icon.svelte';
  import InfoTip from './InfoTip.svelte';

  // A project's Activity by default. The overview's (T-328) gives every project's tasks and claims (`source`, with
  // the default vocabulary's statuses), its own period (`view`), how to open a task (`onopen`) and a line per project
  // (`byProject`, for the chosen period)
  let {
    source = undefined,
    view = dashboard,
    onopen = undefined,
    byProject = undefined,
  }: {
    source?: { tasks: Task[]; claims: ClaimMap };
    view?: { activityRange: ActivityRange };
    onopen?: (id: string) => void;
    byProject?: (period: { start: number; end: number } | null) => { label: string; count: number; open: () => void }[];
  } = $props();
  const tasks = $derived(source ? source.tasks : dashboard.history);
  const vocab = $derived(source ? DEFAULT_VOCABULARY : dashboard.vocab);
  const claims = $derived(source ? source.claims : dashboard.claims);

  // Compact: completions per period (bars) with their rolling average (line, same scale). Click: the deep dive.
  // Archived completions count too once the archive is loaded (dashboard.history)
  const series = $derived(activitySeries(tasks, null, dashboard.loadedAt, vocab));
  let open = $state(false);

  // The panel shows the chosen period (saved with the view); the pill always shows all time
  const range = $derived(view.activityRange);
  const period = $derived(rangeWindow(range, dashboard.loadedAt));
  // Each stepped period in its natural bucket: a week per day, a day per hour (PRESET_BUCKET_MS)
  const detail = $derived(
    range.preset === 'all'
      ? series
      : activitySeries(tasks, period, dashboard.loadedAt, vocab, PRESET_BUCKET_MS[range.preset]),
  );
  const phrase = $derived(periodPhrase(range, dashboard.loadedAt));
  // Where the time went in the period (timeuse.ts, shared with the Progress tab), and who works now: from the live
  // claims, only for a period that runs up to now (T-246)
  const use = $derived(open ? timeUse(tasks, period, dashboard.loadedAt, vocab) : null);
  const workingNow = $derived(
    !period || period.end >= dashboard.loadedAt ? new Set([...claims.values()].map((c) => c.agent)).size : null,
  );
  const projects = $derived(open && byProject ? byProject(period) : []);

  // A period reaching back before tasks.json's oldest completion needs the archive (D-5.2); a project's only
  const oldest = $derived(
    Math.min(...dashboard.tasks.map((t) => Date.parse(t.completed_at ?? '')).filter((t) => !Number.isNaN(t))),
  );
  $effect(() => {
    if (!source && open && dashboard.archivedCount > 0 && (!period || period.start < oldest)) dashboard.wantArchive();
  });

  // <input type="datetime-local"> works in local time without seconds
  const toInput = (t: number) => {
    const d = new Date(t - new Date(t).getTimezoneOffset() * 60_000);
    return d.toISOString().slice(0, 16);
  };
  const fromInput = (v: string) => (v ? new Date(v).getTime() : NaN);

  const stepped = $derived(STEPPED_PRESETS.includes(range.preset));
  const offset = $derived(range.offset ?? 0);
  // ‹ › move a whole hour/day/week/month back or forward, never past now
  const stepPeriod = (by: number) => (view.activityRange = { preset: range.preset, offset: Math.min(0, offset + by) });

  function choose(preset: ActivityPreset) {
    if (preset !== 'custom') return (view.activityRange = { preset });
    // Start a custom range from what is shown now
    const shown =
      period ?? (detail?.buckets.length ? { start: detail.buckets[0].start, end: dashboard.loadedAt } : null);
    const end = shown?.end ?? dashboard.loadedAt;
    view.activityRange = { preset, from: shown?.start ?? end - 24 * 3_600_000, to: end };
  }

  function setCustom(which: 'from' | 'to', value: string) {
    const t = fromInput(value);
    if (Number.isNaN(t) || range.preset !== 'custom') return;
    const next = { ...range, [which]: t };
    if (next.from! < next.to!) view.activityRange = next;
  }
  let root = $state<HTMLElement>();

  const W = 120;
  const H = 18;
  const max = $derived(series ? Math.max(1, ...series.buckets.map((b) => b.completed.length)) : 1);
  const step = $derived(series ? W / series.buckets.length : W);
  const y = (v: number) => H - (v / max) * (H - 2);
  const avgPath = $derived(
    series
      ? series.buckets.map((b, i) => `${i ? 'L' : 'M'}${(i + 0.5) * step},${y(b.average).toFixed(1)}`).join(' ')
      : '',
  );
</script>

<svelte:window
  onclick={(e) => open && !root?.contains(e.target as Node) && (open = false)}
  onkeydown={(e) => e.key === 'Escape' && (open = false)}
/>

{#if series}
  <div class="widget" bind:this={root}>
    <button
      class="widget-card activity-card"
      id="activity-card"
      aria-expanded={open}
      title={sparklineTitle(series, dashboard.loadedAt)}
      aria-label="Activity: {series.totalCompleted} tasks completed"
      onclick={() => (open = !open)}
    >
      <span class="widget-body">
        <svg class="sparkline" width={W} height={H} viewBox="0 0 {W} {H}" aria-hidden="true">
          {#each series.buckets as b, i (b.start)}
            {#if b.completed.length}
              <!-- The current period, not over yet, lighter: it would otherwise look like a drop -->
              <rect
                class:ongoing={ongoing(b, dashboard.loadedAt)}
                x={i * step + 1}
                y={y(b.completed.length)}
                width={Math.max(1, step - 2)}
                height={H - y(b.completed.length)}
                rx="1.5"
                fill="var(--series-remaining)"
              />
            {/if}
          {/each}
          <path d={avgPath} fill="none" stroke="var(--series-scope)" stroke-width="1.5" stroke-linejoin="round" />
        </svg>
      </span>
      <Icon name="chevron-down" size={14} />
    </button>
    {#if open}
      <div class="insights-panel wide" id="activity-panel" role="dialog" aria-label="Activity details">
        <div class="panel-heading activity-heading">
          <!-- Just "Activity" (T-263): the period label beside it names the period, and each chart heading its bar
               size; "per {unitLabel}" here read as the period ("per 3 hours" on All time) -->
          <span id="activity-title"
            >Activity
            <InfoTip label="activity">
              <p>
                {#if source}
                  Every project shown, together (their archives left out), with each project's count below. All times
                  are for the chosen period.
                {:else}
                  The whole project, archived tasks included: the search and filters don't apply here. All times are for
                  the chosen period.
                {/if}
              </p>
              <p>
                <strong>Agents working</strong>: at least one agent at work on a task, from its start until it asked for
                a review or was done. <strong>Agent time</strong> adds up agents working in parallel.
              </p>
              <p>
                <strong>Waiting on you</strong>: nobody working while a task waited for your review or answer.
                <strong>Idle</strong>: nobody working and nothing waiting, for 10 minutes or more, nights included.
              </p>
              <p>
                <strong>Time per task</strong>: from a task's start until it asked for a review, or was done; tasks
                labelled <code>interactive</code> are counted apart, as <em>with you</em>.
              </p>
            </InfoTip></span
          >
          <span class="activity-period">
            {#if stepped}
              <button
                class="period-step"
                aria-label="Earlier"
                title="Earlier"
                id="activity-prev"
                onclick={() => stepPeriod(-1)}><Icon name="chevron-left" size={14} /></button
              >
            {/if}
            <span class="period-label" id="activity-period" aria-live="polite"
              >{rangeLabel(range, dashboard.loadedAt)}</span
            >
            {#if stepped}
              <button
                class="period-step"
                aria-label="Later"
                title="Later"
                id="activity-next"
                disabled={offset >= 0}
                onclick={() => stepPeriod(1)}><Icon name="chevron-right" size={14} /></button
              >
            {/if}
          </span>
          <span class="activity-range" role="group" aria-label="Period">
            {#each ACTIVITY_PRESETS as preset (preset)}
              <button
                class:active={range.preset === preset}
                aria-pressed={range.preset === preset}
                data-range={preset}
                onclick={() => choose(preset)}>{ACTIVITY_PRESET_LABELS[preset]}</button
              >
            {/each}
          </span>
        </div>
        {#if range.preset === 'custom' && range.from !== undefined && range.to !== undefined}
          <div class="activity-custom">
            <label
              >From <input
                type="datetime-local"
                id="activity-from"
                value={toInput(range.from)}
                onchange={(e) => setCustom('from', e.currentTarget.value)}
              /></label
            >
            <label
              >To <input
                type="datetime-local"
                id="activity-to"
                value={toInput(range.to)}
                onchange={(e) => setCustom('to', e.currentTarget.value)}
              /></label
            >
          </div>
        {/if}
        {#if detail}
          <ActivityDeepDive series={detail} period={phrase} {use} {workingNow} {onopen} />
        {/if}
        {#if projects.length}
          <!-- Each project's part (T-328): done in the period; a project opens on its own page -->
          <div class="activity-projects" id="activity-projects">
            <span class="activity-projects-title">Done {phrase}, by project</span>
            {#each projects as p (p.label)}
              <button class="activity-project" onclick={p.open}
                ><span class="activity-project-name">{p.label}</span><span class="activity-project-count"
                  >{p.count}</span
                ></button
              >
            {/each}
          </div>
        {/if}
      </div>
    {/if}
  </div>
{/if}
