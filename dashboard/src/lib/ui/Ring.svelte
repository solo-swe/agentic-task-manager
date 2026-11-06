<script lang="ts">
  // A ring meter: one share of a whole (done / total), the arc in the entity's colour on a neutral track. Skipped
  // tasks are not in the whole: a thin muted arc just outside the ring shows their share of all tasks, ending at the
  // top where the done arc starts.
  let {
    ratio,
    color,
    size = 56,
    stroke = 6,
    label = '',
    skipped = 0,
    total = 0,
    ariaLabel,
    bare = false,
  }: {
    ratio: number;
    color: string;
    size?: number;
    stroke?: number;
    label?: string;
    /** How many tasks were skipped, and how many count (done or not), for the outer arc */
    skipped?: number;
    total?: number;
    /** Replaces the default "<label> N% done" accessible name */
    ariaLabel?: string;
    /** Without the percentage inside, for a ring too small to hold it (the number then sits beside it) */
    bare?: boolean;
  } = $props();
  const r = $derived((size - stroke) / 2);
  const circumference = $derived(2 * Math.PI * r);
  const pct = $derived(Math.round(ratio * 100));
  const skippedShare = $derived(skipped > 0 ? skipped / (total + skipped) : 0);
  const rSkip = $derived(size / 2 + 3);
  const cSkip = $derived(2 * Math.PI * rSkip);
</script>

<svg
  class="ring"
  overflow="visible"
  width={size}
  height={size}
  viewBox="0 0 {size} {size}"
  role="img"
  aria-label={ariaLabel ?? `${label} ${pct}% done${skipped ? `, ${skipped} skipped` : ''}`}
>
  <circle cx={size / 2} cy={size / 2} {r} fill="none" stroke="var(--surface2)" stroke-width={stroke} />
  {#if ratio > 0}
    <circle
      class="ring-arc"
      cx={size / 2}
      cy={size / 2}
      {r}
      fill="none"
      stroke={color}
      stroke-width={stroke}
      stroke-linecap="round"
      stroke-dasharray="{Math.max(0.001, ratio * circumference)} {circumference}"
      transform="rotate(-90 {size / 2} {size / 2})"
    />
  {/if}
  {#if skippedShare > 0}
    <circle
      class="ring-skip-arc"
      cx={size / 2}
      cy={size / 2}
      r={rSkip}
      fill="none"
      stroke-width="2"
      stroke-linecap="round"
      stroke-dasharray="{skippedShare * cSkip} {cSkip}"
      transform="rotate({-90 - skippedShare * 360} {size / 2} {size / 2})"
    />
  {/if}
  {#if !bare}
    <text x="50%" y="50%" dominant-baseline="central" text-anchor="middle" class="ring-value" font-size={size * 0.24}
      >{pct}%</text
    >
  {/if}
</svg>
