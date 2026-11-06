<script lang="ts">
  import { formatMinutes } from '../format';
  import type { TimeHistogram } from '../projectstats';
  import { dashboard } from '../state.svelte';
  import InfoTip from '../ui/InfoTip.svelte';

  // Time per task (docs/project-stats-proposal.md §4.1, T-254): how many tasks took how long, in bins that widen with
  // time; each bar agents alone (solid) under with you (hatched, `interactive` tasks), one hue, told apart by texture.
  // A median marker per kind under the axis; the five slowest tasks under the chart.
  let { histogram }: { histogram: TimeHistogram } = $props();

  const W = 520;
  const H = 96;
  const PAD_L = 26;
  const PAD_R = 4;
  const TOP = 6;
  const plotW = W - PAD_L - PAD_R;
  const bins = $derived(histogram.bins);
  const slot = $derived(plotW / bins.length);
  const maxCount = $derived(Math.max(1, ...bins.map((b) => b.alone.length + b.withYou.length)));
  const y = (count: number) => H - (count / maxCount) * (H - TOP);
  const total = $derived(bins.reduce((n, b) => n + b.alone.length + b.withYou.length, 0));
  // Edges labelled under the axis: enough to read the scale without crowding it
  const TICKS = new Set([0, 10, 30, 60, 120, 240]);
  const tickLabel = (m: number) => (m === 240 ? '4 h+' : m < 60 ? `${m} m` : `${m / 60} h`);
  // Where a median falls: its bin, and how far into it (the open-ended last bin: its middle)
  const xOf = (minutes: number) => {
    const i = Math.max(0, bins.filter((b) => b.from <= minutes).length - 1);
    const b = bins[i];
    const frac = b.to === Infinity ? 0.5 : (minutes - b.from) / (b.to - b.from);
    return PAD_L + (i + frac) * slot;
  };
  const m = (minutes: number | null) => (minutes === null ? '–' : formatMinutes(minutes));
  const binTitle = (b: (typeof bins)[number]) =>
    [
      `${b.label}: ${b.alone.length} alone, ${b.withYou.length} with you`,
      ...[...b.alone, ...b.withYou].slice(0, 5).map((t) => `${t.id} ${t.name} (${formatMinutes(t.minutes)})`),
      ...(b.alone.length + b.withYou.length > 5 ? [`+${b.alone.length + b.withYou.length - 5} more`] : []),
    ].join('\n');
</script>

<div class="time-histogram" id="time-histogram">
  <div class="time-histogram-head">
    <span class="menu-label">Time per task</span>
    <InfoTip label="time per task">
      <p>
        How many tasks took how long, from their start until they asked for a review or were done: {total} tasks, from
        {m(histogram.min)} to {m(histogram.max)}, {m(histogram.average)} on average. The bins widen with time so short and
        long tasks both show.
      </p>
      <p>
        Left out: tasks without a start. A task restarted after its lease expired is measured from the restart, and a
        reopened task by its latest attempt.
      </p>
    </InfoTip>
    <span class="time-histogram-legend">
      <span
        ><span class="hist-swatch alone"></span>agents alone{histogram.medianAlone !== null
          ? ` · median ${m(histogram.medianAlone)}`
          : ''}</span
      >
      <span
        ><span class="hist-swatch with-you"></span>with you{histogram.medianWithYou !== null
          ? ` · median ${m(histogram.medianWithYou)}`
          : ''}</span
      >
    </span>
  </div>
  {#if total}
    <svg viewBox="0 0 {W} {H + 26}" width="100%" role="img" aria-label="Time per task: {total} tasks">
      <defs>
        <pattern id="hist-hatch" width="5" height="5" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
          <rect width="5" height="5" fill="color-mix(in srgb, var(--accent) 18%, transparent)" />
          <line x1="0" y1="0" x2="0" y2="5" stroke="var(--accent)" stroke-width="2" />
        </pattern>
      </defs>
      <line x1={PAD_L} x2={W - PAD_R} y1={TOP} y2={TOP} class="chart-grid" />
      <text x={PAD_L - 6} y={TOP + 3} class="chart-tick" text-anchor="end">{maxCount}</text>
      <text x={PAD_L - 6} y={H} class="chart-tick" text-anchor="end">0</text>
      {#each bins as b, i (b.from)}
        {@const x = PAD_L + i * slot}
        {@const aloneTop = y(b.alone.length)}
        <g class="hist-bin" data-from={b.from}>
          <title>{binTitle(b)}</title>
          <!-- The whole column answers the hover, not only the bar -->
          <rect {x} y={TOP} width={slot} height={H - TOP} fill="transparent" />
          {#if b.alone.length}
            <rect
              class="hist-bar alone"
              x={x + 2}
              y={aloneTop}
              width={slot - 4}
              height={H - aloneTop}
              rx="2"
              fill="var(--accent)"
            />
          {/if}
          {#if b.withYou.length}
            {@const top = y(b.alone.length + b.withYou.length)}
            <rect
              class="hist-bar with-you"
              x={x + 2}
              y={top}
              width={slot - 4}
              height={Math.max(1, aloneTop - top - (b.alone.length ? 2 : 0))}
              rx="2"
              fill="url(#hist-hatch)"
              stroke="var(--accent)"
              stroke-width="1"
            />
          {/if}
        </g>
        {#if TICKS.has(b.from)}
          <text {x} y={H + 24} class="chart-tick" text-anchor={i === 0 ? 'start' : 'middle'}>{tickLabel(b.from)}</text>
        {/if}
      {/each}
      <line x1={PAD_L} x2={W - PAD_R} y1={H} y2={H} class="chart-axis" />
      <!-- Medians under the axis: ▲ agents alone, △ with you -->
      {#if histogram.medianAlone !== null}
        {@const x = xOf(histogram.medianAlone)}
        <path class="hist-median alone" d="M{x},{H + 3} L{x + 5},{H + 11} L{x - 5},{H + 11} Z" fill="var(--accent)"
          ><title>Median, agents alone: {m(histogram.medianAlone)}</title></path
        >
      {/if}
      {#if histogram.medianWithYou !== null}
        {@const x = xOf(histogram.medianWithYou)}
        <path
          class="hist-median with-you"
          d="M{x},{H + 3} L{x + 5},{H + 11} L{x - 5},{H + 11} Z"
          fill="var(--surface)"
          stroke="var(--accent)"
          stroke-width="1.5"><title>Median, with you: {m(histogram.medianWithYou)}</title></path
        >
      {/if}
    </svg>
    <div class="time-histogram-slowest" id="time-histogram-slowest">
      <span class="kpi-label">Slowest</span>
      {#each histogram.slowest as t (t.id)}
        <button class="link-button" data-id={t.id} title={t.name} onclick={() => dashboard.showDetail(t.id)}
          >{t.id} {formatMinutes(t.minutes)}</button
        >
      {/each}
    </div>
  {:else}
    <p class="kpi-label">No finished task has both a start and an end time yet.</p>
  {/if}
</div>
