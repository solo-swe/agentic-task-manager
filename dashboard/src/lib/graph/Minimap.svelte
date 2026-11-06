<script lang="ts">
  import type { GraphNode } from './layout';
  import type { Size, View } from './view';
  import { dashboard } from '../state.svelte';
  import { kindStyle } from '../theme';
  import { kindOf } from '../vocab';

  const MAX_W = 180;
  const MAX_H = 110;

  let {
    nodes,
    nodeSize,
    content,
    view,
    viewport,
    hidden,
    oncenter,
  }: {
    nodes: GraphNode[];
    nodeSize: Size;
    content: Size;
    view: View;
    viewport: Size;
    hidden: boolean;
    /** Center the main view on this content point */
    oncenter: (x: number, y: number) => void;
  } = $props();

  const scale = $derived(Math.min(MAX_W / content.w, MAX_H / content.h));
  let svg: SVGSVGElement;
  let dragging = false;

  function center(e: PointerEvent) {
    const ctm = svg.getScreenCTM();
    if (!ctm) return;
    const p = new DOMPoint(e.clientX, e.clientY).matrixTransform(ctm.inverse());
    oncenter(p.x, p.y);
  }
</script>

<svg
  bind:this={svg}
  id="graph-minimap"
  class="graph-minimap"
  class:hidden
  role="presentation"
  viewBox="0 0 {content.w} {content.h}"
  style:width="{Math.max(60, Math.round(content.w * scale))}px"
  style:height="{Math.max(36, Math.round(content.h * scale))}px"
  onpointerdown={(e) => {
    dragging = true;
    svg.setPointerCapture(e.pointerId);
    center(e);
  }}
  onpointermove={(e) => dragging && center(e)}
  onpointerup={() => (dragging = false)}
  onpointercancel={() => (dragging = false)}
>
  {#each nodes as node (node.task.id)}
    <rect
      x={node.x}
      y={node.y}
      width={nodeSize.w}
      height={nodeSize.h}
      rx="8"
      fill={kindStyle(kindOf(dashboard.vocab, node.task.status)).color}
      opacity="0.8"
    />
  {/each}
  <rect
    id="graph-minimap-view"
    x={-view.x / view.k}
    y={-view.y / view.k}
    width={viewport.w / view.k}
    height={viewport.h / view.k}
    fill="var(--accent)"
    fill-opacity="0.15"
    stroke="var(--accent)"
    stroke-width="1.5"
    vector-effect="non-scaling-stroke"
  />
</svg>
