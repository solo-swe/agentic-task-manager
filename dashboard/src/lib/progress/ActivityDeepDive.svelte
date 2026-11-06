<script lang="ts">
  import { shortAgent } from '../claims';
  import { formatMinutes } from '../format';
  import { cycleScaleMax, median, type ActivitySeries } from '../insights';
  import { dashboard } from '../state.svelte';
  import { typeStyle } from '../theme';
  import { formatDate, formatDateTime } from '../timeline';
  import type { TimeUseWithSplit } from '../timeuse';
  import InfoTip from '../ui/InfoTip.svelte';
  import TimeSplitBar from './TimeSplitBar.svelte';

  // Three charts on one time axis, each answering one question: what got done (completions by type), who was
  // working (agents busy, stacked per agent), and how long tasks took. One hover reads a period across all three.
  // `period` names the selected period for the headings: "this week", "today", "over all time" (periodPhrase).
  // `use` is where the period's time went; `workingNow` the agents working now, null for a period in the past.
  // `onopen` opens a task (the overview's opens it in its project, T-328); a project's shows its details
  let {
    series,
    period,
    use = null,
    workingNow = null,
    onopen = undefined,
    group = undefined,
  }: {
    series: ActivitySeries;
    period: string;
    use?: TimeUseWithSplit | null;
    workingNow?: number | null;
    onopen?: (id: string) => void;
    /** The overview's Activity by project (T-328 review): the series' types are projects, its workers "project ·
     * agent"; completions, who was working and the dots are stacked and coloured by project */
    group?: { color: (project: string) => string; of: (worker: string) => string; order: string[] };
  } = $props();
  const openTask = (id: string) => (onopen ? onopen(id) : dashboard.showDetail(id));
  const colorOf = (type: string) => (group ? group.color(type) : typeStyle(type).color);
  // Who was working: by kind (agents, people), or by project
  const busyIn = (x: (typeof series.buckets)[number], key: string) =>
    group
      ? Object.entries(x.agentBusy).reduce((n, [w, v]) => n + (group.of(w) === key ? v : 0), 0)
      : x.byKind[key as 'agents' | 'people'];

  const W = 640;
  const PAD_L = 34;
  const PAD_R = 8;
  const H = 72;
  const plotW = W - PAD_L - PAD_R;
  const b = $derived(series.buckets);
  const n = $derived(b.length);
  const slot = $derived(plotW / n);
  const xt = (t: number) => PAD_L + ((t - b[0].start) / (b[n - 1].end - b[0].start)) * plotW;
  const scale = (max: number) => (v: number) => H - 2 - (v / Math.max(1e-9, max)) * (H - 8);

  // Agents are shades of the accent colour, so they never look like a task type (types own the other hues). The
  // shade follows the (sorted) agent, not its rank.
  const AGENT_SHADES = [100, 62, 32, 82, 47];
  const agentColor = (agent: string) =>
    agent
      ? `color-mix(in srgb, var(--accent) ${AGENT_SHADES[series.agents.indexOf(agent) % AGENT_SHADES.length]}%, var(--surface))`
      : 'var(--text-muted)';
  const agentName = (agent: string) => (agent ? shortAgent(agent) : 'Unattributed');

  const maxDone = $derived(Math.max(1, ...b.map((x) => x.completed.length)));
  const yDone = $derived(scale(maxDone));
  // Who was working, by kind (T-247): agents in the accent, people in a lighter shade of it; per session in the tooltip
  const KINDS = [
    { key: 'agents', label: 'Agents', color: 'var(--accent)' },
    { key: 'people', label: 'People', color: 'color-mix(in srgb, var(--accent) 45%, var(--surface))' },
  ] as const;
  const kinds = $derived(
    (group
      ? group.order.map((p) => ({ key: p, label: p, color: group.color(p) }))
      : KINDS.map((k) => ({ key: k.key as string, label: k.label as string, color: k.color as string }))
    ).filter((k) => b.some((x) => busyIn(x, k.key) > 0)),
  );
  const busyTotal = (x: (typeof b)[number]) => x.byKind.agents + x.byKind.people;
  // Idle and waiting on you as the Where the time went bar has them (timeuse.ts); idle alone without it
  const idle = $derived(use?.idle ?? series.idle);
  const maxBusy = $derived(Math.max(1, Math.ceil(Math.max(0, ...b.map(busyTotal)))));
  const yBusy = $derived(scale(maxBusy));
  // Capped at the 90th percentile: slower tasks are triangles on the top edge (T-248)
  const maxCycle = $derived(cycleScaleMax(series.cycleTimes.map((c) => c.minutes)));
  const yCycle = $derived(scale(maxCycle));

  const short = $derived(series.bucketMs < 86_400_000);
  const when = (t: number) => (short ? formatDateTime(t) : formatDate(t));
  // Time per task, two medians: tasks agents do alone, and `interactive` ones that include working with you
  const perTask = $derived(
    [
      { key: 'alone', label: 'agents alone', withYou: false },
      { key: 'with-you', label: 'with you', withYou: true },
    ].map((side) => {
      const minutes = series.cycleTimes.filter((c) => c.withYou === side.withYou).map((c) => c.minutes);
      return { ...side, median: median(minutes), count: minutes.length };
    }),
  );
  const pct = (v: number) => `${Math.round(v * 100)}%`;

  let hover = $state<number | null>(null);
  // Where the pointer is (T-350): the tooltip goes beside it, on the side of the popup with room, so it stays inside
  // the popup whatever the bars' width (a single bar across the chart pushed it out). It is placed against the window
  // (position: fixed), not in the popup's content: there, near the bottom, it grew the popup's scroll area, and the
  // popup flickered as its scrollbar came and went
  let area = $state<HTMLElement>();
  let pointer = $state({ x: 0, y: 0, mid: { x: 0, y: 0 } });
  function onMove(e: PointerEvent) {
    const r = (e.currentTarget as SVGElement).getBoundingClientRect();
    const px = ((e.clientX - r.left) / r.width) * W;
    const i = Math.floor((px - PAD_L) / slot);
    hover = i >= 0 && i < n ? i : null;
    // The popup's visible box (the charts' area, outside a popup)
    const box = (area!.closest('.insights-panel') ?? area!).getBoundingClientRect();
    pointer = { x: e.clientX, y: e.clientY, mid: { x: box.left + box.width / 2, y: box.top + box.height / 2 } };
  }
  // Leaving a chart for another: keep the hover, whose move the browser may have delivered before this leave (T-350)
  function onLeave(e: PointerEvent) {
    const to = e.relatedTarget;
    if (to instanceof Element && to.closest('.dive-chart svg')) return;
    hover = null;
  }
  const hb = $derived(hover === null ? null : b[hover]);
  const GAP = 14;
  // Toward the popup's middle from the pointer: left of it in the right half, above it in the lower half
  const tooltipStyle = $derived(
    [
      pointer.x > pointer.mid.x ? `right:${innerWidth - pointer.x + GAP}px` : `left:${pointer.x + GAP}px`,
      pointer.y > pointer.mid.y ? `bottom:${innerHeight - pointer.y + GAP}px` : `top:${pointer.y + GAP}px`,
    ].join(';'),
  );
  /** Tasks named in the tooltip; the rest as "+ N more" */
  const TOOLTIP_TASKS = 5;
</script>

<!-- Three cards, one question each (docs/activity-popup-review.md §3.1): what got done, how long tasks take, and where
     the time went -->
<div class="kpi-row">
  <div class="kpi" id="kpi-done">
    <span class="kpi-head"
      ><span class="kpi-value">{series.totalCompleted}</span>
      <InfoTip label="tasks done">
        <p>Tasks finished in this period: done, not skipped.</p>
      </InfoTip></span
    >
    <span class="kpi-label">tasks done</span>
  </div>
  <div class="kpi kpi-pair" id="kpi-time">
    {#each perTask as side (side.key)}
      <div class="kpi-side" data-side={side.key}>
        <span class="kpi-value">{side.median === null ? '–' : formatMinutes(side.median)}</span>
        <span class="kpi-label">{side.label} <span>({side.count} task{side.count === 1 ? '' : 's'})</span></span>
      </div>
    {/each}
    <InfoTip label="time per task">
      <p>
        The typical (median) time from a task's start until it asked for a review, or was done. <strong
          >Agents alone</strong
        >: tasks agents do by themselves. <strong>With you</strong>: tasks labelled <code>interactive</code>, which
        include working with you.
      </p>
      <p>A task whose lease expired is measured from its restart.</p>
    </InfoTip>
  </div>
  {#if use}
    <div class="kpi kpi-where" id="kpi-where">
      <TimeSplitBar {use} />
      {#if workingNow}<span class="kpi-label" id="kpi-working-now">{workingNow} working now</span>{/if}
    </div>
  {/if}
</div>

{#snippet frame(max: string)}
  <line x1={PAD_L} x2={W - PAD_R} y1={H - 2} y2={H - 2} class="chart-axis" />
  <line x1={PAD_L} x2={W - PAD_R} y1="6" y2="6" class="chart-grid" />
  <text x={PAD_L - 6} y="9" class="chart-tick" text-anchor="end">{max}</text>
  <text x={PAD_L - 6} y={H} class="chart-tick" text-anchor="end">0</text>
  {#if hover !== null}
    <rect x={PAD_L + hover * slot} y="0" width={slot} height={H} class="chart-hover" />
  {/if}
{/snippet}

<!-- Stretches with no agent at work, hatched behind the marks of every chart -->
{#snippet idleBands(labelled: boolean)}
  {#each idle as g (g.start)}
    {@const x0 = Math.max(PAD_L, xt(g.start))}
    {@const x1 = Math.min(W - PAD_R, xt(g.end))}
    {#if x1 > x0}
      <rect x={x0} y="4" width={x1 - x0} height={H - 6} class="idle-band"
        ><title>Idle {formatMinutes((g.end - g.start) / 60_000)}: {when(g.start)} – {when(g.end)}</title></rect
      >
      {#if labelled && x1 - x0 > 46}
        <text x={(x0 + x1) / 2} y={H / 2 + 4} class="idle-label" text-anchor="middle"
          >idle {formatMinutes((g.end - g.start) / 60_000)}</text
        >
      {/if}
    {/if}
  {/each}
{/snippet}

<div class="deep-dive" bind:this={area}>
  <section class="dive-chart" id="dive-completed">
    <header>
      <h4>
        Tasks completed {period}, per {series.unitLabel}, by {group ? 'project' : 'type'}
        <InfoTip label="tasks completed">
          <p>
            Per {series.unitLabel}, the tasks done in it (not skipped), stacked by type, in the dashboard's type
            colours. A task counts when it was done, so one that waited for a review counts at the review's end.
          </p>
          <p>Idle stretches (nobody working, nothing waiting on you) are hatched behind the bars.</p>
        </InfoTip>
      </h4>
      <div class="type-legend">
        {#each series.types as t (t.type)}
          <span class="type-chip"
            ><span class="legend-swatch" style="background:{colorOf(t.type)}"></span>{t.type}<strong>{t.count}</strong
            ></span
          >
        {/each}
      </div>
    </header>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <svg viewBox="0 0 {W} {H + 2}" width="100%" onpointermove={onMove} onpointerleave={onLeave}>
      {@render frame(String(maxDone))}
      {@render idleBands(false)}
      <!-- Stacked by type, most completed type at the bottom; a 1px gap between segments -->
      {#each b as bucket, i (bucket.start)}
        {@const segments = series.types.filter((t) => bucket.byType[t.type])}
        {#each segments as seg, j (seg.type)}
          {@const below = segments.slice(0, j).reduce((sum, t) => sum + bucket.byType[t.type], 0)}
          <rect
            x={PAD_L + i * slot + 1}
            y={yDone(below + bucket.byType[seg.type])}
            width={Math.max(1, slot - 2)}
            height={Math.max(1, yDone(below) - yDone(below + bucket.byType[seg.type]) - (j > 0 ? 1 : 0))}
            rx="1.5"
            fill={colorOf(seg.type)}
          />
        {/each}
      {/each}
    </svg>
  </section>

  <section class="dive-chart" id="dive-agents">
    <header>
      <h4>
        Who was working {period} <span>(1 = one busy the whole {series.unitLabel})</span>
        <InfoTip label="who was working">
          <p>
            Per {series.unitLabel}, how many were at work on tasks: <strong>agents</strong> (Claude Code sessions, and
            work recorded before atm kept who started a task) and <strong>people</strong>. One on two tasks at once
            counts once. Hover for each session.
          </p>
          <p>
            <strong>Waiting on you</strong>: nobody working while a task waited for your review or answer.
            <strong>Idle</strong> (hatched): nobody working and nothing waiting, for 10 minutes or more, nights included.
          </p>
        </InfoTip>
      </h4>
      <div class="type-legend">
        {#each kinds as k (k.key)}
          <span class="type-chip" data-kind={k.key}
            ><span class="legend-swatch" style="background:{k.color}"></span>{k.label}</span
          >
        {:else}
          <span class="chart-empty">No task has a start time in this period</span>
        {/each}
        {#if use?.waitingMs}
          <span class="type-chip" data-kind="waiting"
            ><span class="legend-swatch time-split-waiting"></span>Waiting on you</span
          >
        {/if}
        {#if idle.length}
          <span class="type-chip" data-kind="idle"><span class="legend-swatch time-split-idle"></span>Idle</span>
        {/if}
      </div>
    </header>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <svg viewBox="0 0 {W} {H + 2}" width="100%" onpointermove={onMove} onpointerleave={onLeave}>
      {@render frame(String(maxBusy))}
      {@render idleBands(true)}
      {#each use?.waiting ?? [] as g (g.start)}
        {@const x0 = Math.max(PAD_L, xt(g.start))}
        {@const x1 = Math.min(W - PAD_R, xt(g.end))}
        {#if x1 > x0}
          <rect x={x0} y="4" width={x1 - x0} height={H - 6} class="waiting-band"
            ><title>Waiting on you {formatMinutes((g.end - g.start) / 60_000)}: {when(g.start)} – {when(g.end)}</title
            ></rect
          >
        {/if}
      {/each}
      {#each Array.from({ length: maxBusy - 1 }, (_, k) => k + 1) as level (level)}
        <line x1={PAD_L} x2={W - PAD_R} y1={yBusy(level)} y2={yBusy(level)} class="chart-grid" />
      {/each}
      {#each b as bucket, i (bucket.start)}
        {@const segments = kinds.filter((k) => busyIn(bucket, k.key))}
        {#each segments as k, j (k.key)}
          {@const below = segments.slice(0, j).reduce((sum, x) => sum + busyIn(bucket, x.key), 0)}
          <rect
            x={PAD_L + i * slot + 1}
            y={yBusy(below + busyIn(bucket, k.key))}
            width={Math.max(1, slot - 2)}
            height={Math.max(0.5, yBusy(below) - yBusy(below + busyIn(bucket, k.key)) - (j > 0 ? 1 : 0))}
            rx="1.5"
            fill={k.color}
          />
        {/each}
      {/each}
    </svg>
  </section>

  <section class="dive-chart" id="dive-cycle">
    <header>
      <h4>
        Time per task {period} <span>(from start to done; one dot per task, at its completion)</span>
        <InfoTip label="time per task">
          <p>
            Each task's work time, from its start until it asked for a review or was done, at the time it was done.
            Solid dots and the solid line (the typical time) are tasks agents did alone; <strong>hollow</strong> dots
            and the dashed line are <code>interactive</code> tasks, done with you.
          </p>
          <p>
            The scale stops at the slowest 10%: slower tasks are <strong>triangles</strong> on the top edge, their time in
            the tooltip. Click a task to open it.
          </p>
        </InfoTip>
      </h4>
    </header>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <svg viewBox="0 0 {W} {H + 2}" width="100%" onpointermove={onMove} onpointerleave={onLeave}>
      {@render frame(formatMinutes(maxCycle))}
      {@render idleBands(false)}
      <!-- The typical time of each kind: agents alone solid, labelled on the right; with you dashed, on the left -->
      {#each perTask as side (side.key)}
        {#if side.median !== null}
          {@const y = yCycle(Math.min(side.median, maxCycle))}
          <line x1={PAD_L} x2={W - PAD_R} y1={y} y2={y} class="chart-median" class:solid={!side.withYou} />
          <text
            x={side.withYou ? PAD_L + 4 : W - PAD_R}
            y={y - 4}
            class="chart-tick median-label"
            data-side={side.key}
            text-anchor={side.withYou ? 'start' : 'end'}>{side.label} {formatMinutes(side.median)}</text
          >
        {/if}
      {/each}
      {#each series.cycleTimes as c (c.id)}
        {@const color = colorOf(c.type)}
        {@const over = c.minutes > maxCycle}
        <g
          class="cycle-mark"
          class:with-you={c.withYou}
          class:outlier={over}
          data-id={c.id}
          role="button"
          tabindex="0"
          aria-label="{c.id} {c.name}: {formatMinutes(c.minutes)}"
          onclick={() => openTask(c.id)}
          onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && openTask(c.id)}
        >
          <title>{c.id} {c.name}: {formatMinutes(c.minutes)}{c.withYou ? ' (with you)' : ''}</title>
          {#if over}
            {@const x = xt(c.time)}
            <path
              d="M{x - 5},9 L{x + 5},9 L{x},2 Z"
              fill={c.withYou ? 'var(--surface)' : color}
              stroke={c.withYou ? color : 'var(--surface)'}
              stroke-width="1.5"
            />
          {:else}
            <circle
              cx={xt(c.time)}
              cy={yCycle(c.minutes)}
              r="4"
              fill={c.withYou ? 'var(--surface)' : color}
              stroke={c.withYou ? color : 'var(--surface)'}
              stroke-width={c.withYou ? 2 : 1.5}
            />
          {/if}
        </g>
      {:else}
        <text x={PAD_L + plotW / 2} y={H / 2} class="chart-tick" text-anchor="middle"
          >No task with both a start and a completion time in this period</text
        >
      {/each}
    </svg>
    <div class="dive-axis">
      <span>{when(b[0].start)}</span><span>{when(b[n - 1].end)}</span>
    </div>
  </section>

  {#if hb}
    <!-- Beside the pointer, on the side with room (T-350): inside the charts' area, whatever the bars' width -->
    <div class="deep-dive-tooltip" id="dive-tooltip" style={tooltipStyle}>
      <div class="tooltip-title">{when(hb.start)} – {when(hb.end)}</div>
      <div><strong>{hb.completed.length}</strong> completed</div>
      {#each series.types.filter((t) => hb.byType[t.type]) as t (t.type)}
        <div class="tooltip-type">
          <span class="legend-dot" style="background:{colorOf(t.type)}"></span>{t.type}
          <strong>{hb.byType[t.type]}</strong>
        </div>
      {/each}
      {#if Object.keys(hb.agentBusy).length}
        <div class="tooltip-section">Busy</div>
        {#each series.agents.filter((a) => hb.agentBusy[a]) as a (a)}
          <div class="tooltip-type">
            <span class="legend-dot" style="background:{group ? group.color(group.of(a)) : agentColor(a)}"
            ></span>{agentName(a)}
            <strong>{pct(hb.agentBusy[a])}</strong>
          </div>
        {/each}
      {/if}
      {#if use}
        {@const split = use.split(hb.start, hb.end)}
        {#if split.waitingMs}
          <div class="tooltip-type" id="tooltip-waiting">
            <span class="legend-dot time-split-waiting"></span>Waiting on you
            <strong>{formatMinutes(split.waitingMs / 60_000)}</strong>
          </div>
        {/if}
      {/if}
      {#each hb.completed.slice(0, TOOLTIP_TASKS) as c (c.id)}<div class="tooltip-task">{c.id} {c.name}</div>{/each}
      {#if hb.completed.length > TOOLTIP_TASKS}<div class="tooltip-task">
          +{hb.completed.length - TOOLTIP_TASKS} more
        </div>{/if}
    </div>
  {/if}
</div>
