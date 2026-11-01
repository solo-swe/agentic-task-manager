<script lang="ts">
  import { agentTimeline, timeTicks, UNATTRIBUTED } from '../lib/agents';
  import { approvedBy, isAgent } from '../lib/actors';
  import { shortAgent } from '../lib/claims';
  import Icon from './Icon.svelte';
  import { formatMinutes, formatWhen } from '../lib/format';
  import { dashboard } from '../lib/state.svelte';
  import { typeStyle } from '../lib/theme';
  import { keepWindow, panWindow, resolveWindow, zoomWindow, type TimeWindow } from '../lib/time-window';

  const ZOOM_STEP = 1.5;
  const DRAG_THRESHOLD = 4;
  const PRESETS: [string, number][] = [
    ['1h', 60],
    ['6h', 360],
    ['1d', 1440],
  ];

  // Live: the timeline runs up to now and moves with time, between data reloads too
  const LIVE_TICK_MS = 5_000;
  let clock = $state(Date.now());
  $effect(() => {
    if (!dashboard.agentsLive) return;
    clock = Date.now();
    const timer = setInterval(() => (clock = Date.now()), LIVE_TICK_MS);
    return () => clearInterval(timer);
  });
  const now = $derived(dashboard.agentsLive ? Math.max(clock, dashboard.loadedAt) : dashboard.loadedAt);

  // One lane per agent: each task a bar from start to completion (or to now while in progress), in its type colour.
  // Tasks outside the search and filters stay, greyed out.
  // Archived work joins the timeline with "Include archived" (D-5.2), like the lists and the graph: a bar per task,
  // so thousands of archived tasks would make every visit slow (D-6.3). The Sprint filter scopes only the board views
  const narrowed = $derived(dashboard.unscoped.length < dashboard.base.length);
  const matching = $derived(new Set(dashboard.unscoped.map((t) => t.id)));
  const timeline = $derived(
    agentTimeline(dashboard.base, dashboard.claims, now, narrowed ? matching : undefined, dashboard.vocab),
  );
  const bounds = $derived<TimeWindow>(
    timeline
      ? {
          start: timeline.start,
          end: Math.max(timeline.end, timeline.start + 1, dashboard.agentsLive ? now : 0),
        }
      : { start: 0, end: 1 },
  );

  // The visible time range: null shows everything; a zoomed window pinned to the end follows live work.
  // Saved with the project's view.
  // Live keeps a zoomed window at the newest end.
  const kept = $derived(
    dashboard.agentsLive && dashboard.agentsWindow
      ? { ...dashboard.agentsWindow, followEnd: true }
      : dashboard.agentsWindow,
  );
  const win = $derived(resolveWindow(kept, bounds));
  const span = $derived(win.end - win.start);
  const zoomed = $derived(kept !== null);
  const fullSpan = $derived(bounds.end - bounds.start);

  const pos = (t: number) => ((t - win.start) / span) * 100;
  const overviewPos = (t: number) => ((t - bounds.start) / fullSpan) * 100;
  const ticks = $derived(timeline ? timeTicks(win.start, win.end) : []);
  const types = $derived([...new Set(timeline?.lanes.flatMap((l) => l.bars.map((b) => b.task.type)) ?? [])].sort());
  const working = $derived(!!timeline && timeline.lanes.some((l) => l.current.length > 0));
  const atNow = $derived((working || dashboard.agentsLive) && win.end >= bounds.end);
  const nowInRange = $derived((working || dashboard.agentsLive) && now <= win.end);

  /** A bar's visible part as left/width percentages, or null when it is outside the window */
  function segment(start: number, end: number) {
    if (end < win.start || start > win.end) return null;
    const left = Math.max(0, pos(start));
    return { left, width: Math.min(100, pos(end)) - left };
  }

  // Moving the window into the past ends Live; zooming keeps it (anchored at now)
  function setWindow(w: TimeWindow) {
    const next = keepWindow(w, bounds);
    if (next && !next.followEnd) dashboard.agentsLive = false;
    dashboard.agentsWindow = next;
  }
  const zoomBy = (factor: number, anchor = 0.5) =>
    setWindow(zoomWindow(win, bounds, factor, dashboard.agentsLive ? 1 : anchor));
  const showLast = (minutes: number) => setWindow({ start: bounds.end - minutes * 60_000, end: bounds.end });

  // The tracks' left edge and width, measured from the axis row they share a column with
  let axis = $state<HTMLElement>();
  const trackRatio = (clientX: number) => {
    const r = axis!.getBoundingClientRect();
    return Math.min(1, Math.max(0, (clientX - r.left) / r.width));
  };

  // Drag the lanes to pan. Pointer capture starts only after a real drag, so a click on a bar still opens it.
  let grid = $state<HTMLElement>();
  let drag: { sx: number; win: TimeWindow; id: number; moved: boolean } | null = null;
  let panning = $state(false);
  let suppressClick = false;

  function onPointerDown(e: PointerEvent) {
    if (e.button !== 0 || !(e.target as Element).closest('.agent-track, .agents-axis')) return;
    drag = { sx: e.clientX, win, id: e.pointerId, moved: false };
  }

  function onPointerMove(e: PointerEvent) {
    if (!drag || e.pointerId !== drag.id) return;
    const dx = e.clientX - drag.sx;
    if (!drag.moved) {
      if (Math.abs(dx) < DRAG_THRESHOLD) return;
      drag.moved = true;
      grid!.setPointerCapture(e.pointerId);
      panning = true;
    }
    const width = axis!.getBoundingClientRect().width;
    setWindow(panWindow(drag.win, bounds, (-dx / width) * (drag.win.end - drag.win.start)));
  }

  function endDrag() {
    if (drag?.moved) {
      suppressClick = true;
      setTimeout(() => (suppressClick = false), 0);
    }
    drag = null;
    panning = false;
  }

  function onClickCapture(e: MouseEvent) {
    if (suppressClick) {
      e.stopPropagation();
      e.preventDefault();
    }
  }

  // Ctrl/Cmd + wheel (and trackpad pinch) zooms around the cursor; a sideways scroll pans; a plain wheel scrolls
  // the page. Registered manually: it must be non-passive to preventDefault.
  $effect(() => {
    const el = grid;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      if (e.ctrlKey || e.metaKey) {
        e.preventDefault();
        zoomBy(Math.exp(-e.deltaY * 0.004), trackRatio(e.clientX));
      } else if (Math.abs(e.deltaX) > Math.abs(e.deltaY) || e.shiftKey) {
        e.preventDefault();
        const dx = e.shiftKey && !e.deltaX ? e.deltaY : e.deltaX;
        setWindow(panWindow(win, bounds, (dx / axis!.getBoundingClientRect().width) * span));
      }
    };
    el.addEventListener('wheel', onWheel, { passive: false });
    return () => el.removeEventListener('wheel', onWheel);
  });

  // The overview: drag the window to move it, its edges to resize it, or anywhere else to pick a new range.
  let overview = $state<HTMLElement>();
  let brush: { mode: 'move' | 'start' | 'end' | 'new'; at: number; win: TimeWindow; moved: boolean } | null = null;

  const overviewTime = (clientX: number) => {
    const r = overview!.getBoundingClientRect();
    return bounds.start + Math.min(1, Math.max(0, (clientX - r.left) / r.width)) * fullSpan;
  };

  function onBrushDown(e: PointerEvent) {
    if (e.button !== 0) return;
    // While everything is shown the window fills the overview, so any drag picks a new range
    const handle = (e.target as HTMLElement).dataset.brush as 'move' | 'start' | 'end' | undefined;
    const mode = zoomed && handle ? handle : 'new';
    brush = { mode, at: overviewTime(e.clientX), win, moved: false };
    overview!.setPointerCapture(e.pointerId);
    e.preventDefault();
  }

  function onBrushMove(e: PointerEvent) {
    if (!brush) return;
    const t = overviewTime(e.clientX);
    const delta = t - brush.at;
    if (!brush.moved && Math.abs(delta) < (fullSpan * DRAG_THRESHOLD) / overview!.clientWidth) return;
    brush.moved = true;
    const { start, end } = brush.win;
    if (brush.mode === 'move') setWindow(panWindow(brush.win, bounds, delta));
    else if (brush.mode === 'start') setWindow({ start: Math.min(t, end - 1), end });
    else if (brush.mode === 'end') setWindow({ start, end: Math.max(t, start + 1) });
    else setWindow({ start: Math.min(brush.at, t), end: Math.max(brush.at, t) });
  }

  function onBrushUp() {
    // A click outside the window centres the window there
    if (brush && !brush.moved && brush.mode === 'new')
      setWindow(panWindow(win, bounds, brush.at - (win.start + span / 2)));
    brush = null;
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.ctrlKey || e.metaKey || e.altKey) return;
    const keys: Record<string, () => void> = {
      '+': () => zoomBy(ZOOM_STEP),
      '=': () => zoomBy(ZOOM_STEP),
      '-': () => zoomBy(1 / ZOOM_STEP),
      '0': () => (dashboard.agentsWindow = null),
      ArrowLeft: () => setWindow(panWindow(win, bounds, -span / 5)),
      ArrowRight: () => setWindow(panWindow(win, bounds, span / 5)),
    };
    if (!keys[e.key]) return;
    e.preventDefault();
    keys[e.key]();
  }
</script>

<div class="agents-view" id="agents">
  {#if !timeline}
    <div class="agents-empty">
      No task has a start time yet. Agents appear here once tasks are started with <code>atm</code> (the
      <code>/atm:do-task</code> and <code>/atm:loop-tasks</code> skills do this).
    </div>
  {:else}
    <div class="agents-toolbar">
      <div class="agents-legend">
        {#each types as type (type)}
          <span class="type-chip"
            ><span class="legend-swatch" style="background:{typeStyle(type).color}"></span>{type}</span
          >
        {/each}
        <span class="type-chip"><span class="legend-swatch live-swatch"></span>in progress</span>
      </div>
      <div class="graph-controls agents-controls" id="agents-controls">
        <span class="agents-range" id="agents-range">
          {formatWhen(win.start, now)} – {atNow ? 'now' : formatWhen(win.end, now)}
        </span>
        <label class="agents-live" title="Keep the timeline at the current time, moving with it">
          <input type="checkbox" id="agents-live" bind:checked={dashboard.agentsLive} /><span class="live-dot"></span> Live
        </label>
        <span class="agents-presets" role="group" aria-label="Show the last">
          {#each PRESETS as [label, minutes] (label)}
            {#if minutes * 60_000 < fullSpan}
              <button
                class:active={kept?.followEnd && Math.abs(span - minutes * 60_000) < 1000}
                title="The last {label}"
                data-preset={label}
                onclick={() => showLast(minutes)}>{label}</button
              >
            {/if}
          {/each}
          <button
            class:active={!zoomed}
            title="Everything (0)"
            data-preset="all"
            onclick={() => (dashboard.agentsWindow = null)}>All</button
          >
        </span>
        <button title="Zoom out (-)" aria-label="Zoom out" disabled={!zoomed} onclick={() => zoomBy(1 / ZOOM_STEP)}
          >&minus;</button
        >
        <button title="Zoom in (+)" aria-label="Zoom in" onclick={() => zoomBy(ZOOM_STEP)}>+</button>
      </div>
    </div>

    <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
    <div
      class="agents-grid"
      class:panning
      class:zoomed
      bind:this={grid}
      tabindex="0"
      role="region"
      aria-label="Agent timeline. Plus and minus zoom, arrow keys move, 0 shows everything."
      onpointerdown={onPointerDown}
      onpointermove={onPointerMove}
      onpointerup={endDrag}
      onpointercancel={endDrag}
      onclickcapture={onClickCapture}
      onkeydown={onKeydown}
    >
      <div class="agents-axis-spacer"></div>
      <div class="agents-axis" bind:this={axis}>
        {#each ticks as t (t)}
          <!-- Centred on its time, except near the ends, where the label would stick out of the track -->
          <span
            class="agents-tick"
            style="left:{pos(t)}%;transform:translateX(-{pos(t) < 4 ? 0 : pos(t) > 96 ? 100 : 50}%)"
            >{formatWhen(t, now)}</span
          >
        {/each}
      </div>

      {#each timeline.lanes as lane (lane.agent)}
        <div class="agent-info" class:graph-dimmed={!lane.matches} data-agent={lane.agent}>
          <div class="agent-name" title={lane.agent || 'Tasks started before agents were recorded'}>
            <span class="agent-dot" class:active={lane.current.length > 0}></span>
            {#if lane.agent !== UNATTRIBUTED}<span
                class="agent-kind"
                data-kind={isAgent(lane.agent) ? 'agent' : 'person'}
                aria-label={isAgent(lane.agent) ? 'Agent' : 'Person'}
                ><Icon name={isAgent(lane.agent) ? 'bot' : 'user'} size={12} /></span
              >{/if}
            {lane.agent === UNATTRIBUTED ? 'Unattributed' : shortAgent(lane.agent)}
          </div>
          <div class="agent-stats">
            {[`${lane.done} done`, lane.medianMinutes === null ? '' : `median ${formatMinutes(lane.medianMinutes)}`]
              .filter(Boolean)
              .join(' · ')}
          </div>
          {#if lane.current.length}
            <div class="agent-now">Now: {lane.current.map((t) => t.id).join(', ')}</div>
          {/if}
        </div>
        <div class="agent-track">
          {#each ticks as t (t)}<span class="agents-gridline" style="left:{pos(t)}%"></span>{/each}
          {#each lane.bars as bar (bar.task.id)}
            {@const seg = segment(bar.start, bar.end)}
            {@const approver = approvedBy(bar.task)}
            {#if seg}
              <button
                class="agent-bar"
                class:live={bar.live}
                class:approved={!!approver}
                class:graph-dimmed={narrowed && !matching.has(bar.task.id)}
                class:cut-start={bar.start < win.start}
                class:cut-end={bar.end > win.end}
                data-id={bar.task.id}
                style="left:{seg.left}%;width:max(4px, {seg.width}%);--bar:{typeStyle(bar.task.type).color}"
                title="{bar.task.id} {bar.task.name}&#10;{formatWhen(bar.start, now)} → {bar.live
                  ? 'now'
                  : formatWhen(bar.end, now)} ({formatMinutes((bar.end - bar.start) / 60_000)}){approver
                  ? `\napproved by ${approver}`
                  : ''}"
                onclick={() => dashboard.showDetail(bar.task.id)}
                ><span class="agent-bar-label">{bar.task.id}</span>{#if approver}<span
                    class="agent-bar-approved"
                    data-by={approver}
                  ></span>{/if}</button
              >
            {/if}
          {/each}
          {#if nowInRange}<span class="agents-now" style="left:{pos(now)}%"></span>{/if}
        </div>
      {/each}

      <div class="agents-axis-spacer agents-overview-label">Overview</div>
      <div
        class="agents-overview"
        id="agents-overview"
        bind:this={overview}
        role="slider"
        tabindex="-1"
        aria-label="Visible time range"
        aria-valuemin={bounds.start}
        aria-valuemax={bounds.end}
        aria-valuenow={win.start}
        aria-valuetext="{formatWhen(win.start, now)} to {formatWhen(win.end, now)}"
        onpointerdown={onBrushDown}
        onpointermove={onBrushMove}
        onpointerup={onBrushUp}
        onpointercancel={onBrushUp}
      >
        {#each timeline.lanes as lane, i (lane.agent)}
          {#each lane.bars as bar (bar.task.id)}
            <span
              class="overview-bar"
              class:graph-dimmed={narrowed && !matching.has(bar.task.id)}
              style="left:{overviewPos(bar.start)}%;width:max(2px, {overviewPos(bar.end) -
                overviewPos(bar.start)}%);top:{4 + i * (26 / timeline.lanes.length)}px;height:{Math.max(
                2,
                26 / timeline.lanes.length - 2,
              )}px;background:{typeStyle(bar.task.type).color}"
            ></span>
          {/each}
        {/each}
        <span
          class="agents-brush"
          class:full={!zoomed}
          data-brush="move"
          style="left:{overviewPos(win.start)}%;width:{overviewPos(win.end) - overviewPos(win.start)}%"
          ><span class="brush-handle start" data-brush="start"></span><span class="brush-handle end" data-brush="end"
          ></span></span
        >
      </div>
    </div>
    <div class="agents-hint">
      Drag the lanes to move in time &middot; Ctrl/&#8984; + scroll to zoom &middot; drag on the overview to pick a
      range
    </div>
  {/if}
</div>
