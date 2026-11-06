<script lang="ts">
  import type { Burndown } from '../burndown';
  import { countTicks, timeTickLabel, timeTicks } from '../chart';
  import { formatWhen } from '../format';

  let { data, now }: { data: Burndown; now: number } = $props();

  const HEIGHT = 220;
  // Right margin leaves room for the forecast's end label
  const M = { top: 18, right: 104, bottom: 28, left: 36 };
  const LABEL_GAP = 14;

  let width = $state(0);
  const innerW = $derived(Math.max(0, width - M.left - M.right));
  const innerH = HEIGHT - M.top - M.bottom;

  const points = $derived(data.points);
  const forecast = $derived(data.forecast);
  const t0 = $derived(points[0]?.t ?? now);
  // A closed sprint's chart ends where it closed; otherwise at now, the forecast's end or the due date
  const until = $derived(data.end ?? now);
  const t1 = $derived(
    data.end !== undefined
      ? Math.max(data.end, t0 + 60_000)
      : Math.max(now, forecast?.range.high ?? now, data.due ?? now, t0 + 60_000),
  );
  const y = $derived(countTicks(Math.max(...points.map((p) => p.scope), 1)));
  const time = $derived(timeTicks(t0, t1));

  const x = (t: number) => M.left + ((t - t0) / (t1 - t0)) * innerW;
  const yv = (v: number) => M.top + innerH - (v / y.max) * innerH;

  /** Step-after path: hold each value until the next point. */
  function stepPath(key: 'scope' | 'remaining') {
    if (!points.length) return '';
    return points.reduce((d, p, i) => (i === 0 ? `M${x(p.t)},${yv(p[key])}` : `${d} H${x(p.t)} V${yv(p[key])}`), '');
  }

  const last = $derived(points[points.length - 1]);
  // Direct labels above the current values; nudge them apart when the lines are close
  const labels = $derived.by(() => {
    if (!last) return [];
    const scopeY = yv(last.scope) - 10;
    let remainingY = yv(last.remaining) - 10;
    if (Math.abs(remainingY - scopeY) < LABEL_GAP) remainingY = scopeY + LABEL_GAP + 6;
    return [
      { key: 'scope', text: `Scope ${last.scope}`, y: scopeY },
      { key: 'remaining', text: `Remaining ${last.remaining}`, y: remainingY },
    ];
  });

  // Hover: crosshair + tooltip
  let hover = $state<{ x: number; lines: string[] } | null>(null);
  function onMove(e: PointerEvent) {
    const rect = (e.currentTarget as SVGRectElement).getBoundingClientRect();
    const px = e.clientX - rect.left + M.left;
    const t = t0 + ((px - M.left) / innerW) * (t1 - t0);
    if (t <= until) {
      const p = [...points].reverse().find((q) => q.t <= t) ?? points[0];
      hover = { x: px, lines: [formatWhen(t, now), `Remaining ${p.remaining}`, `Scope ${p.scope}`] };
    } else if (forecast) {
      hover = {
        x: px,
        lines: [
          'Forecast',
          `Done ≈ ${formatWhen(forecast.to.t, now)}`,
          `Range ${formatWhen(forecast.range.low, now)} – ${formatWhen(forecast.range.high, now)}`,
        ],
      };
    } else hover = null;
  }

  const summary = $derived(
    last
      ? `Burndown: ${last.remaining} of ${last.scope} tasks remaining` +
          (forecast ? `, forecast done around ${formatWhen(forecast.to.t, now)}` : '')
      : 'Burndown: no data',
  );
</script>

<div class="burndown" id="burndown" bind:clientWidth={width}>
  <div class="burndown-legend" aria-hidden="true">
    <span><i class="swatch remaining"></i>Remaining</span>
    <span><i class="swatch scope"></i>Scope</span>
    {#if forecast}<span><i class="swatch forecast"></i>Forecast</span>{/if}
  </div>
  {#if width > 0 && points.length}
    <svg {width} height={HEIGHT} role="img" aria-label={summary}>
      {#each y.ticks as v (v)}
        <line class="burndown-grid" x1={M.left} x2={M.left + innerW} y1={yv(v)} y2={yv(v)} />
        <text class="burndown-axis" x={M.left - 8} y={yv(v) + 4} text-anchor="end">{v}</text>
      {/each}
      {#each time.ticks as t (t)}
        <text class="burndown-axis" x={x(t)} y={HEIGHT - 8} text-anchor="middle">{timeTickLabel(t, time.step)}</text>
      {/each}

      {#if data.end === undefined}
        <line class="burndown-now" x1={x(now)} x2={x(now)} y1={M.top} y2={M.top + innerH} />
        <text class="burndown-axis" x={x(now) - 4} y={M.top + innerH - 4} text-anchor="end">now</text>
      {/if}
      {#if data.due !== undefined && data.due >= t0}
        <line class="burndown-due" id="burndown-due" x1={x(data.due)} x2={x(data.due)} y1={M.top} y2={M.top + innerH} />
        <text class="burndown-axis burndown-due-label" x={x(data.due) + 4} y={M.top + 10}>due</text>
      {/if}

      {#if forecast}
        <polygon
          class="burndown-fan"
          points="{x(now)},{yv(forecast.from.remaining)} {x(forecast.range.low)},{yv(forecast.to.remaining)} {x(
            forecast.range.high,
          )},{yv(forecast.to.remaining)}"
        />
        <line
          class="burndown-forecast"
          x1={x(now)}
          y1={yv(forecast.from.remaining)}
          x2={x(forecast.to.t)}
          y2={yv(forecast.to.remaining)}
        />
        <!-- Agent work is fast next to the history, so the forecast can be a sliver: say where it ends -->
        <text class="burndown-label burndown-forecast-label" x={x(forecast.to.t) + 8} y={yv(forecast.to.remaining) + 4}
          >done ≈ {formatWhen(forecast.to.t, now)}</text
        >
        {#if forecast.to.remaining > 0}
          <!-- The forecast covers agent work only: what's left at its end waits on people (T-240) -->
          <text
            class="burndown-label burndown-waiting-label"
            id="burndown-waiting"
            x={x(forecast.to.t) - 6}
            y={yv(forecast.to.remaining) - 10}
            text-anchor="end">{forecast.to.remaining} waiting on review or a decision</text
          >
        {/if}
      {/if}

      <path class="burndown-scope" d={stepPath('scope')} />
      <path class="burndown-remaining" d={stepPath('remaining')} />

      <circle class="burndown-dot scope" cx={x(last.t)} cy={yv(last.scope)} r="4" />
      <circle class="burndown-dot remaining" cx={x(last.t)} cy={yv(last.remaining)} r="4" />
      {#each labels as label (label.key)}
        <text class="burndown-label burndown-label-{label.key}" x={x(last.t) - 8} y={label.y} text-anchor="end"
          >{label.text}</text
        >
      {/each}

      {#if hover}
        <line class="burndown-crosshair" x1={hover.x} x2={hover.x} y1={M.top} y2={M.top + innerH} />
      {/if}
      <rect
        class="burndown-hit"
        x={M.left}
        y={M.top}
        width={innerW}
        height={innerH}
        role="presentation"
        onpointermove={onMove}
        onpointerleave={() => (hover = null)}
      />
    </svg>
    {#if hover}
      <div
        class="burndown-tooltip"
        style:left="{Math.min(hover.x + 12, width - 180)}px"
        style:top="{M.top + 24}px"
        role="tooltip"
      >
        {#each hover.lines as line, i (i)}<div class:title={i === 0}>{line}</div>{/each}
      </div>
    {/if}
  {/if}
</div>
