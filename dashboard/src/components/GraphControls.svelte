<script lang="ts">
  import { dashboard } from '../lib/state.svelte';

  let {
    k,
    onzoom,
    onfit,
    onfitmatches,
    onreset,
    narrowed = false,
    fitted = null,
  }: {
    /** What auto-fit keeps fitted (its button shows pressed); null while auto-fit is off */
    fitted?: 'all' | 'matches' | null;
    k: number;
    onzoom: (factor: number) => void;
    onfit: () => void;
    /** Fit only the tasks the search and filters leave in; given only while that differs from fitting everything
     * (otherwise the button is disabled) */
    onfitmatches?: () => void;
    onreset: () => void;
    /** The search or filters leave some tasks out: offer hiding vs greying them out */
    narrowed?: boolean;
  } = $props();
</script>

<div class="graph-hint">Drag to pan &middot; Ctrl/&#8984; + scroll to zoom</div>
<!-- Groups (T-341): what is shown (filtered out, done), how it is fitted, the zoom -->
<div class="graph-controls graph-toolbar">
  {#if narrowed}
    <span class="graph-group" role="group" aria-label="Tasks outside the filters" id="graph-filtered">
      <span class="graph-group-label">Filtered out</span>
      <span class="graph-seg">
        {#each [['hide', 'Hide'], ['dim', 'Dim']] as [mode, label] (mode)}
          <button
            class:active={dashboard.graphFiltered === mode}
            aria-pressed={dashboard.graphFiltered === mode}
            data-mode={mode}
            onclick={() => (dashboard.graphFiltered = mode as 'hide' | 'dim')}>{label}</button
          >
        {/each}
      </span>
    </span>
  {/if}
  <span class="graph-group">
    <button
      id="graph-dim-done"
      class="graph-toggle"
      class:active={dashboard.graphDimDone}
      aria-pressed={dashboard.graphDimDone}
      title="Grey out done tasks, whatever the filters"
      onclick={() => (dashboard.graphDimDone = !dashboard.graphDimDone)}>Dim done</button
    >
  </span>
  <span class="graph-group" role="group" aria-label="Fit">
    <span class="graph-group-label" aria-hidden="true">&#x2922; Fit</span>
    <span class="graph-seg">
      <button id="graph-fit-all" class:active={fitted === 'all'} title="Fit everything (F)" onclick={onfit}>All</button>
      <button
        id="graph-fit-matches"
        class:active={fitted === 'matches'}
        title={onfitmatches
          ? 'Only the tasks that match the search and filters (Shift+F)'
          : 'Only the matching tasks: with filters on and "Dim", fits them without the greyed-out ones'}
        disabled={!onfitmatches}
        onclick={onfitmatches}>Matches</button
      >
    </span>
    <button
      id="graph-auto-fit"
      class="graph-toggle"
      class:active={dashboard.graphAutoFit}
      aria-pressed={dashboard.graphAutoFit}
      aria-label="Auto-fit"
      title="Auto-fit: keep the graph fitted (All or Matches) as the filters change; panning or zooming turns it off"
      onclick={() => (dashboard.graphAutoFit = !dashboard.graphAutoFit)}>Auto</button
    >
  </span>
  <span class="graph-group" role="group" aria-label="Zoom">
    <span class="graph-seg graph-zoom">
      <button title="Zoom out (-)" onclick={() => onzoom(1 / 1.25)}>&minus;</button>
      <span class="graph-zoom-level" id="graph-zoom-level">{Math.round(k * 100)}%</span>
      <button title="Zoom in (+)" onclick={() => onzoom(1.25)}>+</button>
    </span>
    <button class="graph-toggle" title="Actual size (0)" onclick={onreset}>1:1</button>
  </span>
</div>
