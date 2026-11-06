<script lang="ts">
  import { untrack } from 'svelte';
  import { dependencyChain, layoutGraph, NODE_H, NODE_W } from './layout';
  import {
    anyNodeVisible,
    autoFitView,
    clampView,
    fitBox,
    fitView,
    interpolateView,
    nodesBox,
    isFullyVisible,
    panToContent,
    toViewBox,
    zoomAt,
    type View,
  } from './view';
  import { dashboard } from '../state.svelte';
  import { graphNodeBg, KIND_EMOJI, TYPE_EMOJI, typeStyle } from '../theme';
  import { kindOf } from '../vocab';
  import GraphControls from './GraphControls.svelte';
  import Minimap from './Minimap.svelte';

  const DRAG_THRESHOLD = 4;
  const KEY_PAN = 60;
  const ZOOM_STEP = 1.25;
  /** Room around the matching tasks when fitting them, in content units */
  const MATCH_MARGIN = 20;
  const ANIMATION_MS = 280;

  // "Dim": lay out every task and grey out the ones the search and filters exclude, so the shape stays put
  const dim = $derived(dashboard.graphFiltered === 'dim');
  // Archived tasks only with "Include archived" (D-5.3): dashboard.base has them then
  const shown = $derived(dim ? dashboard.base : dashboard.filtered);
  const matching = $derived(new Set(dashboard.filtered.map((t) => t.id)));
  const narrowed = $derived(dashboard.filtered.length < dashboard.base.length);
  const layout = $derived(layoutGraph(shown));
  // Greyed out: outside the search and filters, or done while "Dim done" is on (whatever the filters)
  const dimmed = $derived(
    new Set(
      layout.nodes
        .filter(
          ({ task }) =>
            !matching.has(task.id) || (dashboard.graphDimDone && kindOf(dashboard.vocab, task.status) === 'done'),
        )
        .map(({ task }) => task.id),
    ),
  );
  const empty = $derived(shown.length === 0);
  const content = $derived({ w: layout.width, h: layout.height });

  // Hovering a task highlights it and everything it waits for, directly or not; the rest fades
  let hovered = $state<string | null>(null);
  const chain = $derived(hovered ? dependencyChain(layout.edges, hovered) : null);
  /** The status's kind icon; none for a status no vocabulary defines */
  const statusEmoji = (status: string) => {
    const kind = kindOf(dashboard.vocab, status);
    return kind ? KIND_EMOJI[kind] : '';
  };
  // When the graph overflows, the initial view centers on the task in progress, else the next one to do
  const focus = $derived(
    layout.nodes.find((n) => matching.has(n.task.id) && n.task.status === 'in_progress') ??
      layout.nodes.find((n) => matching.has(n.task.id) && n.task.status === 'todo'),
  );

  let svg: SVGSVGElement;
  let vw = $state(0);
  let vh = $state(0);
  const viewport = $derived({ w: vw, h: vh });
  // `view` is where the graph goes; `shownView` is what is drawn, animated there for fits, zoom buttons and filter
  // changes (T-341), at once for dragging, the wheel, resizing and with reduced motion. A few lines of
  // requestAnimationFrame rather than svelte/motion's Tween: the shipped file has a size budget.
  let view = $state<View>({ k: 1, x: 0, y: 0 });
  let shownView = $state<View>({ k: 1, x: 0, y: 0 });
  let animateNext = false;
  let frame = 0;
  const reducedMotion = () => matchMedia('(prefers-reduced-motion: reduce)').matches;
  /** Draw `target` at once, stopping any animation */
  function jump(target: View) {
    cancelAnimationFrame(frame);
    shownView = target;
  }
  $effect(() => {
    const target = view;
    const animate = untrack(() => animateNext && !reducedMotion());
    animateNext = false;
    if (!animate) return jump(target);
    cancelAnimationFrame(frame);
    const at = interpolateView(
      untrack(() => shownView),
      target,
      untrack(() => viewport),
    );
    const start = performance.now();
    const step = (now: number) => {
      const t = Math.min(1, (now - start) / ANIMATION_MS);
      shownView = at(1 - (1 - t) ** 3); // ease out (cubic)
      if (t < 1) frame = requestAnimationFrame(step);
    };
    frame = requestAnimationFrame(step);
    return () => cancelAnimationFrame(frame);
  });
  /** Move the view, animated */
  function animateTo(v: View) {
    animateNext = true;
    view = v;
  }
  const active = $derived(dashboard.loaded && !empty && vw > 0 && vh > 0);

  const nodeSize = { w: NODE_W, h: NODE_H };
  // The tasks the search and filters leave in (in "Dim", the others are laid out too, greyed out)
  const matchBox = $derived(
    nodesBox(
      layout.nodes.filter((n) => matching.has(n.task.id)),
      nodeSize,
      MATCH_MARGIN,
    ),
  );
  // Fitting the matches differs from fitting everything only when greyed-out tasks are laid out too
  const canFitMatches = $derived(dim && narrowed && matchBox !== null);
  const matchesView = () => (matchBox ? fitBox(matchBox, viewport) : fitView(content, viewport));
  // What auto-fit keeps fitted: the last fit button pressed ("All" in Hide mode is the matches anyway)
  const targetView = () =>
    dashboard.graphFitTarget === 'all' || !canFitMatches ? fitView(content, viewport) : matchesView();

  // Auto-fit on (T-341, the default): every change of the matching tasks, the data, the mode or the panel's size
  // fits the target again (the last fit button pressed, saved with the view); panning or zooming turns it off. Off,
  // only the first layout is fitted automatically (FR-6.3); while the view is still that automatic fit, resizing
  // re-fits it. Once the user pans/zooms or the visible tasks change, the zoom level is kept; the view only pans
  // when no task would be left on screen (FR-6.4).
  let fitted = false;
  let untouched = true;
  let shownKey = '';
  let lastSize = '';

  $effect(() => {
    const key = dashboard.filtered.map((t) => t.id).join(',');
    const auto = dashboard.graphAutoFit;
    if (!active) return;
    const current = untrack(() => view);
    // Animated, except the first view and while the panel is being resized
    const size = `${vw}x${vh}`;
    animateNext = fitted && size === lastSize;
    lastSize = size;
    if (auto) {
      view = targetView();
    } else if (!fitted) {
      // A saved position (FR-5.7) is used if some task is still on screen there; otherwise the automatic view
      const saved = dashboard.graphView ? clampView(dashboard.graphView, content, viewport) : null;
      if (saved && anyNodeVisible(saved, layout.nodes, nodeSize, viewport)) {
        view = saved;
        untouched = false;
      } else {
        view = autoFitView(content, viewport, focus);
      }
    } else if (key !== shownKey) {
      untouched = false;
      view = anyNodeVisible(current, layout.nodes, nodeSize, viewport)
        ? clampView(current, content, viewport)
        : panToContent(current, content, viewport, focus);
    } else {
      view = untouched ? autoFitView(content, viewport, focus) : clampView(current, content, viewport);
    }
    fitted = true;
    shownKey = key;
  });

  /** The user moved the view: auto-fit goes off. Animated for the zoom buttons and keys, not for dragging. */
  function setView(v: View, animate = false) {
    animateNext = animate;
    view = clampView(v, content, viewport);
    untouched = false;
    dashboard.graphAutoFit = false;
    dashboard.graphView = view; // saved with the project's view
  }

  const zoomBy = (factor: number) => setView(zoomAt(view, view.k * factor, vw / 2, vh / 2), true);
  const resetZoom = () => setView(zoomAt(view, 1, vw / 2, vh / 2), true);
  // The fit buttons leave auto-fit as it is; while it is on, they choose what it keeps fitted
  const fitTo = (target: 'all' | 'matches') => {
    dashboard.graphFitTarget = target;
    animateTo(clampView(targetView(), content, viewport));
    dashboard.graphView = view;
  };
  const fitAll = () => fitTo('all');
  const fitMatches = () => fitTo('matches');

  // Drag to pan. Pointer capture starts only after a real drag, so plain clicks still open task details.
  let drag: { sx: number; sy: number; x: number; y: number; id: number; moved: boolean } | null = null;
  let panning = $state(false);
  let suppressClick = false;

  function onPointerDown(e: PointerEvent) {
    if (e.button !== 0 || !active) return;
    jump(view); // a drag starts where the graph is going, not mid-animation
    drag = { sx: e.clientX, sy: e.clientY, x: view.x, y: view.y, id: e.pointerId, moved: false };
  }

  function onPointerMove(e: PointerEvent) {
    if (!drag || e.pointerId !== drag.id) return;
    const dx = e.clientX - drag.sx;
    const dy = e.clientY - drag.sy;
    if (!drag.moved) {
      if (Math.hypot(dx, dy) < DRAG_THRESHOLD) return;
      drag.moved = true;
      svg.setPointerCapture(e.pointerId);
      panning = true;
    }
    setView({ k: view.k, x: drag.x + dx, y: drag.y + dy });
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

  // Ctrl/Cmd + wheel (and trackpad pinch) zooms around the cursor; a plain wheel scrolls the page.
  // Registered manually: it must be non-passive to preventDefault.
  $effect(() => {
    const onWheel = (e: WheelEvent) => {
      if (!(e.ctrlKey || e.metaKey) || !active) return;
      e.preventDefault();
      const r = svg.getBoundingClientRect();
      setView(zoomAt(view, view.k * Math.exp(-e.deltaY * 0.002), e.clientX - r.left, e.clientY - r.top));
    };
    svg.addEventListener('wheel', onWheel, { passive: false });
    return () => svg.removeEventListener('wheel', onWheel);
  });

  function onKeydown(e: KeyboardEvent) {
    if (!active || e.ctrlKey || e.metaKey) return;
    const keys: Record<string, () => void> = {
      '+': () => zoomBy(ZOOM_STEP),
      '=': () => zoomBy(ZOOM_STEP),
      '-': () => zoomBy(1 / ZOOM_STEP),
      '0': resetZoom,
      f: fitAll,
      // Shift+F: the matches, where they differ from everything
      F: () => (canFitMatches ? fitMatches() : fitAll()),
      ArrowLeft: () => setView({ ...view, x: view.x + KEY_PAN }),
      ArrowRight: () => setView({ ...view, x: view.x - KEY_PAN }),
      ArrowUp: () => setView({ ...view, y: view.y + KEY_PAN }),
      ArrowDown: () => setView({ ...view, y: view.y - KEY_PAN }),
    };
    const action = keys[e.key];
    if (!action) return;
    e.preventDefault();
    action();
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions (keyboard pan/zoom when focused) -->
<div
  class="graph-container"
  id="graph-container"
  tabindex="0"
  role="application"
  aria-label="Dependency graph"
  bind:clientWidth={vw}
  bind:clientHeight={vh}
  onkeydown={onKeydown}
>
  <svg
    bind:this={svg}
    id="dep-graph"
    class="dep-graph"
    class:panning
    role="presentation"
    viewBox={active ? toViewBox(shownView, viewport) : undefined}
    onpointerdown={onPointerDown}
    onpointermove={onPointerMove}
    onpointerup={endDrag}
    onpointercancel={endDrag}
    onclickcapture={onClickCapture}
  >
    {#if dashboard.loaded}
      {#if empty}
        <text x="20" y="54" fill="var(--text-muted)" font-size="14">No tasks match current filters</text>
      {:else}
        <defs
          ><marker id="arrow" viewBox="0 0 10 6" refX="10" refY="3" markerWidth="8" markerHeight="6" orient="auto"
            ><path d="M0,0 L10,3 L0,6 Z" fill="var(--text-muted)" /></marker
          ><marker
            id="arrow-chain"
            viewBox="0 0 10 6"
            refX="10"
            refY="3"
            markerWidth="6"
            markerHeight="4.5"
            orient="auto"><path d="M0,0 L10,3 L0,6 Z" fill="var(--accent)" /></marker
          ></defs
        >
        {#each layout.edges as edge (edge.from + '>' + edge.to)}
          {@const onChain = chain?.has(edge.to) ?? false}
          <path
            d={edge.d}
            fill="none"
            stroke={onChain ? 'var(--accent)' : 'var(--text-muted)'}
            stroke-width={onChain ? 2.5 : 1.5}
            marker-end={onChain ? 'url(#arrow-chain)' : 'url(#arrow)'}
            class:graph-dimmed={dimmed.has(edge.from) || dimmed.has(edge.to)}
            class:graph-chain={onChain}
            class:graph-faded={chain && !onChain}
          />
        {/each}
        {#each layout.nodes as node (node.task.id)}
          {@const t = node.task}
          {@const ts = typeStyle(t.type)}
          <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
          <g
            onclick={() => dashboard.showDetail(t.id)}
            onpointerenter={() => !panning && (hovered = t.id)}
            onpointerleave={() => hovered === t.id && (hovered = null)}
            style="cursor:pointer;--node-color:{ts.color}"
            class="graph-node"
            class:graph-dimmed={dimmed.has(t.id)}
            class:graph-chain={chain?.has(t.id)}
            class:graph-hovered={hovered === t.id}
            class:graph-faded={chain && !chain.has(t.id)}
            data-id={t.id}
          >
            <rect
              x={node.x}
              y={node.y}
              width={NODE_W}
              height={NODE_H}
              rx="8"
              fill={graphNodeBg(t.type)}
              stroke={ts.color}
              stroke-width="1.5"
            />
            <text x={node.x + 8} y={node.y + 18} fill={ts.color} font-weight="700" font-size="12"
              >{`${statusEmoji(t.status)} ${t.id} ${TYPE_EMOJI[t.type] ?? ''}`}</text
            >
            <text x={node.x + 8} y={node.y + 36} fill="var(--text-muted)" font-size="10" font-weight="400"
              >{node.label}</text
            >
          </g>
        {/each}
      {/if}
    {/if}
  </svg>
  {#if dashboard.loaded && !empty}
    <GraphControls
      k={view.k}
      onzoom={zoomBy}
      onfit={fitAll}
      onfitmatches={canFitMatches ? fitMatches : undefined}
      fitted={dashboard.graphAutoFit
        ? dashboard.graphFitTarget === 'all' || !canFitMatches
          ? 'all'
          : 'matches'
        : null}
      onreset={resetZoom}
      {narrowed}
    />
    <!-- The zoom label (above) and whether the minimap shows follow where the view goes; its frame, the view drawn -->
    <Minimap
      nodes={layout.nodes}
      {nodeSize}
      {content}
      view={shownView}
      {viewport}
      hidden={!active || isFullyVisible(view, content, viewport)}
      oncenter={(x, y) => setView({ k: view.k, x: vw / 2 - x * view.k, y: vh / 2 - y * view.k })}
    />
  {/if}
</div>
