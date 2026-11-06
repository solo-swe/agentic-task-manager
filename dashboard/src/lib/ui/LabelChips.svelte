<script lang="ts">
  import { dashboard } from '../state.svelte';
  import type { Task } from '../types';
  import { labelsOf, labelText } from '../vocab';

  // A task's labels as small neutral chips, in meta.labels' order (D-2.4); nothing when it has none.
  // Compact (Kanban cards, T-256): one line, the first COMPACT_SHOWN labels cut short, the rest behind a +N chip
  // whose popover lists them all: on hover or focus, pinned by a click (which doesn't open the task)
  let { task, compact = false }: { task: Task; compact?: boolean } = $props();
  const COMPACT_SHOWN = 2;
  const labels = $derived(labelsOf(dashboard.vocab, task));
  const shown = $derived(compact ? labels.slice(0, COMPACT_SHOWN) : labels);
  const more = $derived(labels.length - shown.length);

  let hovered = $state(false);
  let pinned = $state(false);
  let moreButton = $state<HTMLButtonElement>();
  let root = $state<HTMLElement>();
  // The popover is fixed to the window, so the column's scrolling doesn't clip it
  let at = $state({ left: 0, top: 0 });
  const open = $derived(hovered || pinned);

  function place() {
    const r = moreButton?.getBoundingClientRect();
    if (r) at = { left: r.left, top: r.bottom + 4 };
  }
  function show() {
    place();
    hovered = true;
  }
  function togglePin(e: Event) {
    e.stopPropagation();
    place();
    pinned = !pinned;
  }
</script>

<svelte:window
  onclick={(e) => pinned && !root?.contains(e.target as Node) && (pinned = false)}
  onkeydown={(e) => e.key === 'Escape' && (pinned = false)}
  onscroll={() => open && place()}
/>

{#if labels.length}
  <span class="label-chips" class:compact bind:this={root}
    >{#each shown as label (label)}<span
        class="label-chip"
        data-label={label}
        title="Label: {compact ? labelText(dashboard.vocab, label) : label}">{labelText(dashboard.vocab, label)}</span
      >{/each}{#if more > 0}<button
        class="label-chip label-more"
        bind:this={moreButton}
        aria-expanded={open}
        aria-label="{more} more labels: {labels.map((l) => labelText(dashboard.vocab, l)).join(', ')}"
        onclick={togglePin}
        onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && e.stopPropagation()}
        onmouseenter={show}
        onmouseleave={() => (hovered = false)}
        onfocus={show}
        onblur={() => (hovered = false)}>+{more}</button
      >{#if open}<span class="label-popover" role="tooltip" style:left="{at.left}px" style:top="{at.top}px"
          >{#each labels as label (label)}<span class="label-chip" data-label={label}
              >{labelText(dashboard.vocab, label)}</span
            >{/each}</span
        >{/if}{/if}</span
  >
{/if}
